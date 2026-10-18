//! Terminal tabs: one panel per shell session. The core runs the shell and
//! its emulator; this panel draws the screen and turns keys, text input and
//! the mouse into input for it.
//!
//! Keys reach the terminal before the app's shortcuts (a keystroke
//! interceptor), except the few the app keeps (`input::app_keeps`), so
//! Ctrl+W, Ctrl+K and the like go to the shell. Printable text comes through
//! the platform's text input instead, so IME and AltGr work.

mod colors;
mod input;
mod view;

use std::cell::Cell;
use std::ops::Range;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use carbon_core::lsp::lsp_types;
use carbon_core::terminal::alacritty_terminal::grid::{Dimensions, Scroll};
use carbon_core::terminal::alacritty_terminal::index::{
    Column, Direction, Line, Point as GridPoint, Side,
};
use carbon_core::terminal::alacritty_terminal::selection::{Selection, SelectionType};
use carbon_core::terminal::alacritty_terminal::term::TermMode;
use carbon_core::terminal::alacritty_terminal::term::search::RegexSearch;
use carbon_core::terminal::shells::{self, Profile};
use carbon_core::{Core, TerminalEvent, TerminalId, TerminalSpec};
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::dock::{
    BasePanel, Panel, PanelEvent, PanelInfo, PanelState, panel_handle, register_panel,
};
use gpui_kit::component::input::{self as kit_input, Input, InputEvent, InputState};
use gpui_kit::component::menu::PopupMenu;
use gpui_kit::component::{ActiveTheme, IconName, Sizable, WindowExt, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::style::fixed;
use crate::workspace_ops::current_core;
use colors::Palette;
use view::{Geometry, Snapshot, grid_point};

/// Persistence key in saved layouts. Never change it.
const PANEL_NAME: &str = "Terminal";
/// Key context of a terminal (the interceptor looks for it) and its find bar.
const KEY_CONTEXT: &str = "Terminal";
const FIND_CONTEXT: &str = "TerminalFind";

actions!(terminal, [Restart, Find, CloseFind]);

/// Open a new terminal with a shell profile (by name; empty: the default).
#[derive(Clone, PartialEq, Action)]
#[action(namespace = terminal, no_json)]
pub struct NewTerminal {
    pub profile: SharedString,
}

/// The terminal that had focus last: the interceptor's target, and where
/// new terminals take their working directory from.
struct ActiveTerminal(WeakEntity<TerminalPanel>);

impl Global for ActiveTerminal {}

pub struct TerminalPanel {
    core: Entity<Core>,
    focus_handle: FocusHandle,
    terminal: Option<TerminalId>,
    profile: Option<Profile>,
    /// Tab name: the profile name, numbered when there are several.
    label: SharedString,
    /// Why the shell did not start.
    error: Option<String>,
    exited: Option<Option<u32>>,
    geometry: Rc<Cell<Option<Geometry>>>,
    selecting: bool,
    /// Wheel movement not yet a whole line.
    scroll_rest: f32,
    /// Link under the pointer while Ctrl is held: row, column range.
    link: Option<(usize, usize, usize)>,
    find: Option<FindBar>,
    _events: Task<()>,
}

struct FindBar {
    input: Entity<InputState>,
    /// The match shown (selected), to search on from.
    current: Option<(GridPoint, GridPoint)>,
    _events: Subscription,
}

impl TerminalPanel {
    /// A new terminal tab running `profile` (the default shell when `None`)
    /// in `cwd` (the open folder when `None`).
    pub fn new(
        profile: Option<Profile>,
        label: Option<SharedString>,
        cwd: Option<PathBuf>,
        cx: &mut App,
    ) -> Entity<Self> {
        let core = current_core(cx);
        let profile = profile.or_else(shells::default_profile);
        let label = label.unwrap_or_else(|| {
            profile
                .as_ref()
                .map(|profile| profile.name.clone().into())
                .unwrap_or_else(|| "Terminal".into())
        });
        cx.new(|cx| {
            let mut this = Self {
                core,
                focus_handle: cx.focus_handle(),
                terminal: None,
                profile,
                label,
                error: None,
                exited: None,
                geometry: Rc::new(Cell::new(None)),
                selecting: false,
                scroll_rest: 0.,
                link: None,
                find: None,
                _events: Task::ready(()),
            };
            this.start(cwd, cx);
            this
        })
    }

    pub fn label(&self) -> SharedString {
        self.label.clone()
    }

    /// The shell's working directory (from shell integration, else where it
    /// started).
    pub fn cwd(&self, cx: &App) -> Option<PathBuf> {
        let id = self.terminal?;
        self.core.read(cx).terminals.get(id)?.cwd()
    }

    /// Starts the shell and follows its events.
    fn start(&mut self, cwd: Option<PathBuf>, cx: &mut Context<Self>) {
        let (cols, rows) = self
            .geometry
            .get()
            .map_or((80, 24), |geometry| (geometry.cols, geometry.rows));
        // A folder that is gone (or a path in a form Windows cannot use)
        // falls back to the open folder.
        let cwd = cwd.filter(|cwd| cwd.is_dir()).or_else(|| {
            self.core
                .read(cx)
                .workspace()
                .map(|workspace| workspace.root().to_path_buf())
        });
        let spec = match &self.profile {
            Some(profile) => profile.spec(cwd, cols, rows),
            None => TerminalSpec {
                cwd,
                cols,
                rows,
                ..TerminalSpec::default()
            },
        };
        let spawned = self.core.update(cx, |core, _| core.spawn_terminal(spec));
        match spawned {
            Ok(id) => {
                let events = self.core.read(cx).terminals.get(id).map(|t| t.events());
                self.terminal = Some(id);
                self.error = None;
                self.exited = None;
                if let Some(events) = events {
                    self._events = cx.spawn(async move |this, cx| {
                        while let Ok(event) = events.recv().await {
                            if this
                                .update(cx, |this, cx| this.on_event(event, cx))
                                .is_err()
                            {
                                break;
                            }
                        }
                    });
                }
            }
            Err(err) => self.error = Some(format!("The shell did not start: {err}")),
        }
        cx.notify();
    }

    fn on_event(&mut self, event: TerminalEvent, cx: &mut Context<Self>) {
        match event {
            TerminalEvent::Wakeup => {
                if let Some(terminal) = self
                    .terminal
                    .and_then(|id| self.core.read(cx).terminals.get(id))
                {
                    terminal.ack_wakeup();
                }
                cx.notify();
            }
            TerminalEvent::Clipboard(text) => {
                cx.write_to_clipboard(ClipboardItem::new_string(text))
            }
            TerminalEvent::Exited(code) => {
                self.exited = Some(code);
                cx.notify();
            }
            TerminalEvent::Title(_) | TerminalEvent::Bell => {}
        }
    }

    /// "..." > Restart, or Enter after the shell exited: a fresh shell in the
    /// same folder.
    fn restart(&mut self, _: &Restart, _: &mut Window, cx: &mut Context<Self>) {
        let cwd = self.cwd(cx);
        self.stop(cx);
        self.start(cwd, cx);
    }

    fn stop(&mut self, cx: &mut Context<Self>) {
        if let Some(id) = self.terminal.take() {
            self.core.update(cx, |core, _| {
                _ = core.terminals.close(id);
            });
        }
    }

    /// Runs `f` with the session, if it is running.
    fn with_terminal<R>(&self, cx: &App, f: impl FnOnce(&carbon_core::Terminal) -> R) -> Option<R> {
        let id = self.terminal?;
        self.core.read(cx).terminals.get(id).map(f)
    }

    /// Sends input as typed: back to the bottom, selection gone.
    fn send(&mut self, bytes: &[u8], cx: &mut Context<Self>) {
        self.with_terminal(cx, |terminal| {
            terminal.scroll(Scroll::Bottom);
            terminal.clear_selection();
            _ = terminal.write(bytes);
        });
        cx.notify();
    }

    fn copy(&mut self, cx: &mut Context<Self>) -> bool {
        let Some(text) = self
            .with_terminal(cx, |terminal| terminal.selection_text())
            .flatten()
        else {
            return false;
        };
        cx.write_to_clipboard(ClipboardItem::new_string(text));
        true
    }

    fn paste(&mut self, cx: &mut Context<Self>) {
        let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) else {
            return;
        };
        self.with_terminal(cx, |terminal| {
            terminal.clear_selection();
            _ = terminal.paste(&text);
        });
        cx.notify();
    }

    /// A keystroke while this terminal has focus (from the interceptor).
    /// Returns whether it was used (the app's shortcuts then do not run).
    fn key(&mut self, keystroke: &Keystroke, window: &mut Window, cx: &mut Context<Self>) -> bool {
        let m = keystroke.modifiers;
        let key = keystroke.key.as_str();
        if self.exited.is_some() || self.terminal.is_none() {
            if key == "enter" {
                self.restart(&Restart, window, cx);
                return true;
            }
            return false;
        }
        match (m.control, m.shift, m.alt, key) {
            (true, false, false, "c") if self.copy(cx) => {
                self.with_terminal(cx, |terminal| terminal.clear_selection());
                cx.notify();
                return true;
            }
            (true, true, false, "c") => {
                self.copy(cx);
                return true;
            }
            (true, _, false, "v") => {
                self.paste(cx);
                return true;
            }
            (true, false, false, "f") => {
                self.open_find(window, cx);
                return true;
            }
            (false, true, false, "pageup") => {
                self.scroll(Scroll::PageUp, cx);
                return true;
            }
            (false, true, false, "pagedown") => {
                self.scroll(Scroll::PageDown, cx);
                return true;
            }
            (false, true, false, "home") => {
                self.scroll(Scroll::Top, cx);
                return true;
            }
            (false, true, false, "end") => {
                self.scroll(Scroll::Bottom, cx);
                return true;
            }
            _ => {}
        }
        if input::passes_as_text(keystroke) {
            // The platform's text input brings it (`replace_text_in_range`).
            return false;
        }
        let mode = self
            .with_terminal(cx, |terminal| *terminal.term().lock().mode())
            .unwrap_or_else(TermMode::empty);
        match input::to_bytes(keystroke, mode) {
            Some(bytes) => {
                self.send(&bytes, cx);
                true
            }
            None => false,
        }
    }

    fn scroll(&mut self, scroll: Scroll, cx: &mut Context<Self>) {
        self.with_terminal(cx, |terminal| terminal.scroll(scroll));
        cx.notify();
    }

    // ---- mouse ----------------------------------------------------------

    fn grid_position(
        &self,
        position: Point<Pixels>,
        cx: &App,
    ) -> Option<(GridPoint, Side, usize, usize)> {
        let geometry = self.geometry.get()?;
        let (row, col, right) = geometry.cell_at(position);
        let point =
            self.with_terminal(cx, |terminal| grid_point(&terminal.term().lock(), row, col))?;
        Some((
            point,
            if right { Side::Right } else { Side::Left },
            row,
            col,
        ))
    }

    fn mouse_down(&mut self, event: &MouseDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        self.focus_handle.focus(window, cx);
        let Some((point, side, row, col)) = self.grid_position(event.position, cx) else {
            return;
        };
        if event.modifiers.control {
            self.open_link(row, col, window, cx);
            return;
        }
        let ty = match event.click_count {
            2 => SelectionType::Semantic,
            3.. => SelectionType::Lines,
            _ => SelectionType::Simple,
        };
        self.with_terminal(cx, |terminal| {
            if event.modifiers.shift && terminal.term().lock().selection.is_some() {
                terminal.update_selection(point, side);
            } else {
                terminal.start_selection(ty, point, side);
            }
        });
        self.selecting = true;
        cx.notify();
    }

    fn mouse_move(&mut self, event: &MouseMoveEvent, _: &mut Window, cx: &mut Context<Self>) {
        if self.selecting && event.pressed_button == Some(MouseButton::Left) {
            if let Some((point, side, ..)) = self.grid_position(event.position, cx) {
                self.with_terminal(cx, |terminal| terminal.update_selection(point, side));
                cx.notify();
            }
            return;
        }
        let link = if event.modifiers.control {
            self.grid_position(event.position, cx)
                .and_then(|(_, _, row, col)| self.link_at(row, col, cx))
                .map(|(_, row, range)| (row, range.start, range.end))
        } else {
            None
        };
        if link != self.link {
            self.link = link;
            cx.notify();
        }
    }

    fn mouse_up(&mut self, _: &MouseUpEvent, _: &mut Window, _: &mut Context<Self>) {
        self.selecting = false;
    }

    fn scroll_wheel(&mut self, event: &ScrollWheelEvent, _: &mut Window, cx: &mut Context<Self>) {
        let Some(geometry) = self.geometry.get() else {
            return;
        };
        let lines = match event.delta {
            ScrollDelta::Lines(delta) => delta.y * 3.,
            ScrollDelta::Pixels(delta) => delta.y / geometry.cell.height,
        } + self.scroll_rest;
        let whole = lines.trunc();
        self.scroll_rest = lines - whole;
        if whole == 0. {
            return;
        }
        let whole = whole as i32;
        let mode = self
            .with_terminal(cx, |terminal| *terminal.term().lock().mode())
            .unwrap_or_else(TermMode::empty);
        // Full-screen programs (less, vim) get arrow keys instead.
        if mode.contains(TermMode::ALT_SCREEN | TermMode::ALTERNATE_SCROLL) {
            let arrow: &[u8] = match (whole > 0, mode.contains(TermMode::APP_CURSOR)) {
                (true, true) => b"\x1bOA",
                (true, false) => b"\x1b[A",
                (false, true) => b"\x1bOB",
                (false, false) => b"\x1b[B",
            };
            let bytes = arrow.repeat(whole.unsigned_abs() as usize);
            self.with_terminal(cx, |terminal| _ = terminal.write(&bytes));
        } else {
            self.scroll(Scroll::Delta(whole), cx);
        }
    }

    // ---- links ----------------------------------------------------------

    /// The link at a viewport cell: a URL, an OSC 8 hyperlink, or a file path
    /// (with `:line:col`) that exists, relative to the shell's folder.
    fn link_at(&self, row: usize, col: usize, cx: &App) -> Option<(Link, usize, Range<usize>)> {
        let cwd = self.cwd(cx);
        self.with_terminal(cx, |terminal| {
            let term = terminal.term().lock();
            let point = grid_point(&term, row, col);
            if let Some(link) = term.grid()[point].hyperlink() {
                let range = col..col + 1;
                return Some((Link::Url(link.uri().to_string()), row, range));
            }
            let line = point.line;
            let text: Vec<char> = term
                .bounds_to_string(
                    GridPoint::new(line, Column(0)),
                    GridPoint::new(line, term.last_column()),
                )
                .chars()
                .collect();
            let (start, end) = word_around(&text, col)?;
            let word: String = text[start..end].iter().collect();
            let link = Link::parse(&word, cwd.as_deref())?;
            Some((link, row, start..end))
        })
        .flatten()
    }

    fn open_link(&mut self, row: usize, col: usize, window: &mut Window, cx: &mut Context<Self>) {
        let Some((link, ..)) = self.link_at(row, col, cx) else {
            return;
        };
        match link {
            Link::Url(url) => cx.open_url(&url),
            Link::File { path, line, column } => {
                window.defer(cx, move |window, cx| {
                    let core = current_core(cx);
                    let opened = core.update(cx, |core, cx| {
                        let result = core.open_file(&path).or_else(|_| core.open_any_file(&path));
                        cx.notify();
                        result
                    });
                    match opened {
                        Ok(id) => {
                            crate::workbench::open_buffer(id, window, cx);
                            if let (Some(line), Some(editor)) =
                                (line, crate::workbench::active_editor(cx))
                            {
                                let position = lsp_types::Position::new(
                                    line.saturating_sub(1),
                                    column.unwrap_or(1).saturating_sub(1),
                                );
                                editor.update(cx, |editor, cx| editor.go_to(position, window, cx));
                            }
                        }
                        Err(err) => window.push_notification(
                            format!("Cannot open {}: {err}", path.display()),
                            cx,
                        ),
                    }
                });
            }
        }
    }

    // ---- find -----------------------------------------------------------

    fn open_find(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(find) = &self.find {
            find.input.update(cx, |input, cx| input.focus(window, cx));
            return;
        }
        let input = cx.new(|cx| InputState::new(window, cx).placeholder("Find"));
        let _events = cx.subscribe_in(&input, window, |this, _, event, _, cx| match event {
            InputEvent::PressEnter { shift, .. } => this.find_next(!*shift, cx),
            InputEvent::Change => {
                if let Some(find) = &mut this.find {
                    find.current = None;
                }
                this.find_next(true, cx);
            }
            _ => {}
        });
        // Deferred: a menu that opened this gives focus back when it closes.
        let focus = input.clone();
        window.defer(cx, move |window, cx| {
            focus.update(cx, |input, cx| input.focus(window, cx))
        });
        self.find = Some(FindBar {
            input,
            current: None,
            _events,
        });
        cx.notify();
    }

    fn close_find(&mut self, _: &CloseFind, window: &mut Window, cx: &mut Context<Self>) {
        self.find = None;
        self.with_terminal(cx, |terminal| terminal.clear_selection());
        self.focus_handle.focus(window, cx);
        cx.notify();
    }

    /// Shows the next match, older (`up`) or newer, by selecting it.
    fn find_next(&mut self, up: bool, cx: &mut Context<Self>) {
        let Some(find) = &mut self.find else {
            return;
        };
        let text = find.input.read(cx).value().to_string();
        let Some(id) = self.terminal else {
            return;
        };
        let Some(term) = self
            .core
            .read(cx)
            .terminals
            .get(id)
            .map(|t| t.term().clone())
        else {
            return;
        };
        let mut term = term.lock();
        if text.is_empty() {
            term.selection = None;
            find.current = None;
            drop(term);
            cx.notify();
            return;
        }
        let pattern = if text.chars().any(char::is_uppercase) {
            regex_escape(&text)
        } else {
            format!("(?i){}", regex_escape(&text))
        };
        let Ok(mut regex) = RegexSearch::new(&pattern) else {
            return;
        };
        let origin = match (find.current, up) {
            (Some((start, _)), true) => start,
            (Some((_, end)), false) => end,
            (None, _) => GridPoint::new(Line(term.screen_lines() as i32 - 1), term.last_column()),
        };
        let direction = if up {
            Direction::Left
        } else {
            Direction::Right
        };
        let side = if up { Side::Left } else { Side::Right };
        // Step off the current match so the search moves on.
        let origin = if find.current.is_some() {
            if up {
                origin.sub(
                    &*term,
                    carbon_core::terminal::alacritty_terminal::index::Boundary::None,
                    1,
                )
            } else {
                origin.add(
                    &*term,
                    carbon_core::terminal::alacritty_terminal::index::Boundary::None,
                    1,
                )
            }
        } else {
            origin
        };
        match term.search_next(&mut regex, origin, direction, side, None) {
            Some(found) => {
                let (start, end) = (*found.start(), *found.end());
                let mut selection = Selection::new(SelectionType::Simple, start, Side::Left);
                selection.update(end, Side::Right);
                term.selection = Some(selection);
                term.scroll_to_point(start);
                find.current = Some((start, end));
            }
            None => term.selection = None,
        }
        drop(term);
        cx.notify();
    }

    fn render_find(&self, cx: &mut Context<Self>) -> Option<impl IntoElement> {
        let find = self.find.as_ref()?;
        let theme = cx.theme();
        Some(
            h_flex()
                .key_context(FIND_CONTEXT)
                .on_action(cx.listener(Self::close_find))
                .absolute()
                .top(fixed::TERMINAL_PADDING_TOP)
                .right(fixed::TERMINAL_SCROLLBAR_WIDTH * 2.)
                .w(px(280.))
                .gap_1()
                .p_1()
                .rounded(theme.radius)
                .border_1()
                .border_color(theme.border)
                .bg(theme.popover)
                .child(Input::new(&find.input).small().flex_1())
                .child(
                    Button::new("terminal-find-up")
                        .ghost()
                        .xsmall()
                        .icon(IconName::ChevronUp)
                        .on_click(cx.listener(|this, _, _, cx| this.find_next(true, cx))),
                )
                .child(
                    Button::new("terminal-find-down")
                        .ghost()
                        .xsmall()
                        .icon(IconName::ChevronDown)
                        .on_click(cx.listener(|this, _, _, cx| this.find_next(false, cx))),
                )
                .child(
                    Button::new("terminal-find-close")
                        .ghost()
                        .xsmall()
                        .icon(IconName::Close)
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.close_find(&CloseFind, window, cx)
                        })),
                ),
        )
    }
}

