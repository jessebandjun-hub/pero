//! Host-owned Provider, model, and default preset.
//! The runtime settings file stores the effective value. A rejected edit does not replace it.
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ModelSettings {
    pub provider: String,
    pub model: String,
    pub default_preset: String,
}

impl ModelSettings {
    pub fn valid(&self) -> bool {
        [
            (&self.provider, 128),
            (&self.model, 256),
            (&self.default_preset, 128),
        ]
        .into_iter()
        .all(|(value, max)| {
            !value.is_empty()
                && value.len() <= max
                && value == value.trim()
                && !value.chars().any(char::is_control)
        })
    }
}

#[derive(Debug, Deserialize)]
#[serde(tag = "method", deny_unknown_fields)]
pub enum ModelSettingsRequest {
    #[serde(rename = "settings.model.get")]
    Get,
    #[serde(rename = "settings.model")]
    Set { settings: ModelSettings },
}

#[cfg(test)]
mod tests {
    use super::*;

    fn settings(provider: &str, model: &str, default_preset: &str) -> ModelSettings {
        ModelSettings {
            provider: provider.into(),
            model: model.into(),
            default_preset: default_preset.into(),
        }
    }

    #[test]
    fn model_settings_accept_only_normalized_provider_model_and_preset() {
        assert!(settings("xai", "grok-code-fast-1", "coding").valid());
        for (provider, model, preset) in [
            ("", "grok-code-fast-1", "coding"),
            ("xai", "", "coding"),
            ("xai", "grok-code-fast-1", ""),
            (" xai", "grok-code-fast-1", "coding"),
            ("xai", " grok-code-fast-1", "coding"),
            ("xai", "grok-code-fast-1", "coding "),
            ("xai\n", "grok-code-fast-1", "coding"),
            ("xai", "grok\0fast", "coding"),
            ("xai", "grok-code-fast-1", "coding\t"),
            (&"p".repeat(129), "grok-code-fast-1", "coding"),
            ("xai", &"m".repeat(257), "coding"),
            ("xai", "grok-code-fast-1", &"s".repeat(129)),
        ] {
            assert!(
                !settings(provider, model, preset).valid(),
                "{provider:?} / {model:?} / {preset:?}"
            );
        }
    }

    #[test]
    fn model_settings_requests_reject_unknown_fields() {
        let saved: ModelSettingsRequest = serde_json::from_value(serde_json::json!({
            "method": "settings.model",
            "settings": {"provider": "xai", "model": "grok-code-fast-1", "default_preset": "coding"}
        }))
        .unwrap();
        let ModelSettingsRequest::Set { settings } = saved else {
            panic!("expected set");
        };
        assert!(settings.valid());
        assert!(
            serde_json::from_value::<ModelSettingsRequest>(serde_json::json!({
                "method": "settings.model.get"
            }))
            .is_ok()
        );
        for (label, value) in [
            (
                "unknown setting field",
                serde_json::json!({"method": "settings.model", "settings": {"provider": "xai", "model": "grok-code-fast-1", "default_preset": "coding", "token": "secret"}}),
            ),
            (
                "set without settings",
                serde_json::json!({"method": "settings.model"}),
            ),
        ] {
            assert!(
                serde_json::from_value::<ModelSettingsRequest>(value).is_err(),
                "{label}"
            );
        }
    }
}
