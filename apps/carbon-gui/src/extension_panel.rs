//! Extension panels: a panel an extension declares (`[[panels]]`), saved in
//! layouts by its key `<extension id>/<panel id>`. A `view` panel draws the
//! manifest's declarative view, or (`dynamic`) the widgets the extension's
//! code returns, redrawn after each call into its code. Buttons and list
//! rows run the extension's `[[commands]]`. When the extension is turned off
//! or removed the tab stays and says so. Webview pages are not in this build.

use carbon_core::extension::{Align, ButtonStyle, ExtensionId, ListItem, PanelKind, Widget};
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::dock::{
    BasePanel, Panel, PanelEvent, PanelInfo, PanelState, panel_handle, register_panel,
};
use gpui_kit::component::separator::Separator;
use gpui_kit::component::text::TextView;
use gpui_kit::component::{ActiveTheme, Icon, Sizable, Size, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::app_assets::AppAssets;
use crate::menus::RunExtensionCommand;
use crate::style::fixed;

/// Persistence key in saved layouts. Never change it.
pub const PANEL_NAME: &str = "Extension";
/// `AddPanel` names of extension panels start with this, then the key.
pub const ADD_PREFIX: &str = "extension:";

/// Registers the builder that recreates extension panels from saved layouts.
pub fn register(cx: &mut App) {
    register_panel(cx, PANEL_NAME, |build, _, cx| {
        let text = |key: &str| match build.info() {
            PanelInfo::Panel(value) => value[key].as_str().map(str::to_string),
            _ => None,
        };
        let key = text("key").unwrap_or_default();
        let title = text("title").unwrap_or_else(|| "Extension".into());
        panel_handle(ExtensionPanel::new(key, title, cx))
    });
}

pub struct ExtensionPanel {
    key: String,
    /// The title when it was made (shown while its extension is missing).
    title: SharedString,
    focus_handle: FocusHandle,
    /// Dynamic views: the last widgets the code returned, and for which run.
    rendered: Option<Result<Vec<Widget>, String>>,
    rendered_at: Option<u64>,
    _render: Task<()>,
    _subs: [Subscription; 2],
}

impl ExtensionPanel {
    pub fn new(key: String, title: impl Into<SharedString>, cx: &mut App) -> Entity<Self> {
        let title = title.into();
        cx.new(|cx| Self {
            key,
            title,
            focus_handle: cx.focus_handle(),
            rendered: None,
            rendered_at: None,
            _render: Task::ready(()),
            _subs: [
                cx.observe_global::<crate::extensions::Extensions>(|this: &mut Self, cx| {
                    this.rendered_at = None;
                    cx.notify()
                }),
                cx.observe_global::<crate::extensions::ExtensionCode>(|_, cx| cx.notify()),
            ],
        })
    }

    /// The key `<extension id>/<panel id>`.
    pub fn key(&self) -> &str {
        &self.key
    }

    pub fn title(&self) -> SharedString {
        self.title.clone()
    }

    fn extension(&self) -> ExtensionId {
        ExtensionId::new(self.key.split('/').next().unwrap_or_default())
    }

    /// Asks the code for the view again when it ran since the last time.
    fn refresh_dynamic(&mut self, view: &str, cx: &mut Context<Self>) {
        let runs = crate::extensions::code_runs(cx);
        if self.rendered_at == Some(runs) {
            return;
        }
        self.rendered_at = Some(runs);
        let task = crate::extensions::render_view(self.extension(), view.to_string(), cx);
        self._render = cx.spawn(async move |this, cx| {
            let result = task.await;
            _ = this.update(cx, |this, cx| {
                this.rendered = Some(result);
                // The render call itself moved the counter.
                this.rendered_at = Some(crate::extensions::code_runs(cx));
                cx.notify();
            });
        });
    }
}

/// An icon from Carbon's icon set by its file name (`code`, `bot`).
pub(crate) fn named_icon(name: Option<&str>) -> Option<Icon> {
    let path = format!("icons/{}.svg", name?);
    matches!(AssetSource::load(&AppAssets, &path), Ok(Some(_))).then(|| Icon::empty().path(path))
}

/// Runs command `command` of the extension.
fn run(extension: &ExtensionId, command: &str, window: &mut Window, cx: &mut App) {
    window.dispatch_action(
        Box::new(RunExtensionCommand {
            key: format!("{extension}/{command}").into(),
        }),
        cx,
    );
}

/// Builds the elements of `widgets`; `path` makes element ids unique.
fn widgets(widgets: &[Widget], path: &str, extension: &ExtensionId, cx: &App) -> Vec<AnyElement> {
    widgets
        .iter()
        .enumerate()
        .map(|(ix, widget)| element(widget, &format!("{path}.{ix}"), extension, cx))
        .collect()
}

fn element(widget: &Widget, path: &str, extension: &ExtensionId, cx: &App) -> AnyElement {
    let theme = cx.theme();
    let id = SharedString::from(format!("ext{path}"));
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
            let (extension, command) = (extension.clone(), command.clone());
            // Its own width, also when a column would stretch it.
            h_flex()
                .child(
                    Button::new(id)
                        .label(label.clone())
                        .when_some(named_icon(icon.as_deref()), |button, icon| {
                            button.icon(icon)
                        })
                        .map(|button| match style {
                            ButtonStyle::Primary => button.primary(),
                            ButtonStyle::Secondary => button.outline(),
                            ButtonStyle::Ghost => button.ghost(),
                        })
                        .on_click(move |_, window, cx| run(&extension, &command, window, cx)),
                )
                .into_any_element()
        }
        Widget::Row { children } => h_flex()
            .flex_wrap()
            .items_center()
            .gap(fixed::PANEL_GAP)
            .children(widgets(children, path, extension, cx))
            .into_any_element(),
        Widget::Column { children, align } => v_flex()
            .gap(fixed::PANEL_GAP)
            .when(*align == Align::Center, |column| {
                column.w_full().items_center().text_center()
            })
            .children(widgets(children, path, extension, cx))
            .into_any_element(),
        Widget::Divider => Separator::horizontal().into_any_element(),
        Widget::List { items } => {
            // Rows without an icon keep its space when others have one, so labels line up.
            let icons = items.iter().any(|item| item.icon.is_some());
            v_flex()
                .children(items.iter().enumerate().map(|(ix, item)| {
                    list_row(item, icons, &format!("{path}.{ix}"), extension, cx)
                }))
                .into_any_element()
        }
    }
}

