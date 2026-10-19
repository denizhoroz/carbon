# Command palette and Quick Open

Built 2026-10-19. User request: a slick command palette on top of the app, after looking at what others did. Compared: Superhuman, Linear, Raycast, VS Code, Retool, Warp, Zed (same GPUI framework, closest model). User picked: Ctrl+Shift+P / F1 (Ctrl+K stays a chord prefix), commands + Quick Open in one input with VS Code prefixes, `nucleo-matcher` fuzzy matching with usage ranking, Zed/VS Code look (top-anchored, no backdrop).

## What others do (2026-10-19)

| Product | Idea Carbon took |
|---|---|
| Zed | Commands = everything that can run where focus was; fuzzy via nucleo; used commands tie-break first; matched letters highlighted; shortcut looked up for the previous focus; confirm = give focus back, then dispatch |
| VS Code | One input, prefix picks the mode (`>` `:` `?` `@` `#`); Ctrl+Shift+P, F1, Ctrl+P, Ctrl+G |
| Superhuman, Linear | Shortcut on every row (the palette teaches keys); synonyms; "Recently used" before typing; an icon per command |
| Raycast, Retool | Esc leaves at once; groups with headings; a top result first |

## Behaviour

| Input | Result |
|---|---|
| Ctrl+Shift+P, F1, View > Command Palette | Opens with `>`: every command that can run where focus was, grouped by menu (File, Edit, View, Help, Editor, Terminal, Explorer), "Recently used" (5) on top |
| Ctrl+P, View > Go to File... | Opens empty: open editors, then workspace files; typing fuzzy-matches paths (file name scores higher) |
| Ctrl+G, View > Go to Line... | Opens with `:`: shows the current line/column and line count; `:42`, `:42:7`, `:42,7` go there (clamped to the last line) |
| `?` | Lists the modes; Enter on one switches to it. `@` and `#` (symbols) say "coming soon" |
| Typing | Fuzzy match, fzf scoring; words in any order must all match ("dock left"); smart case; matched letters blue and semibold; synonyms match too ("quit" finds File: Exit) without a highlight |
| Ranking | Best match first; equal matches: more used first. Usage (count, last use) in `target/carbon-gui/command-usage.json` |
| Row | Icon, muted category ("View: "), name, full shortcut on the right (every key of a chord: Ctrl+K Ctrl+O), check mark for on/off items (current section, auto-hide) |
| Enter / click | Focus goes back to where it was, then the command runs there (File: Save saves the focused editor; Explorer: Rename renames the selected row). Files open in an editor tab; Go to line moves the cursor and focuses the editor |
| Up / Down | Move the highlight (wraps, skips message rows) |
| Esc | Closes at once (even with text typed) and gives focus back |
| Click outside, Tab | Closes; the click lands where it was aimed |
| A palette key while open | Switches mode (Ctrl+P → files, Ctrl+G → `:`) |
| Terminal focused | Ctrl+Shift+P, Ctrl+P and F1 open the palette (they no longer reach the shell, as in VS Code); Ctrl+G stays with the shell |

Footer: ↑↓ navigate, ↵ run, esc close; on the right the file count ("first N files" when cut), "listing files…", or the command count.

## Code

