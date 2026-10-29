//! All-settings list of installed plugins.
//!
//! `PluginSettings` is the stored catalog. Enable and disable change a row only
//! when the Host returns that catalog. `plugin_settings_error` carries no
//! catalog, so it is not applied and it is not a successful save.

use pero_contracts::plugins::{PluginCatalog, PluginRecord, PluginSettings};
use serde_json::{Value, json};

#[derive(Clone, Debug, PartialEq, Eq)]
enum Pending {
    Get,
    Change(String),
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct PluginPage {
    settings: Option<PluginSettings>,
    pending: Option<Pending>,
    notice: Option<String>,
}

impl PluginPage {
    pub(crate) fn loaded(&self) -> bool {
        self.settings.is_some()
    }

    pub(crate) fn pending(&self) -> bool {
        self.pending.is_some()
    }

    pub(crate) fn pending_name(&self) -> Option<&str> {
        match &self.pending {
            Some(Pending::Change(name)) => Some(name),
            _ => None,
        }
    }

    /// Failure text. Absent after a valid catalog, never a success marker.
    pub(crate) fn notice(&self) -> Option<&str> {
        self.notice.as_deref()
    }

    pub(crate) fn records(&self) -> &[PluginRecord] {
        self.settings
            .as_ref()
            .map(|settings| settings.catalog().plugins.as_slice())
            .unwrap_or(&[])
    }

    pub(crate) fn snapshot(&self) -> Value {
        json!({
            "loaded": self.loaded(),
            "pending": self.pending(),
            "notice": self.notice(),
            "plugins": self.settings.as_ref().map(|settings| settings.catalog()),
        })
    }

    /// Ask for the stored catalog. A request already in flight is left alone.
    pub(crate) fn load(&mut self) -> Option<Value> {
        if self.pending.is_some() {
            return None;
        }
        self.pending = Some(Pending::Get);
        Some(json!({"method":"settings.plugins.get"}))
    }

    /// Enable or disable one listed plugin. The row stays until the Host answers.
    pub(crate) fn toggle(&mut self, name: &str) -> Option<Value> {
        if self.pending.is_some() {
            return None;
        }
        let (_, enabled) = self.settings.as_ref()?.shown(name)?;
        let method = if enabled {
            "settings.plugins.disable"
        } else {
            "settings.plugins.enable"
        };
        self.pending = Some(Pending::Change(name.to_string()));
        Some(json!({"method": method, "name": name}))
    }

    /// The command never reached the Host. The listed rows stay.
    pub(crate) fn fail_send(&mut self) {
        if self.pending.take().is_some() {
            self.notice = Some("error.plugins".into());
        }
    }

    /// Fold a Host event. An error does not call `show_effective`.
    pub(crate) fn receive(&mut self, kind: &str, data: &Value, message_key: Option<&str>) -> bool {
        match kind {
            "plugin_settings" => {
                self.pending = None;
                let adopted = serde_json::from_value::<PluginCatalog>(data.clone())
                    .is_ok_and(|catalog| self.adopt(catalog));
                self.notice = (!adopted).then(|| "error.plugins".to_string());
                true
            }
            "plugin_settings_error" => {
                self.pending = None;
                self.notice = Some(match message_key {
                    Some(key) if key.starts_with("error.") => key.to_string(),
                    _ => "error.plugins".into(),
                });
                true
            }
            _ => false,
        }
    }

    fn adopt(&mut self, catalog: PluginCatalog) -> bool {
        if let Some(settings) = &mut self.settings {
            return settings.show_effective(catalog);
        }
        match PluginSettings::from_saved(catalog) {
            Some(settings) => {
                self.settings = Some(settings);
                true
            }
            None => false,
        }
    }
}

#[cfg(any(target_family = "wasm", feature = "native"))]
mod view {
    use crate::app::Workbench;
    use gpui_kit::{
        component::{button::*, *},
        prelude::*,
        *,
    };

