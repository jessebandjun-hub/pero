//! Settings page for automations stored by the Host.
//!
//! Pause, resume, update, and create send `settings.automation` commands. The
//! page shows `automation_settings.data`. A rejected command keeps the typed
//! draft, because `automation_settings_error` carries no catalog. A successful
//! create replaces the shown catalog with the stored one.

use pero_contracts::automation::{AutomationCatalog, AutomationDraft, AutomationEditor};
use pero_contracts::HostEvent;
use serde::Serialize;
use serde_json::{json, Value};

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
    Create,
}

/// Uncommitted id, name, and instruction for an automation that is not stored yet.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub(crate) struct AutomationCreateDraft {
    pub id: String,
    pub name: String,
    pub instruction: String,
}

impl AutomationCreateDraft {
    fn blank(&self) -> bool {
        self.id.trim().is_empty()
            && self.name.trim().is_empty()
            && self.instruction.trim().is_empty()
    }
}

/// Text the settings inputs should show. `id` is set only while creating.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct AutomationInputFill {
    pub id: Option<String>,
    pub name: String,
    pub instruction: String,
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
    /// The form is collecting a new id, name, and instruction.
    pub creating: bool,
    pub create: Option<AutomationCreateDraft>,
    pub can_create: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct AutomationPage {
    editor: Option<AutomationEditor>,
    loading: bool,
    error: Option<String>,
    selected: Option<String>,
    pending: Option<AutomationPending>,
    creating: bool,
    create_draft: AutomationCreateDraft,
    fill: Option<AutomationInputFill>,
}

