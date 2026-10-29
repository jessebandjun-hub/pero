//! Desktop page for the Host Provider, model, and default preset.
//!
//! `settings.model.get` and `settings.model` read and write the effective
//! value. Opening the page shows that value. `error.preferences` and
//! connection errors leave the typed draft on screen. Host save checks the
//! shape and writes the settings file; it does not authenticate a provider.

use pero_contracts::HostEvent;
use pero_contracts::model_settings::ModelSettings;
use serde::Serialize;
use serde_json::{Value, json};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ModelScreenState {
    Disabled,
    Loading,
    Empty,
    Error,
    Ready,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct ModelDraft {
    pub provider: String,
    pub model: String,
    pub default_preset: String,
}

impl ModelDraft {
    fn is_blank(&self) -> bool {
        self.provider.is_empty() && self.model.is_empty() && self.default_preset.is_empty()
    }

    fn matches(&self, saved: &ModelSettings) -> bool {
        self.provider == saved.provider
            && self.model == saved.model
            && self.default_preset == saved.default_preset
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ModelScreen {
    pub state: ModelScreenState,
    pub effective: Option<ModelSettings>,
    pub provider: String,
    pub model: String,
    pub default_preset: String,
    /// Typed text differs from the value the Host stored.
    pub draft: bool,
    pub error: Option<String>,
    pub actions_enabled: bool,
    pub saving: bool,
    pub can_retry: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ModelPage {
    /// `None` until a `model_settings` event. `Some(None)` means nothing is saved.
    loaded: Option<Option<ModelSettings>>,
    draft: Option<ModelDraft>,
    loading: bool,
    saving: bool,
    inflight_get: bool,
    show_effective: bool,
    error: Option<String>,
    fill: Option<ModelDraft>,
    applying_fill: bool,
}

impl Default for ModelPage {
    fn default() -> Self {
        Self {
            loaded: None,
            draft: None,
            loading: false,
            saving: false,
            inflight_get: false,
            show_effective: false,
            error: None,
            fill: None,
            applying_fill: false,
        }
    }
}

impl ModelPage {
    pub(crate) fn screen(&self, connected: bool) -> ModelScreen {
        let shown = self.shown();
        let state = if !connected {
            ModelScreenState::Disabled
        } else if self.loading {
            ModelScreenState::Loading
        } else if self.error.is_some() {
            ModelScreenState::Error
        } else if self.effective().is_none() {
            ModelScreenState::Empty
        } else {
            ModelScreenState::Ready
        };
        ModelScreen {
            state,
            effective: self.effective().cloned(),
            provider: shown.provider,
            model: shown.model,
            default_preset: shown.default_preset,
            draft: self.uncommitted(),
            error: self.error.clone(),
            actions_enabled: connected && !self.saving && !self.inflight_get && !self.loading,
            saving: self.saving,
            can_retry: connected
                && self.loaded.is_none()
                && self.error.is_some()
                && !self.saving
                && !self.inflight_get,
        }
    }

    pub(crate) fn uncommitted(&self) -> bool {
        let Some(draft) = &self.draft else {
            return false;
        };
        match self.effective() {
            Some(saved) => !draft.matches(saved),
            None => !draft.is_blank(),
        }
    }

    pub(crate) fn awaiting(&self) -> bool {
        self.saving || self.inflight_get
    }

    /// Ask the Host for the effective value. `show_effective` replaces the
    /// draft with the last known value before the response arrives.
    pub(crate) fn load(&mut self, connected: bool, show_effective: bool) -> Option<Value> {
        if self.saving || self.inflight_get {
            return None;
        }
        if show_effective && self.loaded.is_some() {
            self.draft = None;
            self.error = None;
            self.queue_fill_effective();
        }
        if !connected {
            self.loading = false;
            return None;
        }
        self.show_effective = show_effective;
        self.inflight_get = true;
        self.loading = self.loaded.is_none();
        Some(json!({"method": "settings.model.get"}))
    }

    pub(crate) fn note_typing(&mut self, provider: &str, model: &str, default_preset: &str) {
        if self.applying_fill || self.inflight_get || self.saving {
            return;
        }
        self.draft = Some(ModelDraft {
            provider: provider.to_string(),
            model: model.to_string(),
            default_preset: default_preset.to_string(),
        });
    }

    pub(crate) fn set_applying_fill(&mut self, applying: bool) {
        self.applying_fill = applying;
    }

    /// Record the typed fields and ask the Host to store them. A disconnected
    /// Host keeps the draft and sends nothing.
    pub(crate) fn save(
        &mut self,
        connected: bool,
        provider: String,
        model: String,
        default_preset: String,
    ) -> Option<Value> {
        let draft = ModelDraft {
            provider,
            model,
            default_preset,
        };
        self.draft = Some(draft.clone());
        if !connected || self.saving || self.inflight_get {
            return None;
        }
        self.saving = true;
        self.error = None;
        self.show_effective = false;
        Some(json!({
            "method": "settings.model",
            "settings": {
                "provider": draft.provider,
                "model": draft.model,
                "default_preset": draft.default_preset,
            }
        }))
    }

    pub(crate) fn receive(&mut self, event: &HostEvent) {
        match event.kind.as_str() {
            "model_settings" => match parsed_settings(&event.data) {
                Ok(settings) => self.apply_settings(settings),
                Err(()) => self.rejected("error.preferences"),
            },
            "error" if self.awaiting() => {
                let key = event
                    .message_key
                    .as_deref()
                    .filter(|key| !key.is_empty())
                    .unwrap_or("error.connection");
                self.rejected(key);
            }
            "disconnected" => self.fail_inflight(),
            _ => {}
        }
    }

    pub(crate) fn rejected(&mut self, key: &str) {
        self.saving = false;
        self.inflight_get = false;
        self.loading = false;
        self.show_effective = false;
        self.fill = None;
        self.error = Some(key.to_string());
    }

    pub(crate) fn take_input_fill(&mut self) -> Option<ModelDraft> {
        self.fill.take()
    }

    fn fail_inflight(&mut self) {
        let failed = self.saving || self.inflight_get;
        self.saving = false;
        self.inflight_get = false;
        self.loading = false;
        self.show_effective = false;
        self.fill = None;
        if failed {
            self.error = Some("error.connection".into());
        }
    }

    fn apply_settings(&mut self, settings: Option<ModelSettings>) {
        let from_save = self.saving;
        let show_effective = self.show_effective && !from_save;
        self.saving = false;
        self.inflight_get = false;
        self.loading = false;
        self.show_effective = false;
        self.loaded = Some(settings);
        if from_save && !self.landed() {
            self.fill = None;
            self.error = None;
            return;
        }
        if from_save || show_effective || self.draft.is_none() {
            self.draft = None;
            self.error = None;
            self.queue_fill_effective();
            return;
        }
        self.fill = None;
    }

    fn landed(&self) -> bool {
        match (&self.draft, self.effective()) {
            (None, _) => true,
            (Some(draft), Some(saved)) => draft.matches(saved),
            (Some(_), None) => false,
        }
    }

    fn effective(&self) -> Option<&ModelSettings> {
        self.loaded.as_ref().and_then(Option::as_ref)
    }

    fn shown(&self) -> ModelDraft {
        self.draft.clone().unwrap_or_else(|| self.effective_draft())
    }

    fn effective_draft(&self) -> ModelDraft {
        self.effective()
            .map(|saved| ModelDraft {
                provider: saved.provider.clone(),
                model: saved.model.clone(),
                default_preset: saved.default_preset.clone(),
            })
            .unwrap_or_default()
    }

    fn queue_fill_effective(&mut self) {
        self.fill = Some(self.effective_draft());
    }
}

fn parsed_settings(data: &Value) -> Result<Option<ModelSettings>, ()> {
    if data.is_null() {
        return Ok(None);
    }
    let settings: ModelSettings = serde_json::from_value(data.clone()).map_err(|_| ())?;
    settings.valid().then_some(Some(settings)).ok_or(())
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

    fn saved_event(settings: Option<ModelSettings>) -> HostEvent {
        let mut event = HostEvent::new("model_settings", None, String::new());
        if let Some(settings) = settings {
            event.data = serde_json::to_value(settings).unwrap();
        }
        event
    }

    fn preferences_error() -> HostEvent {
        HostEvent::message("error", None, "error.preferences")
    }

    #[test]
    fn page_shows_the_effective_value_and_keeps_a_rejected_draft() {
        let mut page = ModelPage::default();
        let saved = settings("xai", "grok-code-fast-1", "coding");

        assert_eq!(page.screen(false).state, ModelScreenState::Disabled);
        assert!(page.load(false, true).is_none());
        assert!(!page.screen(false).actions_enabled);

        let get = page.load(true, true).unwrap();
        assert_eq!(get["method"], "settings.model.get");
        assert!(page.awaiting());
        assert_eq!(page.screen(true).state, ModelScreenState::Loading);
        assert!(!page.screen(true).actions_enabled);

        page.receive(&saved_event(None));
        let empty = page.screen(true);
        assert_eq!(empty.state, ModelScreenState::Empty);
        assert!(empty.effective.is_none());
        assert!(
            empty.provider.is_empty() && empty.model.is_empty() && empty.default_preset.is_empty()
        );
        assert!(empty.actions_enabled);
        assert_eq!(page.take_input_fill(), Some(ModelDraft::default()));

        assert!(page.load(true, true).is_some());
        page.receive(&saved_event(Some(saved.clone())));
        let ready = page.screen(true);
        assert_eq!(ready.state, ModelScreenState::Ready);
        assert_eq!(
            (
                ready.provider.as_str(),
                ready.model.as_str(),
                ready.default_preset.as_str()
            ),
            ("xai", "grok-code-fast-1", "coding")
        );
        assert!(!ready.draft);
        assert_eq!(ready.effective, Some(saved.clone()));
        assert_eq!(
            page.take_input_fill(),
            Some(ModelDraft {
                provider: "xai".into(),
                model: "grok-code-fast-1".into(),
                default_preset: "coding".into(),
            })
        );

        page.note_typing("other", "grok-code-fast-1", "coding");
        let typing = page.screen(true);
        assert!(typing.draft);
        assert_eq!(typing.provider, "other");
        assert_eq!(
            typing.effective.as_ref().map(|item| item.provider.as_str()),
            Some("xai")
        );

        let save = page
            .save(
                true,
                "other".into(),
                "grok-code-fast-1".into(),
                "coding".into(),
            )
            .unwrap();
        assert_eq!(save["method"], "settings.model");
        assert_eq!(save["settings"]["provider"], "other");
        assert_eq!(save["settings"]["default_preset"], "coding");
        assert!(page.screen(true).saving);
        page.receive(&preferences_error());
        let rejected = page.screen(true);
        assert_eq!(rejected.state, ModelScreenState::Error);
        assert_eq!(rejected.error.as_deref(), Some("error.preferences"));
        assert_eq!(rejected.provider, "other");
        assert!(rejected.draft);
        assert_eq!(rejected.effective, Some(saved.clone()));
        assert!(page.take_input_fill().is_none());
        assert!(rejected.actions_enabled);

        assert!(
            page.save(true, "other".into(), "draft-model".into(), "coding".into())
                .is_some()
        );
        page.rejected("error.connection");
        let connection = page.screen(true);
        assert_eq!(connection.error.as_deref(), Some("error.connection"));
        assert_eq!(connection.model, "draft-model");
        assert_eq!(
            connection
                .effective
                .as_ref()
                .map(|item| item.model.as_str()),
            Some("grok-code-fast-1")
        );
        assert!(page.take_input_fill().is_none());

        assert!(
            page.save(true, "other".into(), "draft-model".into(), "offline".into())
                .is_some()
        );
        page.receive(&HostEvent::new("disconnected", None, String::new()));
        let offline = page.screen(false);
        assert_eq!(offline.state, ModelScreenState::Disabled);
        assert_eq!(offline.default_preset, "offline");
        assert_eq!(offline.error.as_deref(), Some("error.connection"));
        assert!(!offline.actions_enabled);

        let save = page
            .save(true, "xai".into(), "grok-4".into(), "review".into())
            .unwrap();
        assert_eq!(save["settings"]["model"], "grok-4");
        page.receive(&saved_event(Some(settings("xai", "grok-4", "review"))));
        let landed = page.screen(true);
        assert_eq!(landed.state, ModelScreenState::Ready);
        assert!(!landed.draft);
        assert!(landed.error.is_none());
        assert_eq!(landed.model, "grok-4");
        assert_eq!(landed.effective.unwrap().default_preset, "review");
        assert_eq!(page.take_input_fill().unwrap().model, "grok-4");
        page.receive(&preferences_error());
        assert!(page.screen(true).error.is_none());
        assert_eq!(page.screen(true).model, "grok-4");

        page.note_typing("xai", "grok-4", "scratch");
        assert!(page.screen(true).draft);
        assert!(page.load(true, true).is_some());
        let reopened = page.screen(true);
        assert!(!reopened.draft);
        assert_eq!(reopened.default_preset, "review");
        assert!(!reopened.actions_enabled);
        assert!(page.take_input_fill().is_some());
        page.receive(&saved_event(Some(settings("xai", "grok-4", "review"))));
        assert_eq!(page.screen(true).default_preset, "review");
        assert!(!page.screen(true).draft);
        assert!(page.screen(true).actions_enabled);

        page.note_typing("kept", "grok-4", "review");
        assert!(
            page.save(true, "kept".into(), "grok-4".into(), "review".into())
                .is_some()
        );
        page.receive(&preferences_error());
        assert!(page.load(true, false).is_some());
        page.receive(&saved_event(Some(settings("xai", "grok-4", "review"))));
        let kept = page.screen(true);
        assert_eq!(kept.provider, "kept");
        assert_eq!(kept.error.as_deref(), Some("error.preferences"));
        assert_eq!(kept.effective.unwrap().provider, "xai");
        assert!(page.take_input_fill().is_none());

        let mut bad = HostEvent::new("model_settings", None, String::new());
        bad.data = serde_json::json!({"provider": " xai"});
        page.receive(&bad);
        let still = page.screen(true);
        assert_eq!(still.provider, "kept");
        assert_eq!(still.effective.unwrap().provider, "xai");
        assert_eq!(still.error.as_deref(), Some("error.preferences"));

        page.note_typing("kept", "grok-4", "review");
        assert!(
            page.save(false, "kept".into(), "grok-4".into(), "local-only".into())
                .is_none()
        );
        assert_eq!(page.screen(false).default_preset, "local-only");
    }
}
