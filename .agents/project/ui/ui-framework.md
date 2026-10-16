# GPUI Kit (gpui + gpui-component) for Carbon

Read 2026-10-16 from gpui-kit.com (docs v0.7.3, via the site's `llms-full.txt` dump of all pages), the GitHub README, workspace `Cargo.toml`, `crates/kit/Cargo.toml` and the GitHub releases page. Version read: **gpui-kit 0.7.3** (latest; 0.7.2 yanked). Code snippets are copied from those pages and valid for 0.7.3 only; this API breaks often (see section 10). Items marked (?) are not verified in what was read.

Naming: the project formerly called gpui-component (Longbridge) became **GPUI Kit** at v0.6.0. Repo is now `github.com/longbridge/gpui-kit` (the old `gpui-component` URL still resolves). `gpui-component` is still a crate (the styled layer).


# 1. Overview and versions

| Layer / crate | Role |
|---|---|
| `gpui` (published as `gpui-pre`) | Zed's GPUI runtime, a snapshot of a recorded Zed commit |
| `gpui-base` | Unstyled behaviour, state, dock engine, text input engine, tokens |
| `gpui-component` | Styled component library (what DESIGN.md calls "the kit") |
| `gpui-kit` | The one crate apps depend on; re-exports the layers |
| `gpui-shell` | Separate; JavaScript extension host (QuickJS), capabilities granted explicitly |

| Item | Value |
|---|---|
| Kit version | 0.7.3 (12 Oct 2026); 0.7.1 05 Oct, 0.7.0 28 Sep, 0.6.x Sep, 0.6.0 03 Sep |
| GPUI pin | `gpui-pre` `=0.3.9` (zed commit 93d7456); 0.7.1 was `0.3.8`. Exact `=` pins are required: a caret once let apps drift onto a snapshot the kit did not compile with |
| GPUI crates | `gpui-pre`, `gpui-pre-platform` (features `font-kit`, `x11`, `wayland`, `runtime_shaders`), `gpui-pre-web`, `gpui-pre-macros`, `gpui-pre-reqwest-client`, `gpui-pre-sum-tree` |
| Optional backend | `gpui-fast` 0.1.x (Longbridge fork: retained rendering/layout engine); feature `gpui-fast` switches everything at once; do not mix with direct upstream gpui |
| Licence | Apache-2.0 code; docs prose CC BY 4.0 (credit + link if copied); GPUI is Apache-2.0 (Zed); Lucide icons |
| Rust | 1.92 or later (driven by `oo7 0.6.0` on Linux); no `rust-version` in manifests; workspace edition 2024 |
| Stars | 17.7k (site) |
| Docs | 75+ components, 38 bundled theme presets, ~12 MB minimal hello-world binary (vendor figure, Hello World release build, ?) |

## Depending on it

Crates.io, no git revision needed (the pin is inside `gpui-kit`):

```toml
[dependencies]
gpui-kit = "0.7.3"
# optional: gpui-kit = { version = "0.7.3", features = ["gpui-fast"] }
```

Kit features (`crates/kit/Cargo.toml`): `default = component, assets`; `gpui-fast`; `test-support`; `profiler`; `inspector`; `decimal`; `speech` (needs ALSA on Linux); `tree-sitter`; `syntect`; `tree-sitter-languages` (all grammars); `tree-sitter-<lang>` for astro, bash, c, cmake, cpp, csharp, css, diff, ejs, elixir, erb, go, graphql, html, java, javascript, jsdoc, kotlin, lua, make, markdown, markdown-inline, php, proto, python, ruby, rust, scala, sql, svelte, swift, toml, tsx, typescript, yaml, zig.

Do not override a single `gpui-pre-*` package; update the `gpui-kit` requirement and let Cargo align them. Commit `Cargo.lock` and build releases with `--locked`.

## Platform build requirements (Installation page)

| OS | Needs |
|---|---|
| Windows | Windows 10+, VS 2022 Build Tools/Community "Desktop development with C++" (MSVC + Windows SDK), MSVC Rust toolchain (not GNU), **CMake on PATH** |
| macOS | macOS 15+, Xcode Command Line Tools (`xcode-select --install`); `font-kit` on `gpui-pre-platform` or text does not render (kit enables it) |
| Linux | Verified on Ubuntu 24.04: `gcc g++ clang libfontconfig-dev libwayland-dev libwebkit2gtk-4.1-dev libxkbcommon-x11-dev libx11-xcb-dev libssl-dev libzstd-dev libasound2-dev vulkan-validationlayers libvulkan1`; runtime needs a Wayland/X11 session **and a working Vulkan driver** |

Dev runtime speed (does not speed compilation), root `Cargo.toml`:

```toml
[profile.dev.package]
gpui-pre = { opt-level = 3 }
gpui-component = { opt-level = 3 }
gpui-kit = { opt-level = 3 }
gpui-kit-assets = { opt-level = 3 }
gpui-pre-macros = { opt-level = 3 }
gpui-pre-platform = { opt-level = 3 }
rustybuzz = { opt-level = 3 }
taffy = { opt-level = 3 }
ttf-parser = { opt-level = 3 }
```

Kit's own workspace also sets `[profile.release] strip = "symbols"` and opt-level 3 for `smol`, `ropey`, `cap-std` in dev.

Carbon note: `gui-framework.md` lists `gpui` 0.2.2 on crates.io as stale; the kit now publishes its own snapshot as `gpui-pre`, so crates.io no longer needs a git rev. Zed crate licences beyond `gpui` remain unchecked (?).


# 2. Minimal app skeleton (0.7.3)

```toml
[dependencies]
gpui-kit = "0.7.3"
```

```rust
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::*;

struct HelloWorld;

impl Render for HelloWorld {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().flex().flex_col().size_full().items_center().justify_center().gap_2()
            .child("Hello, World!")
            .child(Button::new("hello").primary().label("Click me")
                .on_click(|_, _, _| println!("Clicked!")))
    }
}

fn main() {
    application()
        .with_assets(assets::Assets)
        .run(|cx| {
            init(cx);
            open_window(WindowOptions::default(), cx, |_, cx| cx.new(|_| HelloWorld))
                .expect("Failed to open window");
        });
}
```

- `application()` creates the app; `.with_assets(assets::Assets)` registers the bundled Lucide icons.
- `init(cx)` initialises all enabled layers, themes included. Call once, before windows/components and before app `cx.bind_keys`.
- `open_window(...)` always wraps your view in a Base `Root`; return your content view, not a `Root`. It returns `(AnyWindowHandle, Entity<V>)`.
- `Root` owns dialogs, sheets, notifications, tooltips, menus, focus traversal, text selection. `init` installs no quit/close-window actions or bindings: the app must define them (and unsaved-change prompts).
- Run the gallery: `cargo run` in the kit repo (story app); examples: `cargo run -p example-dock | example-editor | example-markdown | example-html | hello_world | system_monitor | window_title`.


# 3. Core GPUI concepts used by the kit

| Concept | Rule (from docs) |
|---|---|
| `Entity<T>` | Retained state; `cx.new(\|cx\| ...)`, `.read(cx)`, `.update(cx, \|t, cx\| ...)`. With `T: Render` it is a View. Cloning copies the handle, not `T`. `WeakEntity` / `.downgrade()` for back references |
| `Context<T>` | Per-entity context: `cx.notify()`, `cx.emit(ev)`, `cx.observe`, `cx.subscribe`, `cx.spawn`, `cx.listener`, `cx.focus_handle()`. Mutation does not re-render until `cx.notify()` |
| `Window`, `App`, `AsyncApp`, `AsyncWindowContext` | Window = focus/input/dispatch; App = globals, entities, windows; Async* re-enter after `await` |
| `Render` | `fn render(&mut self, &mut Window, &mut Context<Self>) -> impl IntoElement`; element tree rebuilt each render, state lives in entities |
| `RenderOnce` | Value-like reusable component taking inputs by value |
| `observe` vs `subscribe` | observe = "entity notified"; subscribe = typed event (`impl EventEmitter<E>`). Keep the returned `Subscription` in a field or it is dropped |
| `Task<T>` | Dropping the handle cancels. `cx.spawn(async move \|this, cx\| ...)` foreground; `cx.spawn_in(window, ...)`; `cx.background_spawn(...)` for `Send` owned data, returns a Task to await; bare `cx.spawn(..);` may cancel immediately; update via `this.update(cx, ..)` (errors if view gone) |
| `Global` | Application-wide services only; use `Theme::change` / `Theme::update` for themes, not raw `global_mut` |
| Styling | Tailwind-like fluent builders: `div().flex().flex_col().gap_2().p_3().size_full().items_center()`; rem scale (`_1`=0.25rem, `_2`=0.5rem, `_4`=1rem) |
| Focus | `cx.focus_handle().tab_stop(true)` stored in the entity; `.track_focus(&handle)` per render; `is_focused(window)`, `contains_focused(window, cx)`; implement `Focusable` |
| Actions | `actions!(ns, [A, B])`; data action: `#[derive(Action, Clone, PartialEq)] #[action(namespace = chat, no_json)]`; handler `.on_action(cx.listener(Self::f))`; `MenuItem::action("Send", SendMessage)` |
| Dispatch | `window.dispatch_action(Box::new(A), cx)` (current focus), `focus_handle.dispatch_action(&A, window, cx)` (specific region), `cx.dispatch_action(&A)` (app); none evaluate key contexts |
| Propagation | Action handlers stop propagation by default; `cx.propagate()` to decline; capture via `.capture_action` |
| Key bindings | `cx.bind_keys([KeyBinding::new("secondary-s", Save, Some("Editor"))])`; `secondary` = Cmd on macOS, Ctrl elsewhere; chords `"cmd-k left"`; element `.key_context("Editor mode=normal")`; predicates `Editor && mode == normal`, `Workspace > Editor`, `!Modal`; deeper context wins, later binding wins at same depth |
| Element ids | `.id(..)` stable, derived from domain keys (never index) for reorderable content; ids needed for a11y nodes and keyed state |
| Caching | `Entity<T>::cached(style)` skips re-render of clean subtrees; `window.use_keyed_state`; VirtualList renders visible range only |
| Assets | `AssetSource` registered with `.with_assets(...)`; icons `icons/<name>.svg`; custom: implement `AssetSource` (docs example uses `rust-embed`); `icon_assets!(ExtraIcons, [Accessibility, AlarmClock])` picks catalog icons |
| Fonts | Embed with `include_bytes!` and `cx.text_system().add_fonts(...)` before opening the window, then name the family in the theme |
| Animation | `Spring::new(Duration::from_millis(420)).with_damping(0.68)` exists in the Animation page; motion respects reduced motion (?) |
| History | `gpui_kit::base::{History, UndoHistory}` (nav trail; grouped undo with `start_grouping/end_grouping`); Input keeps its own private undo manager |

Windows: each has its own `Window`, focus and `Root`; share data through one `Entity` cloned into each window's view (multi-window page).

Recommended state shape (Coding Guides): one feature = model `Entity` + view `Entity`, panels get cloned `Entity<Model>` handles; no giant app entity; no work or unconditional `cx.notify()` in `render`; semantic components over custom clickable `div`s; no literal colours/`px()` in app UI (use `cx.theme()` and rem helpers).


# 4. Component catalog

Imports: `gpui_kit::component::...` (verified paths below). "Rel." = relevance to Carbon.

| Component | Purpose | Key API / note | Rel. |
|---|---|---|---|
| Editor | Source-code editing | `component::input::{Editor, EditorState, TabSize}`; `EditorState::new(window, cx).language("rust").line_number(true).folding(true).tab_size(..).default_value(..)`; `Editor::new(&editor).h(px(320.))` (section 6) | Core editor (pillar 1) |
| Input / Textarea / NumberInput | Single-line, multi-line, numeric | `Input::new(&InputState)`; `InputState::new(window, cx).placeholder(..)`; `.aria_label(..)`; since 0.6.0 `InputState` is single-line only, multiline = `TextareaState`, code = `EditorState` | Agent compose box, settings, palette |
| Dock | Tabs + splits + left/right/bottom docks, serialisable | `component::dock::{DockArea, DockSkin, DockLayout, panel_handle, register_panel, DockAreaState, BasePanel, Panel}` (section 5) | App shell (DESIGN s4) |
| Resizable | Split panes | `h_resizable("id").on_resize(..).child(resizable_panel().size(px(200.)).child(..))`; `v_resizable`; `ResizableState::sizes()`; id unique per layout scope | Inner splits, agent panes |
| Tabs | Tab bar | `component::tab::{Tab, TabBar}`; `TabBar::new("tabs").selected_index(0).on_click(..).child(Tab::new().label("A"))` | Editor/agent tabs (a11y roles Tab/TabList) |
| Tree | Hierarchical tree | `component::tree::{tree, TreeState, TreeItem, TreeEntry}`; `TreeState::new(cx).items(vec![TreeItem::new("src","src").expanded(true).child(..)])`; `tree(&state, \|ix, entry, selected, window, cx\| ListItem::new(ix)...)`; Tree story binds Enter to Rename | File explorer, outline |
| List / VirtualList | Lists; virtualised variable-size | `v_virtual_list`, `h_virtual_list`, `VirtualListScrollHandle`, `Scrollbar`; item sizes via `Rc<Vec<Size<Pixels>>>` | Agent logs, search results |
| MessageScroller | Tail-following virtualised message list | `message_scroller::{MessageScroller, MessageScrollerState}` | Agent transcript |
| Message, Bubble, Marker, Attachment, Questionnaire | Chat surfaces | Composable chat structure; Questionnaire = multi-step Q&A | Agents tab (check fit, ?) |
| TextView | Markdown + HTML (not WebView, no CSS layout/scripts) | `component::text::{markdown, TextView}`; selectable | Agent output, docs viewer |
| Diff | Read-only patch display | `component::diff::{Diff, DiffFile, DiffState, DiffMode, DiffAnnotation, DiffConflictResolution, ...}`; parse patch, state created once in owning view | Agent edit review (read-only; per-hunk accept/reject not documented, ?) |
| DataTable / Table | Virtual rows+columns, sort, resize | `component::table::{DataTable, TableState, TableDelegate, Column, ColumnSort, ColumnFixed, TableEvent}`; delegate: `columns_count`, `rows_count`, `column`, `render_td`; `Column::new("id","ID").width(60.).sortable()`; sorting data is the delegate's job. 0.6.0: old `Table` renamed `DataTable`; `Table` is now a declarative element | Lists per DESIGN s6 |
| Command | Command palette | `component::command::{Command, CommandEntry, CommandGroup, CommandItem, CommandState}`; dispatches boxed Actions | Command palette |
| PopupMenu / Menu / ContextMenu / DropdownMenu | Menus | `div().id("x").context_menu(\|menu, window, cx\| menu.menu("Copy", Box::new(Copy)).separator())`; `menu::{PopupMenu, ContextMenuExt, DropdownMenu}` | Context menus, hamburger menu |
| Popover | Floating content | `Popover::new("id").trigger(Button..).child(..)`; `.anchor(Anchor::TopCenter)` names popover's own anchor; clamps to window, does not flip | Info "i" popups (DESIGN s6) |
| Tooltip | Hover hints | `Button::..tooltip("text")`; `component::tooltip::Tooltip` | Name + shortcut tips |
| HoverCard | Hover rich content | listed in sources, API not read (?) | Info tips |
| Dialog / AlertDialog / Sheet | Modals, slide-in | `window.open_dialog(cx, \|dialog, _, _\| dialog.title("Welcome").child(..))` via `component::WindowExt`; rendered by Root | Confirm delete (DESIGN s6: popup under button, not modal) |
| Notification | Toasts | `window.push_notification("text", cx)`; `Notification::new().message(..)`; `NotificationType` | Finished actions |
| Sidebar | Navigation sidebar | `Sidebar, SidebarHeader, SidebarFooter, SidebarGroup, SidebarMenu, SidebarMenuItem, SidebarToggleButton` | Optional activity rail / left dock content |
| StatusBar, Toolbar, TitleBar | Chrome | StatusBar left/center/right regions; TitleBar custom window chrome | Status bar, title bar |
| Settings | Settings UI | `component::setting::{Settings, SettingPage, SettingGroup, SettingItem, SettingField}` | Settings view |
| Chart | Line, Bar, Area, Pie, Radar, Candlestick, Sankey | `chart::{LineChart, BarChart, AreaChart, PieChart, RadarChart, CandlestickChart, SankeyChart}`; charts animate on first paint since 0.7.1 (`.appear(false)` to opt out) | Cost/usage meters |
| Plot primitives | Custom plotting | moved to `gpui_kit::base::plot` in 0.7.0 | Diagram canvas helpers (?) |
| Icon | Lucide, 1,830 names | `Icon::new(IconName::Inbox)`; `IconName::ALL`; icon-only buttons need `.accessibility_label(..)` | One icon set (DESIGN s3) |
| Button, DropdownButton, Toggle, Checkbox, Radio, Switch, Slider, Select, Combobox, DatePicker, ColorPicker, Form, Kbd, Label, Badge, Tag, Avatar, Progress, Spinner, Skeleton, Stepper, Accordion, Collapsible, Alert, Empty, Clipboard, Calendar, Carousel, GroupBox, DescriptionList, Pagination, Rating, OtpInput, TimeField, Speech | Basics and forms | `Button::new(id).primary().label(..).on_click(..)`; `Kbd` shows shortcuts; `Select` replaced older Dropdown name | General UI |
| Scrollable, Focus Trap | Scroll containers, focus trapping | Avoid nested scroll containers without an owner | Overlays |
| Theme | See section 7 | | |
| Not present | Terminal, node/graph diagram canvas, tab-strip drag across windows, minimap, breadcrumbs: no page found | (?) | Carbon must build or embed |

WebView is experimental (Wry; native view covers GPUI elements; macOS/Windows only, Linux example unfinished). Mobile is experimental (iOS). Both are outside Carbon scope.


## Title bar and app menu (read from 0.7.3 source)
- `component::TitleBar::new().child(..)` draws own window controls; open the window with `TitleBar::window_options()` (transparent titlebar, `app_owns_titlebar_drag`). Height 34 px; macOS left pad 80 px for traffic lights.
- `component::menu::AppMenuBar::new(cx)` (Windows/Linux in-window bar) reads `GlobalState::global(cx).app_menus()`. **The kit never fills it:** the app calls `GlobalState::global_mut(cx).set_app_menus(menus.map(Menu::owned))`, plus `cx.set_menus(..)` for the macOS native bar. Left/Right/Esc bound in context `AppMenuBar`.
- Used in `apps/carbon-gui/src/{menus,title_bar}.rs`.


# 5. Layout and docking

- `DockArea` is an `Entity` created once through `DockSkin::dock_area("main-dock", Some(1), window, cx)` (returns `(Entity<DockArea>, Rc<DockSkin>)`; the `Some(1)` is the saved-layout schema version, bump when old layouts can no longer load).
- Panels: implement `BasePanel` (`panel_name() -> &'static str`, used as persistence key), `Panel` (`title(..)`), `Render`, `Focusable`, `EventEmitter<PanelEvent>`; wrap with `panel_handle(entity)`. `Panel::title_bar` lets a panel opt out of the tab title bar (0.6.2).
- Layout is plain data: `DockLayout::h_split().child(DockLayout::tabs().panel_view(panel_handle(files), cx), Some(px(240.))).child(DockLayout::tabs().panel_view(panel_handle(editor), cx), None)`; `None` = fill; apply with `area.set_center(layout, window, cx)`. Only `Split` and `Tabs` containers exist (freeform tiles canvas removed in 0.6.2).
- Edge docks via `DockPlacement` (left, right, bottom); panels can be added, removed, activated, zoomed, moved at runtime. Events: `DockEvent` incl. `LayoutChanged`.
- Persistence: `area.read(cx).dump(cx)` -> `DockAreaState` (Serde) -> `area.load(state, window, cx)`. Register every restorable panel at init: `register_panel(cx, "FilesPanel", |state, window, cx| { ... panel_handle(panel) })`. Unregistered names become a placeholder, not dropped (shell docs); keep a fallback layout.
- Styling: `dock_skin.set_panel_style(PanelStyle::default(), cx)`, `set_toggle_button_visible(true, cx)`; base engine is renderer-independent (custom renderer traits in `gpui-base`).
- Layout state saving (shell docs): `layout_changed` fires on every edit, so debounce saves.
- Multi-window: separate `Root` per window, shared `Entity` models (section 3). Dragging a panel between windows is not documented (?). Pop-out of editor/agent panes would need own implementation.
- Carbon usage (read from 0.7.3 source, `apps/carbon-gui/src/workbench.rs`): close buttons are off by default (`skin.set_close_button_visible(true, cx)`); `set_dock` creates a dock at `PANEL_MIN_SIZE * 2`, so call `set_dock_size` after; a dock keeps open state across `set_dock`, so reset must reopen closed docks; `PanelState.info = PanelInfo::Panel(json)` carries per-panel data, read back via `PanelBuildContext::info()` in `register_panel`. Menu actions with no focused element never reach view handlers: the root view holds a `FocusHandle`, focused on window open.
- T2 patterns (`apps/carbon-gui/src/{menus,workbench}.rs`): data actions `#[derive(Clone, PartialEq, Action)] #[action(namespace = carbon, no_json)]` work in `MenuItem::action` and `KeyBinding`; `MenuItem::checked(bool)` for toggles; dynamic menus = rebuild `cx.set_menus` + `GlobalState::set_app_menus`, then `AppMenuBar::reload`. Focused panel = first `area.panel(id).focus_handle(cx).contains_focused(..)` over `area.layout(placement).panels()`; keyboard move = `area.move_panel(id, InsertTarget::Tabs { node: first Tabs node, .. })` (empty dock: `InsertTarget::Split` on root, untested). No per-dock focus or bounds API: auto-hide uses `area.bounds()` + `dock_size` and a deferred `capture_any_mouse_down`. Dialog: `window.open_dialog(cx, |d, _, _| d.title(..).child(Input::new(&state)).footer(..))`; close with `window.close_dialog(cx)`; Enter via `InputEvent::PressEnter`.
- Extra icons: the default `Assets` bundle has only ~101 component icons (no `Code`). Use `gpui_kit::assets::icon_assets!(ExtraIcons, [Code])` and an own `AssetSource` that tries `ExtraIcons` then `Assets` (`apps/carbon-gui/src/app_assets.rs`); `Icon::new(gpui_kit::assets::IconName::Code)`.
- Status bar: `component::status_bar::StatusBar::new().left(..).right(..)` (center via `child`); it is `Styled`, so `.px(..)` / `.h(..)` work; default ~24 px (`py_1` + `text_xs`); Carbon sets `.h(TITLE_BAR_HEIGHT).py_0()` to match the title bar (34 px). Carbon: `apps/carbon-gui/src/status_bar.rs`.
- `test-support` feature does not resolve in 0.7.3 (pulls optional `gpui-fast`, pin conflicts): no `TestAppContext` tests yet (STATUS error log).
- Shell panels (JS) are out of scope for Carbon v1; `gpui-shell` could be one extension route (QuickJS, capability grants) but UI maturity is unverified (?).


# 6. Code editor component

| Topic | Facts read |
|---|---|
| Model | Rope text buffer (`ropey 2.0.0-beta.1`), separate from highlighting |
| Size claim | README: "stable at 200K lines" (vendor, not audited) |
| Highlighting | Tree-sitter when `tree-sitter-<lang>` feature enabled (`tree-sitter-languages` bundles all); large buffers parsed in background, edits reuse previous parse tree; `syntect` feature also exists (role not described, ?) |
| Features | Line numbers, folding, `show_whitespaces`, search panel (Ctrl/Cmd-F; `open_search`, `set_searchable(false)` + `set_search_query`, `next_search_match`, `replace_all_search_matches`), multi-cursor and column selection, decorations (`create_decorations_collection`, `create_range_decorations_collection`, byte-offset ranges follow edits), read-only mode, completion menu, hover, diagnostics, code actions, go-to-definition on Ctrl/Cmd-click |
| Language rules | `set_language_config("rust", LanguageConfig::default().brackets([BracketPair::new("{","}")]).auto_closing_pairs([AutoClosingPair::new("{","}").not_in([SyntaxContext::String, SyntaxContext::Comment])]).auto_close_before(";:.,=}])>"), cx)`; `.auto_close(bool)`, `.smart_indent(bool)`; `IndentationRules::new(increase, decrease)` (regex) |
| Manual completion | `ShowCompletions` action, **no default shortcut**; context `Input && mode == editor`; `editor.update(cx, \|s, cx\| s.show_completions(window, cx))` |
| Keys | Add cursor: Cmd+Option+Up/Down (mac), Ctrl+Alt+Up/Down (Win), Alt+Shift+Up/Down (Linux); Alt/Option-click adds cursor; Alt+Shift drag = rectangular; Esc keeps one cursor |
| Events | `cx.subscribe(&editor, \|this, state, event: &InputEvent, cx\| if matches!(event, InputEvent::Change) { .. })`; `state.read(cx).value()`; `set_value(.., window, cx)` |
| Font | Theme `mono_font_family` / `mono_font_size`; rows 1.5x font size; override `Editor::new(&e).font_family("JetBrains Mono").text_size(px(15.))`; platform default (Menlo/Consolas/DejaVu Sans Mono) swapped if missing |
| Appearance | `.bordered(true).disabled(false).readonly(false).aria_label("Rust source")` |
| LSP | Docs describe a completion/hover/diagnostics provider seam ("Delegate/Provider" naming, `CompletionProvider` named in coding guide; kit dev-depends on `lsp-types` 0.97). Exact provider trait signatures, how a language server is spawned and wired, diagnostics push API: **not read (?)**; the docs say the editor does not run a server itself (the example completion is "a deterministic provider, not a live language server") |
| Not supported | Monaco JSON or Tree-sitter `.scm` loaders; selection-surrounding; custom `onEnterRules`; persistent Vim visual block; Input-only adornments |
| Bundle size | Per-grammar feature flags control size; numbers per grammar not given (?). Carbon should enable a short list, not `tree-sitter-languages`, and measure release size |
| Carbon gaps | No diff/merge editor integration, minimap, breadcrumbs, inline agent-attributed edits, per-actor undo (Input has private undo manager); these are Carbon build items. Typing latency and 200K-line behaviour need the prototype from PROJECT.md |


# 7. Theming and design tokens

- Access: `use gpui_kit::component::ActiveTheme as _;` then `cx.theme().primary`, `.background`, `.foreground`; full resolved token `cx.theme().tokens.button_primary` (`.color`, `.background` incl. gradients).
- Change at runtime: `Theme::change(...)` for a mode change, `Theme::update(cx, |theme| { ... })` for an edit; **not** raw `Theme::global_mut` (skips sync and window refresh). Zoom: `Theme::update(cx, |theme| theme.font_size = px(18.));` (base font = rem reference, `window.set_rem_size`).
- JSON themes: file with `"themes": [ { "name", "mode": "dark"|"light", "colors": { ... }, "highlight": { ... } } ]` and `$schema` `https://github.com/longbridge/gpui-kit/raw/refs/heads/main/.theme-schema.json`. Colour keys are dotted: `background`, `foreground`, `border`, `ring`, `muted.background`, `muted.foreground`, `primary.background`, `primary.foreground`, `secondary.*`, `accent.*`, `panel.background`, `popover.*`, `list.active.background`, `list.even.background`, `scrollbar.*`, `title_bar.*`, `window_border`, `link.*`, `chart.grid`, `base.red|green|yellow|blue|magenta|cyan`; button tokens like `button.primary.background` accept `linear-gradient(135deg, #4F46E5, #06B6D4)`. `highlight` holds editor tokens (`editor.foreground`, `editor.background`, `editor.active_line.background`, syntax colours).
- Registry: `ThemeRegistry::watch_dir(PathBuf::from("./themes"), cx, move |cx| { ... Theme::update(cx, |current| current.apply_config(&theme)); })` loads and hot-reloads themes from a directory; `ThemeRegistry::global(cx).themes().get(&name)`.
- 20+ built-in theme files in repo `themes/` (Catppuccin, Tokyo Night, Gruvbox, Solarized, Ayu, Everforest, Flexoki, ...). DESIGN.md's wanted starting set (own pair, Catppuccin, Tokyo Night, Gruvbox, Rose Pine, Nord) is mostly present; Rose Pine/Nord not seen in the list (?).
- Sizes: components are `Sizable` (`xsmall/small/medium/large`, ?); density = base font/rem.

Mapping Carbon DESIGN.md tokens (proposal):

| Carbon token (DESIGN s3) | Kit mechanism |
|---|---|
| Colour roles, light/dark | theme JSON `colors` + `mode`; ship two files: `carbon-dark.json`, `carbon-light.json` |
| Syntax colours separate group | `highlight` block (separate from UI `colors`), matches "editor themes vs UI themes can differ" |
| Spacing 4 px scale | rem helpers: `_1`=4px at 16px base; `gap_2`=8px, `p_3`=12px, etc. Set base font to 16 px so 1 rem = 16 px |
| Type scale x text-size setting | `Theme.font_size` (rem base) + `text_sm()`; editor size via `mono_font_size` (separate setting) |
| Density compact/comfortable | two rem bases or component `Size`; no ready "density" setting documented (?) |
| Contrast contract | not enforced by kit; add a CI test over theme JSON |
| Icons single set | Lucide via `IconName`; add via `icon_assets!` or own `AssetSource` |
| VS Code theme import | not provided; needs converter to theme JSON (`colors` + `highlight`) (?) |
| Motion / reduced motion | Animation page; reduced-motion honouring per component unverified (?) |


Carbon setup (read from 0.7.3 source): theme files are `ThemeSet` JSON; missing colour keys fall back to kit defaults; invalid files are skipped with only a `tracing` log, so Carbon tests parse them (`style/theme.rs`). `watch_dir` loads asynchronously then calls `on_load`; set `Theme::global_mut(cx).dark_theme/light_theme` there, then `Theme::change(mode, None, cx)`. The kit's registry observer re-applies themes by those names on hot reload.


# 8. Accessibility, IME, text input, keyboard

What exists (Accessibility page): AccessKit tree built by GPUI; elements enter the tree only with **both** a stable `ElementId` and a non-empty role. Controls with roles/state: Button, Link, Checkbox (mixed), Switch, Toggle, Radio, Input (SetValue action, password values withheld), Select (ComboBox, expanded), Tabs (Tab/TabList), Slider/Progress (numeric, Increment/Decrement), Table (roles, indices). Custom: `div().id("x").role(Role::Button).aria_label(..).track_focus(&h)`, `.on_a11y_action(..)`; custom `Element` implements `a11y_role()` and `write_a11y_info(&self, node: &mut accesskit::Node)`. Tests: `ElementSnapshot::{role,label,value,focused,checked,selected,expanded}` after `window.render_frame(cx)`.

Known gaps stated by the docs:
- Comparison table rates kit accessibility **Partial** (egui, Qt, Slint Yes).
- No general `.aria_disabled(..)`, no `aria-labelledby/describedby/controls` builders; nearby text does not name a control; tooltip is not an accessible name; icon-only buttons need explicit `accessibility_label`.
- Headless tests do not validate platform adapters or announcements; `Role::Status` announcement is not guaranteed; verify with real screen readers per OS. No Narrator/VoiceOver/Orca results are published (?).
- Focus: tracked handle is not automatically a tab stop (`tab_stop(true)`); restore focus to trigger after dialogs/sheets.

IME/text input: Input and Editor have IME preedit handling (completion requests are not issued during preedit and resume after commit; Input has composition-aware undo). Platform IME quality (Windows/macOS/Linux, CJK) is not documented beyond that (?). Text system: Rope, font fallback, CJK needs a font with actual glyphs (Fonts page).

Keyboard: `secondary-` modifier, chords, key contexts (section 3). OS/WM may swallow combos (Alt+F4, Win keys, Linux Ctrl+Alt+Up/Down), test per platform. Editor `ShowCompletions` has no default binding to avoid IME clashes.

Carbon DESIGN s8 requires "AccessKit from the first widget": the kit supplies the mechanism; every custom Carbon widget (diagram canvas, terminal, agent transcript) must set id + role + label itself.


# 9. Testing, dev workflow, packaging

- Tests: feature `test-support` (dev-dependency only), `#[gpui_kit::test]`, `TestAppContext`, `TestWindowExt` (`window.find("id")`, `click`, `input("text")`, `press("backspace")`, `render_frame(cx)`), `TestSupportExt` (`.test_support()` on custom divs). Commands: `cargo test --test ui --locked`, `cargo test -p gpui-kit --features test-support --test ui --locked`. Rendering tests: `--test rendering`. CI: run both with `--locked`.
- Do not `use gpui_kit::*;` blindly in test modules: it imports GPUI's `test` macro and can shadow.
- FPS monitor (`profiler` feature; `/docs/fps`): rows MAX FPS, INTERVAL, FRAME, P95, DROP, INV, GPU, CPU, MEM; 120 Hz = 8.3 ms frame budget; "no cross-framework FPS claim backed" by the docs.
- Hot reload: none for Rust UI (rebuild only); theme JSON hot-reloads via `ThemeRegistry::watch_dir`. `inspector` feature exists (usage not read, ?).
- Packaging (Packaging page): `cargo build --release --locked`. macOS `.app` + Developer ID codesign + hardened runtime + notarytool (`Accepted` required) + DMG. Windows ZIP or Inno Setup `.exe`; test the extracted copy, not `target\release`. Linux `.tar.gz` or `.deb`; needs graphical session + Vulkan. Default icon assets are embedded; resolve other resources relative to the install location. Auto-update page exists (`/docs/auto-update`), not read in detail (?).
- Linux tiled windows: popup edge clamping fixed in 0.6.4.


# 10. Gotchas / experience log

Format: Problem - Cause - Solution.

## Text does not render on macOS
**Cause:** `gpui-pre-platform` without `font-kit` uses `NoopTextSystem`. **Solution:** Use `gpui-kit` (enables it) or add `features = ["font-kit"]` on `gpui_platform` only; check `cargo tree -e features -i gpui-pre-macos`.

## Window opens but nothing draws / no window on Linux
**Cause:** Vulkan loader without GPU driver, or headless shell. **Solution:** Graphical Wayland/X11 session plus real Vulkan driver (`vulkan-validationlayers libvulkan1` are not enough).

## Build breaks after `cargo update` (type or API mismatch)
**Cause:** gpui snapshot mismatch between crates; caret requirement drift. **Solution:** `gpui-kit` pins `gpui-pre-*` with `=`; update only `gpui-kit`; never override one `gpui-pre-*` crate; do not mix `gpui-fast` types with direct upstream gpui.

## Slow dev builds at runtime
**Cause:** Debug build leaves GPUI, layout and text unoptimised. **Solution:** `[profile.dev.package]` opt-level 3 list (section 1). It lengthens first compile.

## Dialogs/notifications never appear, or compile errors on `Root::render_*_layer`
**Cause:** 0.7.0 removed manual layer rendering; `open_window` always mounts `Root`. **Solution:** Use `gpui_kit::init` + `gpui_kit::open_window`; call `window.open_dialog(cx, build)`, `window.push_notification(..)`; delete `.children(Root::render_*_layer)`.

## Keyboard shortcut does nothing
**Cause:** No focus path, missing `key_context`, handler on sibling branch, or a closer handler consumed the action. **Solution:** Check in order: focused handle, `track_focus`, `key_context`, `.on_action`, `cx.propagate()`. Test by dispatching on the `FocusHandle` directly. Direct `dispatch_action` ignores key contexts.

## Dialog closes although `on_click` kept it open (Carbon, 0.7.3 source)
**Cause:** the dialog binds `enter` to `Confirm`, a single-line `Input` propagates Enter, and the default `on_ok` returns true (close). **Solution:** put submit logic in `dialog.on_ok(|_, window, cx| -> bool)`; return false to keep it open. Do not also handle `InputEvent::PressEnter`, or it runs twice.

## Actions from buttons do nothing after a popup closes (Carbon, gpui-pre source)
**Cause:** `window.dispatch_action` starts at the focused node; with nothing focused it starts at the window root (kit `Root`), above the app view's `.on_action` handlers. **Solution:** `cx.on_focus_lost(window, |this, window, cx| this.focus_handle.focus(window, cx))` in the root view.

## Menu shows only "Ctrl+K" for a chord (Carbon, 0.7.3 source)
**Cause:** `Kbd::from_binding` renders `keystrokes().first()`. **Solution:** none in the kit; give menu actions a single-key binding or accept.

## Path text cut at the wrong end
**Solution:** `.overflow_hidden().whitespace_nowrap().text_ellipsis_start()` (GPUI) keeps the end of a path; `.truncate()` cuts the end.

## Task never finishes / is cancelled
**Cause:** `Task` handle dropped. **Solution:** Store it in a field (`_load_task`), await or `.detach()`; use `WeakEntity` update and handle `Err` when the view closed.

## Clicking custom `div` is not keyboard/screen-reader reachable
**Cause:** hitbox without role, tab stop, key handler. **Solution:** Use `Button`/semantic component, or `.id().role().aria_label().track_focus()` with `tab_stop(true)` plus Enter/Space handling.

## Reordering list items loses state
**Cause:** index-based `ElementId`s. **Solution:** Domain-keyed ids.

## `cx.notify()` loops / redraw storms
**Cause:** mutating state or notifying in `render`. **Solution:** Notify the narrowest entity after a coherent change; cache with `Entity::cached` only after measuring.

## Subscriptions silently stop
**Cause:** `Subscription` dropped. **Solution:** Keep in a field named `_sub`.

## Theme edit does not refresh windows
**Cause:** raw `Theme::global_mut`. **Solution:** `Theme::change` / `Theme::update`.

## Multiline input compile errors after 0.6.0
**Cause:** `InputState` is single-line only. **Solution:** `TextareaState` for multiline, `EditorState` for code; `Table` -> `DataTable`; `row_selector` -> `row_header`; `Divider` -> `Separator`; old Dropdown -> `Select`.

## Other breaking changes to expect

| Version | Change |
|---|---|
| 0.7.3 | `TooltipRequest::placement` deprecated for `with_placement`; TextView paragraph gap 0.75rem (restore `paragraph_gap(rems(1.))`); table width and `visible_rows_changed` changes; 0.7.2 yanked |
| 0.7.1 | Charts animate on first paint (`.appear(false)`); single-choice Questionnaire confirms by default |
| 0.7.0 | `Root::bordered` -> `window_border()`; text selection -> `TextSelection::*`; plot primitives -> `gpui_kit::base::plot`; DatePicker Change payload changed |
| 0.6.2 | Dock tiles canvas removed; iOS/Android added |

Rule: the Coding Guides say to read the current source for signatures rather than translate from React or older GPUI examples.

## Carbon-specific risks (inferred)
- Terminal, node-graph diagram canvas, markdown/diagram AST editing, per-hunk diff accept/reject, and multi-window panel dragging are not documented components.
- Weekly-ish minor releases with breaking changes: pin `gpui-kit = "=0.7.3"` or a tilde range and upgrade deliberately with a changelog read.
- Docs are CC BY 4.0: copying text into Carbon docs needs credit; short snippets here are Apache-2.0 code examples.


# 11. Open questions / gaps

- (?) LSP: provider trait signatures, how diagnostics/hover/completion are fed from an external language server, and whether `lsp-types` is public API. Needs reading `crates/base/src/input` and the editor example.
- (?) Editor latency, memory and large-file behaviour beyond the "200K lines" claim; per-grammar binary size.
- (?) Real screen reader results (Narrator, VoiceOver, Orca) and IME quality on Windows/macOS/Linux.
- (?) Whether `syntect` and `tree-sitter` features are alternatives or additive; which feature the Editor uses by default.
- (?) Docking across multiple windows (drag panel to new window), tab drag between groups in different windows.
- (?) Size/density API (`Sizable` levels) and a compact density preset matching DESIGN s1.
- (?) VS Code theme import and Nord/Rose Pine presets.
- (?) Terminal embedding path (kit has none); diagram canvas strategy (custom `Element`/paint pages: Element, Paint, Geometry, Animation were not read in depth).
- (?) Auto-update, system notification, native extension, WebView, WebAssembly, Mobile, Speech, i18n details beyond `rust_i18n::i18n!("locales", fallback = "en"); rust_i18n::extend!(gpui_component);`.
- (?) GitHub issues/discussions and releases before 0.6.0 (page text truncated; v0.5.x notes absent); the last ~26k characters of the releases page were not read.
- (?) Longbridge/Zed relationship for upstream GPUI fixes: `gpui-pre` is a snapshot, so Zed fixes arrive only with a new snapshot.
- Not read: story/gallery source code, individual examples source (only doc excerpts), `docs.rs` API pages, Chinese pages, `gpui-shell` pages in depth.


# 12. Sources (all read 2026-10-16)

- https://gpui-kit.com/docs/ (index; sidebar pages: installation, getting-started, entity, context, window, multi-window, element_id, focus, shared-string, render, render-once, style, action, keybinding, event, task, global, view-cache, element, paint, text-system, geometry, animation, test, accessibility, webassembly, image, assets, fonts, i18n, system-notification, native-extension, webview, mobile, packaging, auto-update, design-guides, coding-guides, comparison, fps)
- https://gpui-kit.com/llms-full.txt (full text of docs, component, base and shell pages, 4.5 MB; component pages used include editor, dock, theme, root, input, tabs, tree, table, virtual-list, resizable, sidebar, menu, popover, dialog, notification, command, settings, diff, chart, text-view, message-scroller)
- https://gpui-kit.com/releases and https://github.com/longbridge/gpui-kit/releases
- https://raw.githubusercontent.com/longbridge/gpui-component/main/README.md
- https://raw.githubusercontent.com/longbridge/gpui-component/main/Cargo.toml
- https://raw.githubusercontent.com/longbridge/gpui-kit/main/crates/kit/Cargo.toml
- https://raw.githubusercontent.com/longbridge/gpui-kit/main/themes/tokyonight.json
- Carbon context: `.agents/PROJECT.md`, `.agents/DESIGN.md`, `.agents/documents/gui-framework.md`
