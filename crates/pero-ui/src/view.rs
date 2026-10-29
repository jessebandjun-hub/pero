use crate::remote_view::phase;
use crate::{
    app::Workbench,
    components::{
        ActionHandler, Ui,
        sidebar::{self, DeleteAffordance, SidebarAction, SidebarProps},
        task_detail::{self, SessionHeader, TaskDetailAction, TaskDetailProps},
        workspace::{self, AgentRow, WorkspaceAction},
    },
};
use gpui_kit::{prelude::*, *};
use pero_contracts::workbench::WorkbenchLayout;
use serde_json::json;
use std::rc::Rc;

impl Workbench {
    fn ui(&self) -> Ui<'_> {
        Ui {
            palette: self.palettes.get(self.dark),
            locale: &self.locale,
        }
    }

    pub fn icon(&self, name: &str) -> Svg {
        self.ui().icon(name)
    }

    fn sidebar(&self, width: f32, cx: &mut Context<Self>) -> Stateful<Div> {
        let on_action = Rc::new(cx.listener(
            |this: &mut Self, action: &SidebarAction, window, cx| {
                match action {
                    SidebarAction::RequestDelete(index) => {
                        this.deleting_chat = this.chats.get(*index).map(|chat| chat.id.clone());
                    }
                    SidebarAction::ConfirmDelete(index) => {
                        if this.deleting_chat.as_deref()
                            == this.chats.get(*index).map(|chat| chat.id.as_str())
                        {
                            this.delete_chat(*index, cx);
                        }
                    }
                    SidebarAction::CancelDelete => this.deleting_chat = None,
                    SidebarAction::NewChat => {
                        this.deleting_chat = None;
                        this.new_chat(cx);
                    }
                    SidebarAction::Navigate(index) => {
                        this.deleting_chat = None;
                        this.navigation = *index;
                        this.sync_follow(cx);
                        if *index == 7 {
                            this.refresh_apps(cx);
                        }
                    }
                    SidebarAction::OpenProject(key) => {
                        this.deleting_chat = None;
                        this.open_project(key.clone(), window, cx);
                    }
                    SidebarAction::ToggleProject(key) => {
                        this.deleting_chat = None;
                        if !this.collapsed_projects.remove(key) {
                            this.collapsed_projects.insert(key.clone());
                        }
                    }
                    SidebarAction::SelectSession(index) => {
                        this.deleting_chat = None;
                        this.navigation = 0;
                        this.focus_chat(Some(*index), 4, cx);
                    }
                    SidebarAction::SelectTask(id) => {
                        this.deleting_chat = None;
                        this.open_task(id.clone(), cx);
                    }
                }
                cx.notify();
            },
        ));
        let query = self.search.read(cx).value().to_lowercase();
        let matches = |title: &str| query.is_empty() || title.to_lowercase().contains(&query);
        let session_row = |i: usize, chat: &crate::conversation::Conversation| sidebar::RecentRow {
            label: chat.title.clone(),
            icon: "terminal",
            active: self.navigation == 0 && self.active_chat == Some(i),
            busy: chat.busy(),
            attention: chat.permission.is_some() || chat.error.is_some(),
            action: SidebarAction::SelectSession(i),
            delete: Some(DeleteAffordance {
                confirming: self.deleting_chat.as_deref() == Some(chat.id.as_str()),
                request: SidebarAction::RequestDelete(i),
                confirm: SidebarAction::ConfirmDelete(i),
                cancel: SidebarAction::CancelDelete,
            }),
        };
        let groups = self
            .project_catalog()
            .into_iter()
            .filter_map(|project| {
                let mut rows = self
                    .chats
                    .iter()
                    .enumerate()
                    .rev()
                    .filter(|(_, chat)| {
                        project.contains_workspace(&chat.workspace) && matches(&chat.title)
                    })
                    .take(8)
                    .map(|(i, chat)| session_row(i, chat))
                    .collect::<Vec<_>>();
                rows.extend(
                    self.remote
                        .tasks()
                        .iter()
                        .filter(|task| {
                            project.contains_task(crate::remote::string(task, "project_id"))
                                && matches(crate::remote::string(task, "title"))
                                && (matches!(
                                    phase(task),
                                    crate::cloud::Phase::Running
                                        | crate::cloud::Phase::Stopping
                                        | crate::cloud::Phase::Failed
                                        | crate::cloud::Phase::Reconcile
                                ) || self
                                    .recent_tasks
                                    .iter()
                                    .any(|id| Some(id.as_str()) == task["id"].as_str())
                                    || self.remote.data["selected_task_id"].as_str()
                                        == task["id"].as_str())
                        })
                        .take(4)
                        .map(|task| sidebar::RecentRow {
                            label: crate::remote::string(task, "title").to_string(),
                            icon: "layout-list",
                            active: self.navigation == 3
                                && self.remote.data["selected_task_id"].as_str()
                                    == task["id"].as_str(),
                            busy: matches!(
                                phase(task),
                                crate::cloud::Phase::Running | crate::cloud::Phase::Stopping
                            ),
                            attention: matches!(
                                phase(task),
                                crate::cloud::Phase::Failed | crate::cloud::Phase::Reconcile
                            ),
                            action: SidebarAction::SelectTask(
                                crate::remote::string(task, "id").to_string(),
                            ),
                            delete: None,
                        }),
                );
                if !query.is_empty() && rows.is_empty() && !matches(&project.name) {
                    return None;
                }
                Some(sidebar::ProjectGroup {
                    active: self.navigation == 3
                        && self.selected_project.as_ref() == Some(&project.key),
                    collapsed: self.collapsed_projects.contains(&project.key),
                    path: project
                        .workspace
                        .unwrap_or_else(|| project.shared_id.unwrap_or_default()),
                    key: project.key,
                    name: project.name,
                    rows,
                })
            })
            .collect();
        let ungrouped = self
            .chats
            .iter()
            .enumerate()
            .rev()
            .filter(|(_, chat)| chat.workspace.is_empty() && matches(&chat.title))
            .map(|(i, chat)| session_row(i, chat))
            .collect();
        sidebar::sidebar(
            self.ui(),
            width,
            SidebarProps {
                navigation: self.navigation,
                local_connected: self.connected,
                groups,
                ungrouped,
                search: &self.search,
            },
            self.settings_menu(cx).into_any_element(),
            on_action,
        )
    }

    fn detail(&self, window: &mut Window, cx: &mut Context<Self>) -> Div {
        let session = self
            .active_chat
            .and_then(|i| self.chats.get(i))
            .map(|chat| SessionHeader {
                title: chat.title.clone(),
                agent: chat.agent.clone(),
                closed: chat.state == "closed",
            });
        let content = if self.tab == 4 {
            self.chat_view(window, cx).into_any_element()
        } else {
            let ui = self.ui();
            div()
                .id("detail-content")
                .flex_1()
                .min_h_0()
                .overflow_y_scroll()
                .p(px(20.))
                .when(self.tab == 0 && session.is_none(), |el| {
                    el.child(ui.t("chat.welcome"))
                })
                .when(self.tab == 0 && session.is_some(), |el| {
                    el.child(ui.t("chat.open_tab"))
                })
                .when(self.tab == 1, |el| el.child(ui.t("chat.no_changes")))
                .when(self.tab == 2, |el| el.child(ui.t("chat.no_verification")))
                .when(self.tab == 3, |el| {
                    el.children(self.events.iter().map(|event| {
                        div()
                            .py(px(8.))
                            .border_b_1()
                            .border_color(ui.palette.color("border"))
                            .child(self.event_text(event))
                    }))
                })
                .into_any_element()
        };
        let on_action = Rc::new(cx.listener(
            |this, action: &TaskDetailAction, _, cx| match action {
                TaskDetailAction::NewSession => this.new_chat(cx),
                TaskDetailAction::CloseSession => {
                    if let Some(chat) = this.active_chat.and_then(|i| this.chats.get(i)) {
                        let id = chat.id.clone();
                        this.command(json!({"action":"close","thread_id":id}), cx);
                    }
                }
                TaskDetailAction::SelectTab(tab) => this.focus_chat(this.active_chat, *tab, cx),
            },
        ));
        task_detail::task_detail(
            self.ui(),
            TaskDetailProps {
                session,
                tab: self.tab,
            },
            content,
            on_action,
        )
    }

    fn workspace_actions(&self, cx: &mut Context<Self>) -> ActionHandler<WorkspaceAction> {
        Rc::new(
            cx.listener(|this, action: &WorkspaceAction, _, cx| match action {
                WorkspaceAction::NewSession(index) => {
                    this.agent_index = *index;
                    this.new_chat(cx);
                }
            }),
        )
    }

    fn environments(&self, cx: &mut Context<Self>) -> Div {
        let agents: Vec<_> = self
            .agents
            .iter()
            .map(|agent| AgentRow {
                name: agent.name.clone(),
                available: agent.available,
            })
            .collect();
        workspace::environments(self.ui(), &agents, self.workspace_actions(cx))
    }

    fn sidebar_resize(&self, sidebar_width: f32, cx: &mut Context<Self>) -> impl IntoElement {
        let workbench = cx.entity().downgrade();
        div()
            .id("sidebar-resize")
            .absolute()
            .left(px(sidebar_width - 3.))
            .top_0()
            .h_full()
            .w(px(6.))
            .child(
                canvas(
                    |bounds, window, _cx| window.insert_hitbox(bounds, HitboxBehavior::BlockMouse),
                    move |_bounds, hitbox, window, _cx| {
                        window.set_cursor_style(CursorStyle::ResizeLeftRight, &hitbox);
                        let down = workbench.clone();
                        window.on_mouse_event(move |event: &MouseDownEvent, phase, window, cx| {
                            if phase.bubble()
                                && event.button == MouseButton::Left
                                && hitbox.is_hovered(window)
                            {
                                let _ = down.update(cx, |this, cx| {
                                    this.sidebar_drag = Some(
                                        this.sidebar_drag
                                            .unwrap_or(this.preferences.layout.sidebar),
                                    );
                                    cx.stop_propagation();
                                    cx.notify();
                                });
                            }
                        });
                        let drag = workbench.clone();
                        window.on_mouse_event(move |event: &MouseMoveEvent, phase, _window, cx| {
                            if !phase.bubble() || !event.dragging() {
                                return;
                            }
                            let _ = drag.update(cx, |this, cx| {
                                if this.sidebar_drag.is_none() {
                                    return;
                                }
                                let next =
                                    WorkbenchLayout::clamp_sidebar(f32::from(event.position.x));
                                if this.sidebar_drag != Some(next) {
                                    this.sidebar_drag = Some(next);
                                    cx.notify();
                                }
                            });
                        });
                        let up = workbench.clone();
                        window.on_mouse_event(move |event: &MouseUpEvent, phase, _window, cx| {
                            if phase.bubble() && event.button == MouseButton::Left {
                                let _ = up.update(cx, |this, cx| this.commit_sidebar_drag(cx));
                            }
                        });
                    },
                )
                .size_full(),
            )
    }
}

