//! Model section of All settings.
//! The form edits the Host value. A rejected save leaves the typed fields in place.

use crate::{
    app::{Workbench, session_command},
    model_page::ModelScreenState,
};
use gpui_kit::{
    component::{
        button::*,
        input::{Input, InputState},
        *,
    },
    prelude::*,
    *,
};
use serde_json::Value;

impl Workbench {
    pub(crate) fn open_general_settings(&mut self, cx: &mut Context<Self>) {
        self.navigation = 10;
        self.sync_follow(cx);
        self.request_model_settings(true, cx);
    }

    pub(crate) fn request_model_settings(&mut self, show_effective: bool, cx: &mut Context<Self>) {
        let Some(command) = self.model.load(self.connected, show_effective) else {
            cx.notify();
            return;
        };
        self.send_model(command, cx);
    }

    pub(crate) fn note_model_typing(&mut self, cx: &mut Context<Self>) {
        let provider = self.model_provider.read(cx).value().to_string();
        let model = self.model_name.read(cx).value().to_string();
        let preset = self.model_preset.read(cx).value().to_string();
        self.model.note_typing(&provider, &model, &preset);
    }

    pub(crate) fn apply_model_inputs(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(draft) = self.model.take_input_fill() else {
            return;
        };
        self.model.set_applying_fill(true);
        self.fill_model_input(&self.model_provider.clone(), &draft.provider, window, cx);
        self.fill_model_input(&self.model_name.clone(), &draft.model, window, cx);
        self.fill_model_input(
            &self.model_preset.clone(),
            &draft.default_preset,
            window,
            cx,
        );
        self.model.set_applying_fill(false);
    }

    fn fill_model_input(
        &self,
        input: &Entity<InputState>,
        value: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if input.read(cx).value().to_string() != value {
            input.update(cx, |input, cx| {
                input.set_value(value.to_string(), window, cx)
            });
        }
    }

    fn send_model(&mut self, command: Value, cx: &mut Context<Self>) {
        if session_command(&command.to_string()).is_err() {
            self.model.rejected("error.connection");
            self.error = Some("error.connection".into());
        }
        cx.notify();
    }

    pub(crate) fn model_section(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let screen = self.model.screen(self.connected);
        let p = self.palettes.get(self.dark);
        let status = match screen.state {
            ModelScreenState::Disabled => self.t("settings.model_disabled"),
            ModelScreenState::Loading => self.t("settings.model_loading"),
            ModelScreenState::Empty => self.t("settings.model_empty"),
            ModelScreenState::Error => {
                self.t(screen.error.as_deref().unwrap_or("error.preferences"))
            }
            ModelScreenState::Ready => "",
        };
        let alert = matches!(screen.state, ModelScreenState::Error);
        let stored = screen.draft.then(|| screen.effective.clone()).flatten();
        let show_form = screen.effective.is_some()
            || screen.draft
            || matches!(
                screen.state,
                ModelScreenState::Empty | ModelScreenState::Ready
            );
        let enabled = screen.actions_enabled;
        div()
            .id("model-settings")
            .border_t_1()
            .border_color(p.color("border"))
            .pt(px(16.))
            .flex()
            .flex_col()
            .gap(px(12.))
            .child(
                div()
                    .text_size(px(16.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(self.t("settings.model")),
            )
            .child(
                div()
                    .text_size(px(13.))
                    .text_color(p.color("text-muted"))
                    .whitespace_normal()
                    .child(self.t("settings.model_hint")),
            )
            .when(!status.is_empty(), |el| {
                el.child(
                    div()
                        .id("model-status")
                        .when(alert, |el| el.role(Role::Alert))
                        .whitespace_normal()
                        .text_color(p.color(if alert { "error" } else { "text" }))
                        .child(status.to_string()),
                )
            })
            .when(screen.can_retry, |el| {
                el.child(
                    Button::new("model-retry")
                        .outline()
                        .label(self.t("settings.model_retry"))
                        .on_click(
                            cx.listener(|this, _, _, cx| this.request_model_settings(true, cx)),
                        ),
                )
            })
            .when(show_form, |el| {
                el.child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(10.))
                        .max_w(px(640.))
                        .child(self.model_field(
                            "settings.model_provider",
                            "model-provider",
                            &self.model_provider,
                            enabled,
                        ))
                        .child(self.model_field(
                            "settings.model_name",
                            "model-name",
                            &self.model_name,
                            enabled,
                        ))
                        .child(self.model_field(
                            "settings.model_preset",
                            "model-preset",
                            &self.model_preset,
                            enabled,
                        ))
                        .when_some(stored, |el, saved| {
                            el.child(
                                div()
                                    .id("model-stored")
                                    .text_size(px(12.))
                                    .text_color(p.color("text-muted"))
                                    .whitespace_normal()
                                    .child(format!(
                                        "{} · {} / {} / {}",
                                        self.t("settings.model_saved"),
                                        saved.provider,
                                        saved.model,
                                        saved.default_preset
                                    )),
                            )
                        })
                        .child(
                            Button::new("model-save")
                                .primary()
                                .label(self.t("settings.model_save"))
                                .loading(screen.saving)
                                .disabled(!enabled)
                                .on_click(cx.listener(|this, _, _, cx| {
                                    let provider = this.model_provider.read(cx).value().to_string();
                                    let model = this.model_name.read(cx).value().to_string();
                                    let preset = this.model_preset.read(cx).value().to_string();
                                    if let Some(command) =
                                        this.model.save(this.connected, provider, model, preset)
                                    {
                                        this.send_model(command, cx);
                                    }
                                })),
                        ),
                )
            })
    }

    fn model_field(
        &self,
        label_key: &str,
        accessibility_id: &'static str,
        input: &Entity<InputState>,
        enabled: bool,
    ) -> impl IntoElement {
        let label = self.t(label_key);
        div().flex().flex_col().gap(px(6.)).child(label).child(
            Input::new(input)
                .accessibility_id(accessibility_id)
                .aria_label(label)
                .disabled(!enabled),
        )
    }
}
