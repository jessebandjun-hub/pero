//! Credential slots stored by the Host. A slot's secret envelope is write-only:
//! views, errors and `Debug` report connection status and never the envelope.
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

const INVALID: &str = "Invalid credential settings";
const MAX_SLOTS: usize = 32;

/// Public connection status. `Configured` means a secret envelope is stored.
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CredentialConnectionStatus {
    Configured,
}

/// One credential as the settings page may show it. No secret field exists.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct CredentialView {
    pub id: String,
    pub label: String,
    pub configured: bool,
    pub status: CredentialConnectionStatus,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct CredentialSettingsView {
    pub credentials: Vec<CredentialView>,
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct StoredCredential {
    id: String,
    label: String,
    secret: String,
}

/// Private credential file state. Serialize it only through [`Self::to_file_value`].
#[derive(Clone, Default)]
pub struct CredentialStore {
    entries: Vec<StoredCredential>,
}

impl std::fmt::Debug for CredentialStore {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("CredentialStore")
            .field("count", &self.entries.len())
            .field(
                "ids",
                &self
                    .entries
                    .iter()
                    .map(|entry| entry.id.as_str())
                    .collect::<Vec<_>>(),
            )
            .finish()
    }
}

impl CredentialStore {
    pub fn view(&self) -> CredentialSettingsView {
        CredentialSettingsView {
            credentials: self
                .entries
                .iter()
                .map(|entry| CredentialView {
                    id: entry.id.clone(),
                    label: entry.label.clone(),
                    configured: true,
                    status: CredentialConnectionStatus::Configured,
                })
                .collect(),
        }
    }

    /// Settings-file array. This is the only value that carries secret envelopes.
    pub fn to_file_value(&self) -> serde_json::Value {
        serde_json::to_value(&self.entries).expect("stored credentials serialize")
    }

    pub fn from_file_value(value: &serde_json::Value) -> Result<Self, &'static str> {
        if value.is_null() {
            return Ok(Self::default());
        }
        let entries = serde_json::from_value(value.clone()).map_err(|_| INVALID)?;
        Self::from_entries(entries)
    }

    pub fn replace(&self, id: &str, label: &str, secret: &str) -> Result<Self, &'static str> {
        if !id_ok(id) || !label_ok(label) || !envelope(secret) {
            return Err(INVALID);
        }
        let mut entries = self.entries.clone();
        if let Some(entry) = entries.iter_mut().find(|entry| entry.id == id) {
            entry.label = label.to_owned();
            entry.secret = secret.to_owned();
        } else if entries.len() == MAX_SLOTS {
            return Err(INVALID);
        } else {
            entries.push(StoredCredential {
                id: id.to_owned(),
                label: label.to_owned(),
                secret: secret.to_owned(),
            });
        }
        Ok(Self { entries })
    }

    pub fn remove(&self, id: &str) -> Result<Self, &'static str> {
        if !id_ok(id) || !self.entries.iter().any(|entry| entry.id == id) {
            return Err(INVALID);
        }
        Ok(Self {
            entries: self
                .entries
                .iter()
                .filter(|entry| entry.id != id)
                .cloned()
                .collect(),
        })
    }

    fn from_entries(entries: Vec<StoredCredential>) -> Result<Self, &'static str> {
        if entries.len() > MAX_SLOTS {
            return Err(INVALID);
        }
        let mut seen = HashSet::new();
        for entry in &entries {
            if !id_ok(&entry.id)
                || !label_ok(&entry.label)
                || !envelope(&entry.secret)
                || !seen.insert(entry.id.as_str())
            {
                return Err(INVALID);
            }
        }
        Ok(Self { entries })
    }
}

#[derive(Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CredentialOperation {
    Replace,
    Remove,
}

/// Host credential command. `Debug` reports that a secret was present, not its value.
#[derive(Deserialize)]
#[serde(tag = "method", deny_unknown_fields)]
pub enum CredentialSettingsRequest {
    #[serde(rename = "settings.credentials.get")]
    Get,
    #[serde(rename = "settings.credentials")]
    Update {
        operation: CredentialOperation,
        id: String,
        #[serde(default)]
        label: String,
        #[serde(default)]
        secret: String,
    },
}

impl CredentialSettingsRequest {
    pub fn get() -> Self {
        Self::Get
    }

    pub fn replace(
        id: impl Into<String>,
        label: impl Into<String>,
        secret: impl Into<String>,
    ) -> Self {
        Self::Update {
            operation: CredentialOperation::Replace,
            id: id.into(),
            label: label.into(),
            secret: secret.into(),
        }
    }

    pub fn remove(id: impl Into<String>) -> Self {
        Self::Update {
            operation: CredentialOperation::Remove,
            id: id.into(),
            label: String::new(),
            secret: String::new(),
        }
    }

    /// `(state, input) -> next state`. Reads and writes nothing.
    pub fn transition(&self, store: &CredentialStore) -> Result<CredentialStore, &'static str> {
        match self {
            Self::Get => Ok(store.clone()),
            Self::Update {
                operation: CredentialOperation::Replace,
                id,
                label,
                secret,
            } => store.replace(id, label, secret),
            Self::Update {
                operation: CredentialOperation::Remove,
                id,
                label,
                secret,
            } => {
                if !label.is_empty() || !secret.is_empty() {
                    return Err(INVALID);
                }
                store.remove(id)
            }
        }
    }

    pub fn persists(&self) -> bool {
        !matches!(self, Self::Get)
    }
}

