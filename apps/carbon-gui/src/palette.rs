//! Command palette and Quick Open (FEATURE WB-01), one input like VS Code's:
//! no prefix finds files, `>` runs commands, `:` goes to a line, `?` lists
//! the modes. Drawn by `CarbonApp` under the title bar, Zed style: no
//! backdrop, closes on Esc, a click outside, or focus leaving it.
//!
//! The rows run nothing themselves. Confirming gives focus back to where it
//! was when the palette opened and only then dispatches, so commands that
//! need a focused editor or Explorer reach them.

mod files;
mod query;
mod rank;
mod row;
mod usage;

use std::collections::HashSet;
use std::ops::Range;
use std::rc::Rc;

use carbon_core::{Core, fuzzy};
use gpui_kit::base::actions::Cancel;
use gpui_kit::component::command::{Command, CommandGroup, CommandItem, CommandState};
use gpui_kit::component::{ActiveTheme, Disableable as _, IndexPath};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::commands::{self, CommandSpec};
use crate::menus::MenuState;
use crate::style::fixed;
pub use files::FileIndex;
use query::Mode;

/// Most rows a query shows (every row is laid out on each render).
const MAX_ROWS: usize = 100;
/// Commands in "Recently used" on an empty `>` query.
const RECENT_COUNT: usize = 5;
const KEY_CONTEXT: &str = "CarbonPalette";

actions!(palette, [ClosePalette]);

/// Loads command usage and binds the palette's own keys. Call at startup.
pub fn register(cx: &mut App) {
    usage::register(cx);
    // Fallback for Esc should the query input and the list both pass on it.
    cx.bind_keys([KeyBinding::new("escape", ClosePalette, Some(KEY_CONTEXT))]);
}

enum Row {
    Command {
        spec: Rc<CommandSpec>,
        /// Matched bytes of the label.
        ranges: Rc<[Range<usize>]>,
    },
    File {
        path: SharedString,
        positions: Rc<[u32]>,
    },
    /// 1-based.
    Line {
        line: u32,
        column: Option<u32>,
    },
    Mode(Mode),
    Info(SharedString),
}

struct Section {
    heading: Option<SharedString>,
    rows: Vec<Row>,
}

impl Section {
    fn plain(rows: Vec<Row>) -> Self {
        Self {
            heading: None,
            rows,
        }
    }

    fn info(text: impl Into<SharedString>) -> Vec<Self> {
        vec![Self::plain(vec![Row::Info(text.into())])]
    }
}

/// The active editor when the palette opened (1-based), for Go to Line.
#[derive(Clone, Copy)]
struct EditorAt {
    line: u32,
    column: u32,
    lines: u32,
}

pub struct Palette {
    focus_handle: FocusHandle,
    state: Entity<CommandState>,
    /// Focus before the palette opened: actions run there, Esc returns there.
    prev_focus: Option<FocusHandle>,
    core: Entity<Core>,
    files: Entity<FileIndex>,
    /// Commands that can run where focus was, menus first.
    commands: Vec<Rc<CommandSpec>>,
    editor: Option<EditorAt>,
    mode: Mode,
    sections: Vec<Section>,
    /// A file search is running in the background.
    searching: bool,
    /// Bumped per query, so a late file search result is dropped.
    query_generation: u64,
    _search: Task<()>,
    dismissed: bool,
    _subscriptions: Vec<Subscription>,
}

impl EventEmitter<DismissEvent> for Palette {}

impl Focusable for Palette {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Palette {
    /// Opens with `query` (a mode prefix) typed. Call while focus is still
    /// where the user was.
    pub fn new(
        query: &str,
        menu: &MenuState,
        core: Entity<Core>,
        files: Entity<FileIndex>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let prev_focus = window.focused(cx);
        // `available_actions` drops actions it cannot rebuild from a name
        // (those with fields), so it only adds the app-wide listeners (Quit);
        // the focus path is asked per action.
        let global: HashSet<&'static str> = window
            .available_actions(cx)
            .iter()
            .map(|action| action.name())
            .collect();
        let commands = commands::all(menu)
            .into_iter()
            .filter(|spec| {
                !spec.disabled
                    && (window.is_action_available(spec.action.as_ref(), cx)
                        || global.contains(spec.action.name()))
            })
            .map(Rc::new)
            .collect();
        let editor = crate::workbench::active_editor(cx)
            .and_then(|editor| editor.read(cx).cursor_and_lines(cx))
            .map(|(cursor, lines)| EditorAt {
                line: cursor.line + 1,
                column: cursor.character + 1,
                lines: lines.max(1) as u32,
            });
        files.update(cx, |files, cx| files.refresh(&core, cx));

        let focus_handle = cx.focus_handle();
        let state = cx.new(|cx| CommandState::new(window, cx));
        let _subscriptions = vec![
            // Tab or a click that focuses something else: close, leave focus there.
            cx.on_focus_out(&focus_handle, window, |this, _, window, cx| {
                this.dismiss(false, window, cx)
            }),
            // A new file list came in.
            cx.observe(&files, |this, _, cx| {
                if this.mode == Mode::Files {
                    this.requery(cx);
                }
            }),
        ];
        state.update(cx, |state, cx| {
            state.set_query(query.to_string(), window, cx);
            state.focus(window, cx);
        });

        let mut this = Self {
            focus_handle,
            state,
            prev_focus,
            core,
            files,
            commands,
            editor,
            mode: Mode::Files,
            sections: Vec::new(),
            searching: false,
            query_generation: 0,
            _search: Task::ready(()),
            dismissed: false,
            _subscriptions,
        };
        this.update_query(query, cx);
        this
    }

