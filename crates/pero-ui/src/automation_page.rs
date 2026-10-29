//! Settings page for automations the Host already stores.
//!
//! Pause, resume, and update send `settings.automation` commands. The page
//! shows `automation_settings.data` and keeps a typed draft when
//! `automation_settings_error` arrives, because that event carries no catalog.
//! Creating an automation is not a command.

use pero_contracts::automation::{AutomationCatalog, AutomationDraft, AutomationEditor};
use pero_contracts::HostEvent;
use serde::Serialize;
use serde_json::{Value, json};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum AutomationScreenState {
    Disabled,
    Loading,
    Empty,
    Error,
    Ready,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum AutomationPending {
    Pause,
    Resume,
    Update,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub(crate) struct AutomationRow {
    pub id: String,
    pub name: String,
    pub instruction: String,
    pub paused: bool,
    /// Typed text differs from the catalog the Host stored.
    pub draft: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AutomationScreen {
    pub state: AutomationScreenState,
    pub data: AutomationCatalog,
    pub rows: Vec<AutomationRow>,
    pub error: Option<String>,
    pub selected: Option<String>,
    pub actions_enabled: bool,
    pub pending: Option<AutomationPending>,
    pub can_retry: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct AutomationPage {
    editor: Option<AutomationEditor>,
    loading: bool,
    error: Option<String>,
    selected: Option<String>,
    pending: Option<AutomationPending>,
    fill: Option<(String, String)>,
}

impl Default for AutomationPage {
    fn default() -> Self {
        Self {
            editor: None,
            loading: false,
            error: None,
            selected: None,
            pending: None,
            fill: None,
        }
    }
}

impl AutomationPage {
    pub(crate) fn screen(&self, connected: bool) -> AutomationScreen {
        let data = self
            .editor
            .as_ref()
            .map(|editor| editor.saved().clone())
            .unwrap_or_default();
        let rows = self.rows();
        let actions_enabled = connected
            && self.pending.is_none()
            && !self.loading
            && self.selected.is_some();
        let can_retry = connected
            && self.editor.is_none()
            && self.error.is_some()
            && !self.loading
            && self.pending.is_none();
        let state = if !connected {
            AutomationScreenState::Disabled
        } else if self.editor.is_none() && self.loading {
            AutomationScreenState::Loading
        } else if self.editor.is_none() {
            AutomationScreenState::Error
        } else if data.automations.is_empty() {
            AutomationScreenState::Empty
        } else if self.error.is_some() {
            AutomationScreenState::Error
        } else {
            AutomationScreenState::Ready
        };
        AutomationScreen {
            state,
            data,
            rows,
            error: self.error.clone(),
            selected: self.selected.clone(),
            actions_enabled,
            pending: self.pending,
            can_retry,
        }
    }

    pub(crate) fn selected_id(&self) -> Option<&str> {
        self.selected.as_deref()
    }

    pub(crate) fn awaiting(&self) -> bool {
        self.pending.is_some() || self.loading
    }

    /// Ask the Host for the stored catalog. Disconnected and in-flight saves
    /// do not start another read.
    pub(crate) fn load(&mut self, connected: bool) -> Option<Value> {
        if !connected || self.pending.is_some() {
            self.loading = false;
            return None;
        }
        self.loading = self.editor.is_none();
        Some(json!({"method":"settings.automation.get"}))
    }

    pub(crate) fn receive(&mut self, event: &HostEvent) {
        match event.kind.as_str() {
            "automation_settings" => match serde_json::from_value::<AutomationCatalog>(event.data.clone())
            {
                Ok(catalog) => {
                    let _ = self.show_catalog(catalog);
                }
                Err(_) => self.rejected("error.automation"),
            },
            "automation_settings_error" => {
                let key = event
                    .message_key
                    .as_deref()
                    .unwrap_or("error.automation");
                self.rejected(key);
            }
            "disconnected" => {
                self.pending = None;
                self.loading = false;
            }
            _ => {}
        }
    }

    pub(crate) fn select(&mut self, id: &str) -> Option<(String, String)> {
        let (name, instruction) = self.editor.as_ref().and_then(|editor| editor.shown_text(id))?;
        self.selected = Some(id.to_string());
        Some((name.to_string(), instruction.to_string()))
    }

    /// Remember text for the selected stored automation.
    pub(crate) fn note_typing(&mut self, id: &str, name: &str, instruction: &str) -> bool {
        if self.selected.as_deref() != Some(id) {
            return false;
        }
        self.editor.as_mut().is_some_and(|editor| {
            editor.edit(AutomationDraft {
                id: id.to_string(),
                name: name.to_string(),
                instruction: instruction.to_string(),
            })
        })
    }

    pub(crate) fn pause(&mut self, connected: bool) -> Option<Value> {
        self.transition(connected, AutomationPending::Pause, "settings.automation.pause")
    }

    pub(crate) fn resume(&mut self, connected: bool) -> Option<Value> {
        self.transition(connected, AutomationPending::Resume, "settings.automation.resume")
    }

    /// Record the typed name and instruction, then ask the Host to store them.
    /// A disconnected Host keeps the draft and sends nothing.
    pub(crate) fn update(
        &mut self,
        connected: bool,
        name: String,
        instruction: String,
    ) -> Option<Value> {
        let id = self.selected.clone()?;
        if !self.note_typing(&id, &name, &instruction) {
            return None;
        }
        if !connected || self.pending.is_some() || self.loading {
            return None;
        }
        self.pending = Some(AutomationPending::Update);
        self.error = None;
        Some(json!({"method":"settings.automation.update","id":id,"name":name,"instruction":instruction}))
    }

    pub(crate) fn rejected(&mut self, key: &str) {
        self.pending = None;
        self.loading = false;
        self.fill = None;
        self.error = Some(key.to_string());
    }

    pub(crate) fn take_input_fill(&mut self) -> Option<(String, String)> {
        self.fill.take()
    }

    fn transition(
        &mut self,
        connected: bool,
        pending: AutomationPending,
        method: &str,
    ) -> Option<Value> {
        if !connected || self.pending.is_some() || self.loading {
            return None;
        }
        let id = self.selected.clone()?;
        if self
            .editor
            .as_ref()
            .and_then(|editor| editor.shown_paused(&id))
            .is_none()
        {
            return None;
        }
        self.pending = Some(pending);
        self.error = None;
        Some(json!({"method": method, "id": id}))
    }

    fn show_catalog(&mut self, catalog: AutomationCatalog) -> bool {
        self.pending = None;
        self.loading = false;
        let created = self.editor.is_none();
        let draft_before = self
            .editor
            .as_ref()
            .and_then(|editor| editor.draft().map(|draft| draft.id.clone()));
        let accepted = if let Some(editor) = &mut self.editor {
            editor.show_effective(catalog)
        } else {
            self.editor = AutomationEditor::from_saved(catalog);
            self.editor.is_some()
        };
        if !accepted {
            self.fill = None;
            self.error = Some("error.automation".into());
            return false;
        }
        self.error = None;
        let selection_changed = self.ensure_selection();
        let draft_after = self
            .editor
            .as_ref()
            .and_then(|editor| editor.draft().map(|draft| draft.id.clone()));
        let cleared = draft_before.is_some() && draft_after.is_none();
        if selection_changed || created || cleared {
            if self.selected.is_some() {
                self.queue_fill();
            } else {
                self.fill = Some((String::new(), String::new()));
            }
        }
        true
    }

    fn ensure_selection(&mut self) -> bool {
        let ids: Vec<String> = self
            .editor
            .as_ref()
            .map(|editor| {
                editor
                    .saved()
                    .automations
                    .iter()
                    .map(|item| item.id.clone())
                    .collect()
            })
            .unwrap_or_default();
        if self
            .selected
            .as_ref()
            .is_some_and(|id| ids.iter().any(|item| item == id))
        {
            return false;
        }
        let next = ids.first().cloned();
        let changed = self.selected != next;
        self.selected = next;
        changed
    }

    fn queue_fill(&mut self) {
        let Some(id) = self.selected.clone() else {
            return;
        };
        let Some((name, instruction)) = self
            .editor
            .as_ref()
            .and_then(|editor| editor.shown_text(&id))
        else {
            return;
        };
        self.fill = Some((name.to_string(), instruction.to_string()));
    }

    fn rows(&self) -> Vec<AutomationRow> {
        let Some(editor) = &self.editor else {
            return Vec::new();
        };
        editor
            .saved()
            .automations
            .iter()
            .map(|item| {
                let (name, instruction) = editor
                    .shown_text(&item.id)
                    .unwrap_or((item.name.as_str(), item.instruction.as_str()));
                let draft = editor.draft().is_some_and(|draft| {
                    draft.id == item.id
                        && (draft.name != item.name || draft.instruction != item.instruction)
                });
                AutomationRow {
                    id: item.id.clone(),
                    name: name.to_string(),
                    instruction: instruction.to_string(),
                    paused: editor.shown_paused(&item.id).unwrap_or(item.paused),
                    draft,
                }
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pero_contracts::automation::AutomationConfig;

    fn sample(paused: bool) -> AutomationCatalog {
        AutomationCatalog {
            automations: vec![AutomationConfig {
                id: "nightly".into(),
                name: "Nightly summary".into(),
                instruction: "Summarize ROADMAP".into(),
                paused,
            }],
        }
    }

    fn named(paused: bool, name: &str, instruction: &str) -> AutomationCatalog {
        let mut catalog = sample(paused);
        catalog.automations[0].name = name.into();
        catalog.automations[0].instruction = instruction.into();
        catalog
    }

    fn catalog_event(catalog: AutomationCatalog) -> HostEvent {
        let mut event = HostEvent::new("automation_settings", None, String::new());
        event.data = serde_json::to_value(catalog).unwrap();
        event
    }

    fn error_event() -> HostEvent {
        HostEvent::message("automation_settings_error", None, "error.automation")
    }

    #[test]
    fn page_shows_catalog_states_and_keeps_a_rejected_draft() {
        let mut page = AutomationPage::default();
        assert_eq!(page.screen(false).state, AutomationScreenState::Disabled);
        assert!(page.load(false).is_none());
        assert!(!page.screen(false).actions_enabled);

        let get = page.load(true).unwrap();
        assert_eq!(get["method"], "settings.automation.get");
        assert!(page.awaiting());
        assert_eq!(page.screen(true).state, AutomationScreenState::Loading);
        assert!(page.screen(true).data.automations.is_empty());
        assert!(!page.screen(true).actions_enabled);

        page.receive(&catalog_event(AutomationCatalog::default()));
        let empty = page.screen(true);
        assert_eq!(empty.state, AutomationScreenState::Empty);
        assert!(empty.data.automations.is_empty());
        assert!(empty.rows.is_empty());
        assert_eq!(page.take_input_fill(), Some((String::new(), String::new())));

        page.receive(&catalog_event(sample(false)));
        let ready = page.screen(true);
        assert_eq!(ready.state, AutomationScreenState::Ready);
        assert_eq!(ready.data, sample(false));
        assert_eq!(ready.selected.as_deref(), Some("nightly"));
        assert_eq!(page.selected_id(), Some("nightly"));
        assert_eq!(ready.rows[0].name, "Nightly summary");
        assert_eq!(ready.rows[0].instruction, "Summarize ROADMAP");
        assert!(!ready.rows[0].paused);
        assert!(!ready.rows[0].draft);
        assert!(ready.actions_enabled);
        assert_eq!(
            page.take_input_fill(),
            Some(("Nightly summary".into(), "Summarize ROADMAP".into()))
        );
        assert!(page.select("missing").is_none());

        assert!(page.note_typing("nightly", " ", "typed but not stored"));
        let update = page
            .update(true, " ".into(), "typed but not stored".into())
            .unwrap();
        assert_eq!(update["method"], "settings.automation.update");
        assert_eq!(update["id"], "nightly");
        assert_eq!(update["name"], " ");
        assert_eq!(update["instruction"], "typed but not stored");
        assert_eq!(page.screen(true).pending, Some(AutomationPending::Update));
        page.receive(&error_event());
        let rejected = page.screen(true);
        assert_eq!(rejected.state, AutomationScreenState::Error);
        assert_eq!(rejected.error.as_deref(), Some("error.automation"));
        assert_eq!(rejected.data, sample(false));
        assert_eq!(rejected.rows[0].name, " ");
        assert_eq!(rejected.rows[0].instruction, "typed but not stored");
        assert!(rejected.rows[0].draft);
        assert!(!rejected.rows[0].paused);
        assert!(rejected.actions_enabled);
        assert!(!rejected.can_retry);
        assert!(page.take_input_fill().is_none());

        let pause = page.pause(true).unwrap();
        assert_eq!(pause["method"], "settings.automation.pause");
        assert_eq!(pause["id"], "nightly");
        assert!(!page.screen(true).actions_enabled);
        page.receive(&catalog_event(sample(true)));
        let paused = page.screen(true);
        assert_eq!(paused.state, AutomationScreenState::Ready);
        assert!(paused.error.is_none());
        assert!(paused.data.automations[0].paused);
        assert_eq!(paused.data.automations[0].name, "Nightly summary");
        assert!(paused.rows[0].paused);
        assert!(paused.rows[0].draft);
        assert_eq!(paused.rows[0].instruction, "typed but not stored");
        assert!(page.take_input_fill().is_none());

        let resume = page.resume(true).unwrap();
        assert_eq!(resume["method"], "settings.automation.resume");
        page.receive(&catalog_event(sample(false)));
        assert!(!page.screen(true).rows[0].paused);
        assert!(page.screen(true).rows[0].draft);
        assert_eq!(page.screen(true).data, sample(false));

        let stored = named(false, "Morning summary", "Summarize overnight changes");
        let update = page
            .update(
                true,
                "Morning summary".into(),
                "Summarize overnight changes".into(),
            )
            .unwrap();
        assert_eq!(update["name"], "Morning summary");
        page.receive(&catalog_event(stored.clone()));
        let saved = page.screen(true);
        assert_eq!(saved.state, AutomationScreenState::Ready);
        assert!(!saved.rows[0].draft);
        assert_eq!(saved.rows[0].name, "Morning summary");
        assert_eq!(saved.data, stored);
        assert_eq!(
            page.take_input_fill(),
            Some((
                "Morning summary".into(),
                "Summarize overnight changes".into()
            ))
        );

        page.receive(&HostEvent::new("disconnected", None, String::new()));
        let disabled = page.screen(false);
        assert_eq!(disabled.state, AutomationScreenState::Disabled);
        assert!(!disabled.actions_enabled);
        assert_eq!(disabled.rows[0].name, "Morning summary");
        assert!(page.pause(false).is_none());
        assert!(page.resume(false).is_none());
        assert!(
            page.update(
                false,
                "Morning summary".into(),
                "Summarize overnight changes".into(),
            )
            .is_none()
        );
        assert_eq!(page.screen(false).data, stored);

        let mut failed = AutomationPage::default();
        assert!(failed.load(true).is_some());
        failed.receive(&error_event());
        let failed_screen = failed.screen(true);
        assert_eq!(failed_screen.state, AutomationScreenState::Error);
        assert!(failed_screen.rows.is_empty());
        assert!(failed_screen.data.automations.is_empty());
        assert!(failed_screen.can_retry);
        assert!(failed.take_input_fill().is_none());
        let retry = failed.load(true).unwrap();
        assert_eq!(retry["method"], "settings.automation.get");
        assert_eq!(failed.screen(true).state, AutomationScreenState::Loading);

        let mut kept = AutomationPage::default();
        kept.receive(&catalog_event(sample(false)));
        let _ = kept.take_input_fill();
        assert!(kept.note_typing("nightly", " ", "typed but not stored"));
        let mut invalid = HostEvent::new("automation_settings", None, String::new());
        invalid.data = json!({
            "automations": [{
                "id": "nightly",
                "name": "",
                "instruction": "x",
                "paused": true
            }]
        });
        kept.receive(&invalid);
        let kept_screen = kept.screen(true);
        assert_eq!(kept_screen.state, AutomationScreenState::Error);
        assert_eq!(kept_screen.data, sample(false));
        assert_eq!(kept_screen.rows[0].name, " ");
        assert!(kept_screen.rows[0].draft);
        assert!(!kept_screen.rows[0].paused);
        assert!(kept.take_input_fill().is_none());
    }
}
