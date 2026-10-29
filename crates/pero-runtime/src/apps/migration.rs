//! Domain records under `app-data/<name>` when an App version changes.
//! `migrate` is the functional core. The other functions are the shell that
//! loads a snapshot, publishes the next tree, and restores the previous one.

use super::definition::{AppDefinition, AppName, require_version};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Component, Path, PathBuf},
};

pub(super) const RECEIPT_FILE: &str = ".pero-migration.json";
pub(super) const ERROR_FILE: &str = ".pero-migration-error.json";

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct DataRecord {
    pub path: String,
    pub bytes: Vec<u8>,
}

/// One direct version step declared by the incoming package.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DataMigration {
    pub from: String,
    pub to: String,
    #[serde(default)]
    pub rename: BTreeMap<String, String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct RecordCheck {
    pub before: String,
    pub after: String,
    pub sha256: String,
}

/// Links every domain record to its path after the version step.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct MigrationReceipt {
    pub from: String,
    pub to: String,
    pub records: Vec<RecordCheck>,
}

#[derive(Clone, Debug)]
pub(super) enum Prepared {
    Absent,
    Keep(MigrationReceipt),
    Staged,
}

#[derive(Serialize)]
struct MigrationErrorRecord {
    from: String,
    to: String,
    error: String,
}

pub(super) fn validate_declarations(migrations: &[DataMigration]) -> Result<(), String> {
    let mut seen = BTreeSet::new();
    for migration in migrations {
        validate_migration(migration)?;
        if !seen.insert((migration.from.clone(), migration.to.clone())) {
            return Err(format!(
                "duplicate migration from {} to {}",
                migration.from, migration.to
            ));
        }
    }
    Ok(())
}

pub(super) fn direct<'a>(
    migrations: &'a [DataMigration],
    from: &str,
    to: &str,
) -> Option<&'a DataMigration> {
    migrations
        .iter()
        .find(|migration| migration.from == from && migration.to == to)
}

pub(super) fn migrate(
    from: &str,
    to: &str,
    records: &[DataRecord],
    migration: Option<&DataMigration>,
) -> Result<(Vec<DataRecord>, MigrationReceipt), String> {
    if let Some(migration) = migration {
        validate_migration(migration)?;
        if migration.from != from || migration.to != to {
            return Err(format!(
                "migration from {} to {} does not apply to {from} to {to}",
                migration.from, migration.to
            ));
        }
    }
    let mut present = BTreeMap::new();
    for record in records {
        if !valid_record_path(&record.path) {
            return Err(format!("domain record path is invalid: {}", record.path));
        }
        if present.insert(record.path.clone(), record).is_some() {
            return Err(format!("duplicate domain record {}", record.path));
        }
    }
    let mut rename = BTreeMap::new();
    if let Some(migration) = migration {
        rename.clone_from(&migration.rename);
    }
    let mut landing: BTreeMap<String, String> = BTreeMap::new();
    let mut next = Vec::new();
    let mut checks = Vec::new();
    for (path, record) in &present {
        let after = rename.remove(path).unwrap_or_else(|| path.clone());
        if let Some(previous) = landing.get(&after) {
            return Err(format!("migration maps {previous} and {path} onto {after}"));
        }
        if let Some((other_path, other_source)) = landing
            .iter()
            .find(|(existing, _)| paths_overlap(existing, &after))
        {
            return Err(format!(
                "migration places {path} and {other_source} at overlapping paths {after} and {other_path}"
            ));
        }
        landing.insert(after.clone(), path.clone());
        next.push(DataRecord {
            path: after.clone(),
            bytes: record.bytes.clone(),
        });
        checks.push(RecordCheck {
            before: path.clone(),
            after,
            sha256: sha256_hex(&record.bytes),
        });
    }
    next.sort_by(|left, right| left.path.cmp(&right.path));
    checks.sort_by(|left, right| left.before.cmp(&right.before));
    Ok((
        next,
        MigrationReceipt {
            from: from.to_owned(),
            to: to.to_owned(),
            records: checks,
        },
    ))
}

