//! Host-owned catalog of installed plugins.
//!
//! Enable and disable return the catalog that was stored. A rejected change
//! leaves that catalog in place, including each plugin's version. Installing,
//! updating, and removing packages is a later slice; this catalog only records
//! the name, version, and enabled flag those operations will store.

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PluginRejected;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PluginRecord {
    pub name: String,
    pub version: String,
    pub enabled: bool,
}

impl PluginRecord {
    pub fn valid(&self) -> bool {
        plugin_name(&self.name) && plugin_version(&self.version)
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PluginCatalog {
    pub plugins: Vec<PluginRecord>,
}

impl PluginCatalog {
    pub fn valid(&self) -> bool {
        self.plugins.len() <= 64
            && self.plugins.iter().enumerate().all(|(index, plugin)| {
                plugin.valid()
                    && !self.plugins[..index]
                        .iter()
                        .any(|other| other.name == plugin.name)
            })
    }

    /// The catalog after enable or disable, or rejection when the named plugin
    /// is not in a valid catalog. Version and every other plugin stay put.
    pub fn apply(&self, edit: PluginEdit) -> Result<Self, PluginRejected> {
        if !self.valid() {
            return Err(PluginRejected);
        }
        let mut next = self.clone();
        let Some(plugin) = next
            .plugins
            .iter_mut()
            .find(|plugin| plugin.name == edit.name())
        else {
            return Err(PluginRejected);
        };
        match edit {
            PluginEdit::Enable { .. } => plugin.enabled = true,
            PluginEdit::Disable { .. } => plugin.enabled = false,
        }
        next.valid().then_some(next).ok_or(PluginRejected)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PluginEdit {
    Enable { name: String },
    Disable { name: String },
}

impl PluginEdit {
    fn name(&self) -> &str {
        match self {
            Self::Enable { name } | Self::Disable { name } => name,
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(tag = "method", deny_unknown_fields)]
pub enum PluginSettingsRequest {
    #[serde(rename = "settings.plugins.get")]
    Get,
    #[serde(rename = "settings.plugins.enable")]
    Enable { name: String },
    #[serde(rename = "settings.plugins.disable")]
    Disable { name: String },
}

/// Settings-page state. The catalog is the last one the Host stored.
/// A rejection is not a catalog, so the page keeps these rows and can retry.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PluginSettings {
    catalog: PluginCatalog,
}

impl PluginSettings {
    pub fn from_saved(catalog: PluginCatalog) -> Option<Self> {
        catalog.valid().then_some(Self { catalog })
    }

    pub fn catalog(&self) -> &PluginCatalog {
        &self.catalog
    }

    /// Adopt a Host result. An invalid catalog is ignored.
    pub fn show_effective(&mut self, catalog: PluginCatalog) -> bool {
        if !catalog.valid() {
            return false;
        }
        self.catalog = catalog;
        true
    }

    /// Version and enabled flag for one stored plugin.
    pub fn shown(&self, name: &str) -> Option<(&str, bool)> {
        self.catalog
            .plugins
            .iter()
            .find(|plugin| plugin.name == name)
            .map(|plugin| (plugin.version.as_str(), plugin.enabled))
    }
}

fn plugin_name(value: &str) -> bool {
    text(value, 128)
}

fn plugin_version(value: &str) -> bool {
    text(value, 64) && !value.chars().any(char::is_whitespace)
}

fn text(value: &str, max: usize) -> bool {
    !value.trim().is_empty()
        && value.chars().count() <= max
        && value == value.trim()
        && !value.chars().any(char::is_control)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> PluginCatalog {
        PluginCatalog {
            plugins: vec![
                PluginRecord {
                    name: "notes".into(),
                    version: "1.2.0".into(),
                    enabled: true,
                },
                PluginRecord {
                    name: "lint".into(),
                    version: "0.4.1".into(),
                    enabled: false,
                },
            ],
        }
    }

    #[test]
    fn enable_and_disable_report_the_stored_catalog_or_leave_it() {
        let origin = sample();
        let disabled = origin
            .apply(PluginEdit::Disable {
                name: "notes".into(),
            })
            .unwrap();
        assert_eq!(
            disabled.plugins[0],
            PluginRecord {
                name: "notes".into(),
                version: "1.2.0".into(),
                enabled: false,
            }
        );
        assert_eq!(disabled.plugins[1], origin.plugins[1]);

        let enabled = disabled
            .apply(PluginEdit::Enable {
                name: "lint".into(),
            })
            .unwrap();
        assert!(enabled.plugins[1].enabled);
        assert_eq!(enabled.plugins[1].version, "0.4.1");
        assert!(!enabled.plugins[0].enabled);

        let again = enabled
            .apply(PluginEdit::Disable {
                name: "notes".into(),
            })
            .unwrap();
        assert_eq!(again, enabled);

        for (catalog, edit) in [
            (
                origin.clone(),
                PluginEdit::Disable {
                    name: "missing".into(),
                },
            ),
            (origin.clone(), PluginEdit::Enable { name: " ".into() }),
            (
                origin.clone(),
                PluginEdit::Enable {
                    name: "notes\n".into(),
                },
            ),
            (
                PluginCatalog {
                    plugins: vec![PluginRecord {
                        name: "notes".into(),
                        version: "1.2.0 ".into(),
                        enabled: true,
                    }],
                },
                PluginEdit::Disable {
                    name: "notes".into(),
                },
            ),
            (
                PluginCatalog {
                    plugins: vec![origin.plugins[0].clone(), origin.plugins[0].clone()],
                },
                PluginEdit::Disable {
                    name: "notes".into(),
                },
            ),
        ] {
            let before = catalog.clone();
            assert_eq!(catalog.apply(edit), Err(PluginRejected));
            assert_eq!(catalog, before);
        }
        assert_eq!(origin, sample());
    }

    #[test]
    fn failed_change_keeps_the_shown_plugin_and_success_replaces_it() {
        let mut page = PluginSettings::from_saved(sample()).unwrap();
        assert_eq!(page.shown("notes"), Some(("1.2.0", true)));
        assert_eq!(page.shown("lint"), Some(("0.4.1", false)));
        assert!(
            sample()
                .apply(PluginEdit::Disable {
                    name: "missing".into(),
                })
                .is_err()
        );
        assert_eq!(page.catalog(), &sample());

        let disabled = sample()
            .apply(PluginEdit::Disable {
                name: "notes".into(),
            })
            .unwrap();
        assert!(page.show_effective(disabled.clone()));
        assert_eq!(page.shown("notes"), Some(("1.2.0", false)));
        assert_eq!(page.shown("lint"), Some(("0.4.1", false)));

        let enabled = disabled
            .apply(PluginEdit::Enable {
                name: "lint".into(),
            })
            .unwrap();
        assert!(page.show_effective(enabled));
        assert_eq!(page.shown("lint"), Some(("0.4.1", true)));
        assert_eq!(page.shown("notes"), Some(("1.2.0", false)));

        assert!(!page.show_effective(PluginCatalog {
            plugins: vec![PluginRecord {
                name: "notes".into(),
                version: String::new(),
                enabled: true,
            }],
        }));
        assert_eq!(page.shown("notes"), Some(("1.2.0", false)));
        assert!(page.shown("missing").is_none());
        assert!(
            PluginSettings::from_saved(PluginCatalog {
                plugins: vec![sample().plugins[0].clone(), sample().plugins[0].clone()],
            })
            .is_none()
        );
    }
}