fn list_row(
    item: &ListItem,
    icon_space: bool,
    path: &str,
    extension: &ExtensionId,
    cx: &App,
) -> AnyElement {
    let theme = cx.theme();
    let (muted, hover) = (theme.muted_foreground, theme.list_hover);
    h_flex()
        .id(SharedString::from(format!("ext{path}")))
        .gap(fixed::PANEL_GAP)
        .px(fixed::TREE_PADDING)
        .py(fixed::TREE_ITEM_GAP)
        .rounded(theme.radius)
        .map(|row| match named_icon(item.icon.as_deref()) {
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
            let extension = extension.clone();
            row.cursor_pointer()
                .hover(move |style| style.bg(hover))
                .on_click(move |_, window, cx| run(&extension, &command, window, cx))
        })
        .into_any_element()
}

/// A muted line of text filling the panel's body.
fn message(text: impl Into<SharedString>, cx: &App) -> Vec<AnyElement> {
    vec![
        div()
            .text_sm()
            .text_color(cx.theme().muted_foreground)
            .child(text.into())
            .into_any_element(),
    ]
}

impl EventEmitter<PanelEvent> for ExtensionPanel {}

impl Focusable for ExtensionPanel {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl BasePanel for ExtensionPanel {
    fn panel_name(&self) -> &'static str {
        PANEL_NAME
    }

    fn dump(&self, _: &App) -> PanelState {
        let mut state = PanelState::new(PANEL_NAME);
        state.info = PanelInfo::Panel(serde_json::json!({
            "key": self.key,
            "title": self.title.as_ref(),
        }));
        state
    }

    /// Tabs in the Explorer section stay, like the Explorer.
    fn closable(&self, cx: &App) -> bool {
        !crate::extensions::registry(cx)
            .panel(&self.key)
            .is_some_and(|(_, panel)| panel.explorer_section)
    }
}

impl Panel for ExtensionPanel {
    fn tab_name(&self, cx: &App) -> Option<SharedString> {
        let registry = crate::extensions::registry(cx);
        Some(
            registry
                .panel(&self.key)
                .map(|(_, panel)| panel.title.clone().into())
                .unwrap_or_else(|| self.title.clone()),
        )
    }

    fn title(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.tab_name(cx).unwrap_or_default()
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

impl Render for ExtensionPanel {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let registry = crate::extensions::registry(cx);
        let extension = self.extension();
        let found = registry
            .panel(&self.key)
            .map(|(loaded, panel)| (loaded.clone(), panel.clone()));
        let mut centered = false;
        let body = match found {
            None => message(
                format!(
                    "This panel's extension ({extension}) is not installed. Install it in Manage Extensions, or close the tab."
                ),
                cx,
            ),
            Some((loaded, _)) if !registry.is_active(&extension) => message(
                match registry.blocked(&extension) {
                    Some(carbon_core::extension::Blocked::Disabled) | None => format!(
                        "{} is turned off. Turn it on in Manage Extensions.",
                        loaded.def.name
                    ),
                    Some(why) => format!("{} is not in use: {why}", loaded.def.name),
                },
                cx,
            ),
            Some((_, panel)) if panel.kind == PanelKind::Webview => {
                message("Web page panels are not in this build of Carbon.", cx)
            }
            Some((loaded, panel)) => {
                let name = panel.view.clone().unwrap_or_default();
                let fixed_view = loaded.def.views.get(&name).cloned();
                let view = if panel.dynamic {
                    self.refresh_dynamic(&name, cx);
                    match &self.rendered {
                        Some(Ok(widgets)) => Ok(widgets.clone()),
                        Some(Err(err)) => Err(err.clone()),
                        None => Ok(fixed_view.unwrap_or_default()),
                    }
                } else {
                    Ok(fixed_view.unwrap_or_default())
                };
                match view {
                    Ok(view) => {
                        // A view that is one centered column is an empty state: center it vertically too.
                        centered = matches!(
                            view.as_slice(),
                            [Widget::Column {
                                align: Align::Center,
                                ..
                            }]
                        );
                        widgets(&view, "", &extension, cx)
                    }
                    Err(err) => message(format!("The view could not be drawn: {err}"), cx),
                }
            }
        };
        v_flex()
            .id("extension-panel")
            .track_focus(&self.focus_handle)
            .size_full()
            .overflow_y_scroll()
            .p(fixed::PANEL_PADDING)
            .gap(fixed::PANEL_GAP)
            .when(centered, |panel| panel.justify_center())
            .children(body)
    }
}
