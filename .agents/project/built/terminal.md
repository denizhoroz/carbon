# Integrated terminal tier 1

Built 2026-10-18. User request: plan and build the terminal after comparing alternatives (`../../documents/terminal.md`). User picked: `alacritty_terminal` emulator in the core (screen readable without a UI), `portable-pty` kept, tier 1 = base + selection/copy/paste/scrollback + shell profiles + shell integration (OSC 633/133) + links and find; Windows default shell PowerShell 7, else Windows PowerShell.

## Behaviour

| Input | Result |
|---|---|
| Code Editor section start, View > Add Panel > Terminal | A terminal tab with the default shell (PowerShell 7, else Windows PowerShell; `$SHELL` on macOS/Linux) in the open folder, or in the folder of the last focused terminal |
| Terminal "..." menu | New <shell> for every installed shell (Windows: PowerShell, Windows PowerShell, Command Prompt, Git Bash, WSL; Unix: `$SHELL`, `/etc/shells` bash/zsh/fish/sh), Find, Restart, Zoom, Close |
| Tab name | The shell's name, numbered for more of the same ("Git Bash 2") |
| Typing | Text (IME, AltGr included) through the platform text input; Enter, Tab, Backspace, arrows (app cursor mode), Home/End, PgUp/PgDn, Insert/Delete, F1-F12, Ctrl+letter, Alt+key as xterm sends them |
| Shortcuts while a terminal has focus | The shell gets every key (Ctrl+W, Ctrl+K, Ctrl+N, ...) except Ctrl+B, Ctrl+Alt+B, Ctrl+J, Ctrl+Q, Ctrl+Alt+1..9, Ctrl+Alt+Shift+arrows (VS Code's "skip shell" idea) |
| Ctrl+C | Copies when text is selected, otherwise interrupts (^C). Ctrl+Shift+C copies; Ctrl+V / Ctrl+Shift+V paste (bracketed paste when the program asks) |
| Mouse | Drag selects; double-click a word, triple-click a line; Shift+click extends. Wheel scrolls the scrollback (in full-screen programs like less/vim: arrow keys). Edit menu Copy/Paste work |
| Shift+PgUp/PgDn/Home/End | Scroll the scrollback (10,000 lines) |
| Ctrl+F or "..." > Find | Find bar (top right): searches as you type (case-insensitive unless the text has capitals), Enter = older match, Shift+Enter = newer, arrows, Escape closes; the match is selected and scrolled to |
| Ctrl+click | URLs and OSC 8 links open in the browser; file paths (`path`, `path:line`, `path:line:col`, `path(line,col)`), relative to the shell's folder, open in the editor at that line. Hover with Ctrl underlines the link |
| Shell integration (PowerShell, bash/Git Bash, zsh) | A dot in the left gutter at each prompt: green exit 0, red other, grey running/unknown; the shell's folder is tracked (new terminals and links use it); the core keeps each command's line, exit code, folder and output (last 256 KB) for agents |
| Resize (dock edge, window) | The grid follows; the shell rewraps |
| Shell exits | "The shell exited with code N. Press Enter to restart." Enter or "..." > Restart: a new shell in the same folder |
| Close the tab | Ends the shell |
| Restart Carbon | Terminal tabs come back with their shell and folder (a fresh shell; sessions are not persisted) |

## Code

| Part | File | What |
|---|---|---|
| Core session | `apps/carbon-core/src/terminal/mod.rs` | `Terminal` (PTY + `Arc<FairMutex<Term>>`), `TerminalStore`, `TerminalSpec`, `TerminalEvent` (`Wakeup` once until `ack_wakeup`, `Title`, `Bell`, `Clipboard`, `Exited`), `write` (never blocks), `paste`, `resize`/`resize_cells`, `scroll`, selection, `screen_text`, `cwd`, `commands`, `command_marks`, `exit_status`; `alacritty_terminal` re-exported for the view |
| Emulator plumbing | `apps/carbon-core/src/terminal/emulator.rs` | Reader thread parses under the lock (`vte` `Processor`), one wakeup per burst; writer thread for input and the emulator's answers (`PtyWrite`: ConPTY's `ESC[6n` answered); `Listener`; command records from marks |
| Shell integration parsing | `apps/carbon-core/src/terminal/osc.rs` | `Sniffer`: takes OSC 133/633 out of the byte stream (split across reads too), passes all else through; `Mark` |
| Profiles and scripts | `apps/carbon-core/src/terminal/shells.rs`, `terminal/scripts/carbon.{ps1,bash,zsh}` | Detection, default, `Profile::spec` (writes the script to `%TEMP%\carbon-shell-integration`, adds `-NoExit -Command`, `--init-file`, or a `ZDOTDIR` shim); Carbon's own small scripts (633 A/B/C/D/E/P;Cwd; Git Bash reports Windows paths via `cygpath`) |
| Panel | `apps/carbon-gui/src/terminal_panel.rs` | `TerminalPanel`: session lifecycle, keystroke interceptor (`cx.intercept_keystrokes`, runs before app shortcuts), `EntityInputHandler` for text, mouse, links, find bar, restart, "..." menu, persistence (`profile`, `label`, `cwd`); root forwards "..." actions (`on_root_action`) |
| Drawing | `apps/carbon-gui/src/terminal_panel/view.rs` | `Geometry` (cell size from the mono font's advance), `Snapshot` (rows of styled runs + background rects + cursor + marks, copied under the lock), `paint` (canvas element) with cursor shapes, link underline, gutter marks, scrollbar |
| Keys | `apps/carbon-gui/src/terminal_panel/input.rs` | `app_keeps`, `passes_as_text`, `to_bytes` (xterm sequences, modifier parameters) |
| Colours | `apps/carbon-gui/src/terminal_panel/colors.rs` | ANSI 16 from theme base colours (`red`, `red_light`, ...), 256-colour cube, true colour, program overrides (OSC 4/10/11) |
| Wiring | `workbench.rs` (`add_panel` terminals: profile, numbering, folder), `layouts.rs` (Code preset), `test_panel.rs` (old "Terminal" placeholders restore as terminals), `main.rs`, `style/fixed.rs` (line height 1.3, gutter 14 px, padding, scrollbar, mark size) | |
| Deps | root `Cargo.toml` | `alacritty_terminal` 0.26.0 (Apache-2.0; its 55 dependencies MIT / Apache-2.0 / Unlicense) |

## Speed gate (release build, Windows, 2026-10-18)

Measured with temporary timing logs (removed).

| Test | Result |
|---|---|
| 50 MB / 664k lines (`cmd /c type`) | 29.0 s in the panel; 26.5 s core only (no window); 25.4 s raw ConPTY read, no emulator. ConPTY itself is the limit (~2 MB/s); Carbon adds ~4% (emulator) + ~10% (drawing) |
| Frame work while that output streams (snapshot + paint, 4,239 frames) | p50 1.1 ms, p99 1.6 ms, max 3.9 ms |
| Key to first screen update (20 keys into PowerShell) | median 0.3 ms, max 7.1 ms |

Pass (budget: frame < 16 ms, keystroke < 16 ms).

## Verified

- `cargo test --workspace`: 67 pass, 4 ignored. New: `tests/terminal.rs` (screen shows the output, size, exit code; close kills; a default shell is found; ignored real-PowerShell integration test: command line, exit code 0, output, cwd, passes when run), `osc.rs` unit tests (4: marks, split reads, other escapes untouched, unescaping), `input.rs` (2), `terminal_panel.rs` link parsing (2). Clippy and fmt clean.
- Click-tested (preview builds, window messages): PowerShell prompt and typing, Enter, output with colours (Write-Host colours, PSReadLine syntax colours), wrapping, scrollback with the wheel and scrollbar, marks green/red/grey including after shrink and grow, drag-select + Edit > Copy (clipboard checked) + Edit > Paste, "..." > Find with Enter / Escape, New Git Bash (colours, `false` red, starts in the PowerShell terminal's folder), `cd` then exit and Enter-restart in the same folder, tabs restored after restart, Explorer picks up folders made in the terminal.
- Not tested: Ctrl+C/V/F, Ctrl+click links and other modifier keys (cannot be simulated; the same code paths ran through menus), IME composition, zsh (no zsh here), WSL, macOS/Linux.

## Limits

- Mouse reporting to programs (clicks in htop, vim) not sent; wheel in full-screen programs sends arrow keys.
- IME composition text is not shown while composing (the result is sent).
- Command marks count lines from the top of the scrollback; after 10,000 lines of scrollback they drift. On Windows, ConPTY repaints after a resize; growing is corrected for, other ConPTY repaints may shift marks.
- No shell integration for cmd, WSL and fish. Git Bash's prompt mark sits one line above its prompt (its prompt starts with a blank line).
- No split terminals inside one tab (use dock splits), no persistent sessions, no inline images, no terminal settings (font, size, scrollback length, cursor style) yet.
- Pasting through the input handler is not bracketed (the paste key and menu paths are).
