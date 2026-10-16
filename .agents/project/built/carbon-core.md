# carbon-core (in-process core)

Built 2026-10-16. User decision: option (a), an in-process core crate before real panels, so panel state lives in plain Rust behind a clean API and can move behind a daemon later (`PROJECT.md` architecture). No UI dependency. The GUI holds one `Entity<Core>` per window (first reader: status bar).

## API

| Module | Types | What it does |
|---|---|---|
| `lib.rs` | `Core` | Facade per window: `open_workspace`, `create_workspace`, `close_workspace`, `workspace`, `branch`, `list_dir`, `tree`/`tree_mut`, `fs_changes`, `apply_fs_changes`, `create_file`, `create_dir`, `rename` (buffers follow), `trash`, `move_entries`, `copy_entries`, `undo_file_op`, `git`/`set_git`, `marks`, `open_file` (sets `active_buffer`), `spawn_terminal` (cwd defaults to workspace root); public stores `buffers`, `terminals`, `agents`, `jobs` |
| `workspace` | `Workspace` | Canonical root (`dunce`, no `\\?\`), name; `resolve` (relative or absolute, lexical `..`, then canonical) refuses paths outside the root, symlink escapes included; `relative` |
| `file_tree` | `list_dir`, `FileEntry`, `EntryKind`, `FileTree`, `Row` | One directory at a time (lazy explorer); folders first, case-insensitive; `.git/.hg/.svn/CVS/.DS_Store/Thumbs.db` hidden; `.gitignore`/`.ignore` matches (incl. parent folders, no git repo needed) flagged `ignored` for dimming (inherited inside ignored folders); dotfiles shown. `FileTree`: expanded set + loaded listings, `visible_rows` (`file-tree.md`) |
| `file_ops` | `validate_name`, `create_file`, `create_folder`, `rename`, `move_to_trash` | Names: no separators, Windows-forbidden chars, `.`/`..`, trailing dot; never overwrites; rename stays in the folder (case-only allowed); trash via `trash` crate (restorable) |
| `git_status` | `git_status`, `GitStatus`, `GitStatusMap` | gitoxide (`gix`) status of the repository containing the workspace, per file and rolled up per folder; `None` outside a repository |
| `marks` | `Marks`, `Mark` | Problem counts and agent-edited flag per file, folder roll-up, follow moves (`Core::marks`) |
| `history` | `FileOp`, `HISTORY_LIMIT` | Undo stack entries for create / rename / move / copy |
| `watcher` | `FsWatcher`, `FsChanges` | `notify` recursive watch of the workspace; 100 ms debounced batches of folders to re-list (or `rescan`) on an `async_channel` |
| `buffer` | `Buffer`, `BufferStore`, `BufferId`, `LineEnding` | Rope (`ropey 2.0.0-beta.1`, same as the kit), byte offsets on char boundaries; insert/remove/replace (validated, one revision each); `revision`, `is_dirty`; line endings kept as on disk, CRLF/LF detected; UTF-8 only (`NotUtf8` for binary); atomic save (temp file + rename + fsync); `save_as`; store opens each file once; `close` refuses dirty unless forced |
| `terminal` | `Terminal`, `TerminalStore`, `TerminalSpec`, `TerminalEvent` | PTY via `portable-pty` (ConPTY on Windows); default shell = `$SHELL` / `%COMSPEC%`; reader + waiter threads send `Output(bytes)` / `Exited(code)` on an `async_channel` (blocking and async receive); `write`, `resize`, `kill`; `close` kills |
| `vcs` | `Branch`, `current_branch` | Git branch from `.git/HEAD` (searches parents; `.git` file for worktrees/submodules); detached = short id; no git binary/library |
| `jobs` | `JobStore`, `Job`, `JobId`, `JobStatus` | Background job registry (Running/Succeeded/Failed) for the status bar; model only |
| `agent` | `AgentSession`, `AgentStore`, `Message`, `Role`, `AgentStatus` | Data model only: title, status (Idle/Running/AwaitingApproval/Finished/Failed), messages; no backend (protocol undecided) |
| `error` | `CoreError`, `Result` | `thiserror`; Io (with path), InvalidName, AlreadyExists, NotADirectory, OutsideWorkspace, NoWorkspace, NotUtf8, InvalidOffset, NoPath, Dirty, NotFound, Terminal, Watch, Trash, WorkspaceRoot, IntoItself, Git, Undo, NothingToUndo |

## Verified

`cargo test -p carbon-core`: 45 integration tests + 2 ignored (real Recycle Bin) in `apps/carbon-core/tests/` (workspace 4, create_workspace 4, file_tree 4, explorer_tree 7, file_ops 4, explorer_t3 7, buffer 7, terminal 2, agents 1, status 5), all pass on Windows; clippy clean; deps all MIT/Apache/BSD/Unlicense/CC0 (gitoxide adds 94 crates, all MIT/Apache-2.0/Unlicense). macOS/Linux untested.

## Findings

- **Windows ConPTY sends `ESC[6n` (cursor position request) first and blocks until answered** (`ESC[row;colR`). The terminal panel's emulator must answer it; the test does so by hand.
- `ropey` is a beta pinned `=2.0.0-beta.1` to share the kit's rope type; revisit when the editor panel is built.

## Not in scope yet

File watching (`notify`), undo/redo, anchors and per-actor edits (ED-11), encodings other than UTF-8, terminal emulation (VT parser/grid), agent protocol, persistence of sessions, daemon split.