/// What a Ctrl+click in the terminal opens.
#[derive(Clone, Debug, PartialEq)]
enum Link {
    Url(String),
    File {
        path: PathBuf,
        line: Option<u32>,
        column: Option<u32>,
    },
}

impl Link {
    fn parse(word: &str, cwd: Option<&Path>) -> Option<Self> {
        let word = word.trim_end_matches(['.', ',', ';', ')', ']', '}', '\'', '"']);
        if word.starts_with("http://") || word.starts_with("https://") {
            return Some(Link::Url(word.to_string()));
        }
        // path, path:line, path:line:col (also "path(line,col)" from MSBuild).
        let (path, line, column) = split_position(word);
        if path.is_empty() {
            return None;
        }
        let path = PathBuf::from(path);
        let path = if path.is_absolute() {
            path
        } else {
            cwd?.join(path)
        };
        path.is_file().then_some(Link::File { path, line, column })
    }
}

fn split_position(word: &str) -> (&str, Option<u32>, Option<u32>) {
    if let Some(open) = word.rfind('(')
        && word.ends_with(')')
    {
        let inside = &word[open + 1..word.len() - 1];
        let mut parts = inside.split(',').map(|part| part.trim().parse().ok());
        if let Some(Some(line)) = parts.next() {
            return (&word[..open], Some(line), parts.next().flatten());
        }
    }
    let mut parts = word.rsplitn(3, ':');
    let last = parts.next().unwrap_or_default();
    let middle = parts.next();
    let first = parts.next();
    match (first, middle, last.parse::<u32>()) {
        (Some(path), Some(line), Ok(column)) if line.parse::<u32>().is_ok() => {
            (path, line.parse().ok(), Some(column))
        }
        (_, Some(_), Ok(line)) => (&word[..word.len() - last.len() - 1], Some(line), None),
        _ => (word, None, None),
    }
}

