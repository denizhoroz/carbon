# Code editor tier 1 (editing, tabs, syntax colours, language servers)

Built 2026-10-17. User request: build a text/code editor; compare what other editors use (`../../documents/code-editor.md`). User picked: gpui-kit `Editor` for the view with the core buffer as the source of truth; tier 1 = edit + save + tabs, syntax colours, large-file speed check, language servers. rust-analyzer installed with the user's permission (`rustup component add rust-analyzer`); language servers are to become installable language packages later (user).

## Behaviour

| Input | Result |
|---|---|
| Click a file in the Explorer (or Enter) | Opens it in its tab if it has one, else in an empty Editor tab, else a new tab in the center; the editor gets focus |
| Type | Kit editor edits; the core buffer takes the text after each change; tab shows a dot while unsaved |
| Ctrl+S (anywhere; File > Save; tab "..." menu: Save) | Saves the active editor's file (atomic write; line endings kept as on disk, CRLF checked). An untitled file asks where (system Save dialog, starts in the open folder); the tab then takes the file's name, colours and language server |
| Ctrl+N or File > New File | New tab "Untitled-N" (plain text). Not restored after a restart (its tab is dropped) |
| Edit menu: Undo, Redo, Cut, Copy, Paste | Act on the focused editor (the kit's own actions) |
| Quit: File > Exit, Ctrl+Q, window x, Alt+F4 | With unsaved files: dialog naming them, Save (All) / Don't Save / Cancel (Enter = Save). Save All asks a path for untitled files; a cancelled path or a failed save keeps the app open |
| Tabs | Editor files always show as tabs (30 px, close button), one file or several; the empty editor alone in its area has no bar at all (`Panel::title_bar` false; user) and shows "No file open / New file: Ctrl+N". Code starts right under the tabs either way. Since 2026-10-17 every Code Editor panel has tabs the same way (`panel-layout.md` s8) |
| Line numbers | 24 px between the numbers and the code (vendored `gpui-base` patch; the kit only left that room with folding on) |
| Ctrl+W, tab x, or tab "..." menu: Close (the kit's item, routed through `request_close`) | Closes the tab; with unsaved changes asks Save / Don't Save / Cancel (Enter = Save). The last tab clears to the empty editor instead of leaving the area blank |
| File changed on disk by another program | No unsaved changes: reloads quietly (cursor kept). Unsaved changes: bar "This file changed on disk. You have unsaved changes." with Reload / Keep Mine |
| Close Folder / open another folder | Editor tabs stay open (intended, user 2026-10-17) |
| Rename a file in the Explorer | Its tab follows; colours and language server follow the new extension |
| Restart | Editor tabs come back (path saved in the layout). Tabs whose file is not in the open folder are dropped (layouts are not per folder yet); if none is left, the empty editor shows |
| Syntax colours | tree-sitter for rust, toml, json, markdown, yaml, python, javascript, typescript, tsx, html, css, bash (+ c, cpp, go, sql, diff names mapped, grammars not compiled in); dark or light highlight theme follows the app mode |
| `.rs` file in a folder | rust-analyzer starts (status bar right: "rust-analyzer: starting", "rust-analyzer: <work>", "rust-analyzer"; "not installed" / "stopped" with the reason on hover) |
| Errors and warnings | Squiggles in the editor (hover shows the message); Explorer problem markers (counts on files, dots on folders) now come from the server |
| Hover a symbol | Server hover (signature, docs) |
| Type an identifier, `.` or `::` | Completion menu, narrowed to the typed word (items starting with it first) |
| F12 (or right-click > Go to Definition, Ctrl+click) | Jumps in the file, or opens the target file in a tab. Files outside the folder (std sources) open read-only |

## Code

| Part | File | What |
|---|---|---|
| Core buffer | `apps/carbon-core/src/buffer.rs` | `set_rope`, `reload`, `changed_on_disk` (modified time of last load/save), `ignore_disk_change`, `has_disk_conflict`; `BufferStore::find` |
| Core facade | `apps/carbon-core/src/lib.rs` | `save_buffer`, `close_buffer`, `set_active_buffer`, `sync_buffers_with_disk` (reload clean / flag dirty) -> `DiskSync`, `open_any_file`, `lsp_open_buffers`, `lsp_change`, `set_diagnostics` (also sets problem markers); moved files are closed and reopened in the server |
| Watcher | `apps/carbon-core/src/watcher.rs` | `FsChanges.files`: every changed entry outside `.git` |
| LSP client | `apps/carbon-core/src/lsp.rs`, `lsp/transport.rs`, `lsp/uri.rs` | Server table (`SERVERS`: rust -> rust-analyzer, install hint), `find_program` on PATH, `initialize` (client capabilities), `LspStore` (servers, status, progress, open documents with versions, diagnostics with generations), full-text sync, UTF-16 position helpers; transport: stdio JSON-RPC, reader thread answers server requests (`workspace/configuration` -> nulls), stderr tail for errors, no console window on Windows, shutdown + kill on drop |
| GUI editor | `apps/carbon-gui/src/editor_panel.rs` (+ `editor_panel/language.rs`, `editor_panel/lsp.rs`) | One `EditorPanel` per buffer (`PANEL_NAME` "Editor", `dump` saves the path); sync both ways by revision; conflict bar; save/close dialog; LSP hooks (completion with narrowing, hover, definition, `show_document` for other files); diagnostics pushed into the kit's set (UTF-16 -> char columns); read-only outside the folder; F12 binding |
| Saving and quitting | `apps/carbon-gui/src/save_ops.rs` | `save_buffers` (in order; untitled ones ask a path via `prompt_for_new_path`), `quit` / `request_quit` / `can_close` (window `on_window_should_close`, title bar `on_close_window` for Linux) and the unsaved-changes dialog; core `new_file`, `save_buffer_as`, `dirty_buffers`, `Buffer::name` |
| Server lifecycle | `apps/carbon-gui/src/lsp_ops.rs` | Starts a server on first matching file, initializes it, reads diagnostics and `$/progress` until it stops |
| Tabs | `apps/carbon-gui/src/workbench.rs` | `CurrentWorkbench` global; `open_buffer` (own tab / empty tab / new center tab, saves layout), `close_editor` (deferred; the last tab clears), `active_editor` |
| App root | `apps/carbon-gui/src/main.rs` | Ctrl+S / Ctrl+W bound app-wide; root handlers act on the active editor (menus leave focus empty) |
| Kit patches | `vendor/gpui-component/`, `vendor/gpui-base/` (each with `CARBON-PATCHES.md`), root `Cargo.toml` `[patch.crates-io]` | `Panel::always_tab_bar`, `Panel::request_close` (tab x and "..." Close), 30 px tabs, centered dialogs; gutter gap |
| Status bar | `apps/carbon-gui/src/status_bar.rs` | Language server item on the right |
| Theme | `apps/carbon-gui/src/style/theme.rs` | Kit default highlight theme for the mode when the theme file has no `highlight` section |
| Deps | root `Cargo.toml` | gpui-kit tree-sitter grammar features (13 grammars, MIT); `lsp-types 0.97` (MIT, was in the lock); `anyhow` (MIT/Apache, was in the lock); ropey `metric_utf16` |

## Speed gate (release build, Windows, 2026-10-17)

Measured with temporary timing logs (removed): Change event to next frame, and file open to first frame.

| File | Open | Keystroke | Notes |
|---|---|---|---|
| big.rs: 200,000 lines Rust, 4.8 MB | 52 ms | 2.5-9 ms | memory 270 MB |
| big.txt: 100,000 lines, 9 MB plain | 50 ms | 1.3-5.6 ms | |
| mid.rs: 20,000 lines Rust, 480 KB | 44 ms | 0-6 ms | |
| tiny.rs: 8 lines Rust | 30 ms | 1-7 ms | |

Pass (budget: open < 1 s, keystroke < 16 ms) with folding off and soft wrap off. With the kit defaults it failed:

| Setting | Effect measured |
|---|---|
| Folding on | Every keystroke in a Rust file waited ~25-30 ms for its frame, even an 8-line file (main thread free after ~5 ms, so the kit delays the redraw); opening big.rs took 12.6 s; and a broken parse of a big file (letters typed at the start of big.rs) overflowed a background thread's stack in the kit's recursive `extract_fold_ranges` (crash) |
| Soft wrap on | Opening big.rs took 2.4 s (every line wrapped up front) |

## Verified

- 2026-10-17 (save/quit): 58 pass + 3 ignored (new: untitled Save As). Click-tested: window close (WM_CLOSE) with an unsaved file -> dialog, Cancel stays, Don't Save quits with the disk untouched; File > Exit with 2 unsaved -> list, Save All writes both and quits; File > New File -> focused Untitled-1, File > Save -> Save dialog in the folder, saved as main.rs (tab renamed, Rust colours, rust-analyzer started), tab restored after restart; Edit > Undo in the editor; Edit > Copy/Paste on an Explorer file. The title bar x itself (OS caption button) and Ctrl+N/Ctrl+Q keys not clicked (window messages cannot hit them).
- `cargo test --workspace`: 57 pass, 3 ignored (2 real Recycle Bin, 1 real rust-analyzer: `cargo test -p carbon-core --test lsp -- --ignored`, passes in ~1.3 s); new tests: `tests/editor_buffer.rs` (5), `tests/lsp.rs` (2 + 1 ignored), unit tests for message framing and URI round trip. Clippy and fmt clean.
- Click-tested (preview build, window messages): open, type, save via tab menu (CRLF kept), dirty dot (single and multi-tab), second file in a new tab, quiet reload, conflict bar + Keep Mine, close dirty -> dialog -> Don't Save (disk untouched), tabs restored after restart, highlighting dark theme, big-file open/typing/scroll, rust-analyzer squiggle + Explorer marker + status bar, hover, completion narrowing, F12 in file and into std (read-only).
- Not tested: Ctrl+S / Ctrl+W keys (modifier keys cannot be simulated; same actions tested through the menu), Ctrl+click definition, IME, macOS/Linux.

## Limits

- No code folding (kit cost and crash above); no soft-wrap setting.
- No Save As for files that have a path; no auto save yet (to be a setting, user 2026-10-17; the Settings dialog exists since 2026-10-22); undo back to the saved text still shows the file as unsaved (dirty is by revision).
- Language servers: rust-analyzer only, from a fixed table; no settings sent; full-text sync (no incremental edits); no signature help, references, rename, code actions, formatting.
- Completion items insert the kit's way (no snippets); the kit does not filter, Carbon narrows by subsequence.
- Kit columns are characters, LSP columns UTF-16: converted for diagnostics and requests; other kit features (kit-side definition jumps within a file) use the kit's own conversion.
- Text lives twice (kit rope + core rope, shared nodes); no anchors; agents cannot edit an open editor's range yet.
- Layout (with tabs) is global, not per folder; library tabs (outside the folder) are not restored.
- rust-analyzer writes `target/` and `Cargo.lock` in the folder (its `cargo check`), as in other editors.
