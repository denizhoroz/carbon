//! View panel: draws one of a workflow's declarative views (tier 1,
//! workflow-system.md Q17-Q18; core `workflow::Widget`) with Carbon's own
//! widgets. No code runs: buttons and list rows run the workflow's
//! `[[commands]]`. The view is read from the loaded workflows at each
//! render, so Manage Workflows > Reload shows a changed manifest at once.

use carbon_core::workflow::{Align, ButtonStyle, ListItem, Widget, WorkflowId};
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::dock::{
    BasePanel, Panel, PanelEvent, PanelInfo, PanelState, panel_handle, register_panel,
};
use gpui_kit::component::separator::Separator;
use gpui_kit::component::text::TextView;
use gpui_kit::component::{ActiveTheme, Sizable, Size, WindowExt, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::style::fixed;
use crate::workflows;

/// Persistence key in saved layouts. Never change it.
const PANEL_NAME: &str = "View";

/// Registers the builder that recreates view panels when a saved layout loads.
pub fn register(cx: &mut App) {
    register_panel(cx, PANEL_NAME, |build, _, cx| {
        let text = |key: &str| match build.info() {
            PanelInfo::Panel(value) => value[key].as_str().map(str::to_string),
            _ => None,
        };
        let title = text("title").unwrap_or_else(|| "View".into());
        let workflow = WorkflowId::new(text("workflow").unwrap_or_default());
        let view = text("view").unwrap_or_default();
        panel_handle(ViewPanel::new(title, workflow, view, cx))
    });
}

pub struct ViewPanel {
    title: SharedString,
    workflow: WorkflowId,
    view: String,
    focus_handle: FocusHandle,
}

impl ViewPanel {
    pub fn new(
        title: impl Into<SharedString>,
        workflow: WorkflowId,
        view: String,
        cx: &mut App,
    ) -> Entity<Self> {
        let title = title.into();
        cx.new(|cx| Self {
            title,
            workflow,
            view,
            focus_handle: cx.focus_handle(),
        })
    }
}

/// Runs workflow `workflow`'s command `command`, or says it cannot.
fn run(workflow: &WorkflowId, command: &str, window: &mut Window, cx: &mut App) {
    match workflows::command_action(workflow, command, cx) {
        Some(action) => window.dispatch_action(action, cx),
        None => window.push_notification(format!("Command `{command}` is not available"), cx),
    }
}

/// Builds the elements of `widgets`; `path` makes element ids unique.
fn widgets(widgets: &[Widget], path: &str, workflow: &WorkflowId, cx: &App) -> Vec<AnyElement> {
    widgets
        .iter()
        .enumerate()
        .map(|(ix, widget)| element(widget, &format!("{path}.{ix}"), workflow, cx))
        .collect()
}

fn element(widget: &Widget, path: &str, workflow: &WorkflowId, cx: &App) -> AnyElement {
    let theme = cx.theme();
    let id = SharedString::from(format!("view{path}"));
    match widget {
        Widget::Heading { text } => div()
            .text_lg()
            .font_weight(FontWeight::SEMIBOLD)
            .child(text.clone())
            .into_any_element(),
        Widget::Text { text, muted } => div()
            .text_sm()
            .when(*muted, |el| el.text_color(theme.muted_foreground))
            .child(text.clone())
            .into_any_element(),
        Widget::Markdown { text } => TextView::markdown(id, text.clone())
            .selectable(true)
            .into_any_element(),
        Widget::Button {
            label,
            command,
            icon,
            style,
        } => {
            let (workflow, command) = (workflow.clone(), command.clone());
            // Its own width, also when a column would stretch it.
            h_flex()
                .child(
                    Button::new(id)
                        .label(label.clone())
                        .when_some(workflows::named_icon(icon.as_deref()), |button, icon| {
                            button.icon(icon)
                        })
                        .map(|button| match style {
                            ButtonStyle::Primary => button.primary(),
                            ButtonStyle::Secondary => button.outline(),
                            ButtonStyle::Ghost => button.ghost(),
                        })
                        .on_click(move |_, window, cx| run(&workflow, &command, window, cx)),
                )
                .into_any_element()
        }
        Widget::Row { children } => h_flex()
            .flex_wrap()
            .items_center()
            .gap(fixed::PANEL_GAP)
            .children(widgets(children, path, workflow, cx))
            .into_any_element(),
        Widget::Column { children, align } => v_flex()
            .gap(fixed::PANEL_GAP)
            .when(*align == Align::Center, |column| {
                column.w_full().items_center().text_center()
            })
            .children(widgets(children, path, workflow, cx))
            .into_any_element(),
        Widget::Divider => Separator::horizontal().into_any_element(),
        Widget::List { items } => {
            // Rows without an icon keep its space when others have one, so labels line up.
            let icons = items.iter().any(|item| item.icon.is_some());
            v_flex()
                .children(
                    items.iter().enumerate().map(|(ix, item)| {
                        list_row(item, icons, &format!("{path}.{ix}"), workflow, cx)
                    }),
                )
                .into_any_element()
        }
    }
}

fn list_row(
    item: &ListItem,
    icon_space: bool,
    path: &str,
    workflow: &WorkflowId,
    cx: &App,
) -> AnyElement {
    let theme = cx.theme();
    let (muted, hover) = (theme.muted_foreground, theme.list_hover);
    h_flex()
        .id(SharedString::from(format!("view{path}")))
        .gap(fixed::PANEL_GAP)
        .px(fixed::TREE_PADDING)
        .py(fixed::TREE_ITEM_GAP)
        .rounded(theme.radius)
        .map(|row| match workflows::named_icon(item.icon.as_deref()) {
            Some(icon) => row.child(
                icon.with_size(Size::Size(fixed::TREE_ICON_SIZE))
                    .text_color(muted),
            ),
            None if icon_space => row.child(div().flex_none().size(fixed::TREE_ICON_SIZE)),
            None => row,
        })
        .child(
            v_flex()
                .min_w_0()
                .child(div().text_sm().child(item.label.clone()))
                .when_some(item.description.clone(), |column, text| {
                    column.child(div().text_xs().text_color(muted).child(text))
                }),
        )
        .when_some(item.command.clone(), |row, command| {
            let workflow = workflow.clone();
            row.cursor_pointer()
                .hover(move |style| style.bg(hover))
                .on_click(move |_, window, cx| run(&workflow, &command, window, cx))
        })
        .into_any_element()
}

impl EventEmitter<PanelEvent> for ViewPanel {}

impl Focusable for ViewPanel {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl BasePanel for ViewPanel {
    fn panel_name(&self) -> &'static str {
        PANEL_NAME
    }

    fn dump(&self, _: &App) -> PanelState {
        let mut state = PanelState::new(PANEL_NAME);
        state.info = PanelInfo::Panel(serde_json::json!({
            "title": self.title.as_ref(),
            "workflow": self.workflow.as_str(),
            "view": self.view,
        }));
        state
    }
}

impl Panel for ViewPanel {
    fn tab_name(&self, _: &App) -> Option<SharedString> {
        Some(self.title.clone())
    }

    fn title(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        self.title.clone()
    }

    fn always_tab_bar(&self, _: &App) -> bool {
        true
    }

    fn inner_padding(&self, _: &App) -> bool {
        false
    }

    fn request_close(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        crate::workbench::close_panel(cx.entity(), window, cx);
        true
    }
}

impl Render for ViewPanel {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let registry = workflows::registry();
        let view = registry
            .get(&self.workflow)
            .and_then(|def| def.views.get(&self.view));
        // A view that is one centered column is an empty state: center it vertically too.
        let centered = matches!(
            view.map(Vec::as_slice),
            Some([Widget::Column {
                align: Align::Center,
                ..
            }])
        );
        let body = match view {
            Some(view) => widgets(view, "", &self.workflow, cx),
            None => vec![
                div()
                    .text_sm()
                    .text_color(cx.theme().muted_foreground)
                    .child(format!(
                        "View `{}` of workflow `{}` is not available.",
                        self.view, self.workflow
                    ))
                    .into_any_element(),
            ],
        };
        v_flex()
            .id("view-panel")
            .track_focus(&self.focus_handle)
            .size_full()
            .overflow_y_scroll()
            .p(fixed::PANEL_PADDING)
            .gap(fixed::PANEL_GAP)
            .when(centered, |panel| panel.justify_center())
            .children(body)
    }
}