    impl Workbench {
        pub(crate) fn open_plugin_settings(&mut self, cx: &mut Context<Self>) {
            if let Some(command) = self.plugins.load() {
                self.command(command, cx);
            } else {
                cx.notify();
            }
        }

        pub(crate) fn receive_plugin_settings(
            &mut self,
            event: &pero_contracts::HostEvent,
            cx: &mut Context<Self>,
        ) -> bool {
            if !self.plugins.receive(
                event.kind.as_str(),
                &event.data,
                event.message_key.as_deref(),
            ) {
                return false;
            }
            if self.plugins.notice().is_none() {
                self.error = None;
            }
            cx.notify();
            true
        }

        pub(crate) fn plugin_settings_section(&self, cx: &mut Context<Self>) -> impl IntoElement {
            let p = self.palettes.get(self.dark);
            let records = self.plugins.records().to_vec();
            let pending = self.plugins.pending();
            div()
                .id("plugin-settings")
                .border_t_1()
                .border_color(p.color("border"))
                .pt(px(16.))
                .flex()
                .flex_col()
                .gap(px(10.))
                .child(self.t("plugins.title"))
                .when(self.plugins.notice().is_some(), |el| {
                    el.child(
                        div()
                            .id("plugin-settings-error")
                            .role(Role::Alert)
                            .p(px(10.))
                            .bg(p.color("error-bg"))
                            .text_color(p.color("error"))
                            .whitespace_normal()
                            .child(self.t(self.plugins.notice().unwrap_or("error.plugins"))),
                    )
                })
                .when(pending && self.plugins.pending_name().is_none(), |el| {
                    el.child(
                        div()
                            .text_color(p.color("text-muted"))
                            .child(self.t("plugins.loading")),
                    )
                })
                .when(
                    self.plugins.notice().is_some() || (!self.plugins.loaded() && !pending),
                    |el| {
                        el.child(
                            Button::new("plugins-retry")
                                .outline()
                                .small()
                                .label(self.t("plugins.retry"))
                                .loading(pending && self.plugins.pending_name().is_none())
                                .disabled(pending)
                                .on_click(
                                    cx.listener(|this, _, _, cx| this.open_plugin_settings(cx)),
                                ),
                        )
                    },
                )
                .when(self.plugins.loaded() && records.is_empty(), |el| {
                    el.child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(4.))
                            .child(self.t("plugins.empty"))
                            .child(
                                div()
                                    .text_color(p.color("text-muted"))
                                    .whitespace_normal()
                                    .child(self.t("plugins.empty_hint")),
                            ),
                    )
                })
                .children(records.iter().enumerate().map(|(index, plugin)| {
                    let name = plugin.name.clone();
                    let on = plugin.enabled;
                    let action = self.t(if on {
                        "plugins.disable"
                    } else {
                        "plugins.enable"
                    });
                    div()
                        .py(px(10.))
                        .border_b_1()
                        .border_color(p.color("border"))
                        .flex()
                        .items_center()
                        .gap(px(10.))
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .flex()
                                .items_center()
                                .gap(px(8.))
                                .child(div().font_weight(FontWeight::SEMIBOLD).child(name.clone()))
                                .child(
                                    div()
                                        .text_size(px(12.))
                                        .text_color(p.color("text-muted"))
                                        .child(plugin.version.clone()),
                                ),
                        )
                        .child(
                            div()
                                .text_size(px(12.))
                                .text_color(p.color("text-muted"))
                                .child(self.t(if on {
                                    "plugins.enabled"
                                } else {
                                    "plugins.disabled"
                                })),
                        )
                        .child(
                            Button::new(("plugin-toggle", index))
                                .outline()
                                .small()
                                .label(action)
                                .accessibility_label(format!("{} {}", name, action))
                                .loading(self.plugins.pending_name() == Some(name.as_str()))
                                .disabled(pending)
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    if let Some(command) = this.plugins.toggle(&name) {
                                        this.command(command, cx);
                                    }
                                })),
                        )
                }))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(notes: bool, lint: bool) -> Value {
        json!({
            "plugins": [
                {"name": "notes", "version": "1.2.0", "enabled": notes},
                {"name": "lint", "version": "0.4.1", "enabled": lint}
            ]
        })
    }

    fn listed(page: &PluginPage) -> Vec<(String, String, bool)> {
        page.records()
            .iter()
            .map(|plugin| (plugin.name.clone(), plugin.version.clone(), plugin.enabled))
            .collect()
    }

    #[test]
    fn host_rejection_keeps_the_listed_plugins_and_leaves_the_control_ready() {
        let stored = vec![
            ("notes".into(), "1.2.0".into(), true),
            ("lint".into(), "0.4.1".into(), false),
        ];
        let mut page = PluginPage::default();
        let get = page.load().expect("get");
        assert_eq!(get["method"], "settings.plugins.get");
        assert!(page.pending());
        assert!(page.load().is_none());
        assert!(listed(&page).is_empty());
        assert!(!page.loaded());

        page.receive(
            "plugin_settings_error",
            &sample(false, true),
            Some("error.plugins"),
        );
        assert!(listed(&page).is_empty());
        assert!(!page.loaded());
        assert!(!page.pending());
        assert_eq!(page.notice(), Some("error.plugins"));
        assert!(page.snapshot().get("saved").is_none());
        assert!(page.snapshot()["plugins"].is_null());

        assert_eq!(
            page.load().expect("retry get")["method"],
            "settings.plugins.get"
        );
        page.receive("plugin_settings", &sample(true, false), None);
        assert_eq!(listed(&page), stored);
        assert!(page.notice().is_none());
        assert!(page.loaded());
        assert!(!page.pending());

        let disable = page.toggle("notes").expect("disable");
        assert_eq!(disable["method"], "settings.plugins.disable");
        assert_eq!(disable["name"], "notes");
        assert_eq!(listed(&page), stored);
        assert!(page.toggle("notes").is_none());
        assert!(page.toggle("missing").is_none());
        assert_eq!(page.pending_name(), Some("notes"));

        page.receive(
            "plugin_settings_error",
            &sample(false, true),
            Some("status.connected"),
        );
        assert_eq!(listed(&page), stored);
        assert_eq!(page.notice(), Some("error.plugins"));
        assert!(!page.pending());
        assert_eq!(page.snapshot()["notice"], "error.plugins");

        let retry = page.toggle("notes").expect("retry disable");
        assert_eq!(retry, disable);
        page.fail_send();
        assert_eq!(listed(&page), stored);
        assert_eq!(page.notice(), Some("error.plugins"));
        assert!(!page.pending());
        assert!(page.toggle("notes").is_some());

        page.receive(
            "plugin_settings",
            &sample(false, false),
            Some("error.plugins"),
        );
        assert_eq!(
            listed(&page),
            vec![
                ("notes".into(), "1.2.0".into(), false),
                ("lint".into(), "0.4.1".into(), false),
            ]
        );
        assert!(page.notice().is_none());

        page.toggle("lint").unwrap();
        for payload in [
            json!({"plugins":[{"name":"lint","version":" ","enabled":true}]}),
            json!({"plugins":[{"name":"notes","version":"9.9.9","enabled":true,"source":"package"}]}),
        ] {
            let before = listed(&page);
            page.receive("plugin_settings", &payload, None);
            assert_eq!(listed(&page), before);
            assert_eq!(page.notice(), Some("error.plugins"));
            assert!(!page.pending());
            page.toggle("lint").unwrap();
        }
        page.receive("plugin_settings", &sample(false, true), None);
        assert_eq!(listed(&page)[1], ("lint".into(), "0.4.1".into(), true));
        assert_eq!(listed(&page)[0].1, "1.2.0");
        assert!(page.notice().is_none());
    }
}