    /// Replaces the query (a mode prefix), as when a palette key is pressed
    /// while it is open.
    pub fn set_query(&mut self, query: &str, window: &mut Window, cx: &mut Context<Self>) {
        self.state.update(cx, |state, cx| {
            state.set_query(query.to_string(), window, cx)
        });
    }

    fn requery(&mut self, cx: &mut Context<Self>) {
        let query = self.state.read(cx).query(cx);
        self.update_query(&query, cx);
    }

    fn update_query(&mut self, query: &str, cx: &mut Context<Self>) {
        let (mode, rest) = query::parse(query);
        let previous = std::mem::replace(&mut self.mode, mode);
        self.query_generation += 1;
        self.searching = false;
        self._search = Task::ready(());
        self.sections = match mode {
            Mode::Commands => self.command_sections(rest, cx),
            Mode::Files => self.file_sections(rest, previous == Mode::Files, cx),
            Mode::Line => self.line_sections(rest),
            Mode::Help => vec![Section::plain(
                Mode::ALL
                    .into_iter()
                    .filter(|mode| *mode != Mode::Help)
                    .map(Row::Mode)
                    .collect(),
            )],
            Mode::Symbols | Mode::WorkspaceSymbols => {
                Section::info("Symbol search needs language-server symbols: coming soon")
            }
        };
        cx.notify();
    }

    fn command_sections(&self, query: &str, cx: &App) -> Vec<Section> {
        let usage = usage::get(cx);
        let row = |spec: &Rc<CommandSpec>, ranges: Vec<Range<usize>>| Row::Command {
            spec: spec.clone(),
            ranges: ranges.into(),
        };
        if query.is_empty() {
            let recent: Vec<&Rc<CommandSpec>> = usage
                .recent(usize::MAX)
                .into_iter()
                .filter_map(|id| self.commands.iter().find(|spec| spec.id.as_ref() == id))
                .take(RECENT_COUNT)
                .collect();
            let mut sections = Vec::new();
            if !recent.is_empty() {
                sections.push(Section {
                    heading: Some("Recently used".into()),
                    rows: recent.iter().map(|spec| row(spec, Vec::new())).collect(),
                });
            }
            for spec in &self.commands {
                if recent.iter().any(|used| used.id == spec.id) {
                    continue;
                }
                match sections.last_mut() {
                    Some(section) if section.heading.as_ref() == Some(&spec.group) => {
                        section.rows.push(row(spec, Vec::new()))
                    }
                    _ => sections.push(Section {
                        heading: Some(spec.group.clone()),
                        rows: vec![row(spec, Vec::new())],
                    }),
                }
            }
            return sections;
        }

        let candidates: Vec<rank::Candidate> = self
            .commands
            .iter()
            .map(|spec| rank::Candidate {
                label: &spec.label,
                keywords: spec.keywords,
                usage: usage.count(&spec.id),
            })
            .collect();
        let rows = rank::rank(query, &candidates)
            .into_iter()
            .take(MAX_ROWS)
            .map(|ranked| {
                let spec = &self.commands[ranked.index];
                row(spec, fuzzy::byte_ranges(&spec.label, &ranked.positions))
            })
            .collect();
        vec![Section::plain(rows)]
    }

