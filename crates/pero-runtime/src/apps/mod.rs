//! Local App packages and the single owner of their installed state and backends.
mod definition;
mod process;
mod recovery;
pub use definition::{AppDefinition, AppName, AppSummary};
use process::AppProcess;
pub use process::BackendAccess;
use recovery::RecoveryFailure;
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct InstallRecord {
    enabled: bool,
}

struct AppInstallation {
    definition: AppDefinition,
    enabled: bool,
}

#[derive(Serialize)]
pub struct AppAccess {
    pub root: PathBuf,
    pub entry: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub backend: Option<BackendAccess>,
}

/// Imperative shell: owns disk state and backend children. Domain declarations
/// are parsed once, at install or startup, before entering this store.
pub struct AppStore {
    root: PathBuf,
    data: PathBuf,
    installed: BTreeMap<AppName, AppInstallation>,
    processes: BTreeMap<AppName, AppProcess>,
}

impl AppStore {
    pub fn open(root: PathBuf) -> Result<Self, String> {
        fs::create_dir_all(&root).map_err(|error| error.to_string())?;
        let root = dunce::canonicalize(root).map_err(|error| error.to_string())?;
        let data = root
            .parent()
            .ok_or("App root needs a parent directory")?
            .join("app-data");
        let mut installed = BTreeMap::new();
        for entry in fs::read_dir(&root).map_err(|error| error.to_string())? {
            let entry = entry.map_err(|error| error.to_string())?;
            let Some(directory) = entry.file_name().to_str().map(str::to_owned) else {
                continue;
            };
            if directory.starts_with('.') {
                continue;
            }
            if !entry
                .file_type()
                .map_err(|error| error.to_string())?
                .is_dir()
            {
                continue;
            }
            let record_path = entry.path().join(".installation.json");
            if !record_path.is_file() {
                continue;
            }
            let definition = AppDefinition::parse(
                &fs::read(entry.path().join("app.json")).map_err(|error| error.to_string())?,
            )?;
            if definition.name.as_str() != directory {
                return Err("Installed App identity differs from its directory".into());
            }
            let record: InstallRecord =
                serde_json::from_slice(&fs::read(record_path).map_err(|error| error.to_string())?)
                    .map_err(|error| error.to_string())?;
            installed.insert(
                definition.name.clone(),
                AppInstallation {
                    definition,
                    enabled: record.enabled,
                },
            );
        }
        Ok(Self {
            root,
            data,
            installed,
            processes: BTreeMap::new(),
        })
    }

    pub fn list(&self) -> Vec<AppSummary> {
        self.installed
            .values()
            .map(|app| app.definition.summary(app.enabled))
            .collect()
    }

    /// Trusted composition-root catalog. Seed once; uninstall is durable across
    /// restart rather than silently restoring an App the user removed.
    pub fn seed_builtins(&mut self, catalog: &Path) -> Result<(), String> {
        let marker = self.root.join(".builtins-seeded");
        if marker.exists() {
            return Ok(());
        }
        if !catalog.is_dir() {
            return Ok(());
        }
        if catalog.is_dir() {
            for entry in fs::read_dir(catalog).map_err(|error| error.to_string())? {
                let entry = entry.map_err(|error| error.to_string())?;
                if !entry
                    .file_type()
                    .map_err(|error| error.to_string())?
                    .is_dir()
                    || !entry.path().join("app.json").is_file()
                {
                    continue;
                }
                // Already-installed builtins are retained with their user's
                // enable state if a prior seed pass stopped partway through.
                let directory = entry.file_name().to_string_lossy().into_owned();
                if self.installed.keys().any(|name| name.as_str() == directory) {
                    continue;
                }
                let name = self.install_package(&entry.path())?;
                self.set_enabled(&name, true)?;
            }
        }
        fs::write(marker, b"1\n").map_err(|error| error.to_string())
    }

    pub fn install(&mut self, source: &Path) -> Result<Vec<AppSummary>, String> {
        self.install_package(source)?;
        Ok(self.list())
    }

    fn install_package(&mut self, source: &Path) -> Result<AppName, String> {
        if fs::symlink_metadata(source)
            .map_err(|error| error.to_string())?
            .file_type()
            .is_symlink()
        {
            return Err("App installation source cannot be a symlink".into());
        }
        let source = dunce::canonicalize(source).map_err(|error| error.to_string())?;
        if source.starts_with(&self.root) || self.root.starts_with(&source) {
            return Err("App source and managed installation root must be separate".into());
        }
        let definition = AppDefinition::parse(
            &fs::read(source.join("app.json")).map_err(|error| error.to_string())?,
        )?;
        let target = self.root.join(definition.name.as_str());
        if target.exists() {
            return Err(
                "App is already installed; uninstall it before installing another version".into(),
            );
        }
        let staging = self.root.join(format!(".install-{}", uuid::Uuid::new_v4()));
        fs::create_dir(&staging).map_err(|error| error.to_string())?;
        let copied = (|| {
            let mut total = (0usize, 0u64);
            copy_package(&source, &staging, &mut total)?;
            for entry in definition
                .contributes
                .panels
                .iter()
                .map(|panel| panel.entry.as_str())
                .chain(
                    definition
                        .backend
                        .iter()
                        .map(|backend| backend.entry.as_str()),
                )
            {
                if !staging.join(entry).is_file() {
                    return Err(format!("App entry is missing: {entry}"));
                }
            }
            write_record(&staging, false)?;
            fs::rename(&staging, &target).map_err(|error| error.to_string())
        })();
        if let Err(error) = copied {
            let _ = fs::remove_dir_all(staging);
            return Err(error);
        }
        let name = definition.name.clone();
        self.installed.insert(
            name.clone(),
            AppInstallation {
                definition,
                enabled: false,
            },
        );
        Ok(name)
    }

