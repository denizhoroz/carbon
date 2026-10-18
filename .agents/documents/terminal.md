# Integrated terminal: what other editors use, and what Carbon picked

Written 2026-10-18 before building the terminal. User asked what alternatives use and which choices Carbon has to make. Decision at the end. Sources in s4.

## 1. What other editors and terminals use

| Product | Emulator (escape-code parser, grid, scrollback) | PTY layer | Drawing | Shell integration | Licence (reusable?) |
|---|---|---|---|---|---|
| VS Code | xterm.js (TypeScript) | node-pty | DOM or WebGL | OSC 633 + injected scripts | MIT, JavaScript: ideas only |
| Zed | `alacritty_terminal` (Zed fork) | alacritty tty | own GPUI element (`terminal_view`) | partial | Zed crates GPL; `alacritty_terminal` Apache-2.0 |
| Lapce | `alacritty_terminal` | alacritty tty | Floem | no | Apache-2.0 |
| WezTerm | own `wezterm-term` / `termwiz` | `portable-pty` (Carbon uses it) | wgpu | OSC 133 | MIT |
| Warp | own, based on Alacritty's grid | own | own GPU UI | command blocks | proprietary |
| Windows Terminal | own (C++) | ConPTY | DirectX | OSC 133 | MIT, C++ |
| Ghostty | libghostty-vt (Zig) | own | Metal/OpenGL | OSC 133 | MIT; no stable library release found (last Ghostty tag March 2026) |
| JetBrains | JediTerm (Java) | pty4j | Swing | command blocks (new terminal) | LGPL/Apache, Java |
| `gpui-terminal` crate | `alacritty_terminal` | any (`portable-pty` in its example) | GPUI | no | licence and GPUI version not checked (likely Zed's GPUI, not `gpui-pre`) |

gpui-kit has no terminal widget, so the view is Carbon's own either way. Earlier research (`../RESEARCH.md`, terminal row) already pointed to `alacritty_terminal` + OSC 633/133.

## 2. Choices Carbon had to make

| Question | Options | Picked |
|---|---|---|
| Emulator | `alacritty_terminal` / `vt100` crate / wait for libghostty-vt / own | **`alacritty_terminal`** (0.26.0, Apache-2.0) |
| Where the screen lives | Core (agents and the daemon read it without a UI) / GUI panel (Zed's way) | **Core** |
| Tier 1 | base (shell, draw, type, resize, colours) + selection/copy/paste/scrollback + profiles + shell integration + links/find | **All** |
| Windows default shell | PowerShell 7, else Windows PowerShell / cmd / Git Bash | **PowerShell 7, else Windows PowerShell** |
| PTY | keep `portable-pty` / alacritty's tty | **`portable-pty`** (already in the core) |

## 3. Decision (user, 2026-10-18)

As in s2. Built: `../project/built/terminal.md`.

## 4. Sources

- Zed `crates/terminal` and `crates/terminal_view` READMEs and source (mirror git.secluded.site): alacritty `Term` + event loop, GPUI view, three input paths.
- docs.rs `gpui-terminal` (alacritty_terminal + generic read/write streams).
- Fedora release monitoring for `rust-alacritty_terminal` (0.25.1, Oct 2025); `cargo search` on 2026-10-18: 0.26.0.
- Ghostty release discussion (zeli.app aggregate): v1.3.1 March 2026, 1.4 planned October 2026; no libghostty-vt release page found.
