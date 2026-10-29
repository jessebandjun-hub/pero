//! Credential settings page. A draft secret is write-only: public snapshots,
//! `Debug`, and connection status omit it. A rejected save keeps the draft and
//! does not report success. A saved replace drops the secret from the draft.

use pero_contracts::HostEvent;
use pero_contracts::credentials::{
    CredentialConnectionStatus, CredentialSettingsView, CredentialView,
};
use serde_json::{Value, json};
use std::collections::VecDeque;

pub(crate) const CREDENTIALS_NAVIGATION: usize = 11;

#[derive(Clone, Copy, PartialEq, Eq)]
enum CredentialWrite {
    Replace,
    Remove,
}

#[derive(PartialEq, Eq)]
pub(crate) struct CredentialSettingsPage {
    slots: Vec<CredentialView>,
    draft_id: String,
    draft_label: String,
    draft_secret: String,
    error: Option<&'static str>,
    saved: bool,
    /// Outstanding credential commands, oldest first. `None` is a refresh.
    pending: VecDeque<Option<CredentialWrite>>,
    pending_remove: Option<String>,
    clear_secret: bool,
    suppress_edit: bool,
}

impl Default for CredentialSettingsPage {
    fn default() -> Self {
        Self {
            slots: Vec::new(),
            draft_id: String::new(),
            draft_label: String::new(),
            draft_secret: String::new(),
            error: None,
            saved: false,
            pending: VecDeque::new(),
            pending_remove: None,
            clear_secret: false,
            suppress_edit: false,
        }
    }
}

impl std::fmt::Debug for CredentialSettingsPage {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("CredentialSettingsPage")
            .field("slots", &self.slots)
            .field("draft_id", &self.draft_id)
            .field("draft_label", &self.draft_label)
            .field("secret_present", &!self.draft_secret.is_empty())
            .field("error", &self.error)
            .field("saved", &self.saved)
            .field("writing", &self.writing())
            .field("pending_remove", &self.pending_remove)
            .finish()
    }
}

impl CredentialSettingsPage {
    pub(crate) fn slots(&self) -> &[CredentialView] {
        &self.slots
    }

    pub(crate) fn draft_id(&self) -> &str {
        &self.draft_id
    }

    pub(crate) fn draft_label(&self) -> &str {
        &self.draft_label
    }

    pub(crate) fn draft_secret(&self) -> &str {
        &self.draft_secret
    }

    pub(crate) fn error(&self) -> Option<&str> {
        self.error
    }

    pub(crate) fn saved(&self) -> bool {
        self.saved
    }

    pub(crate) fn writing(&self) -> bool {
        self.pending.iter().any(|item| item.is_some())
    }

    pub(crate) fn queue_refresh(&mut self) {
        self.pending.push_back(None);
    }

    pub(crate) fn pending_remove(&self) -> Option<&str> {
        self.pending_remove.as_deref()
    }

    pub(crate) fn note_edit(&mut self) {
        if self.suppress_edit {
            return;
        }
        self.saved = false;
        self.error = None;
    }

    pub(crate) fn set_draft(
        &mut self,
        id: impl Into<String>,
        label: impl Into<String>,
        secret: impl Into<String>,
    ) {
        self.draft_id = id.into();
        self.draft_label = label.into();
        self.draft_secret = secret.into();
        self.saved = false;
    }

    /// Copy a stored slot into the draft. The stored secret is not available.
    pub(crate) fn select(&mut self, id: &str) {
        let Some(slot) = self.slots.iter().find(|slot| slot.id == id) else {
            return;
        };
        self.draft_id = slot.id.clone();
        self.draft_label = slot.label.clone();
        self.draft_secret.clear();
        self.saved = false;
        self.error = None;
        self.pending_remove = None;
    }

    pub(crate) fn ask_remove(&mut self, id: &str) {
        if self.slots.iter().any(|slot| slot.id == id) {
            self.pending_remove = Some(id.to_owned());
        }
    }