    pub fn set_enabled(
        &mut self,
        name: &AppName,
        enabled: bool,
    ) -> Result<Vec<AppSummary>, String> {
        let app = self.installed.get_mut(name).ok_or("App is not installed")?;
        write_record(&self.root.join(name.as_str()), enabled)?;
        app.enabled = enabled;
        if !enabled {
            self.processes.remove(name);
        }
        Ok(self.list())
    }

    pub fn uninstall(&mut self, name: &AppName) -> Result<Vec<AppSummary>, String> {
        if !self.installed.contains_key(name) {
            return Err("App is not installed".into());
        }
        self.processes.remove(name);
        fs::remove_dir_all(self.root.join(name.as_str())).map_err(|error| error.to_string())?;
        self.installed.remove(name);
        // Domain data is deliberately retained outside the installed package.
        Ok(self.list())
    }

    /// Open an enabled App. A backend that failed to start or has exited is
    /// started again against the existing `app-data/<name>` directory. Domain
    /// files stay in place. A failed restart is written to
    /// `app-data/<name>/.pero/recovery-error.json`; the returned error includes
    /// that path. A later successful open removes the record.
    pub fn resolve(&mut self, name: &AppName) -> Result<AppAccess, String> {
        let (enabled, entry, definition) = {
            let app = self.installed.get(name).ok_or("App is not installed")?;
            (
                app.enabled,
                app.definition.contributes.panels[0].entry.clone(),
                app.definition.clone(),
            )
        };
        if !enabled {
            return Err("App is disabled; enable it before opening".into());
        }
        let root = self.root.join(name.as_str());
        let data = self.data.join(name.as_str());
        if self
            .processes
            .get_mut(name)
            .is_some_and(|process| process.access().is_err())
        {
            self.processes.remove(name);
        }
        if definition.backend.is_some() && !self.processes.contains_key(name) {
            match AppProcess::start(&definition, &root, &data) {
                Ok(process) => {
                    self.processes.insert(name.clone(), process);
                }
                Err(error) => return Err(note_recovery_failure(name, &data, &error)),
            }
        }
        let backend = match self
            .processes
            .get_mut(name)
            .map(AppProcess::access)
            .transpose()
        {
            Ok(backend) => backend,
            Err(error) => {
                self.processes.remove(name);
                return Err(note_recovery_failure(name, &data, &error));
            }
        };
        RecoveryFailure::clear(&data)?;
        Ok(AppAccess {
            root,
            entry,
            backend,
        })
    }
}

fn note_recovery_failure(name: &AppName, data: &Path, error: &str) -> String {
    match RecoveryFailure::store(data, name, error) {
        Ok(path) => format!("{error}; recovery record: {}", path.display()),
        Err(write) => format!("{error}; recovery record was not saved: {write}"),
    }
}

fn write_record(root: &Path, enabled: bool) -> Result<(), String> {
    let pending = root.join(".installation.pending");
    fs::write(
        &pending,
        serde_json::to_vec(&InstallRecord { enabled }).map_err(|error| error.to_string())?,
    )
    .and_then(|_| fs::rename(pending, root.join(".installation.json")))
    .map_err(|error| error.to_string())
}

fn copy_package(source: &Path, target: &Path, total: &mut (usize, u64)) -> Result<(), String> {
    for entry in fs::read_dir(source).map_err(|error| error.to_string())? {
        let entry = entry.map_err(|error| error.to_string())?;
        let name = entry.file_name();
        if matches!(
            name.to_str(),
            Some("node_modules" | ".git" | "__pycache__" | ".venv" | ".DS_Store")
        ) {
            continue;
        }
        if name.to_string_lossy().starts_with(".installation") {
            return Err("Package contains Host installation metadata".into());
        }
        let kind = entry.file_type().map_err(|error| error.to_string())?;
        let destination = target.join(&name);
        total.0 += 1;
        if total.0 > 10000 {
            return Err("App package exceeds 10,000 entries".into());
        }
        if kind.is_symlink() {
            return Err("App package cannot contain symlinks".into());
        }
        if kind.is_dir() {
            fs::create_dir(&destination).map_err(|error| error.to_string())?;
            copy_package(&entry.path(), &destination, total)?;
        } else if kind.is_file() {
            total.1 += entry.metadata().map_err(|error| error.to_string())?.len();
            if total.1 > 256 * 1024 * 1024 {
                return Err("App package exceeds 256 MiB".into());
            }
            fs::copy(entry.path(), destination).map_err(|error| error.to_string())?;
        } else {
            return Err("App package contains a special file".into());
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests;