impl Render for Workbench {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.composer_focused = self.composer.read(cx).focus_handle(cx).is_focused(window);
        self.apply_composer_context(window, cx);
        if self.input_locale != self.locale {
            self.input_locale = self.locale.clone();
            let placeholder = self.t("chat.placeholder");
            self.composer.update(cx, |input, cx| {
                input.set_placeholder(placeholder, window, cx)
            });
            let placeholder = self.t("sessions.search");
            self.search.update(cx, |input, cx| {
                input.set_placeholder(placeholder, window, cx)
            });
            let placeholder = self.t("project.search_tasks");
            self.remote.search.update(cx, |input, cx| {
                input.set_placeholder(placeholder, window, cx)
            });
            let placeholder = self.t("input.placeholder");
            self.input.update(cx, |input, cx| {
                input.set_placeholder(placeholder, window, cx)
            });
        }
        self.apply_local_execution(window, cx);
        let width: f32 = window.viewport_size().width.into();
        let height: f32 = window.viewport_size().height.into();
        let compact = width < WorkbenchLayout::COMPACT_VIEWPORT;
        if compact {
            self.sidebar_drag = None;
        }
        let sidebar_width = self
            .sidebar_drag
            .filter(|_| !compact)
            .map(|px| px as f32)
            .unwrap_or_else(|| self.preferences.layout.sidebar_px(width));
        self.follow_task_pages(window, cx);
        self.sync_app_surface(sidebar_width, width, height);
        let p = self.palettes.get(self.dark);
        div()
            .size_full()
            .relative()
            .flex()
            .bg(p.color("surface"))
            .text_color(p.color("text"))
            .font_family("Noto Sans CJK SC")
            .text_size(px(14.))
            .child(self.sidebar(sidebar_width, cx))
            .when(!compact, |el| {
                el.child(self.sidebar_resize(sidebar_width, cx))
            })
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .h_full()
                    .flex()
                    .flex_col()
                    .when(self.error.is_some(), |el| {
                        el.child(
                            div()
                                .id("workbench-error")
                                .role(Role::Alert)
                                .px(px(20.))
                                .py(px(10.))
                                .bg(p.color("error-bg"))
                                .text_color(p.color("error"))
                                .child(
                                    self.error
                                        .as_deref()
                                        .map(|error| {
                                            if error.starts_with("error.") {
                                                self.t(error)
                                            } else {
                                                error
                                            }
                                        })
                                        .unwrap_or_default()
                                        .to_string(),
                                ),
                        )
                    })
                    .when(self.navigation == 3, |el| {
                        el.child(self.project_workspace(width < 1050., cx))
                    })
                    .when(self.navigation == 0, |el| el.child(self.detail(window, cx)))
                    .when(self.navigation == 5, |el| el.child(self.workers_view(cx)))
                    .when(self.show_local_execution_onboarding(), |el| {
                        el.child(
                            div()
                                .id("first-run-setup")
                                .flex_1()
                                .min_h_0()
                                .overflow_y_scroll()
                                .child(self.local_execution_setup_view(true, cx)),
                        )
                    })
                    .when(
                        self.navigation == 1 && !self.show_local_execution_onboarding(),
                        |el| el.child(self.projects_view(cx)),
                    )
                    .when(self.navigation == 2, |el| el.child(self.environments(cx)))
                    .when(self.navigation == 7, |el| el.child(self.apps_view(cx)))
                    .when(self.navigation == 8, |el| el.child(self.insights_view(cx)))
                    .when(self.navigation == 9, |el| {
                        el.child(self.remote_connection_view(cx))
                    })
                    .when(self.navigation == 10, |el| {
                        el.child(self.general_settings(cx))
                    }),
            )
    }
}