    pub(crate) fn replace_command(&mut self) -> Value {
        self.pending.push_back(Some(CredentialWrite::Replace));
        self.saved = false;
        self.error = None;
        self.pending_remove = None;
        json!({
            "method": "settings.credentials",
            "operation": "replace",
            "id": self.draft_id,
            "label": self.draft_label,
            "secret": self.draft_secret,
        })
    }

    pub(crate) fn remove_command(&mut self) -> Option<Value> {
        let id = self.pending_remove.clone()?;
        self.pending.push_back(Some(CredentialWrite::Remove));
        self.saved = false;
        self.error = None;
        Some(json!({
            "method": "settings.credentials",
            "operation": "remove",
            "id": id,
        }))
    }

    /// Apply a host event. An unreadable credential view is not shown, so a
    /// secret envelope on that payload cannot reach the page.
    pub(crate) fn receive(&mut self, event: &HostEvent) -> bool {
        if event.kind == "credential_settings" {
            match serde_json::from_value::<CredentialSettingsView>(event.data.clone()) {
                Ok(view) => self.receive_view(view),
                Err(_) => self.receive_reject(),
            }
            return true;
        }
        if event.kind == "error" && event.message_key.as_deref() == Some("error.credentials") {
            self.receive_reject();
            return true;
        }
        false
    }

    pub(crate) fn take_clear_secret(&mut self) -> bool {
        let clear = self.clear_secret;
        self.clear_secret = false;
        if clear {
            self.suppress_edit = true;
        }
        clear
    }

    pub(crate) fn end_secret_sync(&mut self) {
        self.suppress_edit = false;
    }

    /// Public page state. `secret_present` reports that a draft exists.
    pub(crate) fn public_state(&self) -> Value {
        json!({
            "slots": self.slots,
            "draftId": self.draft_id,
            "draftLabel": self.draft_label,
            "secretPresent": !self.draft_secret.is_empty(),
            "error": self.error,
            "saved": self.saved,
            "writing": self.writing(),
            "pendingRemove": self.pending_remove,
        })
    }

    fn receive_view(&mut self, view: CredentialSettingsView) {
        let wrote = self.pending.pop_front().flatten();
        self.slots = view.credentials;
        self.error = None;
        match wrote {
            Some(CredentialWrite::Replace) => {
                self.draft_secret.clear();
                self.clear_secret = true;
                self.saved = true;
            }
            Some(CredentialWrite::Remove) => {
                self.saved = true;
                self.pending_remove = None;
            }
            None => self.saved = false,
        }
    }

    fn receive_reject(&mut self) {
        let _ = self.pending.pop_front();
        self.saved = false;
        self.error = Some("error.credentials");
    }
}

#[cfg(any(target_family = "wasm", feature = "native"))]
impl crate::app::Workbench {
    pub(crate) fn open_credentials(&mut self, cx: &mut gpui_kit::Context<Self>) {
        self.navigation = CREDENTIALS_NAVIGATION;
        self.credentials.queue_refresh();
        self.command(json!({"method":"settings.credentials.get"}), cx);
    }

    pub(crate) fn select_credential(
        &mut self,
        id: &str,
        window: &mut gpui_kit::Window,
        cx: &mut gpui_kit::Context<Self>,
    ) {
        self.credentials.select(id);
        let draft_id = self.credentials.draft_id().to_owned();
        let draft_label = self.credentials.draft_label().to_owned();
        self.credential_id
            .update(cx, |input, cx| input.set_value(draft_id, window, cx));
        self.credential_label
            .update(cx, |input, cx| input.set_value(draft_label, window, cx));
        self.credential_secret
            .update(cx, |input, cx| input.set_value(String::new(), window, cx));
        let focus = gpui_kit::Focusable::focus_handle(&self.credential_secret, cx);
        window.focus(&focus, cx);
        cx.notify();
    }

    pub(crate) fn save_credential(&mut self, cx: &mut gpui_kit::Context<Self>) {
        let id = self.credential_id.read(cx).value().trim().to_owned();
        let label = self.credential_label.read(cx).value().trim().to_owned();
        let secret = self.credential_secret.read(cx).value().trim().to_owned();
        self.credentials.set_draft(id, label, secret);
        let command = self.credentials.replace_command();
        if self.error.as_deref() == Some("error.credentials") {
            self.error = None;
        }
        self.command(command, cx);
    }