pub(super) fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

pub(super) fn prepare(
    data_root: &Path,
    name: &AppName,
    from: &str,
    to: &str,
    migrations: &[DataMigration],
) -> Result<Prepared, String> {
    let Some(records) = load(data_root, name)? else {
        return Ok(Prepared::Absent);
    };
    let (next, receipt) = migrate(from, to, &records, direct(migrations, from, to))?;
    if same_records(&records, &next) {
        return Ok(Prepared::Keep(receipt));
    }
    stage(data_root, name, &next, &receipt)?;
    Ok(Prepared::Staged)
}

pub(super) fn commit(data_root: &Path, name: &AppName, prepared: &Prepared) -> Result<(), String> {
    if !matches!(prepared, Prepared::Staged) {
        return Ok(());
    }
    let staging = staging_path(data_root, name);
    let live = data_root.join(name.as_str());
    let rollback = rollback_path(data_root, name);
    if rollback.exists() {
        let _ = fs::remove_dir_all(&staging);
        return Err(format!(
            "domain backup {} is already present",
            rollback.display()
        ));
    }
    if live.exists()
        && let Err(error) = fs::rename(&live, &rollback)
    {
        let _ = fs::remove_dir_all(&staging);
        return Err(error.to_string());
    }
    if let Err(error) = fs::rename(&staging, &live) {
        if rollback.exists() {
            let _ = fs::remove_dir_all(&live);
            if let Err(restore_error) = fs::rename(&rollback, &live) {
                return Err(format!(
                    "{error}. Restoring domain data failed: {restore_error}. Backup remains at {}",
                    rollback.display()
                ));
            }
        }
        let _ = fs::remove_dir_all(&staging);
        return Err(error.to_string());
    }
    Ok(())
}

pub(super) fn finish(data_root: &Path, name: &AppName, prepared: &Prepared) -> Result<(), String> {
    match prepared {
        Prepared::Absent => Ok(()),
        Prepared::Keep(receipt) => {
            let live = data_root.join(name.as_str());
            write_json(&live, RECEIPT_FILE, receipt)?;
            let _ = fs::remove_file(live.join(ERROR_FILE));
            Ok(())
        }
        Prepared::Staged => {
            let _ = fs::remove_dir_all(rollback_path(data_root, name));
            Ok(())
        }
    }
}

pub(super) fn restore(data_root: &Path, name: &AppName, prepared: &Prepared) -> Result<(), String> {
    if !matches!(prepared, Prepared::Staged) {
        return Ok(());
    }
    let live = data_root.join(name.as_str());
    let rollback = rollback_path(data_root, name);
    if !rollback.exists() {
        return Ok(());
    }
    if live.exists() {
        fs::remove_dir_all(&live).map_err(|error| error.to_string())?;
    }
    fs::rename(&rollback, &live).map_err(|error| error.to_string())
}

pub(super) fn abort(data_root: &Path, name: &AppName) {
    let _ = fs::remove_dir_all(staging_path(data_root, name));
}

pub(super) fn write_failure(
    data_root: &Path,
    name: &AppName,
    from: &str,
    to: &str,
    error: &str,
) -> Result<PathBuf, String> {
    let live = data_root.join(name.as_str());
    let metadata = fs::symlink_metadata(&live).map_err(|error| error.to_string())?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(format!("domain data {} is not a directory", live.display()));
    }
    write_json(
        &live,
        ERROR_FILE,
        &MigrationErrorRecord {
            from: from.to_owned(),
            to: to.to_owned(),
            error: error.to_owned(),
        },
    )?;
    Ok(live.join(ERROR_FILE))
}