/// The run of non-space characters around `col` (quotes and brackets end it).
fn word_around(text: &[char], col: usize) -> Option<(usize, usize)> {
    let stop = |c: &char| c.is_whitespace() || matches!(c, '"' | '\'' | '<' | '>' | '`' | '|');
    if col >= text.len() || stop(&text[col]) {
        return None;
    }
    let start = text[..col].iter().rposition(stop).map_or(0, |i| i + 1);
    let end = text[col..]
        .iter()
        .position(stop)
        .map_or(text.len(), |i| col + i);
    Some((start, end))
}

fn regex_escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        if "\\.+*?()|[]{}^$#&-~".contains(c) {
            out.push('\\');
        }
        out.push(c);
    }
    out
}

/// Registers terminal panels for saved layouts, the find bar's keys and the
/// keystroke interceptor that sends keys to a focused terminal.
pub fn register(cx: &mut App) {
    cx.bind_keys([KeyBinding::new("escape", CloseFind, Some(FIND_CONTEXT))]);
    register_panel(cx, PANEL_NAME, |context, _, cx| {
        let (profile, label, cwd) = match context.info() {
            PanelInfo::Panel(info) => (
                info.get("profile")
                    .and_then(|p| p.as_str())
                    .and_then(shells::profile_named),
                info.get("label")
                    .and_then(|p| p.as_str())
                    .map(|label| SharedString::from(label.to_string())),
                info.get("cwd")
                    .and_then(|p| p.as_str())
                    .map(PathBuf::from)
                    .filter(|path| path.is_dir()),
            ),
            _ => (None, None, None),
        };
        panel_handle(TerminalPanel::new(profile, label, cwd, cx))
    });
    cx.intercept_keystrokes(|event, window, cx| {
        if !event
            .context_stack
            .iter()
            .any(|context| context.contains(KEY_CONTEXT))
            || input::app_keeps(&event.keystroke)
        {
            return;
        }
        let Some(panel) = cx
            .try_global::<ActiveTerminal>()
            .and_then(|active| active.0.upgrade())
        else {
            return;
        };
        if !panel.read(cx).focus_handle.is_focused(window) {
            return;
        }
        let keystroke = event.keystroke.clone();
        if panel.update(cx, |panel, cx| panel.key(&keystroke, window, cx)) {
            cx.stop_propagation();
        }
    })
    .detach();
}

