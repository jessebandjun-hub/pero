//! Runtime-owned preferences. Desktop forwards changes rather than owning a second store.
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

#[derive(Debug)]
pub enum ChannelSettingsError {
    Rejected(pero_contracts::channel::ChannelFailure),
    Unavailable,
}

fn read_channels(path: &Path) -> io::Result<pero_contracts::channel::ChannelCatalog> {
    let bytes = match std::fs::read(path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return Ok(pero_contracts::channel::ChannelCatalog::default());
        }
        Err(_) => return Err(io::Error::other("channel settings are unreadable")),
    };
    let value: serde_json::Value = serde_json::from_slice(&bytes)
        .map_err(|_| io::Error::other("channel settings are unreadable"))?;
    if !value.is_object() {
        return Err(io::Error::other("channel settings are unreadable"));
    }
    match value.get("channels") {
        None | Some(serde_json::Value::Null) => {
            Ok(pero_contracts::channel::ChannelCatalog::default())
        }
        Some(section) => pero_contracts::channel::ChannelCatalog::from_file_value(section)
            .map_err(|_| io::Error::other("channel settings are unreadable")),
    }
}

/// Apply a channel command. Rejected commands and unreadable files are not rewritten.
pub fn apply_channel_settings(
    path: &Path,
    request: &pero_contracts::channel::ChannelSettingsRequest,
) -> Result<pero_contracts::channel::ChannelCatalogView, ChannelSettingsError> {
    let current = read_channels(path).map_err(|_| ChannelSettingsError::Unavailable)?;
    match request.apply(&current) {
        Ok(change) => {
            if change.persists() {
                let section = change.catalog().to_file_value();
                update(path, |value| value["channels"] = section)
                    .map_err(|_| ChannelSettingsError::Unavailable)?;
            }
            Ok(change.catalog().view())
        }
        Err(failure) => Err(ChannelSettingsError::Rejected(failure)),
    }
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
    fn channel_settings_survive_restart_and_rejected_credentials_keep_the_file() {
        use pero_contracts::channel::{
            ChannelFailureReason, ChannelKind, ChannelRecovery, ChannelSettingsRequest,
            ChannelStatus,
        };
        let path =
            std::env::temp_dir().join(format!("pero-channel-settings-{}.json", std::process::id()));
        std::fs::write(&path, r#"{"locale":"zh-CN","other":{"enabled":true}}"#).unwrap();
        let empty = apply_channel_settings(&path, &ChannelSettingsRequest::get()).unwrap();
        assert!(empty.channels.is_empty());
        let credential = {
            // "slack-bot-token" as a standard envelope, matching the domain table.
            "c2xhY2stYm90LXRva2Vu".to_string()
        };
        let rejected = "cmVqZWN0Ondyb25nLXRva2Vu".to_string();
        let failure = apply_channel_settings(
            &path,
            &ChannelSettingsRequest::set(
                "slack-main",
                ChannelKind::Slack,
                "https://slack.example/api",
                "T0123",
                &rejected,
            ),
        )
        .unwrap_err();
        let ChannelSettingsError::Rejected(failure) = failure else {
            panic!("expected rejection");
        };
        assert_eq!(failure.reason, ChannelFailureReason::CredentialRejected);
        assert_eq!(failure.recovery, ChannelRecovery::ReplaceCredential);
        let before = std::fs::read(&path).unwrap();
        assert!(
            apply_channel_settings(
                &path,
                &ChannelSettingsRequest::set(
                    "slack-main",
                    ChannelKind::Slack,
                    "https://slack.example/api",
                    "T0123",
                    "====",
                ),
            )
            .is_err()
        );
        assert_eq!(std::fs::read(&path).unwrap(), before);
        let saved = apply_channel_settings(
            &path,
            &ChannelSettingsRequest::set(
                "slack-main",
                ChannelKind::Slack,
                "https://slack.example/api",
                "工作区",
                &credential,
            ),
        )
        .unwrap();
        assert_eq!(saved.channels[0].status, ChannelStatus::Disconnected);
        assert_eq!(saved.channels[0].account, "工作区");
        let connected =
            apply_channel_settings(&path, &ChannelSettingsRequest::connect("slack-main")).unwrap();
        assert_eq!(connected.channels[0].status, ChannelStatus::Connected);
        let value: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        assert_eq!(value["locale"], "zh-CN");
        assert_eq!(value["other"]["enabled"], true);
        assert_eq!(value["channels"][0]["credential"], credential);
        assert!(
            serde_json::to_string(&connected)
                .unwrap()
                .contains("工作区")
        );
        assert!(
            !serde_json::to_string(&connected)
                .unwrap()
                .contains(&credential)
        );
        let disconnected =
            apply_channel_settings(&path, &ChannelSettingsRequest::disconnect("slack-main"))
                .unwrap();
        assert_eq!(disconnected.channels[0].status, ChannelStatus::Disconnected);
        let reloaded = apply_channel_settings(&path, &ChannelSettingsRequest::get()).unwrap();
        assert_eq!(reloaded, disconnected);
        let unreadable =
            format!(r#"{{"locale":"zh-CN","channels":{{"credential":"{credential}"}}}}"#);
        std::fs::write(&path, &unreadable).unwrap();
        assert!(matches!(
            apply_channel_settings(&path, &ChannelSettingsRequest::get()),
            Err(ChannelSettingsError::Unavailable)
        ));
        assert_eq!(std::fs::read_to_string(&path).unwrap(), unreadable);
        std::fs::remove_file(path).unwrap();
    }
}