    pub(crate) fn remove_credential(&mut self, id: &str, cx: &mut gpui_kit::Context<Self>) {
        if self.credentials.pending_remove() != Some(id) {
            self.credentials.ask_remove(id);
            cx.notify();
            return;
        }
        let Some(command) = self.credentials.remove_command() else {
            return;
        };
        if self.error.as_deref() == Some("error.credentials") {
            self.error = None;
        }
        self.command(command, cx);
    }

    pub(crate) fn receive_credentials(
        &mut self,
        event: &HostEvent,
        cx: &mut gpui_kit::Context<Self>,
    ) -> bool {
        if !self.credentials.receive(event) {
            return false;
        }
        if let Some(key) = self.credentials.error() {
            self.error = Some(key.to_owned());
        } else if self.error.as_deref() == Some("error.credentials") {
            self.error = None;
        }
        cx.notify();
        true
    }

    pub(crate) fn sync_credential_secret(
        &mut self,
        window: &mut gpui_kit::Window,
        cx: &mut gpui_kit::Context<Self>,
    ) {
        if !self.credentials.take_clear_secret() {
            return;
        }
        self.credential_secret
            .update(cx, |input, cx| input.set_value(String::new(), window, cx));
        self.credentials.end_secret_sync();
    }

    pub(crate) fn set_credential_placeholders(
        &mut self,
        window: &mut gpui_kit::Window,
        cx: &mut gpui_kit::Context<Self>,
    ) {
        let id = self.t("settings.credentials.id");
        self.credential_id
            .update(cx, |input, cx| input.set_placeholder(id, window, cx));
        let label = self.t("settings.credentials.label");
        self.credential_label
            .update(cx, |input, cx| input.set_placeholder(label, window, cx));
        let secret = self.t("settings.credentials.secret");
        self.credential_secret
            .update(cx, |input, cx| input.set_placeholder(secret, window, cx));
    }