/// Drop leftover staging. Restore a data backup unless the installed package
/// version is the receipt's target version, in which case the backup is the
/// previous tree from a finished publish.
pub(super) fn recover(apps_root: &Path, data_root: &Path) -> Result<(), String> {
    if !data_root.is_dir() {
        return Ok(());
    }
    let mut staging = Vec::new();
    let mut rollbacks = Vec::new();
    for entry in fs::read_dir(data_root).map_err(|error| error.to_string())? {
        let entry = entry.map_err(|error| error.to_string())?;
        let Some(file_name) = entry.file_name().to_str().map(str::to_owned) else {
            continue;
        };
        if !entry
            .file_type()
            .map_err(|error| error.to_string())?
            .is_dir()
        {
            continue;
        }
        if let Some(name) = file_name.strip_prefix(".staging-") {
            if AppName::try_from(name.to_owned()).is_ok() {
                staging.push(entry.path());
            }
        } else if let Some(name) = file_name.strip_prefix(".rollback-") {
            if AppName::try_from(name.to_owned()).is_ok() {
                rollbacks.push((name.to_owned(), entry.path()));
            }
        }
    }
    for path in staging {
        fs::remove_dir_all(path).map_err(|error| error.to_string())?;
    }
    for (name, rollback) in rollbacks {
        let live = data_root.join(&name);
        if published(apps_root, &name, &live) {
            fs::remove_dir_all(&rollback).map_err(|error| error.to_string())?;
        } else {
            if live.exists() {
                fs::remove_dir_all(&live).map_err(|error| error.to_string())?;
            }
            fs::rename(&rollback, &live).map_err(|error| error.to_string())?;
        }
    }
    Ok(())
}

fn validate_migration(migration: &DataMigration) -> Result<(), String> {
    require_version(&migration.from)?;
    require_version(&migration.to)?;
    let mut destinations = BTreeSet::new();
    for (source, destination) in &migration.rename {
        if !valid_record_path(source) || !valid_record_path(destination) {
            return Err(format!(
                "migration path must stay inside the domain data directory: {source} -> {destination}"
            ));
        }
        if !destinations.insert(destination) {
            return Err(format!(
                "migration maps more than one record onto {destination}"
            ));
        }
    }
    Ok(())
}

fn valid_record_path(value: &str) -> bool {
    !value.is_empty()
        && !value.contains(['\\', ':', '\0'])
        && Path::new(value).components().all(|part| match part {
            Component::Normal(name) => name
                .to_str()
                .is_some_and(|name| !name.is_empty() && !name.starts_with('.')),
            _ => false,
        })
}

fn paths_overlap(left: &str, right: &str) -> bool {
    left != right
        && (right.starts_with(&format!("{left}/")) || left.starts_with(&format!("{right}/")))
}

fn same_records(before: &[DataRecord], after: &[DataRecord]) -> bool {
    before.len() == after.len()
        && before
            .iter()
            .zip(after)
            .all(|(left, right)| left.path == right.path && left.bytes == right.bytes)
}

fn load(data_root: &Path, name: &AppName) -> Result<Option<Vec<DataRecord>>, String> {
    let live = data_root.join(name.as_str());
    if !live.exists() {
        return Ok(None);
    }
    let metadata = fs::symlink_metadata(&live).map_err(|error| error.to_string())?;
    if metadata.file_type().is_symlink() {
        return Err("domain data directory is a symlink".into());
    }
    if !metadata.is_dir() {
        return Err(format!("domain data {} is not a directory", live.display()));
    }
    let mut records = Vec::new();
    walk(&live, &live, &mut records)?;
    records.sort_by(|left, right| left.path.cmp(&right.path));
    Ok(Some(records))
}

fn walk(root: &Path, dir: &Path, records: &mut Vec<DataRecord>) -> Result<(), String> {
    for entry in fs::read_dir(dir).map_err(|error| error.to_string())? {
        let entry = entry.map_err(|error| error.to_string())?;
        let Some(name) = entry.file_name().to_str().map(str::to_owned) else {
            return Err("domain data file name is not utf-8".into());
        };
        if name.starts_with('.') {
            continue;
        }
        let kind = entry.file_type().map_err(|error| error.to_string())?;
        let path = entry.path();
        if kind.is_symlink() {
            return Err(format!("domain data contains a symlink: {name}"));
        }
        if kind.is_dir() {
            walk(root, &path, records)?;
        } else if kind.is_file() {
            let relative = relative_path(root, &path)?;
            let bytes = fs::read(&path).map_err(|error| error.to_string())?;
            records.push(DataRecord {
                path: relative,
                bytes,
            });
        } else {
            return Err(format!("domain data contains a special file: {name}"));
        }
    }
    Ok(())
}

