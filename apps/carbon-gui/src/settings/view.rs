//! The Settings dialog (user decisions 2026-10-22): a search bar across the
//! top with the scope switch (User | Workspace) and the JSON button, the
//! category tree on the left, every setting in one long list on the right.
//! Searching filters both; clicking a category scrolls the list to it, and
//! the tree follows the list as it scrolls.

use std::collections::{HashMap, HashSet};

use carbon_core::settings::Scope;
use carbon_core::settings::search::{self, Query};
use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::scroll::ScrollableElement;
use gpui_kit::component::{
    ActiveTheme, Disableable, Icon, Sizable, Size, WindowExt, h_flex, v_flex,
};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use super::tree::{self, Item, Outline};
use crate::style::fixed;

actions!(
    settings_dialog,
    [TreeUp, TreeDown, TreeExpand, TreeCollapse]
);

const TREE_CONTEXT: &str = "SettingsTree";

/// Arrow keys in the category tree.
pub(super) fn register(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("up", TreeUp, Some(TREE_CONTEXT)),
        KeyBinding::new("down", TreeDown, Some(TREE_CONTEXT)),
        KeyBinding::new("right", TreeExpand, Some(TREE_CONTEXT)),
        KeyBinding::new("left", TreeCollapse, Some(TREE_CONTEXT)),
    ]);
}

/// Opens the Settings dialog (Settings in the app menu, File > Preferences,
/// the palette), with the search box focused.
pub fn open_dialog(window: &mut Window, cx: &mut App) {
    let view = cx.new(|cx| SettingsView::new(window, cx));
    let search = view.read(cx).search.clone();
    window.open_dialog(cx, move |dialog, window, _| {
        let viewport = window.viewport_size();
        let width = (viewport.width * fixed::SETTINGS_WIDTH_SHARE)
            .min(fixed::SETTINGS_MAX_WIDTH)
            .max(fixed::SETTINGS_MIN_WIDTH);
        let height = (viewport.height * fixed::SETTINGS_HEIGHT_SHARE)
            .min(fixed::SETTINGS_MAX_HEIGHT)
            .max(fixed::SETTINGS_MIN_HEIGHT);
        let view = view.clone();
        dialog
            .title("Settings")
            .w(width)
            .pb_0()
            .content(move |content, _, _| {
                content.px_0().child(
                    div()
                        .h(height - fixed::SETTINGS_TITLE_HEIGHT)
                        .child(view.clone()),
                )
            })
    });
    // After the dialog takes focus for itself.
    window.defer(cx, move |window, cx| {
        search.update(cx, |search, cx| search.focus(window, cx))
    });
}

pub struct SettingsView {
    /// The scope being edited (the file values are written to).
    pub(super) scope: Scope,
    pub(super) search: Entity<InputState>,
    tree_focus: FocusHandle,
    query: Query,
    pub(super) outline: Outline,
    list: ListState,
    /// Collapsed categories, by path (ignored while searching).
    collapsed: HashSet<Vec<String>>,
    /// The category clicked or picked with the keys; it stays highlighted
    /// until the list is scrolled by hand.
    pinned: Option<usize>,
    /// Where the list was when a search started, to go back when it ends.
    scroll_before_search: Option<ListOffset>,
    /// The settings generation and registry size the outline was built for.
    built_for: (u64, usize),
    /// Text and number boxes, by scope and key (they hold what is typed).
    pub(super) inputs: HashMap<(Scope, String), Entity<InputState>>,
    pub(super) input_subs: Vec<Subscription>,
    _subs: Vec<Subscription>,
}

