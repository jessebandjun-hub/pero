//! A failed restart of an installed App, stored next to its domain data.
use super::definition::AppName;
use serde::{Deserialize, Serialize};
use std::{fs, path::Path};

/// Host record of one failed App recovery. Domain files in the parent data
/// directory are not part of this record and are left unchanged.
#[derive(Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RecoveryFailure {
    pub name: AppName,
    pub data: String,
    pub error: String,
}

impl RecoveryFailure {
    pub(crate) fn path(data_dir: &Path) -> std::path::PathBuf {
        data_dir.join(".pero").join("recovery-error.json")
    }

    /// Persist `error` for `name` under `data_dir` and return the record path.
    pub(crate) fn store(
        data_dir: &Path,
        name: &AppName,
        error: &str,
    ) -> Result<std::path::PathBuf, String> {
        let path = Self::path(data_dir);
        let directory = path
            .parent()
            .ok_or("Recovery record needs a parent directory")?;
        fs::create_dir_all(directory).map_err(|error| error.to_string())?;
        let record = Self {
            name: name.clone(),
            data: data_dir.display().to_string(),
            error: error.to_string(),
        };
        let bytes = serde_json::to_vec_pretty(&record).map_err(|error| error.to_string())?;
        let pending = directory.join("recovery-error.pending");
        fs::write(&pending, &bytes)
            .and_then(|_| fs::rename(&pending, &path))
            .map_err(|error| error.to_string())?;
        Ok(path)
    }

    pub(crate) fn clear(data_dir: &Path) -> Result<(), String> {
        let directory = data_dir.join(".pero");
        for name in ["recovery-error.json", "recovery-error.pending"] {
            let file = directory.join(name);
            if file.is_file() {
                fs::remove_file(&file).map_err(|error| error.to_string())?;
            }
        }
        let _ = fs::remove_dir(directory);
        Ok(())
    }
}