    /// `keep`: the shown rows are files, so they stay while a search runs.
    fn file_sections(&mut self, query: &str, keep: bool, cx: &mut Context<Self>) -> Vec<Section> {
        let core = self.core.read(cx);
        let Some(root) = core
            .workspace()
            .map(|workspace| workspace.root().to_path_buf())
        else {
            return Section::info("Open a folder to search its files");
        };
        let index = self.files.read(cx);
        let files = index.files.clone();
        if query.is_empty() {
            let open: Vec<String> = core
                .buffers
                .ids()
                .filter_map(|id| core.buffers.get(id)?.path()?.strip_prefix(&root).ok())
                .map(|path| {
                    let parts: Vec<_> = path
                        .components()
                        .map(|part| part.as_os_str().to_string_lossy())
                        .collect();
                    parts.join("/")
                })
                .collect();
            let file_row = |path: &str| Row::File {
                path: path.to_string().into(),
                positions: Rc::from([]),
            };
            let mut sections = Vec::new();
            if !open.is_empty() {
                sections.push(Section {
                    heading: Some("Open editors".into()),
                    rows: open.iter().map(|path| file_row(path)).collect(),
                });
            }
            let rest: Vec<Row> = files
                .iter()
                .filter(|path| !open.contains(path))
                .take(MAX_ROWS)
                .map(|path| file_row(path))
                .collect();
            if !rest.is_empty() {
                sections.push(Section {
                    heading: Some("Files".into()),
                    rows: rest,
                });
            }
            if sections.is_empty() && index.scanning {
                return Section::info("Listing files…");
            }
            return sections;
        }

        // Match off the UI thread; the old rows stay until the answer is in.
        let generation = self.query_generation;
        let query = query.to_string();
        self.searching = true;
        self._search = cx.spawn(async move |this, cx| {
            let found = cx
                .background_spawn(async move { files::match_files(&query, &files, MAX_ROWS) })
                .await;
            _ = this.update(cx, |this, cx| {
                if this.query_generation != generation {
                    return;
                }
                this.searching = false;
                this.sections = vec![Section::plain(
                    found
                        .into_iter()
                        .map(|found| Row::File {
                            path: found.path.into(),
                            positions: found.positions.into(),
                        })
                        .collect(),
                )];
                cx.notify();
            });
        });
        if keep {
            std::mem::take(&mut self.sections)
        } else {
            Vec::new()
        }
    }

    fn line_sections(&self, query: &str) -> Vec<Section> {
        let Some(at) = self.editor else {
            return Section::info("Open a file to go to a line");
        };
        if query.is_empty() {
            return Section::info(format!(
                "Current line {}, column {}. Type a line number between 1 and {}",
                at.line, at.column, at.lines
            ));
        }
        match query::parse_line(query) {
            Some((line, column)) => vec![Section::plain(vec![Row::Line {
                line: line.min(at.lines),
                column,
            }])],
            None => Section::info(format!("Type a line number between 1 and {}", at.lines)),
        }
    }

    fn confirm(&mut self, ix: IndexPath, window: &mut Window, cx: &mut Context<Self>) {
        let Some(row) = self
            .sections
            .get(ix.section)
            .and_then(|section| section.rows.get(ix.row))
        else {
            return;
        };
        match row {
            Row::Mode(mode) => {
                if mode.available() {
                    let prefix = mode.prefix();
                    self.set_query(prefix, window, cx);
                }
            }
            Row::Info(_) => {}
            Row::Command { spec, .. } => {
                let action = spec.action.boxed_clone();
                let id = spec.id.clone();
                self.dismiss(true, window, cx);
                usage::record(&id, cx);
                // Dispatches from the focus just restored.
                window.dispatch_action(action, cx);
            }
            Row::File { path, .. } => {
                let Some(root) = self
                    .core
                    .read(cx)
                    .workspace()
                    .map(|w| w.root().to_path_buf())
                else {
                    return;
                };
                let path = root.join(path.as_ref());
                self.dismiss(true, window, cx);
                crate::workbench::open_file_at(&path, window, cx);
            }
            &Row::Line { line, column } => {
                self.dismiss(true, window, cx);
                if let Some(editor) = crate::workbench::active_editor(cx) {
                    let position = gpui_kit::component::input::Position::new(
                        line - 1,
                        column.unwrap_or(1) - 1,
                    );
                    editor.update(cx, |editor, cx| {
                        editor.go_to(position, window, cx);
                        editor.focus_editor(window, cx);
                    });
                }
            }
        }
    }

    /// Closes once; `restore` gives focus back to where it was.
    fn dismiss(&mut self, restore: bool, window: &mut Window, cx: &mut Context<Self>) {
        if self.dismissed {
            return;
        }
        self.dismissed = true;
        if restore && let Some(focus) = &self.prev_focus {
            focus.focus(window, cx);
        }
        cx.emit(DismissEvent);
    }