fn relative_path(root: &Path, path: &Path) -> Result<String, String> {
    let relative = path.strip_prefix(root).map_err(|error| error.to_string())?;
    let mut parts = Vec::new();
    for component in relative.components() {
        match component {
            Component::Normal(part) => {
                let Some(part) = part.to_str() else {
                    return Err("domain data file name is not utf-8".into());
                };
                parts.push(part);
            }
            _ => return Err("domain record path is invalid".into()),
        }
    }
    let joined = parts.join("/");
    if !valid_record_path(&joined) {
        return Err(format!("domain record path is invalid: {joined}"));
    }
    Ok(joined)
}

fn stage(
    data_root: &Path,
    name: &AppName,
    records: &[DataRecord],
    receipt: &MigrationReceipt,
) -> Result<(), String> {
    let staging = staging_path(data_root, name);
    if staging.exists() {
        fs::remove_dir_all(&staging).map_err(|error| error.to_string())?;
    }
    fs::create_dir(&staging).map_err(|error| error.to_string())?;
    let written = (|| {
        for record in records {
            let relative = fs_path(&record.path)?;
            let destination = staging.join(&relative);
            if !destination.starts_with(&staging) {
                return Err(format!("domain record path is invalid: {}", record.path));
            }
            if let Some(parent) = destination.parent() {
                if parent != staging {
                    fs::create_dir_all(parent).map_err(|error| error.to_string())?;
                }
            }
            fs::write(&destination, &record.bytes).map_err(|error| error.to_string())?;
        }
        write_json(&staging, RECEIPT_FILE, receipt)
    })();
    if let Err(error) = written {
        let _ = fs::remove_dir_all(&staging);
        return Err(error);
    }
    Ok(())
}

fn fs_path(value: &str) -> Result<PathBuf, String> {
    if !valid_record_path(value) {
        return Err(format!("domain record path is invalid: {value}"));
    }
    Ok(value.split('/').collect())
}

fn published(apps_root: &Path, name: &str, live: &Path) -> bool {
    let Some(receipt) = fs::read(live.join(RECEIPT_FILE))
        .ok()
        .and_then(|bytes| serde_json::from_slice::<MigrationReceipt>(&bytes).ok())
    else {
        return false;
    };
    let Some(definition) = fs::read(apps_root.join(name).join("app.json"))
        .ok()
        .and_then(|bytes| AppDefinition::parse(&bytes).ok())
    else {
        return false;
    };
    definition.name.as_str() == name && definition.version == receipt.to
}

fn write_json(dir: &Path, file: &str, value: &impl Serialize) -> Result<(), String> {
    let pending = dir.join(format!("{file}.pending"));
    let result = (|| {
        fs::write(
            &pending,
            serde_json::to_vec_pretty(value).map_err(|error| error.to_string())?,
        )
        .map_err(|error| error.to_string())?;
        fs::rename(&pending, dir.join(file)).map_err(|error| error.to_string())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&pending);
    }
    result
}

fn staging_path(data_root: &Path, name: &AppName) -> PathBuf {
    data_root.join(format!(".staging-{}", name.as_str()))
}