| Part | File | What |
|---|---|---|
| Fuzzy matching | core `fuzzy.rs` | `match_list(query, haystacks, paths)` over `nucleo-matcher` (smart case, multi-word, path scoring), `byte_ranges` (char positions → merged byte ranges for highlighting) |
| File list | core `file_index.rs` | `list_files(root, limit)`: recursive walk, same rules as the Explorer (`.gitignore`/`.ignore` with or without git, hidden files kept, `ALWAYS_EXCLUDED` skipped, links not followed), `/` paths, sorted, cut at 100,000 |
| Change counter | core `lib.rs` | `Core::files_generation()`: bumped on open/close workspace and on watcher batches that touch folder listings |
| Registry | gui `commands.rs` | `CommandSpec` (id, label, group, keywords, icon, action, checked, disabled); `all(&MenuState)` = every menu action (`menus::app_menus`, labels "Menu: Item") + palette-only extras (Close Editor, terminal profiles/Find/Restart, dock edge moves, Explorer actions); icons and synonyms by action name |
| Palette view | gui `palette.rs` | `Palette`: mode, sections, confirm (focus restore + dispatch), dismiss (Esc, click out, focus out), animation, footer; hosts the kit `Command` with `filterable(false)` and custom rows |
| Modes | gui `palette/query.rs` | `Mode`, `parse` (prefix), `parse_line` |
| Ranking | gui `palette/rank.rs` | Score first, usage breaks ties; keywords as fallback (weight 3/4) |
| Usage | gui `palette/usage.rs` | `Usage` global, `record`, `recent`, 500 entries max (least recent dropped) |
| Quick Open data | gui `palette/files.rs` | `FileIndex` entity (re-lists in the background when root/generation changed), `match_files` (score, then shorter path, then name), `split_path` |
| Rows | gui `palette/row.rs` | Highlighted text, shortcut chips for the full chord, file/line/mode/info rows, footer, empty message |
| Wiring | `main.rs` (`open_palette`, overlay under the title bar via `deferred(anchored())`, actions), `menus.rs` (`QuickOpen`, `GoToLine`, View items, `app_bindings()`), `terminal_panel/input.rs` (`app_keeps`), `workbench.rs` (`open_file_at`), `editor_panel.rs` (`cursor_and_lines`), `style/fixed.rs` (`PALETTE_*`), `app_assets.rs` (10 Lucide icons) | |
| Deps | root `Cargo.toml` | `nucleo-matcher` 0.3.1 (MPL-2.0, Helix) |

Rule: a menu item is a palette command automatically. A command that is not in a menu goes in `commands::extras`. Every new action needs one or the other (tests check both menus and key bindings).

## Design notes

- Not hosted in the kit `Dialog`: it always tints the window or freezes clicks, adds a title and the Enter-confirm gotcha. `CarbonApp` draws the palette itself (`deferred` + `anchored`), 600 px wide, 6 px under the title bar, `occlude` so clicks do not fall through.
- Rows carry no kit `.action()`: the kit would dispatch from the palette's own focus, so editor and Explorer commands would find no handler. `on_confirm` restores the previous focus, then `window.dispatch_action`.
- Commands are filtered at open time with `window.is_action_available` on the previous focus (plus `available_actions` for app-wide listeners such as Quit): Undo/Cut/Paste only with a text field or the Explorer focused, Explorer commands only from the Explorer, About (no handler) never. `available_actions` alone is not enough: it drops every action with fields (ToggleDock, AddPanel, ...), found in the click test.
- Open is deferred one tick: the in-window menu gives focus back after running its action, so the palette must look later.

## Verified

- Click-tested 2026-10-19 (separate `target/palette-smoke` build, window messages + PrintWindow; layout file restored afterwards): F1 opens under the title bar with `>`, groups, icons, "Ctrl+K Ctrl+O" chord, check on the current section; "sav" and "toggle bottom" highlight matches; Enter ran Toggle Bottom Dock (dock hid), reopening showed it under "Recently used", Enter toggled it back; Esc closed at once with text typed; Backspace to Files mode: 607 files, `target/` absent, "palrank" found `apps/carbon-gui/src/palette/rank.rs` with name and folder highlights, Enter opened it; `:` showed "Current line 1, column 1 ... 1 and 146", `:40:5` jumped there with the editor focused; `?` listed modes with "coming soon", Enter on "Show and Run Commands" switched to `>`; with an Explorer folder focused 53 commands incl. Explorer ones with their Explorer keys (F2, Delete), from the editor none. Not tested: Ctrl+Shift+P / Ctrl+P / Ctrl+G (modifier keys cannot be sent as window messages; F1 runs the same action), click outside, the opening animation, light theme.
- `cargo test --workspace`: 96 pass, 4 ignored (was 67). New: core `fuzzy` (6), `file_index` (2), `files_generation` (1); gui `commands` (6: every menu action once, every bound action has a command, unique ids/labels, labels, checks, profiles), `palette::query` (3), `palette::rank` (5), `palette::usage` (3), `palette::files` (2), `palette::row` (1), terminal `app_keeps` palette keys. Clippy (`-D warnings`) and fmt clean.

## Limits

- `@` / `#` symbol search waits for language-server document/workspace symbols.
- No live preview while typing a line number (moving the cursor would take focus from the palette).
- No query history on Up at the top row (Zed has it).
- At most 100 rows per query; custom rows are laid out on every render (kit `CommandItem` with a custom child is never "unchanged").
- The command list is taken when the palette opens; saving a layout while it is open shows up next time.
- Usage ids include the label: renaming a menu item resets its count.
- File list walk is single-threaded; very large trees list slowly the first time (shown as "listing files…").
