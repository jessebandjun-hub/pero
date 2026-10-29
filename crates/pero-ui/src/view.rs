use crate::remote_view::phase;
use crate::{
    app::{ColumnResize, Workbench},
    components::{
        ActionHandler, Ui,
        sidebar::{self, DeleteAffordance, SidebarAction, SidebarProps},
        task_detail::{self, SessionHeader, TaskDetailAction, TaskDetailProps},
        workspace::{self, AgentRow, WorkspaceAction},
    },
};
use gpui_kit::{
    component::{button::*, *},
    prelude::*,
    *,
};
use pero_contracts::layout::{LayoutChrome, LayoutColumn, LayoutMode};
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
                        this.close_drawer();
                    }
                    SidebarAction::Navigate(index) => {
                        this.deleting_chat = None;
                        this.navigation = *index;
                        this.sync_follow(cx);
                        if *index == 7 {
                            this.refresh_apps(cx);
                        }
                        this.close_drawer();
                    }
                    SidebarAction::OpenProject(key) => {
                        this.deleting_chat = None;
                        this.open_project(key.clone(), window, cx);
                        this.close_drawer();
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
                        this.close_drawer();
                    }
                    SidebarAction::SelectTask(id) => {
                        this.deleting_chat = None;
                        this.open_task(id.clone(), cx);
                        this.close_drawer();
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

    pub(crate) fn chrome(&self) -> LayoutChrome {
        self.preferences
            .layout
            .present(self.viewport_width, self.drawer_open)
    }

    fn close_drawer(&mut self) {
        self.drawer_open = false;
    }

    fn end_resize(&mut self, cx: &mut Context<Self>) {
        let changed = self.resize.take().is_some_and(|resize| {
            self.preferences.layout != resize.origin && !self.resize_restored
        });
        self.resize_restored = false;
        if changed {
            let preferences = self.preferences.clone();
            self.save_preferences(preferences, cx);
        }
    }

    pub(crate) fn resize_handle(
        &self,
        column: LayoutColumn,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let label = self.t(match column {
            LayoutColumn::Sidebar => "nav.resize_sidebar",
            LayoutColumn::Panel => "nav.resize_panel",
        });
        let id = match column {
            LayoutColumn::Sidebar => "sidebar-resize",
            LayoutColumn::Panel => "panel-resize",
        };
        let dragged = column;
        div()
            .id(id)
            .w(px(6.))
            .cursor(CursorStyle::ResizeLeftRight)
            .aria_label(label)
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, event: &MouseDownEvent, _, cx| {
                    if event.click_count >= 2 {
                        this.resize = None;
                        this.resize_restored = true;
                        let restored = this.preferences.layout.restore();
                        if restored != this.preferences.layout {
                            this.preferences.layout = restored;
                            let preferences = this.preferences.clone();
                            this.save_preferences(preferences, cx);
                        }
                        cx.notify();
                        return;
                    }
                    let chrome = this.chrome();
                    this.resize_restored = false;
                    this.resize = Some(ColumnResize {
                        column,
                        origin_x: event.position.x.into(),
                        origin: this.preferences.layout,
                        displayed: match column {
                            LayoutColumn::Sidebar => chrome.sidebar,
                            LayoutColumn::Panel => chrome.panel,
                        },
                        viewport: this.viewport_width,
                    });
                }),
            )
            .on_drag(dragged, |_: &LayoutColumn, _, _, cx| {
                cx.new(|_| ColumnResizeGhost)
            })
            .on_drag_move(
                cx.listener(move |this, event: &DragMoveEvent<LayoutColumn>, _, cx| {
                    if this.resize_restored {
                        return;
                    }
                    let Some(resize) = this.resize else {
                        return;
                    };
                    if resize.column != *event.drag(cx) {
                        return;
                    }
                    let delta = f32::from(event.event.position.x) - resize.origin_x;
                    let next =
                        resize
                            .origin
                            .drag(resize.viewport, resize.column, resize.displayed, delta);
                    if next != this.preferences.layout {
                        this.preferences.layout = next;
                        cx.notify();
                    }
                }),
            )
    }
}

struct ColumnResizeGhost;

impl Render for ColumnResizeGhost {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().size(px(0.))
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
        self.viewport_width = width;
        let chrome = self.chrome();
        if chrome.mode == LayoutMode::Docked {
            self.drawer_open = false;
        }
        self.follow_task_pages(window, cx);
        self.sync_app_surface(chrome.reserved_left as f32, width, height);
        let show_sidebar = chrome.mode == LayoutMode::Docked || chrome.drawer_open;
        let p = self.palettes.get(self.dark);
        div()
            .id("workbench")
            .relative()
            .size_full()
            .flex()
            .bg(p.color("surface"))
            .text_color(p.color("text"))
            .font_family("Noto Sans CJK SC")
            .text_size(px(14.))
            .when(show_sidebar, |el| {
                el.child(self.sidebar(chrome.sidebar as f32, cx))
            })
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .h_full()
                    .flex()
                    .flex_col()
                    .when(chrome.mode == LayoutMode::Drawer, |el| {
                        el.child(
                            div()
                                .id("nav-drawer-bar")
                                .h(px(40.))
                                .px(px(8.))
                                .flex_shrink_0()
                                .flex()
                                .items_center()
                                .border_b_1()
                                .border_color(p.color("border"))
                                .child(
                                    Button::new("nav-drawer")
                                        .ghost()
                                        .small()
                                        .h(px(32.))
                                        .icon(Icon::default().path("icons/layout-list.svg"))
                                        .label(self.t("nav.menu"))
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            this.drawer_open = !this.drawer_open;
                                            cx.notify();
                                        })),
                                ),
                        )
                    })
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
            .when(show_sidebar, |el| {
                el.child(
                    self.resize_handle(LayoutColumn::Sidebar, cx)
                        .absolute()
                        .left(px(chrome.sidebar as f32 - 3.))
                        .top(px(0.))
                        .h_full()
                        .occlude(),
                )
            })
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, _: &MouseUpEvent, _, cx| this.end_resize(cx)),
            )
            .on_mouse_up_out(
                MouseButton::Left,
                cx.listener(|this, _: &MouseUpEvent, _, cx| this.end_resize(cx)),
            )
    }
}
