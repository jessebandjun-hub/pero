//! Session journal location.
//!
//! Development history is `{workspace}/target/sessions.jsonl`. The live journal
//! is the user-data file. Startup copies development records that are not
//! already stored there and leaves the development file unchanged. A failed
//! copy does not replace either readable copy.
use pero_contracts::HostEvent;
use std::{
    collections::{BTreeSet, HashMap},
    fs,
    io::{self, Write},
    path::{Path, PathBuf},
};

#[derive(Clone, Debug, PartialEq, Eq)]
struct SessionSnapshot {
    sessions: BTreeSet<String>,
    records: Vec<String>,
}

impl SessionSnapshot {
    fn parse(text: &str) -> Self {
        let records = lines_of(text);
        let mut sessions = BTreeSet::new();
        for record in &records {
            let Ok(event) = serde_json::from_str::<HostEvent>(record) else {
                continue;
            };
            if event.kind == "session_created" {
                if let Some(thread_id) = event.thread_id {
                    sessions.insert(thread_id);
                }
            }
        }
        Self { sessions, records }
    }

    fn read(path: &Path) -> io::Result<Self> {
        Ok(Self::parse(&fs::read_to_string(path)?))
    }
}

pub struct SessionJournal {
    pub path: PathBuf,
    pub error: Option<String>,
}

impl SessionJournal {
    /// `history` is `PERO_HISTORY_PATH`. `settings` is `PERO_SETTINGS_PATH`.
    /// `user_data` is the platform directory used when neither path is set.
    pub fn open(
        workspace: &Path,
        history: Option<&Path>,
        settings: Option<&Path>,
        user_data: &Path,
    ) -> Self {
        let destination = resolve_destination(history, settings, user_data);
        let legacy = workspace.join("target").join("sessions.jsonl");
        if let Err(error) = recover(&destination) {
            return Self {
                path: fallback(&legacy, &destination),
                error: Some(error),
            };
        }
        match relocate(&legacy, &destination) {
            Ok(()) => Self {
                path: destination,
                error: None,
            },
            Err(error) => Self {
                path: fallback(&legacy, &destination),
                error: Some(error),
            },
        }
    }
}

pub fn user_data_dir() -> PathBuf {
    #[cfg(target_os = "windows")]
    {
        let base = std::env::var_os("APPDATA")
            .map(PathBuf::from)
            .filter(|path| path.is_absolute())
            .unwrap_or_else(|| home_dir().join("AppData").join("Roaming"));
        return base.join("pero");
    }
    #[cfg(target_os = "macos")]
    {
        return home_dir()
            .join("Library")
            .join("Application Support")
            .join("pero");
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        let base = std::env::var_os("XDG_DATA_HOME")
            .map(PathBuf::from)
            .filter(|path| path.is_absolute())
            .unwrap_or_else(|| home_dir().join(".local").join("share"));
        base.join("pero")
    }
}

fn home_dir() -> PathBuf {
    std::env::var_os("HOME")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var_os("USERPROFILE")
                .filter(|value| !value.is_empty())
                .map(PathBuf::from)
        })
        .unwrap_or_else(|| PathBuf::from("."))
}

fn resolve_destination(
    history: Option<&Path>,
    settings: Option<&Path>,
    user_data: &Path,
) -> PathBuf {
    if let Some(path) = history.filter(|path| !path.as_os_str().is_empty()) {
        return path.to_path_buf();
    }
    if let Some(parent) = settings
        .and_then(Path::parent)
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        return parent.join("sessions.jsonl");
    }
    user_data.join("sessions.jsonl")
}