fn rollback_path(data_root: &Path, name: &AppName) -> PathBuf {
    data_root.join(format!(".rollback-{}", name.as_str()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record(path: &str, body: &str) -> DataRecord {
        DataRecord {
            path: path.to_owned(),
            bytes: body.as_bytes().to_vec(),
        }
    }

    fn rename(pairs: &[(&str, &str)]) -> DataMigration {
        DataMigration {
            from: "0.1.0".into(),
            to: "0.2.0".into(),
            rename: pairs
                .iter()
                .map(|(source, destination)| ((*source).into(), (*destination).into()))
                .collect(),
        }
    }

    #[test]
    fn migration_records_stay_checkable() {
        let draft = record("draft.json", "user draft");
        let notes = record("notes.txt", "kept");
        let other = record("records/draft.json", "other");
        let cases: Vec<(
            &str,
            Vec<DataRecord>,
            Option<DataMigration>,
            Result<Vec<(&str, &str)>, &str>,
        )> = vec![
            (
                "keep",
                vec![draft.clone()],
                None,
                Ok(vec![("draft.json", "draft.json")]),
            ),
            (
                "rename",
                vec![draft.clone(), notes.clone()],
                Some(rename(&[("draft.json", "records/draft.json")])),
                Ok(vec![
                    ("draft.json", "records/draft.json"),
                    ("notes.txt", "notes.txt"),
                ]),
            ),
            (
                "swap",
                vec![draft.clone(), notes.clone()],
                Some(rename(&[
                    ("draft.json", "notes.txt"),
                    ("notes.txt", "draft.json"),
                ])),
                Ok(vec![
                    ("draft.json", "notes.txt"),
                    ("notes.txt", "draft.json"),
                ]),
            ),
            (
                "absent source",
                vec![notes.clone()],
                Some(rename(&[("draft.json", "records/draft.json")])),
                Ok(vec![("notes.txt", "notes.txt")]),
            ),
            (
                "collision",
                vec![draft.clone(), other.clone()],
                Some(rename(&[("draft.json", "records/draft.json")])),
                Err("onto records/draft.json"),
            ),
            (
                "overlap",
                vec![draft.clone(), notes.clone()],
                Some(rename(&[("draft.json", "notes.txt/child")])),
                Err("overlapping paths"),
            ),
            (
                "escape",
                vec![draft.clone()],
                Some(rename(&[("draft.json", "../outside.json")])),
                Err("domain data directory"),
            ),
        ];
        for (name, records, migration, expected) in cases {
            let result = migrate("0.1.0", "0.2.0", &records, migration.as_ref());
            match expected {
                Ok(pairs) => {
                    let (next, receipt) = result.unwrap_or_else(|error| panic!("{name}: {error}"));
                    assert_eq!(receipt.from, "0.1.0", "{name}");
                    assert_eq!(receipt.to, "0.2.0", "{name}");
                    assert_eq!(receipt.records.len(), pairs.len(), "{name}");
                    for (before, after) in pairs {
                        let check = receipt
                            .records
                            .iter()
                            .find(|check| check.before == before)
                            .unwrap_or_else(|| panic!("{name} missing {before}"));
                        assert_eq!(check.after, after, "{name}");
                        let original = records.iter().find(|record| record.path == before).unwrap();
                        let migrated = next.iter().find(|record| record.path == after).unwrap();
                        assert_eq!(migrated.bytes, original.bytes, "{name}");
                        assert_eq!(check.sha256, sha256_hex(&original.bytes), "{name}");
                    }
                }
                Err(fragment) => {
                    let error = result.expect_err(name);
                    assert!(error.contains(fragment), "{name}: {error}");
                }
            }
        }

        let step = rename(&[]);
        assert!(direct(&[step.clone()], "0.1.0", "0.3.0").is_none());
        assert_eq!(
            direct(&[step.clone()], "0.1.0", "0.2.0").unwrap().to,
            "0.2.0"
        );
        let duplicate = validate_declarations(&[step.clone(), step]).unwrap_err();
        assert!(duplicate.contains("duplicate"), "{duplicate}");
        let mut collided = rename(&[("a.json", "c.json"), ("b.json", "c.json")]);
        collided.from = "0.1.0".into();
        let declared = validate_declarations(&[collided]).unwrap_err();
        assert!(declared.contains("onto c.json"), "{declared}");
    }
}