impl std::fmt::Debug for CredentialSettingsRequest {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Get => formatter.write_str("Get"),
            Self::Update {
                operation,
                id,
                label,
                secret,
            } => formatter
                .debug_struct("Update")
                .field("operation", operation)
                .field("id", id)
                .field("label", label)
                .field("secret_present", &!secret.is_empty())
                .finish(),
        }
    }
}

fn id_ok(id: &str) -> bool {
    let mut chars = id.chars();
    matches!(chars.next(), Some(first) if first.is_ascii_alphanumeric())
        && id.len() <= 64
        && chars.all(|char| char.is_ascii_alphanumeric() || matches!(char, '.' | '_' | '-'))
}

fn label_ok(label: &str) -> bool {
    let count = label.chars().count();
    (1..=128).contains(&count) && label == label.trim() && !label.chars().any(char::is_control)
}

/// Opaque envelope, same alphabet and padding rules as a stored remote token.
fn envelope(secret: &str) -> bool {
    let body = secret.trim_end_matches('=');
    (4..=4096).contains(&secret.len())
        && secret.len().is_multiple_of(4)
        && secret.len() - body.len() <= 2
        && !body.is_empty()
        && body
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'+' | b'/'))
}

#[cfg(test)]
mod tests {
    use super::*;

    const SECRET: &str = "c2VjcmV0LW1hcmtlci1kb25vdC1lY2hv";
    const PLAINTEXT: &str = "secret-marker-donot-echo";
    const NEXT: &str = "bmV4dC1zZWNyZXQ=";

    fn leaked(value: &str) -> bool {
        value.contains(SECRET) || value.contains(PLAINTEXT) || value.contains(NEXT)
    }

    #[test]
    fn replace_remove_and_reload_keep_secrets_out_of_views() {
        let store = CredentialStore::default();
        let saved = store.replace("github", "GitHub", SECRET).unwrap();
        let view = saved.view();
        assert_eq!(view.credentials.len(), 1);
        assert!(view.credentials[0].configured);
        assert_eq!(
            view.credentials[0].status,
            CredentialConnectionStatus::Configured
        );
        let public = serde_json::to_string(&view).unwrap();
        assert!(!leaked(&public));
        assert!(!leaked(&format!("{saved:?}")));
        let file = saved.to_file_value();
        assert_eq!(file[0]["secret"], SECRET);
        assert!(!file.to_string().contains(PLAINTEXT));
        let loaded = CredentialStore::from_file_value(&file).unwrap();
        assert_eq!(loaded.view(), view);
        let replaced = loaded.replace("github", "令牌", NEXT).unwrap();
        assert_eq!(replaced.view().credentials.len(), 1);
        assert_eq!(replaced.view().credentials[0].label, "令牌");
        assert!(!leaked(&serde_json::to_string(&replaced.view()).unwrap()));
        assert_eq!(replaced.to_file_value()[0]["secret"], NEXT);
        assert!(!replaced.to_file_value().to_string().contains(SECRET));
        let removed = replaced.remove("github").unwrap();
        assert!(removed.view().credentials.is_empty());
        assert_eq!(removed.to_file_value(), serde_json::json!([]));
    }

    #[test]
    fn rejected_commands_leave_the_store_unchanged_and_do_not_echo_secrets() {
        let mut store = CredentialStore::default();
        for index in 0..MAX_SLOTS {
            store = store
                .replace(&format!("slot{index}"), "Saved", "YWJj")
                .unwrap();
        }
        let file = store.to_file_value();
        let too_long = "YWJj".repeat(1025);
        let cases = [
            ("", "Saved", SECRET),
            ("../github", "Saved", SECRET),
            ("slot0", " ", SECRET),
            ("slot0", " padded", SECRET),
            ("slot0", "Saved", "===="),
            ("slot0", "Saved", PLAINTEXT),
            ("slot0", "Saved", too_long.as_str()),
            ("overflow", "Saved", SECRET),
        ];
        for (id, label, secret) in cases {
            let error = store.replace(id, label, secret).unwrap_err();
            assert_eq!(error, INVALID);
            assert!(!leaked(error));
            assert_eq!(store.to_file_value(), file);
        }
        assert_eq!(store.remove("missing").unwrap_err(), INVALID);
        assert_eq!(store.to_file_value(), file);
        let request = CredentialSettingsRequest::replace("slot0", "Saved", "====");
        assert_eq!(request.transition(&store).unwrap_err(), INVALID);
        assert!(!leaked(&format!("{request:?}")));
        let echoed: CredentialSettingsRequest = serde_json::from_value(serde_json::json!({
            "method": "settings.credentials",
            "operation": "remove",
            "id": "slot0",
            "secret": SECRET,
        }))
        .unwrap();
        assert_eq!(echoed.transition(&store).unwrap_err(), INVALID);
        assert!(!leaked(&format!("{echoed:?}")));
        assert!(
            serde_json::from_value::<CredentialSettingsRequest>(serde_json::json!({
                "method": "settings.credentials",
                "operation": "replace",
                "id": "github",
                "label": "GitHub",
                "secret": SECRET,
                "token": PLAINTEXT,
            }))
            .is_err()
        );
        assert!(
            CredentialStore::from_file_value(&serde_json::json!([
                {"id": "github", "label": "GitHub", "secret": SECRET},
                {"id": "github", "label": "Other", "secret": NEXT},
            ]))
            .is_err()
        );
        assert!(CredentialStore::from_file_value(&serde_json::json!({"secret": SECRET})).is_err());
        assert_eq!(store.to_file_value(), file);
    }
}