fn relocate(legacy: &Path, destination: &Path) -> Result<(), String> {
    if same_file(legacy, destination) {
        return Ok(());
    }
    let Some(legacy_bytes) = read_journal(legacy)? else {
        return Ok(());
    };
    let legacy_text = decode(legacy, &legacy_bytes)?;
    let legacy_snapshot = SessionSnapshot::parse(&legacy_text);
    if legacy_snapshot.records.is_empty() {
        return Ok(());
    }
    if let Some(meta) = fs::symlink_metadata(destination).ok() {
        if !meta.file_type().is_file() {
            return Err(format!(
                "user data session history {} is not a file. The development journal was not changed.",
                destination.display()
            ));
        }
    }
    let destination_bytes = read_journal(destination)?;
    let destination_text = match &destination_bytes {
        Some(bytes) => decode(destination, bytes)?,
        None => String::new(),
    };
    let destination_records = lines_of(&destination_text);
    let merged = merge_records(&destination_records, &legacy_snapshot.records);
    if destination_bytes.is_some() && destination_records == merged {
        return Ok(());
    }
    write_records(destination, &merged)?;
    let written = SessionSnapshot::read(destination).map_err(|error| {
        format!(
            "could not read session history {} after writing it: {error}. The development journal was not changed.",
            destination.display()
        )
    })?;
    if written.records != merged || !legacy_snapshot.sessions.is_subset(&written.sessions) {
        return Err(format!(
            "session history {} does not match the development journal. The development journal was not changed.",
            destination.display()
        ));
    }
    if fs::read(legacy).ok().as_deref() != Some(legacy_bytes.as_slice()) {
        return Err(format!(
            "session history migration changed the development journal {}",
            legacy.display()
        ));
    }
    Ok(())
}

fn fallback(legacy: &Path, destination: &Path) -> PathBuf {
    if usable_journal(destination) {
        destination.to_path_buf()
    } else if usable_journal(legacy) {
        legacy.to_path_buf()
    } else {
        destination.to_path_buf()
    }
}

fn usable_journal(path: &Path) -> bool {
    fs::symlink_metadata(path)
        .ok()
        .is_some_and(|meta| meta.file_type().is_file())
        && fs::read_to_string(path).is_ok()
}

fn read_journal(path: &Path) -> Result<Option<Vec<u8>>, String> {
    if !path.exists() {
        return Ok(None);
    }
    let meta = fs::symlink_metadata(path).map_err(|error| unchanged(path, &error))?;
    if !meta.file_type().is_file() {
        return Err(format!(
            "session history {} is not a file. It was not changed.",
            path.display()
        ));
    }
    fs::read(path)
        .map(Some)
        .map_err(|error| unchanged(path, &error))
}

fn decode(path: &Path, bytes: &[u8]) -> Result<String, String> {
    String::from_utf8(bytes.to_vec()).map_err(|_| {
        format!(
            "session history {} is not UTF-8. It was not changed.",
            path.display()
        )
    })
}

fn unchanged(path: &Path, error: &io::Error) -> String {
    format!(
        "could not read session history {}: {error}. It was not changed.",
        path.display()
    )
}

fn same_file(left: &Path, right: &Path) -> bool {
    if left == right {
        return true;
    }
    matches!(
        (fs::canonicalize(left), fs::canonicalize(right)),
        (Ok(left), Ok(right)) if left == right
    )
}

fn lines_of(text: &str) -> Vec<String> {
    text.lines()
        .filter(|line| !line.is_empty())
        .map(str::to_owned)
        .collect()
}

fn merge_records(destination: &[String], legacy: &[String]) -> Vec<String> {
    let mut remaining = HashMap::<&str, usize>::new();
    for line in destination {
        *remaining.entry(line).or_default() += 1;
    }
    let mut merged = destination.to_vec();
    for line in legacy {
        let count = remaining.entry(line).or_default();
        if *count == 0 {
            merged.push(line.clone());
        } else {
            *count -= 1;
        }
    }
    merged
}

fn covers(records: &[String], previous: &[String]) -> bool {
    let mut remaining = HashMap::<&str, usize>::new();
    for line in records {
        *remaining.entry(line).or_default() += 1;
    }
    previous.iter().all(|line| {
        let count = remaining.entry(line).or_default();
        if *count == 0 {
            false
        } else {
            *count -= 1;
            true
        }
    })
}

fn sidecar(path: &Path, suffix: &str) -> PathBuf {
    let name = path.file_name().unwrap_or_default().to_string_lossy();
    path.with_file_name(format!("{name}.{suffix}"))
}