impl Default for AutomationPage {
    fn default() -> Self {
        Self {
            editor: None,
            loading: false,
            error: None,
            selected: None,
            pending: None,
            creating: false,
            create_draft: AutomationCreateDraft::default(),
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
        let actions_enabled =
            connected && self.pending.is_none() && !self.loading && self.selected.is_some();
        let can_retry = connected
            && self.editor.is_none()
            && self.error.is_some()
            && !self.loading
            && self.pending.is_none();
        let can_create =
            connected && self.editor.is_some() && self.pending.is_none() && !self.loading;
        let creating = self.creating && self.editor.is_some();
        let state = if !connected {
            AutomationScreenState::Disabled
        } else if self.editor.is_none() && self.loading {
            AutomationScreenState::Loading
        } else if self.editor.is_none() {
            AutomationScreenState::Error
        } else if self.error.is_some() {
            AutomationScreenState::Error
        } else if data.automations.is_empty() {
            AutomationScreenState::Empty
        } else {
            AutomationScreenState::Ready
        };
        AutomationScreen {
            state,
            data,
            rows,
            error: self.error.clone(),
            selected: if creating {
                None
            } else {
                self.selected.clone()
            },
            actions_enabled,
            pending: self.pending,
            can_retry,
            creating,
            create: creating.then(|| self.create_draft.clone()),
            can_create,
        }
    }

    pub(crate) fn selected_id(&self) -> Option<&str> {
        self.selected.as_deref()
    }

    pub(crate) fn creating(&self) -> bool {
        self.creating && self.editor.is_some()
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
            "automation_settings" => {
                match serde_json::from_value::<AutomationCatalog>(event.data.clone()) {
                    Ok(catalog) => {
                        let _ = self.show_catalog(catalog);
                    }
                    Err(_) => self.rejected("error.automation"),
                }
            }
            "automation_settings_error" => {
                let key = event.message_key.as_deref().unwrap_or("error.automation");
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
        let (name, instruction) = self
            .editor
            .as_ref()
            .and_then(|editor| editor.shown_text(id))?;
        self.creating = false;
        self.selected = Some(id.to_string());
        Some((name.to_string(), instruction.to_string()))
    }

    /// Remember id, name, and instruction for an automation that is not stored yet.
    pub(crate) fn note_create(&mut self, id: &str, name: &str, instruction: &str) -> bool {
        if !self.creating || self.editor.is_none() {
            return false;
        }
        self.create_draft = AutomationCreateDraft {
            id: id.to_string(),
            name: name.to_string(),
            instruction: instruction.to_string(),
        };
        true
    }

    /// Show the create form. A catalog must already be loaded, and no command
    /// may be in flight. The previous create draft stays in the inputs.
    pub(crate) fn begin_create(&mut self) -> bool {
        if self.editor.is_none() || self.pending.is_some() || self.loading {
            return false;
        }
        self.creating = true;
        self.error = None;
        self.queue_create_fill();
        true
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
        self.transition(
            connected,
            AutomationPending::Pause,
            "settings.automation.pause",
        )
    }

    pub(crate) fn resume(&mut self, connected: bool) -> Option<Value> {
        self.transition(
            connected,
            AutomationPending::Resume,
            "settings.automation.resume",
        )
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
        Some(
            json!({"method":"settings.automation.update","id":id,"name":name,"instruction":instruction}),
        )
    }

    /// Record the typed id, name, and instruction, then ask the Host to append
    /// them. A disconnected Host keeps the draft and sends nothing.
    pub(crate) fn create(
        &mut self,
        connected: bool,
        id: String,
        name: String,
        instruction: String,
    ) -> Option<Value> {
        if self.editor.is_none() {
            return None;
        }
        self.creating = true;
        self.create_draft = AutomationCreateDraft {
            id: id.clone(),
            name: name.clone(),
            instruction: instruction.clone(),
        };
        if !connected || self.pending.is_some() || self.loading {
            return None;
        }
        self.pending = Some(AutomationPending::Create);
        self.error = None;
        Some(
            json!({"method":"settings.automation.create","id":id,"name":name,"instruction":instruction}),
        )
    }

    pub(crate) fn rejected(&mut self, key: &str) {
        self.pending = None;
        self.loading = false;
        self.fill = None;
        self.error = Some(key.to_string());
    }

    pub(crate) fn take_input_fill(&mut self) -> Option<AutomationInputFill> {
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
        let was_create = self.pending == Some(AutomationPending::Create);
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
        let draft_id = self.create_draft.id.clone();
        let draft_name = self.create_draft.name.clone();
        let draft_instruction = self.create_draft.instruction.clone();
        let landed = was_create
            && self.editor.as_ref().is_some_and(|editor| {
                editor.saved().automations.iter().any(|item| {
                    item.id == draft_id
                        && item.name == draft_name
                        && item.instruction == draft_instruction
                })
            });
        let saved_empty = self
            .editor
            .as_ref()
            .is_some_and(|editor| editor.saved().automations.is_empty());
        let was_creating = self.creating;
        if landed {
            self.creating = false;
            self.selected = Some(draft_id);
            self.create_draft = AutomationCreateDraft::default();
        } else if was_create || saved_empty {
            self.creating = true;
        } else if self.creating && !self.create_draft.blank() {
            self.creating = true;
        } else {
            self.creating = false;
        }
        let selection_changed = self.ensure_selection();
        let draft_after = self
            .editor
            .as_ref()
            .and_then(|editor| editor.draft().map(|draft| draft.id.clone()));
        let cleared = draft_before.is_some() && draft_after.is_none();
        let mode_changed = was_creating != self.creating;
        if self.creating && (mode_changed || created) {
            self.queue_create_fill();
        } else if !self.creating
            && (landed || mode_changed || selection_changed || created || cleared)
        {
            if self.selected.is_some() {
                self.queue_fill();
            } else {
                self.fill = Some(AutomationInputFill {
                    id: None,
                    name: String::new(),
                    instruction: String::new(),
                });
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
        self.fill = Some(AutomationInputFill {
            id: None,
            name: name.to_string(),
            instruction: instruction.to_string(),
        });
    }

    fn queue_create_fill(&mut self) {
        self.fill = Some(AutomationInputFill {
            id: Some(self.create_draft.id.clone()),
            name: self.create_draft.name.clone(),
            instruction: self.create_draft.instruction.clone(),
        });
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

    fn stored_fill(name: &str, instruction: &str) -> AutomationInputFill {
        AutomationInputFill {
            id: None,
            name: name.into(),
            instruction: instruction.into(),
        }
    }

    fn create_fill(id: &str, name: &str, instruction: &str) -> AutomationInputFill {
        AutomationInputFill {
            id: Some(id.into()),
            name: name.into(),
            instruction: instruction.into(),
        }
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
        assert!(empty.creating);
        assert!(empty.can_create);
        assert_eq!(page.take_input_fill(), Some(create_fill("", "", "")));

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
        assert!(!ready.creating);
        assert_eq!(
            page.take_input_fill(),
            Some(stored_fill("Nightly summary", "Summarize ROADMAP"))
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
            Some(stored_fill(
                "Morning summary",
                "Summarize overnight changes"
            ))
        );

        page.receive(&HostEvent::new("disconnected", None, String::new()));
        let disabled = page.screen(false);
        assert_eq!(disabled.state, AutomationScreenState::Disabled);
        assert!(!disabled.actions_enabled);
        assert_eq!(disabled.rows[0].name, "Morning summary");
        assert!(page.pause(false).is_none());
        assert!(page.resume(false).is_none());
        assert!(page
            .update(
                false,
                "Morning summary".into(),
                "Summarize overnight changes".into(),
            )
            .is_none());
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

    #[test]
    fn page_creates_an_automation_and_keeps_a_rejected_draft() {
        let mut page = AutomationPage::default();
        assert!(!page.begin_create());
        assert!(page
            .create(
                true,
                "weekly".into(),
                "Weekly digest".into(),
                "Summarize the week".into(),
            )
            .is_none());

        page.receive(&catalog_event(AutomationCatalog::default()));
        let opened = page.screen(true);
        assert_eq!(opened.state, AutomationScreenState::Empty);
        assert!(opened.creating);
        assert!(page.creating());
        assert_eq!(opened.create.unwrap(), AutomationCreateDraft::default());
        assert_eq!(page.take_input_fill(), Some(create_fill("", "", "")));

        assert!(page.note_create("weekly", "Weekly digest", "Summarize the week"));
        assert!(page
            .create(
                false,
                "weekly".into(),
                "Weekly digest".into(),
                "Summarize the week".into(),
            )
            .is_none());
        let disconnected = page.screen(false);
        assert_eq!(disconnected.state, AutomationScreenState::Disabled);
        assert!(!disconnected.can_create);
        assert_eq!(
            disconnected.create.as_ref().unwrap().instruction,
            "Summarize the week"
        );
        assert!(page.take_input_fill().is_none());

        let rejected_command = page
            .create(
                true,
                "weekly".into(),
                " ".into(),
                "Summarize the week".into(),
            )
            .unwrap();
        assert_eq!(rejected_command["method"], "settings.automation.create");
        assert_eq!(rejected_command["id"], "weekly");
        assert_eq!(rejected_command["name"], " ");
        assert_eq!(rejected_command["instruction"], "Summarize the week");
        assert_eq!(page.screen(true).pending, Some(AutomationPending::Create));
        assert!(!page.screen(true).can_create);
        page.receive(&error_event());
        let rejected = page.screen(true);
        assert_eq!(rejected.state, AutomationScreenState::Error);
        assert!(rejected.creating);
        assert!(rejected.data.automations.is_empty());
        assert_eq!(rejected.error.as_deref(), Some("error.automation"));
        let draft = rejected.create.unwrap();
        assert_eq!(draft.id, "weekly");
        assert_eq!(draft.name, " ");
        assert_eq!(draft.instruction, "Summarize the week");
        assert!(rejected.can_create);
        assert!(page.take_input_fill().is_none());

        let created_command = page
            .create(
                true,
                "weekly".into(),
                "Weekly digest".into(),
                "Summarize the week".into(),
            )
            .unwrap();
        assert_eq!(created_command["name"], "Weekly digest");
        let created = AutomationCatalog {
            automations: vec![AutomationConfig {
                id: "weekly".into(),
                name: "Weekly digest".into(),
                instruction: "Summarize the week".into(),
                paused: false,
            }],
        };
        page.receive(&catalog_event(created.clone()));
        let ready = page.screen(true);
        assert_eq!(ready.state, AutomationScreenState::Ready);
        assert!(!ready.creating);
        assert!(!page.creating());
        assert!(ready.create.is_none());
        assert!(ready.error.is_none());
        assert_eq!(ready.data, created);
        assert_eq!(ready.selected.as_deref(), Some("weekly"));
        assert_eq!(ready.rows[0].name, "Weekly digest");
        assert!(!ready.rows[0].draft);
        assert!(!ready.rows[0].paused);
        assert_eq!(
            page.take_input_fill(),
            Some(stored_fill("Weekly digest", "Summarize the week"))
        );

        assert!(page.begin_create());
        assert_eq!(page.take_input_fill(), Some(create_fill("", "", "")));
        let duplicate = page
            .create(
                true,
                "weekly".into(),
                "Other".into(),
                "Should not replace".into(),
            )
            .unwrap();
        assert_eq!(duplicate["id"], "weekly");
        page.receive(&error_event());
        let kept = page.screen(true);
        assert_eq!(kept.state, AutomationScreenState::Error);
        assert!(kept.creating);
        assert_eq!(kept.data, created);
        assert_eq!(kept.rows[0].name, "Weekly digest");
        assert_eq!(kept.create.as_ref().unwrap().name, "Other");
        assert_eq!(
            kept.create.as_ref().unwrap().instruction,
            "Should not replace"
        );
        assert!(page.take_input_fill().is_none());

        assert_eq!(
            page.select("weekly"),
            Some(("Weekly digest".into(), "Summarize the week".into()))
        );
        let editing = page.screen(true);
        assert!(!editing.creating);
        assert_eq!(editing.selected.as_deref(), Some("weekly"));

        assert!(page.begin_create());
        let _ = page.take_input_fill();
        let second = page
            .create(
                true,
                "morning".into(),
                "Morning note".into(),
                "List overnight changes".into(),
            )
            .unwrap();
        assert_eq!(second["method"], "settings.automation.create");
        let both = AutomationCatalog {
            automations: vec![
                created.automations[0].clone(),
                AutomationConfig {
                    id: "morning".into(),
                    name: "Morning note".into(),
                    instruction: "List overnight changes".into(),
                    paused: false,
                },
            ],
        };
        page.receive(&catalog_event(both.clone()));
        let refreshed = page.screen(true);
        assert_eq!(refreshed.state, AutomationScreenState::Ready);
        assert!(!refreshed.creating);
        assert_eq!(refreshed.data, both);
        assert_eq!(refreshed.selected.as_deref(), Some("morning"));
        assert_eq!(refreshed.rows.len(), 2);
        assert_eq!(
            page.take_input_fill(),
            Some(stored_fill("Morning note", "List overnight changes"))
        );
    }
}
