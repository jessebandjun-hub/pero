//! Automation section of the Desktop settings page.
use crate::{
    app::{session_command, Workbench},
    automation_page::{AutomationPending, AutomationScreenState},
};
use gpui_kit::{
    component::{
        button::*,
        input::{Input, InputContent, Textarea},
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
        self.request_automations(cx);
    }

    pub(crate) fn request_automations(&mut self, cx: &mut Context<Self>) {
        let Some(command) = self.automation.load(self.connected) else {
            cx.notify();
            return;
        };
        self.send_automation(command, cx);
    }

    pub(crate) fn note_automation_typing(&mut self, cx: &mut Context<Self>) {
        let name = self.automation_name.read(cx).value().to_string();
        let instruction = self.automation_instruction.read(cx).value().to_string();
        if self.automation.creating() {
            let id = self.automation_id.read(cx).value().to_string();
            self.automation.note_create(&id, &name, &instruction);
            return;
        }
        let Some(id) = self.automation.selected_id().map(str::to_string) else {
            return;
        };
        self.automation.note_typing(&id, &name, &instruction);
    }

    pub(crate) fn apply_automation_inputs(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(fill) = self.automation.take_input_fill() else {
            return;
        };
        self.fill_automation_inputs(fill.id, fill.name, fill.instruction, window, cx);
    }

    fn fill_automation_inputs(
        &mut self,
        id: Option<String>,
        name: String,
        instruction: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(id) = id {
            if self.automation_id.read(cx).value().to_string() != id {
                self.automation_id
                    .update(cx, |input, cx| input.set_value(id, window, cx));
            }
        }
        if self.automation_name.read(cx).value().to_string() != name {
            self.automation_name
                .update(cx, |input, cx| input.set_value(name, window, cx));
        }
        if self.automation_instruction.read(cx).value().to_string() != instruction {
            self.automation_instruction.update(cx, |input, cx| {
                input.set_value(InputContent::new(instruction), window, cx);
            });
        }
    }

    fn send_automation(&mut self, command: Value, cx: &mut Context<Self>) {
        if session_command(&command.to_string()).is_err() {
            self.automation.rejected("error.connection");
            self.error = Some("error.connection".into());
        }
        cx.notify();
    }

    pub(crate) fn automation_section(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let screen = self.automation.screen(self.connected);
        let p = self.palettes.get(self.dark);
        let status = match screen.state {
            AutomationScreenState::Disabled => self.t("settings.automations_disabled"),
            AutomationScreenState::Loading => self.t("settings.automations_loading"),
            AutomationScreenState::Empty => self.t("settings.automations_empty"),
            AutomationScreenState::Error => {
                self.t(screen.error.as_deref().unwrap_or("error.automation"))
            }
            AutomationScreenState::Ready => "",
        };
        let alert = matches!(screen.state, AutomationScreenState::Error);
        let selected = screen.selected.clone();
        let selected_row = screen
            .rows
            .iter()
            .find(|row| Some(&row.id) == selected.as_ref())
            .cloned();
        let stored_name = selected_row.as_ref().and_then(|row| {
            screen
                .data
                .automations
                .iter()
                .find(|item| item.id == row.id && row.draft)
                .map(|item| item.name.clone())
        });
        let paused = selected_row.as_ref().is_some_and(|row| row.paused);
        let actions_enabled = screen.actions_enabled;
        let fields_enabled = if screen.creating {
            screen.can_create
        } else {
            actions_enabled
        };
        let show_edit = !screen.creating
            && selected_row.is_some()
            && !matches!(screen.state, AutomationScreenState::Loading);
        let show_create =
            screen.creating && !matches!(screen.state, AutomationScreenState::Loading);
        div()
            .id("automation-settings")
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
                    .child(self.t("settings.automations")),
            )
            .child(
                div()
                    .text_size(px(13.))
                    .text_color(p.color("text-muted"))
                    .whitespace_normal()
                    .child(self.t("settings.automations_hint")),
            )
            .when(!status.is_empty(), |el| {
                el.child(
                    div()
                        .id("automation-status")
                        .when(alert, |el| el.role(Role::Alert))
                        .whitespace_normal()
                        .text_color(p.color(if alert { "error" } else { "text" }))
                        .child(status.to_string()),
                )
            })
            .when(screen.can_retry, |el| {
                el.child(
                    Button::new("automation-retry")
                        .outline()
                        .label(self.t("settings.automations_retry"))
                        .on_click(cx.listener(|this, _, _, cx| this.request_automations(cx))),
                )
            })
            .when(!screen.creating && !screen.rows.is_empty(), |el| {
                el.child(
                    Button::new("automation-new")
                        .outline()
                        .label(self.t("settings.automations_new"))
                        .disabled(!screen.can_create)
                        .on_click(cx.listener(|this, _, window, cx| {
                            if this.automation.begin_create() {
                                this.apply_automation_inputs(window, cx);
                            }
                        })),
                )
            })
            .when(!screen.rows.is_empty(), |el| {
                el.child(div().flex().flex_wrap().gap(px(8.)).children(
                    screen.rows.iter().enumerate().map(|(index, row)| {
                        let id = row.id.clone();
                        let status = if row.paused {
                            self.t("settings.automations_paused")
                        } else {
                            self.t("settings.automations_active")
                        };
                        let name = if row.name.trim().is_empty() {
                            row.id.clone()
                        } else {
                            row.name.clone()
                        };
                        Button::new(("automation-row", index))
                            .outline()
                            .label(format!("{name} · {status}"))
                            .toggled(selected.as_deref() == Some(row.id.as_str()))
                            .on_click(cx.listener(move |this, _, window, cx| {
                                if let Some((name, instruction)) = this.automation.select(&id) {
                                    this.fill_automation_inputs(
                                        None,
                                        name,
                                        instruction,
                                        window,
                                        cx,
                                    );
                                }
                            }))
                    }),
                ))
            })
            .when(show_create || show_edit, |el| {
                el.child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(10.))
                        .max_w(px(640.))
                        .when(show_create, |el| {
                            el.child(self.t("settings.automations_id")).child(
                                Input::new(&self.automation_id)
                                    .accessibility_id("automation-id")
                                    .aria_label(self.t("settings.automations_id"))
                                    .disabled(!fields_enabled),
                            )
                        })
                        .child(self.t("settings.automations_name"))
                        .child(
                            Input::new(&self.automation_name)
                                .accessibility_id("automation-name")
                                .aria_label(self.t("settings.automations_name"))
                                .disabled(!fields_enabled),
                        )
                        .child(self.t("settings.automations_instruction"))
                        .child(
                            Textarea::new(&self.automation_instruction)
                                .accessibility_id("automation-instruction")
                                .aria_label(self.t("settings.automations_instruction"))
                                .disabled(!fields_enabled)
                                .h(px(96.)),
                        )
                        .when(show_edit, |el| {
                            el.when_some(stored_name, |el, name| {
                                el.child(
                                    div()
                                        .id("automation-stored")
                                        .text_size(px(12.))
                                        .text_color(p.color("text-muted"))
                                        .whitespace_normal()
                                        .child(format!(
                                            "{} · {name}",
                                            self.t("settings.automations_stored")
                                        )),
                                )
                            })
                            .child(
                                div()
                                    .flex()
                                    .flex_wrap()
                                    .gap(px(8.))
                                    .child(
                                        Button::new("automation-pause")
                                            .outline()
                                            .label(self.t("settings.automations_pause"))
                                            .loading(
                                                screen.pending == Some(AutomationPending::Pause),
                                            )
                                            .disabled(!actions_enabled || paused)
                                            .on_click(cx.listener(|this, _, _, cx| {
                                                if let Some(command) =
                                                    this.automation.pause(this.connected)
                                                {
                                                    this.send_automation(command, cx);
                                                }
                                            })),
                                    )
                                    .child(
                                        Button::new("automation-resume")
                                            .outline()
                                            .label(self.t("settings.automations_resume"))
                                            .loading(
                                                screen.pending == Some(AutomationPending::Resume),
                                            )
                                            .disabled(!actions_enabled || !paused)
                                            .on_click(cx.listener(|this, _, _, cx| {
                                                if let Some(command) =
                                                    this.automation.resume(this.connected)
                                                {
                                                    this.send_automation(command, cx);
                                                }
                                            })),
                                    )
                                    .child(
                                        Button::new("automation-save")
                                            .primary()
                                            .label(self.t("settings.automations_save"))
                                            .loading(
                                                screen.pending == Some(AutomationPending::Update),
                                            )
                                            .disabled(!actions_enabled)
                                            .on_click(cx.listener(|this, _, _, cx| {
                                                let name = this
                                                    .automation_name
                                                    .read(cx)
                                                    .value()
                                                    .to_string();
                                                let instruction = this
                                                    .automation_instruction
                                                    .read(cx)
                                                    .value()
                                                    .to_string();
                                                if let Some(command) = this.automation.update(
                                                    this.connected,
                                                    name,
                                                    instruction,
                                                ) {
                                                    this.send_automation(command, cx);
                                                }
                                            })),
                                    ),
                            )
                        })
                        .when(show_create, |el| {
                            el.child(
                                Button::new("automation-create")
                                    .primary()
                                    .label(self.t("settings.automations_create"))
                                    .loading(screen.pending == Some(AutomationPending::Create))
                                    .disabled(!screen.can_create)
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        let id = this.automation_id.read(cx).value().to_string();
                                        let name =
                                            this.automation_name.read(cx).value().to_string();
                                        let instruction = this
                                            .automation_instruction
                                            .read(cx)
                                            .value()
                                            .to_string();
                                        if let Some(command) = this.automation.create(
                                            this.connected,
                                            id,
                                            name,
                                            instruction,
                                        ) {
                                            this.send_automation(command, cx);
                                        }
                                    })),
                            )
                        }),
                )
            })
    }
}