    pub(crate) fn credentials_view(
        &self,
        cx: &mut gpui_kit::Context<Self>,
    ) -> impl gpui_kit::IntoElement {
        use gpui_kit::component::button::*;
        use gpui_kit::component::input::{Input, InputContentType};
        use gpui_kit::component::{Disableable, Sizable};
        use gpui_kit::prelude::*;
        use gpui_kit::*;

        let p = self.palettes.get(self.dark);
        let writing = self.credentials.writing();
        let rows: Vec<(String, String, CredentialConnectionStatus)> = self
            .credentials
            .slots()
            .iter()
            .map(|slot| (slot.id.clone(), slot.label.clone(), slot.status))
            .collect();
        let pending = self.credentials.pending_remove().map(str::to_owned);
        div()
            .id("credential-settings")
            .flex_1()
            .min_h_0()
            .overflow_y_scroll()
            .p(px(24.))
            .child(
                div()
                    .max_w(px(720.))
                    .flex()
                    .flex_col()
                    .gap(px(16.))
                    .child(
                        div()
                            .text_size(px(24.))
                            .font_weight(FontWeight::BOLD)
                            .child(self.t("settings.credentials.title")),
                    )
                    .child(
                        div()
                            .whitespace_normal()
                            .text_color(p.color("text-muted"))
                            .child(self.t("settings.credentials.secret_help")),
                    )
                    .when(rows.is_empty(), |el| {
                        el.child(
                            div()
                                .id("credential-empty")
                                .text_color(p.color("text-muted"))
                                .child(self.t("settings.credentials.empty")),
                        )
                    })
                    .children(
                        rows.into_iter()
                            .enumerate()
                            .map(|(index, (id, label, status))| {
                                let status = match status {
                                    CredentialConnectionStatus::Configured => {
                                        self.t("settings.credentials.configured")
                                    }
                                };
                                let confirming = pending.as_deref() == Some(id.as_str());
                                let select_id = id.clone();
                                let remove_id = id.clone();
                                div()
                                    .id(("credential-slot", index))
                                    .flex()
                                    .flex_wrap()
                                    .items_center()
                                    .gap(px(8.))
                                    .py(px(8.))
                                    .border_b_1()
                                    .border_color(p.color("border"))
                                    .child(
                                        div().flex_1().min_w(px(160.)).child(label).child(
                                            div()
                                                .text_size(px(12.))
                                                .text_color(p.color("text-muted"))
                                                .child(format!("{id} · {status}")),
                                        ),
                                    )
                                    .child(
                                        Button::new(("credential-replace", index))
                                            .outline()
                                            .small()
                                            .label(self.t("settings.credentials.replace"))
                                            .disabled(writing)
                                            .on_click(cx.listener(move |this, _, window, cx| {
                                                this.select_credential(&select_id, window, cx);
                                            })),
                                    )
                                    .child(
                                        Button::new(("credential-remove", index))
                                            .outline()
                                            .small()
                                            .label(self.t(if confirming {
                                                "settings.credentials.confirm_remove"
                                            } else {
                                                "settings.credentials.remove"
                                            }))
                                            .disabled(writing)
                                            .on_click(cx.listener(move |this, _, _, cx| {
                                                this.remove_credential(&remove_id, cx);
                                            })),
                                    )
                            }),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(6.))
                            .child(self.t("settings.credentials.id"))
                            .child(
                                Input::new(&self.credential_id)
                                    .id("credential-id")
                                    .aria_label(self.t("settings.credentials.id"))
                                    .w_full()
                                    .disabled(writing),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(6.))
                            .child(self.t("settings.credentials.label"))
                            .child(
                                Input::new(&self.credential_label)
                                    .id("credential-label")
                                    .aria_label(self.t("settings.credentials.label"))
                                    .w_full()
                                    .disabled(writing),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(6.))
                            .child(self.t("settings.credentials.secret"))
                            .child(
                                Input::new(&self.credential_secret)
                                    .id("credential-secret")
                                    .aria_label(self.t("settings.credentials.secret"))
                                    .content_type(InputContentType::Password)
                                    .w_full()
                                    .disabled(writing),
                            ),
                    )
                    .child(
                        div().flex().items_center().gap(px(10.)).child(
                            Button::new("credential-save")
                                .primary()
                                .label(self.t(if writing {
                                    "settings.credentials.saving"
                                } else {
                                    "settings.credentials.save"
                                }))
                                .disabled(writing)
                                .on_click(cx.listener(|this, _, _, cx| this.save_credential(cx))),
                        ),
                    )
                    .when(self.credentials.saved(), |el| {
                        el.child(
                            div()
                                .id("credential-saved")
                                .text_color(p.color("text-muted"))
                                .child(self.t("settings.credentials.saved")),
                        )
                    }),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SECRET: &str = "c2VjcmV0LW1hcmtlci1kb25vdC1lY2hv";
    const PLAINTEXT: &str = "secret-marker-donot-echo";

    fn hidden(page: &CredentialSettingsPage) {
        let debug = format!("{page:?}");
        let public = page.public_state().to_string();
        assert!(
            !debug.contains(SECRET) && !debug.contains(PLAINTEXT),
            "{debug}"
        );
        assert!(
            !public.contains(SECRET) && !public.contains(PLAINTEXT),
            "{public}"
        );
    }

    fn view(id: &str, label: &str) -> CredentialSettingsView {
        CredentialSettingsView {
            credentials: vec![CredentialView {
                id: id.into(),
                label: label.into(),
                configured: true,
                status: CredentialConnectionStatus::Configured,
            }],
        }
    }

    fn event(kind: &str, data: Value) -> HostEvent {
        let mut event = HostEvent::new(kind, None, String::new());
        event.data = data;
        event
    }

    #[test]
    fn rejected_save_keeps_the_draft_and_success_drops_the_secret() {
        let mut page = CredentialSettingsPage::default();
        page.set_draft("github", "GitHub", SECRET);
        hidden(&page);
        assert!(page.public_state()["secretPresent"].as_bool().unwrap());

        let refresh = event(
            "credential_settings",
            serde_json::to_value(view("github", "GitHub")).unwrap(),
        );
        assert!(page.receive(&refresh));
        assert_eq!(page.draft_secret(), SECRET);
        assert!(!page.saved());
        assert_eq!(page.slots().len(), 1);
        assert!(page.slots()[0].configured);
        hidden(&page);

        page.queue_refresh();
        let command = page.replace_command();
        assert_eq!(command["secret"], SECRET);
        assert!(page.writing());
        assert!(page.receive(&refresh));
        assert_eq!(page.draft_secret(), SECRET);
        assert!(!page.saved());
        assert!(page.writing());
        hidden(&page);
        let rejected = HostEvent::message("error", None, "error.credentials");
        assert!(page.receive(&rejected));
        assert_eq!(page.draft_secret(), SECRET);
        assert_eq!(page.draft_id(), "github");
        assert_eq!(page.draft_label(), "GitHub");
        assert_eq!(page.error(), Some("error.credentials"));
        assert!(!page.saved());
        assert!(!page.writing());
        hidden(&page);

        let again = page.replace_command();
        assert_eq!(again["secret"], SECRET);
        assert!(page.receive(&event(
            "credential_settings",
            serde_json::to_value(view("github", "GitHub")).unwrap(),
        )));
        assert!(page.draft_secret().is_empty());
        assert!(page.saved());
        assert!(page.error().is_none());
        assert!(page.take_clear_secret());
        assert!(!page.take_clear_secret());
        hidden(&page);

        page.set_draft("github", "GitHub", SECRET);
        page.ask_remove("github");
        let remove = page.remove_command().unwrap();
        assert!(remove.get("secret").is_none());
        assert!(!remove.to_string().contains(SECRET));
        assert!(page.receive(&rejected));
        assert_eq!(page.draft_secret(), SECRET);
        assert_eq!(page.pending_remove(), Some("github"));
        assert!(!page.saved());
        hidden(&page);

        let _ = page.remove_command();
        assert!(page.receive(&event(
            "credential_settings",
            serde_json::to_value(CredentialSettingsView::default()).unwrap(),
        )));
        assert!(page.saved());
        assert!(page.pending_remove().is_none());
        assert_eq!(page.draft_secret(), SECRET);
        assert!(page.slots().is_empty());
        hidden(&page);

        page.set_draft("github", "GitHub", SECRET);
        let mut leaked = event(
            "credential_settings",
            json!({"credentials":[{"id":"github","label":"GitHub","configured":true,"status":"configured","secret":SECRET}]}),
        );
        leaked.text = SECRET.into();
        assert!(page.receive(&leaked));
        assert_eq!(page.draft_secret(), SECRET);
        assert!(!page.saved());
        assert_eq!(page.error(), Some("error.credentials"));
        assert!(page.slots().is_empty());
        hidden(&page);
        assert!(!page.receive(&HostEvent::new("ready", None, String::new())));
    }

    #[test]
    fn credential_page_copy_exists() {
        for key in [
            "settings.credentials",
            "settings.credentials.title",
            "settings.credentials.empty",
            "settings.credentials.id",
            "settings.credentials.label",
            "settings.credentials.secret",
            "settings.credentials.secret_help",
            "settings.credentials.save",
            "settings.credentials.saving",
            "settings.credentials.remove",
            "settings.credentials.confirm_remove",
            "settings.credentials.replace",
            "settings.credentials.configured",
            "settings.credentials.saved",
            "error.credentials",
        ] {
            assert_ne!(
                pero_contracts::i18n::text("en", key),
                "[missing translation]",
                "{key}"
            );
            assert_ne!(
                pero_contracts::i18n::text("zh-CN", key),
                "[missing translation]",
                "{key}"
            );
            assert_ne!(
                pero_contracts::i18n::text("en", key),
                pero_contracts::i18n::text("zh-CN", key)
            );
        }
    }
}
