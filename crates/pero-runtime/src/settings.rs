//! Runtime-owned preferences. Desktop forwards changes rather than owning a second store.
use pero_contracts::credentials::CredentialSettingsRequest;
use pero_contracts::RemoteConnectionSettings;
use std::{
    io::{self, Write},
    path::Path,
};

pub fn load_locale(path: &Path) -> Option<String> {
    let value: serde_json::Value = serde_json::from_slice(&std::fs::read(path).ok()?).ok()?;
    value
        .get("locale")?
        .as_str()
        .map(|locale| pero_contracts::i18n::normalize(locale).into())
}

fn update(path: &Path, edit: impl FnOnce(&mut serde_json::Value)) -> io::Result<()> {
    let mut value = match std::fs::read(path) {
        Ok(bytes) => serde_json::from_slice::<serde_json::Value>(&bytes)?,
        Err(error) if error.kind() == io::ErrorKind::NotFound => serde_json::json!({}),
        Err(error) => return Err(error),
    };
    if !value.is_object() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Settings must be an object",
        ));
    }
    edit(&mut value);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let pending = path.with_extension(format!(
        "{}.{}.pending",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
    ));
    let result = (|| {
        let mut options = std::fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options.open(&pending)?;
        file.write_all(&serde_json::to_vec_pretty(&value)?)?;
        file.sync_all()?;
        std::fs::rename(&pending, path)
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&pending);
    }
    result
}

pub fn save_locale(path: &Path, locale: &str) -> io::Result<()> {
    update(path, |value| {
        value["locale"] = pero_contracts::i18n::normalize(locale).into()
    })
}

pub fn load_workbench(path: &Path) -> pero_contracts::workbench::WorkbenchPreferences {
    std::fs::read(path).ok()
        .and_then(|bytes| serde_json::from_slice::<serde_json::Value>(&bytes).ok())
        .and_then(|value| serde_json::from_value(value["workbench"].clone()).ok())
        .filter(pero_contracts::workbench::WorkbenchPreferences::valid)
        .unwrap_or_default()
}

pub fn save_workbench(path: &Path, preferences: &pero_contracts::workbench::WorkbenchPreferences) -> io::Result<()> {
    if !preferences.valid() {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, "Invalid workbench preferences"));
    }
    let preferences = serde_json::to_value(preferences)?;
    update(path, |value| value["workbench"] = preferences)
}

const UNREADABLE: &str = "Credential settings are unreadable";

fn read_credential_store(path: &Path) -> io::Result<pero_contracts::credentials::CredentialStore> {
    let bytes = match std::fs::read(path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return Ok(pero_contracts::credentials::CredentialStore::default());
        }
        Err(error) => return Err(error),
    };
    let value: serde_json::Value = serde_json::from_slice(&bytes)
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, UNREADABLE))?;
    if !value.is_object() {
        return Err(io::Error::new(io::ErrorKind::InvalidData, UNREADABLE));
    }
    match value.get("credentials") {
        None | Some(serde_json::Value::Null) => {
            Ok(pero_contracts::credentials::CredentialStore::default())
        }
        Some(section) => pero_contracts::credentials::CredentialStore::from_file_value(section)
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, UNREADABLE)),
    }
}

/// Apply a credential command. Rejected commands and unreadable files are not rewritten.
pub fn apply_credential_settings(
    path: &Path,
    request: &CredentialSettingsRequest,
) -> io::Result<pero_contracts::credentials::CredentialSettingsView> {
    let current = read_credential_store(path)?;
    let next = request
        .transition(&current)
        .map_err(|message| io::Error::new(io::ErrorKind::InvalidInput, message))?;
    if request.persists() {
        let section = next.to_file_value();
        update(path, |value| value["credentials"] = section)?;
    }
    Ok(next.view())
}