/// The tab bar's "..." menu dispatches above the panel, so the app root
/// forwards its actions to the last focused terminal.
pub fn on_root_action(action: &dyn Action, window: &mut Window, cx: &mut App) {
    let Some(panel) = cx
        .try_global::<ActiveTerminal>()
        .and_then(|active| active.0.upgrade())
    else {
        return;
    };
    panel.update(cx, |panel, cx| {
        if action.as_any().is::<Find>() {
            panel.open_find(window, cx);
        } else if action.as_any().is::<Restart>() {
            panel.restart(&Restart, window, cx);
        }
    });
}

/// The terminal a new one takes its folder from: the last focused.
pub fn active_terminal_cwd(cx: &App) -> Option<PathBuf> {
    let panel = cx.try_global::<ActiveTerminal>()?.0.upgrade()?;
    panel.read(cx).cwd(cx)
}

impl EntityInputHandler for TerminalPanel {
    fn text_for_range(
        &mut self,
        _: Range<usize>,
        _: &mut Option<Range<usize>>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<String> {
        None
    }

    fn selected_text_range(
        &mut self,
        _: bool,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        Some(UTF16Selection {
            range: 0..0,
            reversed: false,
        })
    }

    fn marked_text_range(&self, _: &mut Window, _: &mut Context<Self>) -> Option<Range<usize>> {
        None
    }

    fn unmark_text(&mut self, _: &mut Window, _: &mut Context<Self>) {}

    /// Typed text (and IME results): straight to the shell.
    fn replace_text_in_range(
        &mut self,
        _: Option<Range<usize>>,
        text: &str,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.exited.is_none() && !text.is_empty() {
            self.send(text.as_bytes(), cx);
        }
    }

    /// IME composition is not shown in tier 1; the result arrives through
    /// `replace_text_in_range`.
    fn replace_and_mark_text_in_range(
        &mut self,
        _: Option<Range<usize>>,
        _: &str,
        _: Option<Range<usize>>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) {
    }

    /// The IME window opens at the cursor.
    fn bounds_for_range(
        &mut self,
        _: Range<usize>,
        _: Bounds<Pixels>,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        let geometry = self.geometry.get()?;
        let cursor =
            self.with_terminal(cx, |terminal| terminal.term().lock().grid().cursor.point)?;
        let origin = geometry.origin
            + point(
                geometry.cell.width * cursor.column.0 as f32,
                geometry.cell.height * cursor.line.0 as f32,
            );
        Some(Bounds::new(origin, geometry.cell))
    }

    fn character_index_for_point(
        &mut self,
        _: Point<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<usize> {
        None
    }
}

impl EventEmitter<PanelEvent> for TerminalPanel {}

impl Focusable for TerminalPanel {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl BasePanel for TerminalPanel {
    fn panel_name(&self) -> &'static str {
        PANEL_NAME
    }

    /// Closing the tab ends the shell.
    fn on_removed(&mut self, _: &mut Window, cx: &mut Context<Self>) {
        self.stop(cx);
    }

    fn dump(&self, cx: &App) -> PanelState {
        let mut state = PanelState::new(PANEL_NAME);
        state.info = PanelInfo::panel(serde_json::json!({
            "profile": self.profile.as_ref().map(|profile| profile.name.clone()),
            "label": self.label.as_ref(),
            "cwd": self.cwd(cx),
        }));
        state
    }
}

impl Panel for TerminalPanel {
    fn tab_name(&self, _: &App) -> Option<SharedString> {
        Some(self.label.clone())
    }

    fn title(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        self.label.clone()
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

    fn dropdown_menu(
        &mut self,
        mut menu: PopupMenu,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> PopupMenu {
        for profile in shells::profiles() {
            menu = menu.menu(
                format!("New {}", profile.name),
                Box::new(NewTerminal {
                    profile: profile.name.into(),
                }),
            );
        }
        menu.separator()
            .menu("Find", Box::new(Find))
            .menu("Restart", Box::new(Restart))
    }
}

impl Render for TerminalPanel {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let focused = self.focus_handle.is_focused(window);
        if focused {
            cx.set_global(ActiveTerminal(cx.weak_entity()));
        }
        let theme = cx.theme();
        let palette = Palette::new(theme);
        let font = Font {
            family: theme.mono_font_family.clone(),
            ..font(theme.mono_font_family.clone())
        };
        let font_size = theme.mono_font_size;
        let muted = theme.muted_foreground;

        let base = div()
            .id("terminal")
            .track_focus(&self.focus_handle)
            .key_context(KEY_CONTEXT)
            .on_action(cx.listener(Self::restart))
            .on_action(cx.listener(|this, _: &Find, window, cx| this.open_find(window, cx)))
            .on_action(cx.listener(|this, _: &kit_input::Copy, _, cx| {
                this.copy(cx);
            }))
            .on_action(cx.listener(|this, _: &kit_input::Paste, _, cx| this.paste(cx)))
            .relative()
            .size_full()
            .bg(palette.background);

        if let Some(error) = &self.error {
            return base.child(
                v_flex()
                    .size_full()
                    .items_center()
                    .justify_center()
                    .gap(fixed::PANEL_GAP)
                    .child(error.clone())
                    .child(
                        div()
                            .text_sm()
                            .text_color(muted)
                            .child("Restart from the \"...\" menu."),
                    ),
            );
        }

        let entity = cx.entity();
        let core = self.core.clone();
        let terminal = self.terminal;
        let geometry_cell = self.geometry.clone();
        let link = self.link;
        let focus = self.focus_handle.clone();
        let exited = self.exited;
        base.cursor(CursorStyle::IBeam)
            .on_mouse_down(MouseButton::Left, cx.listener(Self::mouse_down))
            .on_mouse_move(cx.listener(Self::mouse_move))
            .on_mouse_up(MouseButton::Left, cx.listener(Self::mouse_up))
            .on_scroll_wheel(cx.listener(Self::scroll_wheel))
            .child(
                canvas(
                    {
                        let font = font.clone();
                        let palette = palette.clone();
                        move |bounds, window, cx| {
                            let geometry = Geometry::measure(bounds, &font, font_size, window);
                            geometry_cell.set(Some(geometry));
                            let core = core.read(cx);
                            let terminal = terminal.and_then(|id| core.terminals.get(id))?;
                            let size = terminal.size();
                            if (size.cols, size.rows) != (geometry.cols, geometry.rows) {
                                let cell = geometry.cell;
                                _ = terminal.resize_cells(
                                    geometry.cols,
                                    geometry.rows,
                                    f32::from(cell.width) as u16,
                                    f32::from(cell.height) as u16,
                                );
                            }
                            let marks = terminal.command_marks();
                            let term = terminal.term().lock();
                            Some((geometry, Snapshot::take(&term, &palette, &marks, link)))
                        }
                    },
                    move |bounds, state, window, cx| {
                        let Some((geometry, snapshot)) = state else {
                            return;
                        };
                        let focused = focus.is_focused(window);
                        snapshot.paint(
                            bounds, &geometry, &palette, &font, font_size, focused, window, cx,
                        );
                        window.handle_input(&focus, ElementInputHandler::new(bounds, entity), cx);
                    },
                )
                .size_full(),
            )
            .when_some(exited, |this, code| {
                let text = match code {
                    Some(code) => {
                        format!("The shell exited with code {code}. Press Enter to restart.")
                    }
                    None => "The shell exited. Press Enter to restart.".to_string(),
                };
                this.child(
                    div()
                        .absolute()
                        .bottom(fixed::TERMINAL_PADDING_TOP)
                        .left(fixed::TERMINAL_GUTTER)
                        .text_sm()
                        .text_color(muted)
                        .child(text),
                )
            })
            .children(self.render_find(cx))
    }
}

#[cfg(test)]
mod tests {
    use super::{split_position, word_around};

    #[test]
    fn positions_in_paths() {
        assert_eq!(split_position("src/main.rs"), ("src/main.rs", None, None));
        assert_eq!(
            split_position("src/main.rs:12"),
            ("src/main.rs", Some(12), None)
        );
        assert_eq!(
            split_position("src/main.rs:12:5"),
            ("src/main.rs", Some(12), Some(5))
        );
        assert_eq!(
            split_position(r"C:\a\b.rs:3:1"),
            (r"C:\a\b.rs", Some(3), Some(1))
        );
        assert_eq!(split_position(r"C:\a\b.rs"), (r"C:\a\b.rs", None, None));
        assert_eq!(split_position("b.cs(10,4)"), ("b.cs", Some(10), Some(4)));
    }

    #[test]
    fn words_end_at_spaces_and_quotes() {
        let text: Vec<char> = "error at \"src/a.rs:3\" now".chars().collect();
        assert_eq!(word_around(&text, 12), Some((10, 20)));
        assert_eq!(word_around(&text, 5), None);
    }
}