/// Finish an interrupted replacement. `*.previous` is the last user-data
/// journal; `*.pending` is an uncommitted write.
fn recover(path: &Path) -> Result<(), String> {
    let pending = sidecar(path, "pending");
    if pending.is_file() {
        let _ = fs::remove_file(&pending);
    }
    let previous = sidecar(path, "previous");
    if !previous.is_file() {
        return Ok(());
    }
    if !path.exists() {
        return fs::rename(&previous, path).map_err(|error| {
            format!(
                "could not restore session history {} from {}: {error}. That backup was left in place.",
                path.display(),
                previous.display()
            )
        });
    }
    if !path.is_file() {
        return Err(format!(
            "session history {} is not a file. The backup at {} was left in place.",
            path.display(),
            previous.display()
        ));
    }
    let prior = read_text(&previous)?;
    let dest = fs::read_to_string(path).unwrap_or_default();
    if !covers(&lines_of(&dest), &lines_of(&prior)) {
        fs::copy(&previous, path).map_err(|error| {
            format!(
                "could not restore session history {} from {}: {error}. That backup was left in place.",
                path.display(),
                previous.display()
            )
        })?;
    }
    let dest = read_text(path)?;
    if covers(&lines_of(&dest), &lines_of(&prior)) {
        let _ = fs::remove_file(&previous);
        Ok(())
    } else {
        Err(format!(
            "session history {} is missing records from {}. That backup was left in place.",
            path.display(),
            previous.display()
        ))
    }
}

fn read_text(path: &Path) -> Result<String, String> {
    let bytes = fs::read(path).map_err(|error| {
        format!(
            "could not read session history {}: {error}. It was left in place.",
            path.display()
        )
    })?;
    String::from_utf8(bytes).map_err(|_| {
        format!(
            "session history {} is not UTF-8. It was left in place.",
            path.display()
        )
    })
}