pub fn save_remote_connection(
    path: &Path,
    connection: Option<&RemoteConnectionSettings>,
) -> io::Result<()> {
    if connection.is_some_and(|connection| !connection.valid()) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Invalid remote preference",
        ));
    }
    let remote = serde_json::to_value(connection)?;
    update(path, |value| value["remote_connection"] = remote)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn preferences_survive_restart_and_preserve_other_settings() {
        let path =
            std::env::temp_dir().join(format!("pero-i18n-settings-{}.json", std::process::id()));
        std::fs::write(&path, r#"{"other":{"enabled":true}}"#).unwrap();
        save_locale(&path, "en").unwrap();
        assert_eq!(load_locale(&path).as_deref(), Some("en"));
        save_locale(&path, "zh-CN").unwrap();
        assert_eq!(load_locale(&path).as_deref(), Some("zh-CN"));
        let value: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        assert_eq!(value["other"]["enabled"], true);
        let connection = RemoteConnectionSettings {
            url: "https://app.openpero.com".into(),
            mode: pero_contracts::RemoteConnectionMode::Client,
            encrypted_token: "ZW5jcnlwdGVk".into(),
        };
        save_remote_connection(&path, Some(&connection)).unwrap();
        let preferences = pero_contracts::workbench::WorkbenchPreferences {
            appearance: pero_contracts::workbench::Appearance::Dark,
            projects: vec![pero_contracts::workbench::ProjectBinding {
                workspace: "/work/pero".into(), project_id: "pero".into(),
            }],
        };
        save_workbench(&path, &preferences).unwrap();
        assert_eq!(load_workbench(&path), preferences);
        save_locale(&path, "en").unwrap();
        let value: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        assert_eq!(value["remote_connection"]["mode"], "client");
        assert_eq!(
            value["remote_connection"]["encrypted_token"],
            "ZW5jcnlwdGVk"
        );
        save_remote_connection(&path, None).unwrap();
        let value: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        assert!(value["remote_connection"].is_null());
        assert_eq!(load_locale(&path).as_deref(), Some("en"));
        assert_eq!(load_workbench(&path), preferences);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
                0o600
            );
        }
        std::fs::write(&path, "invalid").unwrap();
        assert!(save_locale(&path, "en").is_err());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "invalid");
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn credential_saves_persist_and_rejected_saves_keep_the_file() {
        let path = std::env::temp_dir().join(format!(
            "pero-credential-settings-{}.json",
            std::process::id()
        ));
        let secret = "c2VjcmV0LW1hcmtlci1kb25vdC1lY2hv";
        let plaintext = "secret-marker-donot-echo";
        assert!(!path.exists());
        let empty = apply_credential_settings(&path, &CredentialSettingsRequest::get()).unwrap();
        assert!(empty.credentials.is_empty());
        assert!(!path.exists());
        std::fs::write(&path, r#"{"locale":"zh-CN","other":{"enabled":true}}"#).unwrap();
        let saved = apply_credential_settings(
            &path,
            &CredentialSettingsRequest::replace("github", "GitHub", secret),
        )
        .unwrap();
        assert!(saved.credentials[0].configured);
        let public = serde_json::to_string(&saved).unwrap();
        assert!(!public.contains(secret) && !public.contains(plaintext));
        let value: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        assert_eq!(value["locale"], "zh-CN");
        assert_eq!(value["other"]["enabled"], true);
        assert_eq!(value["credentials"][0]["secret"], secret);
        assert!(!value.to_string().contains(plaintext));
        let bytes = std::fs::read(&path).unwrap();
        assert_eq!(
            apply_credential_settings(&path, &CredentialSettingsRequest::get()).unwrap(),
            saved
        );
        let rejected = [
            CredentialSettingsRequest::replace("github", " ", secret),
            CredentialSettingsRequest::replace("github", "GitHub", "===="),
            CredentialSettingsRequest::replace("github", "GitHub", plaintext),
            CredentialSettingsRequest::remove("missing"),
        ];
        for request in rejected {
            let error = apply_credential_settings(&path, &request).unwrap_err();
            assert!(!error.to_string().contains(secret));
            assert!(!error.to_string().contains(plaintext));
            assert_eq!(std::fs::read(&path).unwrap(), bytes);
        }
        let unreadable = format!(r#"{{"locale":"en","credentials":{{"secret":"{secret}"}}}}"#);
        std::fs::write(&path, &unreadable).unwrap();
        let error = apply_credential_settings(
            &path,
            &CredentialSettingsRequest::replace("github", "GitHub", "YWJj"),
        )
        .unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
        assert!(!error.to_string().contains(secret));
        assert_eq!(std::fs::read_to_string(&path).unwrap(), unreadable);
        std::fs::write(&path, "invalid").unwrap();
        assert!(apply_credential_settings(&path, &CredentialSettingsRequest::get()).is_err());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "invalid");
        std::fs::remove_file(&path).unwrap();
    }
}
