//! Host-owned automation catalog.
//!
//! Pause, resume, update, and create return the catalog that was stored. A
//! rejected edit leaves that catalog in place. [`AutomationEditor`] keeps the
//! typed draft until an effective catalog contains the same name and instruction.

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AutomationRejected;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AutomationConfig {
    pub id: String,
    pub name: String,
    pub instruction: String,
    pub paused: bool,
}

impl AutomationConfig {
    pub fn valid(&self) -> bool {
        identifier(&self.id) && label(&self.name) && instruction(&self.instruction)
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AutomationCatalog {
    pub automations: Vec<AutomationConfig>,
}

impl AutomationCatalog {
    pub fn valid(&self) -> bool {
        self.automations.len() <= 64
            && self.automations.iter().enumerate().all(|(index, item)| {
                item.valid()
                    && !self.automations[..index]
                        .iter()
                        .any(|other| other.id == item.id)
            })
    }

    /// The catalog after a pause, resume, update, or create. Rejection leaves
    /// the receiver unchanged: create refuses a duplicate or invalid record,
    /// and the other edits must name an existing valid record.
    pub fn apply(&self, edit: AutomationEdit) -> Result<Self, AutomationRejected> {
        if !self.valid() {
            return Err(AutomationRejected);
        }
        let mut next = self.clone();
        match edit {
            AutomationEdit::Create {
                id,
                name,
                instruction,
            } => {
                if next.automations.iter().any(|item| item.id == id) {
                    return Err(AutomationRejected);
                }
                next.automations.push(AutomationConfig {
                    id,
                    name,
                    instruction,
                    paused: false,
                });
            }
            AutomationEdit::Pause { id } => {
                let item = next
                    .automations
                    .iter_mut()
                    .find(|item| item.id == id)
                    .ok_or(AutomationRejected)?;
                item.paused = true;
            }
            AutomationEdit::Resume { id } => {
                let item = next
                    .automations
                    .iter_mut()
                    .find(|item| item.id == id)
                    .ok_or(AutomationRejected)?;
                item.paused = false;
            }
            AutomationEdit::Update {
                id,
                name,
                instruction,
            } => {
                let item = next
                    .automations
                    .iter_mut()
                    .find(|item| item.id == id)
                    .ok_or(AutomationRejected)?;
                item.name = name;
                item.instruction = instruction;
            }
        }
        next.valid().then_some(next).ok_or(AutomationRejected)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AutomationEdit {
    Pause {
        id: String,
    },
    Resume {
        id: String,
    },
    Update {
        id: String,
        name: String,
        instruction: String,
    },
    Create {
        id: String,
        name: String,
        instruction: String,
    },
}

#[derive(Debug, Deserialize)]
#[serde(tag = "method", deny_unknown_fields)]
pub enum AutomationSettingsRequest {
    #[serde(rename = "settings.automation.get")]
    Get,
    #[serde(rename = "settings.automation.pause")]
    Pause { id: String },
    #[serde(rename = "settings.automation.resume")]
    Resume { id: String },
    #[serde(rename = "settings.automation.update")]
    Update {
        id: String,
        name: String,
        instruction: String,
    },
    #[serde(rename = "settings.automation.create")]
    Create {
        id: String,
        name: String,
        instruction: String,
    },
}

/// Uncommitted text for one existing automation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AutomationDraft {
    pub id: String,
    pub name: String,
    pub instruction: String,
}

/// Settings-page state. The saved catalog is what the Host stored. The draft
/// is what the user has typed and is shown until a stored record matches it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AutomationEditor {
    saved: AutomationCatalog,
    draft: Option<AutomationDraft>,
}

impl AutomationEditor {
    pub fn from_saved(saved: AutomationCatalog) -> Option<Self> {
        saved.valid().then_some(Self { saved, draft: None })
    }

    pub fn saved(&self) -> &AutomationCatalog {
        &self.saved
    }

    pub fn draft(&self) -> Option<&AutomationDraft> {
        self.draft.as_ref()
    }

    /// Remember text for an automation that is already stored.
    pub fn edit(&mut self, draft: AutomationDraft) -> bool {
        if !self
            .saved
            .automations
            .iter()
            .any(|item| item.id == draft.id)
        {
            return false;
        }
        self.draft = Some(draft);
        true
    }

    /// Adopt a Host result. Clears the draft only when that result stored the
    /// draft's name and instruction. An invalid catalog is ignored.
    pub fn show_effective(&mut self, saved: AutomationCatalog) -> bool {
        if !saved.valid() {
            return false;
        }
        let landed = self.draft.as_ref().is_some_and(|draft| {
            saved.automations.iter().any(|item| {
                item.id == draft.id
                    && item.name == draft.name
                    && item.instruction == draft.instruction
            })
        });
        self.saved = saved;
        if landed {
            self.draft = None;
        }
        true
    }

    pub fn shown_paused(&self, id: &str) -> Option<bool> {
        self.saved
            .automations
            .iter()
            .find(|item| item.id == id)
            .map(|item| item.paused)
    }

    /// Draft text while a save is uncommitted, otherwise the stored text.
    pub fn shown_text(&self, id: &str) -> Option<(&str, &str)> {
        if let Some(draft) = &self.draft {
            if draft.id == id {
                return Some((&draft.name, &draft.instruction));
            }
        }
        self.saved
            .automations
            .iter()
            .find(|item| item.id == id)
            .map(|item| (item.name.as_str(), item.instruction.as_str()))
    }
}

fn identifier(value: &str) -> bool {
    text(value, 128, false) && !value.chars().any(char::is_whitespace)
}

fn label(value: &str) -> bool {
    text(value, 128, false)
}

fn instruction(value: &str) -> bool {
    text(value, 4096, true)
}

fn text(value: &str, max: usize, multiline: bool) -> bool {
    !value.trim().is_empty()
        && value.chars().count() <= max
        && !value
            .chars()
            .any(|c| c.is_control() && !(multiline && matches!(c, '\n' | '\t')))
}

#[cfg(test)]
mod tests {
    use super::*;

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

    #[test]
    fn apply_returns_the_stored_catalog_or_leaves_it() {
        let origin = sample(false);
        let paused = origin
            .apply(AutomationEdit::Pause {
                id: "nightly".into(),
            })
            .unwrap();
        assert!(paused.automations[0].paused);
        assert_eq!(paused.automations[0].name, "Nightly summary");

        let edited = paused
            .apply(AutomationEdit::Update {
                id: "nightly".into(),
                name: "Morning summary".into(),
                instruction: "Summarize overnight changes\nKeep the list.".into(),
            })
            .unwrap();
        assert_eq!(edited.automations[0].name, "Morning summary");
        assert!(edited.automations[0].paused);
        assert!(edited.automations[0].instruction.contains('\n'));

        let resumed = edited
            .apply(AutomationEdit::Resume {
                id: "nightly".into(),
            })
            .unwrap();
        assert!(!resumed.automations[0].paused);
        assert_eq!(
            resumed.automations[0].instruction,
            edited.automations[0].instruction
        );

        let empty = AutomationCatalog::default();
        let created = empty
            .apply(AutomationEdit::Create {
                id: "weekly".into(),
                name: "Weekly digest".into(),
                instruction: "Summarize the week\nKeep the list.".into(),
            })
            .unwrap();
        assert_eq!(
            created.automations,
            vec![AutomationConfig {
                id: "weekly".into(),
                name: "Weekly digest".into(),
                instruction: "Summarize the week\nKeep the list.".into(),
                paused: false,
            }]
        );
        assert!(empty.automations.is_empty());
        let with_both = created
            .apply(AutomationEdit::Create {
                id: "nightly".into(),
                name: "Nightly summary".into(),
                instruction: "Summarize ROADMAP".into(),
            })
            .unwrap();
        assert_eq!(with_both.automations.len(), 2);
        assert_eq!(with_both.automations[0].id, "weekly");
        assert!(
            empty
                .apply(AutomationEdit::Create {
                    id: "weekly".into(),
                    name: " ".into(),
                    instruction: "kept".into(),
                })
                .is_err()
        );
        assert!(
            empty
                .apply(AutomationEdit::Create {
                    id: "weekly".into(),
                    name: "Weekly digest".into(),
                    instruction: " ".into(),
                })
                .is_err()
        );
        assert!(empty.automations.is_empty());
        let full = AutomationCatalog {
            automations: (0..64)
                .map(|index| AutomationConfig {
                    id: format!("a{index}"),
                    name: format!("Automation {index}"),
                    instruction: "Run".into(),
                    paused: false,
                })
                .collect(),
        };
        assert!(full.valid());
        assert!(
            full.apply(AutomationEdit::Create {
                id: "extra".into(),
                name: "Extra".into(),
                instruction: "Run".into(),
            })
            .is_err()
        );
        assert_eq!(full.automations.len(), 64);

        for (catalog, edit) in [
            (
                origin.clone(),
                AutomationEdit::Update {
                    id: "nightly".into(),
                    name: " ".into(),
                    instruction: "kept".into(),
                },
            ),
            (
                origin.clone(),
                AutomationEdit::Update {
                    id: "missing".into(),
                    name: "Other".into(),
                    instruction: "Other".into(),
                },
            ),
            (
                origin.clone(),
                AutomationEdit::Pause {
                    id: "missing".into(),
                },
            ),
            (
                origin.clone(),
                AutomationEdit::Resume {
                    id: "missing".into(),
                },
            ),
            (
                origin.clone(),
                AutomationEdit::Update {
                    id: "nightly".into(),
                    name: "x".repeat(129),
                    instruction: "kept".into(),
                },
            ),
            (
                origin.clone(),
                AutomationEdit::Create {
                    id: "nightly".into(),
                    name: "Other".into(),
                    instruction: "Other".into(),
                },
            ),
            (
                origin.clone(),
                AutomationEdit::Create {
                    id: "weekly".into(),
                    name: " ".into(),
                    instruction: "kept".into(),
                },
            ),
            (
                origin.clone(),
                AutomationEdit::Create {
                    id: "weekly".into(),
                    name: "Weekly digest".into(),
                    instruction: " ".into(),
                },
            ),
            (
                origin.clone(),
                AutomationEdit::Create {
                    id: " ".into(),
                    name: "Weekly digest".into(),
                    instruction: "kept".into(),
                },
            ),
        ] {
            assert_eq!(catalog.apply(edit), Err(AutomationRejected));
            assert_eq!(catalog, origin);
        }
        assert_eq!(origin, sample(false));
    }

    #[test]
    fn failed_save_keeps_the_draft_and_effective_status_comes_from_the_host() {
        let mut editor = AutomationEditor::from_saved(sample(false)).unwrap();
        assert!(editor.edit(AutomationDraft {
            id: "nightly".into(),
            name: " ".into(),
            instruction: "typed but not stored".into(),
        }));
        assert!(
            sample(false)
                .apply(AutomationEdit::Update {
                    id: "nightly".into(),
                    name: " ".into(),
                    instruction: "typed but not stored".into(),
                })
                .is_err()
        );
        assert_eq!(editor.shown_text("nightly").unwrap().0, " ");
        assert_eq!(editor.saved(), &sample(false));
        assert_eq!(editor.shown_paused("nightly"), Some(false));

        let paused = sample(false)
            .apply(AutomationEdit::Pause {
                id: "nightly".into(),
            })
            .unwrap();
        assert!(editor.show_effective(paused.clone()));
        assert_eq!(editor.shown_paused("nightly"), Some(true));
        assert_eq!(
            editor.shown_text("nightly").unwrap().1,
            "typed but not stored"
        );
        assert_eq!(editor.draft().unwrap().name, " ");

        assert!(editor.edit(AutomationDraft {
            id: "nightly".into(),
            name: "Morning summary".into(),
            instruction: "Summarize overnight changes".into(),
        }));
        let stored = paused
            .apply(AutomationEdit::Update {
                id: "nightly".into(),
                name: "Morning summary".into(),
                instruction: "Summarize overnight changes".into(),
            })
            .unwrap();
        assert!(editor.show_effective(stored.clone()));
        assert!(editor.draft().is_none());
        assert_eq!(
            editor.shown_text("nightly"),
            Some(("Morning summary", "Summarize overnight changes"))
        );
        assert_eq!(editor.shown_paused("nightly"), Some(true));

        let resumed = stored
            .apply(AutomationEdit::Resume {
                id: "nightly".into(),
            })
            .unwrap();
        assert!(editor.show_effective(resumed));
        assert_eq!(editor.shown_paused("nightly"), Some(false));
        assert_eq!(editor.shown_text("nightly").unwrap().0, "Morning summary");

        assert!(!editor.show_effective(AutomationCatalog {
            automations: vec![AutomationConfig {
                id: "nightly".into(),
                name: String::new(),
                instruction: "x".into(),
                paused: false,
            }],
        }));
        assert_eq!(editor.shown_text("nightly").unwrap().0, "Morning summary");
        assert!(!editor.edit(AutomationDraft {
            id: "missing".into(),
            name: "Other".into(),
            instruction: "Other".into(),
        }));
        assert!(editor.draft().is_none());
        assert!(
            AutomationEditor::from_saved(AutomationCatalog {
                automations: vec![
                    sample(false).automations[0].clone(),
                    sample(false).automations[0].clone(),
                ],
            })
            .is_none()
        );
    }
}