    /// A click outside: after it is handled, close; give focus back only if
    /// the click did not move it somewhere.
    fn click_out(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let this = cx.entity().downgrade();
        window.defer(cx, move |window, cx| {
            _ = this.update(cx, |this, cx| {
                let restore =
                    this.focus_handle.contains_focused(window, cx) || window.focused(cx).is_none();
                this.dismiss(restore, window, cx);
            });
        });
    }

    fn item(&self, row: &Row) -> CommandItem {
        let item = CommandItem::new();
        match row {
            Row::Command { spec, ranges } => {
                let (spec, ranges) = (spec.clone(), ranges.clone());
                let focus = self.prev_focus.clone();
                item.label(spec.label.clone()).child(move |window, cx| {
                    row::command(&spec, &ranges, focus.as_ref(), window, cx)
                })
            }
            Row::File { path, positions } => {
                let (path, positions) = (path.clone(), positions.clone());
                item.label(path.clone()).child(move |_, cx| {
                    let ((dir, dir_positions), (name, name_positions)) =
                        files::split_path(&path, &positions);
                    row::file(
                        (name, fuzzy::byte_ranges(name, &name_positions)),
                        (dir, fuzzy::byte_ranges(dir, &dir_positions)),
                        cx,
                    )
                })
            }
            &Row::Line { line, column } => item.child(move |_, cx| row::line(line, column, cx)),
            &Row::Mode(mode) => item
                .label(mode.title())
                .disabled(!mode.available())
                .child(move |_, cx| row::mode(mode, cx)),
            Row::Info(text) => {
                let text = text.clone();
                item.disabled(true)
                    .child(move |_, cx| row::info(text.clone(), cx))
            }
        }
    }

    fn footer_note(&self, cx: &App) -> Option<SharedString> {
        match self.mode {
            Mode::Files => {
                let index = self.files.read(cx);
                if index.scanning {
                    Some("listing files…".into())
                } else if index.truncated {
                    Some(format!("first {} files", index.files.len()).into())
                } else if self.core.read(cx).workspace().is_some() {
                    Some(format!("{} files · > commands · ? help", index.files.len()).into())
                } else {
                    Some("> commands · ? help".into())
                }
            }
            Mode::Commands => Some(format!("{} commands", self.commands.len()).into()),
            Mode::Help => None,
            _ => Some("? help".into()),
        }
    }
}

impl Render for Palette {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let this = cx.entity().downgrade();
        let empty: SharedString = match self.mode {
            _ if self.searching => "Searching…".into(),
            Mode::Commands => "No matching commands".into(),
            Mode::Files => "No matching files".into(),
            _ => "Nothing here".into(),
        };
        let note = self.footer_note(cx);
        let mut command = Command::new(&self.state)
            .filterable(false)
            .max_h(fixed::PALETTE_LIST_MAX_HEIGHT)
            .placeholder(self.mode.placeholder())
            .on_query({
                let this = this.clone();
                move |query, _, cx| {
                    _ = this.update(cx, |this, cx| this.update_query(query, cx));
                }
            })
            .on_confirm(move |ix, window, cx| {
                _ = this.update(cx, |this, cx| this.confirm(ix, window, cx));
            })
            .empty(move |_, _, cx| row::empty(empty.clone(), cx))
            .footer(move |_, _, cx| row::footer(note.clone(), cx));
        for section in &self.sections {
            let mut group = CommandGroup::new();
            if let Some(heading) = &section.heading {
                group = group.label(heading.clone());
            }
            command = command.group(group.items(section.rows.iter().map(|row| self.item(row))));
        }

        let palette = div()
            .id("carbon-palette")
            .key_context(KEY_CONTEXT)
            .track_focus(&self.focus_handle)
            .occlude()
            .w_full()
            // Close on the first Esc, even with text typed (the kit's
            // Cancel would clear the query first).
            .capture_action(cx.listener(|this, _: &Cancel, window, cx| {
                cx.stop_propagation();
                this.dismiss(true, window, cx);
            }))
            .on_action(
                cx.listener(|this, _: &ClosePalette, window, cx| this.dismiss(true, window, cx)),
            )
            .on_mouse_down_out(cx.listener(|this, _, window, cx| this.click_out(window, cx)))
            .when(cx.theme().shadow, |this| this.shadow_lg())
            .rounded(cx.theme().radius_lg)
            .child(command);
        if cx.reduce_motion() {
            return palette.into_any_element();
        }
        palette
            .with_animation(
                "carbon-palette-open",
                Animation::new(fixed::PALETTE_FADE).with_easing(ease_out_quint()),
                |palette, delta| {
                    palette
                        .opacity(delta)
                        .mt(fixed::PALETTE_FADE_TRAVEL * (delta - 1.))
                },
            )
            .into_any_element()
    }
}