fn write_records(path: &Path, records: &[String]) -> Result<(), String> {
    let parent = match path.parent() {
        Some(parent) if !parent.as_os_str().is_empty() => parent,
        _ => Path::new("."),
    };
    fs::create_dir_all(parent).map_err(|error| {
        format!(
            "could not create the user data directory {}: {error}. The development journal was not changed.",
            parent.display()
        )
    })?;
    let pending = sidecar(path, "pending");
    let previous = sidecar(path, "previous");
    let wrote = (|| -> io::Result<()> {
        let mut options = fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options.open(&pending)?;
        for record in records {
            writeln!(file, "{record}")?;
        }
        file.sync_all()?;
        Ok(())
    })();
    if let Err(error) = wrote {
        let _ = fs::remove_file(&pending);
        return Err(format!(
            "could not write session history {}: {error}. The development journal was not changed.",
            path.display()
        ));
    }
    let had_destination = path.is_file();
    if had_destination {
        if let Err(error) = fs::copy(path, &previous) {
            let _ = fs::remove_file(&pending);
            return Err(format!(
                "could not preserve the existing user data journal {}: {error}. It was not replaced.",
                path.display()
            ));
        }
    }
    if let Err(error) = fs::rename(&pending, path) {
        let _ = fs::remove_file(&pending);
        if had_destination {
            let _ = fs::remove_file(&previous);
        }
        return Err(format!(
            "could not replace session history {}: {error}. The development journal was not changed.",
            path.display()
        ));
    }
    let verified = fs::read_to_string(path)
        .ok()
        .is_some_and(|text| lines_of(&text) == records);
    if verified {
        let _ = fs::remove_file(&previous);
        return Ok(());
    }
    if had_destination && fs::copy(&previous, path).is_ok() {
        let restored = read_text(path)
            .ok()
            .zip(read_text(&previous).ok())
            .is_some_and(|(dest, prior)| covers(&lines_of(&dest), &lines_of(&prior)));
        if restored {
            let _ = fs::remove_file(&previous);
        }
        return Err(format!(
            "replaced session history {} did not match. The previous user data copy remains readable{}.",
            path.display(),
            if previous.is_file() {
                format!(" at {}", previous.display())
            } else {
                String::new()
            }
        ));
    }
    if !had_destination {
        let _ = fs::remove_file(path);
    }
    Err(format!(
        "wrote session history {} but could not read it back. The development journal was not changed.",
        path.display()
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch() -> PathBuf {
        use std::sync::atomic::{AtomicU64, Ordering};
        static N: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "pero-session-journal-{}-{}",
            std::process::id(),
            N.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        path
    }

    struct Temp(PathBuf);
    impl Drop for Temp {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn record(thread: &str, kind: &str, text: &str) -> String {
        let mut event = HostEvent::new(kind, None, text.into());
        event.thread_id = Some(thread.into());
        if kind == "session_created" {
            event.data = serde_json::json!({"agent":"fixture","workspace":"/work"});
        }
        serde_json::to_string(&event).unwrap()
    }

    fn write_journal(path: &Path, records: &[String]) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        let mut body = records.join("\n");
        if !records.is_empty() {
            body.push('\n');
        }
        fs::write(path, body).unwrap();
    }

    fn open_at(root: &Path, history: Option<&Path>, settings: Option<&Path>) -> SessionJournal {
        SessionJournal::open(root, history, settings, &root.join("user-data"))
    }

    #[test]
    fn merged_records_keep_every_copy() {
        let cases = [
            (vec![], vec!["a", "b"], vec!["a", "b"]),
            (vec!["a"], vec!["b", "a"], vec!["a", "b"]),
            (vec!["a", "a"], vec!["a"], vec!["a", "a"]),
            (vec!["b"], vec!["b", "b"], vec!["b", "b"]),
            (vec!["a", "c"], vec!["a", "b"], vec!["a", "c", "b"]),
        ];
        for (destination, legacy, expect) in cases {
            let destination: Vec<_> = destination.into_iter().map(str::to_owned).collect();
            let legacy: Vec<_> = legacy.into_iter().map(str::to_owned).collect();
            let expect: Vec<_> = expect.into_iter().map(str::to_owned).collect();
            assert_eq!(merge_records(&destination, &legacy), expect);
        }
    }

    #[test]
    fn migration_preserves_count_and_content_across_restart() {
        let root = Temp(scratch());
        let workspace = root.0.join("workspace");
        let legacy = workspace.join("target").join("sessions.jsonl");
        let destination = root.0.join("profile").join("sessions.jsonl");
        let records = vec![
            record("alpha", "session_created", ""),
            record("alpha", "user_message", "迁移前的内容"),
            record("beta", "session_created", ""),
            record("beta", "user_message", "第二条会话"),
            "{not-json".into(),
        ];
        write_journal(&legacy, &records);
        let legacy_bytes = fs::read(&legacy).unwrap();
        let opened = open_at(&workspace, Some(&destination), None);
        assert_eq!(opened.error, None);
        assert_eq!(opened.path, destination);
        let migrated = SessionSnapshot::read(&destination).unwrap();
        assert_eq!(migrated.sessions.len(), 2);
        assert_eq!(migrated.records, records);
        assert_eq!(fs::read(&legacy).unwrap(), legacy_bytes);
        let again = open_at(&workspace, Some(&destination), None);
        assert_eq!(again.error, None);
        assert_eq!(again.path, destination);
        assert_eq!(SessionSnapshot::read(&again.path).unwrap(), migrated);
        assert_eq!(fs::read(&legacy).unwrap(), legacy_bytes);
        let continued = record("alpha", "user_message", "重启后继续读取");
        let mut continued_records = migrated.records.clone();
        continued_records.push(continued.clone());
        write_journal(&destination, &continued_records);
        let restarted = open_at(&workspace, Some(&destination), None);
        assert_eq!(restarted.path, destination);
        let after = SessionSnapshot::read(&restarted.path).unwrap();
        assert_eq!(after.sessions, migrated.sessions);
        assert_eq!(after.records, continued_records);
        assert_eq!(fs::read(&legacy).unwrap(), legacy_bytes);
        assert!(
            !fs::read_to_string(&legacy)
                .unwrap()
                .contains("重启后继续读取")
        );
    }

    #[test]
    fn migration_keeps_sessions_already_in_user_data() {
        let root = Temp(scratch());
        let workspace = root.0.join("workspace");
        let legacy = workspace.join("target").join("sessions.jsonl");
        let destination = root.0.join("profile").join("sessions.jsonl");
        let existing = vec![
            record("kept", "session_created", ""),
            record("kept", "user_message", "用户目录里的会话"),
        ];
        let developed = vec![
            record("moved", "session_created", ""),
            record("moved", "user_message", "开发目录里的会话"),
        ];
        write_journal(&destination, &existing);
        write_journal(&legacy, &developed);
        let legacy_bytes = fs::read(&legacy).unwrap();
        let opened = open_at(&workspace, Some(&destination), None);
        assert_eq!(opened.error, None);
        let snapshot = SessionSnapshot::read(&opened.path).unwrap();
        assert_eq!(snapshot.sessions.len(), 2);
        assert!(snapshot.sessions.contains("kept") && snapshot.sessions.contains("moved"));
        assert_eq!(
            snapshot.records,
            existing
                .iter()
                .chain(&developed)
                .cloned()
                .collect::<Vec<_>>()
        );
        assert_eq!(fs::read(&legacy).unwrap(), legacy_bytes);
        let again = open_at(&workspace, Some(&destination), None);
        assert_eq!(SessionSnapshot::read(&again.path).unwrap(), snapshot);
        assert!(!sidecar(&destination, "previous").exists());
        assert!(!sidecar(&destination, "pending").exists());
    }

    #[test]
    fn failed_migration_leaves_the_previous_journal_readable() {
        let root = Temp(scratch());
        let workspace = root.0.join("workspace");
        let legacy = workspace.join("target").join("sessions.jsonl");
        let records = vec![
            record("only", "session_created", ""),
            record("only", "user_message", "唯一可读的会话"),
        ];
        write_journal(&legacy, &records);
        let legacy_bytes = fs::read(&legacy).unwrap();
        let blocked = root.0.join("blocked");
        fs::write(&blocked, "not a directory").unwrap();
        let destination = blocked.join("sessions.jsonl");
        let opened = open_at(&workspace, Some(&destination), None);
        let error = opened.error.expect("migration should fail");
        assert!(
            error.contains("not changed") || error.contains("left in place"),
            "{error}"
        );
        assert_eq!(opened.path, legacy);
        assert_eq!(
            SessionSnapshot::read(&opened.path).unwrap().records,
            records
        );
        assert_eq!(fs::read(&legacy).unwrap(), legacy_bytes);
        assert_eq!(fs::read_to_string(&blocked).unwrap(), "not a directory");
        assert!(!destination.exists());
        let again = open_at(&workspace, Some(&destination), None);
        assert!(again.error.is_some());
        assert_eq!(again.path, legacy);
        assert_eq!(SessionSnapshot::read(&again.path).unwrap().records, records);
        assert_eq!(fs::read(&legacy).unwrap(), legacy_bytes);

        let occupied = root.0.join("occupied");
        fs::create_dir_all(occupied.join("sessions.jsonl")).unwrap();
        fs::write(occupied.join("sessions.jsonl").join("keep"), "marker").unwrap();
        let directory = occupied.join("sessions.jsonl");
        let opened = open_at(&workspace, Some(&directory), None);
        assert!(opened.error.is_some());
        assert_eq!(opened.path, legacy);
        assert_eq!(
            fs::read_to_string(directory.join("keep")).unwrap(),
            "marker"
        );
        assert_eq!(fs::read(&legacy).unwrap(), legacy_bytes);
    }

    #[test]
    fn journal_location_and_interrupted_user_data_restore() {
        let root = Temp(scratch());
        let workspace = root.0.join("workspace");
        fs::create_dir_all(&workspace).unwrap();
        let settings = root.0.join("profile").join("runtime-settings.json");
        let beside_settings =
            SessionJournal::open(&workspace, None, Some(&settings), &root.0.join("unused"));
        assert_eq!(
            beside_settings.path,
            root.0.join("profile").join("sessions.jsonl")
        );
        assert_eq!(beside_settings.error, None);
        let explicit = root.0.join("custom").join("history.jsonl");
        let chosen = SessionJournal::open(
            &workspace,
            Some(&explicit),
            Some(&settings),
            &root.0.join("unused"),
        );
        assert_eq!(chosen.path, explicit);
        let fallback = SessionJournal::open(&workspace, None, None, &root.0.join("user-data"));
        assert_eq!(
            fallback.path,
            root.0.join("user-data").join("sessions.jsonl")
        );
        let legacy = workspace.join("target").join("sessions.jsonl");
        let records = vec![record("same", "session_created", "")];
        write_journal(&legacy, &records);
        let same = open_at(&workspace, Some(&legacy), None);
        assert_eq!(same.error, None);
        assert_eq!(same.path, legacy);
        assert_eq!(SessionSnapshot::read(&legacy).unwrap().records, records);

        let destination = root.0.join("restored").join("sessions.jsonl");
        let saved = vec![
            record("backup", "session_created", ""),
            record("backup", "user_message", "中断前的用户数据"),
        ];
        write_journal(&sidecar(&destination, "previous"), &saved);
        let restore_workspace = root.0.join("restore-workspace");
        fs::create_dir_all(&restore_workspace).unwrap();
        let restored = SessionJournal::open(
            &restore_workspace,
            Some(&destination),
            None,
            &root.0.join("unused"),
        );
        assert_eq!(restored.error, None);
        assert_eq!(restored.path, destination);
        assert_eq!(SessionSnapshot::read(&destination).unwrap().records, saved);
        assert!(!sidecar(&destination, "previous").exists());
    }
}