impl SettingsView {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let search = cx.new(|cx| {
            InputState::new(window, cx).placeholder("Search settings (@modified, @id:, @ext:)")
        });
        let list = ListState::new(0, ListAlignment::Top, px(200.));
        // A hand scroll lets the tree follow the list again (the list
        // re-renders this view itself).
        let this = cx.weak_entity();
        list.set_scroll_handler(move |_, _, cx| {
            _ = this.update(cx, |this, _| this.pinned = None);
        });
        let _subs = vec![
            cx.subscribe_in(&search, window, |this, _, event: &InputEvent, _, cx| {
                if matches!(event, InputEvent::Change) {
                    this.on_search(cx);
                }
            }),
            cx.observe_global::<super::AppSettings>(|_, cx| cx.notify()),
        ];
        let mut this = Self {
            scope: Scope::User,
            search,
            tree_focus: cx.focus_handle().tab_stop(true),
            query: Query::default(),
            outline: tree::build(&[], Vec::new()),
            list,
            collapsed: HashSet::new(),
            pinned: None,
            scroll_before_search: None,
            built_for: (0, 0),
            inputs: HashMap::new(),
            input_subs: Vec::new(),
            _subs,
        };
        this.rebuild(false, cx);
        this
    }

    /// Re-runs the search and rebuilds the tree and the list. `keep_scroll`
    /// keeps the list where it is (a value changed under `@modified`).
    fn rebuild(&mut self, keep_scroll: bool, cx: &mut Context<Self>) {
        let store = super::get(cx);
        let found = search::search(store, self.scope, &self.query);
        self.outline = tree::build(store.definitions(), found);
        self.built_for = (super::generation(cx), store.definitions().len());
        let top = self.list.logical_scroll_top();
        self.list.reset(self.outline.items.len());
        if keep_scroll {
            self.list.scroll_to(top);
        }
        self.pinned = None;
    }

    fn on_search(&mut self, cx: &mut Context<Self>) {
        let query = Query::parse(&self.search.read(cx).value());
        if query == self.query {
            return;
        }
        if self.query.is_empty() {
            self.scroll_before_search = Some(self.list.logical_scroll_top());
        }
        self.query = query;
        self.rebuild(false, cx);
        if self.query.is_empty()
            && let Some(top) = self.scroll_before_search.take()
        {
            self.list.scroll_to(top);
        }
        cx.notify();
    }

    fn set_scope(&mut self, scope: Scope, cx: &mut Context<Self>) {
        if self.scope != scope {
            self.scope = scope;
            self.rebuild(true, cx);
            cx.notify();
        }
    }

    fn clear_search(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.search.update(cx, |search, cx| {
            search.set_value("", window, cx);
            search.focus(window, cx);
        });
        self.on_search(cx);
    }

    fn searching(&self) -> bool {
        !self.query.is_empty()
    }

    /// Categories shown in the tree, in order: those not under a collapsed one.
    fn visible_nodes(&self) -> Vec<usize> {
        let searching = self.searching();
        (0..self.outline.nodes.len())
            .filter(|&ix| {
                searching || {
                    let mut parent = self.outline.nodes[ix].parent;
                    let mut shown = true;
                    while let Some(p) = parent {
                        if self.collapsed.contains(&self.outline.nodes[p].path) {
                            shown = false;
                            break;
                        }
                        parent = self.outline.nodes[p].parent;
                    }
                    shown
                }
            })
            .collect()
    }

    /// The category the list shows at its top (or the one picked last).
    fn current_node(&self) -> Option<usize> {
        if let Some(node) = self.pinned {
            return Some(node);
        }
        let top = self.list.logical_scroll_top().item_ix;
        self.outline
            .item_node
            .get(top)
            .or(self.outline.item_node.last())
            .copied()
    }

    /// The tree row to highlight: the current category, or its closest
    /// shown ancestor when it is folded away.
    fn highlighted(&self, visible: &[usize]) -> Option<usize> {
        let mut node = self.current_node()?;
        loop {
            if visible.contains(&node) {
                return Some(node);
            }
            node = self.outline.nodes[node].parent?;
        }
    }

    fn is_collapsed(&self, node: usize) -> bool {
        !self.searching() && self.collapsed.contains(&self.outline.nodes[node].path)
    }

    /// Scrolls the list to `node` (and unfolds it).
    fn select_node(&mut self, node: usize, cx: &mut Context<Self>) {
        self.collapsed.remove(&self.outline.nodes[node].path);
        self.pinned = Some(node);
        self.list.scroll_to(ListOffset {
            item_ix: self.outline.nodes[node].first_item,
            offset_in_item: px(0.),
        });
        cx.notify();
    }

    fn toggle_node(&mut self, node: usize, cx: &mut Context<Self>) {
        let path = self.outline.nodes[node].path.clone();
        if !self.collapsed.remove(&path) {
            self.collapsed.insert(path);
        }
        cx.notify();
    }

    fn tree_step(&mut self, down: bool, window: &mut Window, cx: &mut Context<Self>) {
        let visible = self.visible_nodes();
        let at = self
            .highlighted(&visible)
            .and_then(|node| visible.iter().position(|&ix| ix == node));
        let next = match (at, down) {
            (None, _) => visible.first().copied(),
            (Some(at), true) => visible.get(at + 1).copied(),
            (Some(0), false) => {
                // Up from the first category: back to the search box.
                self.search
                    .update(cx, |search, cx| search.focus(window, cx));
                None
            }
            (Some(at), false) => visible.get(at - 1).copied(),
        };
        if let Some(node) = next {
            self.select_node(node, cx);
        }
    }

    fn tree_expand(&mut self, _: &TreeExpand, _: &mut Window, cx: &mut Context<Self>) {
        let visible = self.visible_nodes();
        let Some(node) = self.highlighted(&visible) else {
            return;
        };
        if self.is_collapsed(node) {
            self.toggle_node(node, cx);
        } else if self.outline.nodes[node].has_children {
            self.select_node(node + 1, cx);
        }
    }

    fn tree_collapse(&mut self, _: &TreeCollapse, _: &mut Window, cx: &mut Context<Self>) {
        let visible = self.visible_nodes();
        let Some(node) = self.highlighted(&visible) else {
            return;
        };
        if self.outline.nodes[node].has_children && !self.is_collapsed(node) && !self.searching() {
            self.toggle_node(node, cx);
        } else if let Some(parent) = self.outline.nodes[node].parent {
            self.select_node(parent, cx);
        }
    }

    fn render_top_bar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let has_workspace = super::get(cx).has_workspace();
        let found = self.searching().then(|| self.outline.row_count());
        let scope_button = |scope: Scope, cx: &mut Context<Self>| {
            let disabled = scope == Scope::Workspace && !has_workspace;
            Button::new(scope.label())
                .label(scope.label())
                .small()
                .map(|button| {
                    if self.scope == scope {
                        button.primary()
                    } else {
                        button.ghost()
                    }
                })
                .disabled(disabled)
                .when(disabled, |button| {
                    button.tooltip("Open a folder to change settings for it")
                })
                .when(scope == Scope::Workspace && has_workspace, |button| {
                    button.tooltip("Settings for the open folder (.carbon/settings.json)")
                })
                .on_click(cx.listener(move |this, _, _, cx| this.set_scope(scope, cx)))
        };
        h_flex()
            .flex_none()
            .gap(fixed::PANEL_GAP)
            .px(fixed::SETTINGS_BAR_PADDING)
            .pb(fixed::SETTINGS_BAR_PADDING)
            .border_b_1()
            .border_color(cx.theme().border)
            .child(
                div().flex_1().min_w_0().child(
                    Input::new(&self.search)
                        .prefix(
                            Icon::new(IconName::Search)
                                .with_size(Size::Small)
                                .text_color(cx.theme().muted_foreground),
                        )
                        .when_some(found, |input, found| {
                            input.suffix(
                                div()
                                    .text_xs()
                                    .text_color(cx.theme().muted_foreground)
                                    .child(match found {
                                        1 => "1 setting".to_string(),
                                        n => format!("{n} settings"),
                                    }),
                            )
                        })
                        .cleanable(true),
                ),
            )
            .child(
                h_flex()
                    .gap_1()
                    .child(scope_button(Scope::User, cx))
                    .child(scope_button(Scope::Workspace, cx)),
            )
            .child(
                Button::new("open-json")
                    .icon(IconName::Braces)
                    .ghost()
                    .small()
                    .tooltip(match self.scope {
                        Scope::User => "Open User Settings (JSON)",
                        Scope::Workspace => "Open Workspace Settings (JSON)",
                    })
                    .on_click(cx.listener(|this, _, window, cx| {
                        let scope = this.scope;
                        window.close_dialog(cx);
                        super::open_file(scope, window, cx);
                    })),
            )
    }

    fn render_tree(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let visible = self.visible_nodes();
        let highlighted = self.highlighted(&visible);
        let searching = self.searching();
        let theme = cx.theme().clone();
        let rows = visible.iter().map(|&ix| {
            let node = &self.outline.nodes[ix];
            let active = highlighted == Some(ix);
            let collapsed = self.is_collapsed(ix);
            let chevron = node.has_children.then(|| {
                div()
                    .id(("settings-chevron", ix))
                    .flex_none()
                    .child(
                        Icon::new(if collapsed {
                            IconName::ChevronRight
                        } else {
                            IconName::ChevronDown
                        })
                        .with_size(Size::Size(fixed::SETTINGS_CHEVRON_SIZE))
                        .text_color(theme.muted_foreground),
                    )
                    .on_click(cx.listener(move |this, _, _, cx| {
                        cx.stop_propagation();
                        this.toggle_node(ix, cx);
                    }))
            });
            h_flex()
                .id(("settings-node", ix))
                .h(fixed::TREE_ROW_HEIGHT)
                .flex_none()
                .gap(fixed::TREE_ITEM_GAP)
                .pl(fixed::TREE_PADDING + fixed::TREE_INDENT * node.depth as f32)
                .pr(fixed::TREE_PADDING)
                .rounded(theme.radius)
                .text_sm()
                .cursor_pointer()
                .when(node.depth == 0, |row| row.font_weight(FontWeight::MEDIUM))
                .map(|row| {
                    if active {
                        row.bg(theme.list_active).text_color(theme.foreground)
                    } else {
                        row.text_color(theme.muted_foreground)
                            .hover(|row| row.bg(theme.list_hover))
                    }
                })
                .child(match chevron {
                    Some(chevron) => chevron.into_any_element(),
                    None => div()
                        .flex_none()
                        .w(fixed::SETTINGS_CHEVRON_SIZE)
                        .into_any_element(),
                })
                .child(div().flex_1().min_w_0().truncate().child(node.name.clone()))
                .when(searching, |row| {
                    row.child(
                        div()
                            .flex_none()
                            .text_xs()
                            .text_color(theme.muted_foreground)
                            .child(node.count.to_string()),
                    )
                })
                .on_click(cx.listener(move |this, _, window, cx| {
                    this.select_node(ix, cx);
                    window.focus(&this.tree_focus, cx);
                }))
        });
        v_flex()
            .id("settings-tree")
            .track_focus(&self.tree_focus)
            .key_context(TREE_CONTEXT)
            .on_action(
                cx.listener(|this, _: &TreeUp, window, cx| this.tree_step(false, window, cx)),
            )
            .on_action(
                cx.listener(|this, _: &TreeDown, window, cx| this.tree_step(true, window, cx)),
            )
            .on_action(cx.listener(Self::tree_expand))
            .on_action(cx.listener(Self::tree_collapse))
            .w(fixed::SETTINGS_TREE_WIDTH)
            .flex_none()
            .h_full()
            .p(fixed::TREE_PADDING)
            .border_r_1()
            .border_color(theme.border)
            .overflow_y_scroll()
            .children(rows)
    }

    /// One list entry: a category title or a setting row.
    fn render_item(
        &mut self,
        ix: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        match &self.outline.items[ix] {
            Item::Header(node) => {
                let path = self.outline.nodes[*node].path.join(" › ");
                div()
                    .px(fixed::SETTINGS_ROW_PADDING_X)
                    .pt(fixed::SETTINGS_ROW_GAP)
                    .pb(fixed::SETTINGS_ROW_PADDING_Y)
                    .text_lg()
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(path)
                    .into_any_element()
            }
            Item::Row(setting, hit) => {
                let (setting, hit) = (*setting, hit.clone());
                self.render_row(setting, &hit, window, cx)
            }
        }
    }

    /// The scope's file could not be read: say so above the list.
    fn render_file_error(&self, cx: &mut Context<Self>) -> Option<impl IntoElement> {
        let error = super::get(cx).file_error(self.scope)?.to_string();
        let theme = cx.theme();
        Some(
            h_flex()
                .flex_none()
                .gap(fixed::PANEL_GAP)
                .mx(fixed::SETTINGS_ROW_PADDING_X)
                .mt(fixed::SETTINGS_BAR_PADDING)
                .p(fixed::SETTINGS_BAR_PADDING)
                .rounded(theme.radius)
                .border_1()
                .border_color(theme.danger)
                .text_sm()
                .child(Icon::new(IconName::TriangleAlert).text_color(theme.danger))
                .child(div().flex_1().min_w_0().child(format!(
                    "{} settings file has errors, so its values are not used: {error}",
                    self.scope.label()
                )))
                .child(
                    Button::new("fix-json")
                        .label("Open File")
                        .small()
                        .outline()
                        .on_click(cx.listener(|this, _, window, cx| {
                            let scope = this.scope;
                            window.close_dialog(cx);
                            super::open_file(scope, window, cx);
                        })),
                ),
        )
    }

    fn render_empty(&self, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .flex_1()
            .items_center()
            .justify_center()
            .gap(fixed::PANEL_GAP)
            .text_color(cx.theme().muted_foreground)
            .child("No settings match")
            .when(self.searching(), |this| {
                this.child(
                    Button::new("clear-search")
                        .label("Clear Search")
                        .small()
                        .outline()
                        .on_click(cx.listener(|this, _, window, cx| this.clear_search(window, cx))),
                )
            })
    }
}

impl Render for SettingsView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // `@modified` depends on values, and extensions may add settings.
        let current = (super::generation(cx), super::get(cx).definitions().len());
        if current != self.built_for && (self.query.modified || current.1 != self.built_for.1) {
            self.rebuild(true, cx);
        }
        let body = if self.outline.items.is_empty() {
            self.render_empty(cx).into_any_element()
        } else {
            div()
                .relative()
                .flex_1()
                .min_h_0()
                .child(
                    list(self.list.clone(), cx.processor(Self::render_item))
                        .size_full()
                        .pb(fixed::SETTINGS_ROW_GAP),
                )
                .vertical_scrollbar(&self.list)
                .into_any_element()
        };
        v_flex().size_full().child(self.render_top_bar(cx)).child(
            h_flex()
                .flex_1()
                .min_h_0()
                .items_start()
                .child(self.render_tree(cx))
                .child(
                    v_flex()
                        .flex_1()
                        .min_w_0()
                        .h_full()
                        .children(self.render_file_error(cx))
                        .child(body),
                ),
        )
    }
}
