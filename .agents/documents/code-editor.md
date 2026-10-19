# Code editor: what other editors use, and how far the gpui-kit editor scales

Written 2026-10-17 before building the editor. User asked what alternatives use (Zed especially) and whether building on the gpui-kit editor can scale later. Decision at the end. Sources: the editors' repos and blogs (links in s4), kit source read from the cargo registry (`gpui-base-0.7.3`, `gpui-component-0.7.3`).

## 1. What other editors use

| Editor | UI / rendering | Buffer | Highlighting | Display layers | Language features | Licence (reusable?) |
|---|---|---|---|---|---|---|
| VS Code (Cursor, Windsurf) | Monaco, DOM in Electron | Piece tree (piece table in a red-black tree) | TextMate grammars + LSP semantic tokens; tree-sitter opt-in since 1.97 (TypeScript) | View-model line mapping | LSP via extensions | MIT, TypeScript: ideas only |
| Zed | GPUI (GPU) | SumTree rope; CRDT `text::Buffer` with anchors and per-replica undo; `language::Buffer`; `MultiBuffer` (excerpts of many files in one editor) | tree-sitter, incremental, background, injections | DisplayMap: inlay, fold, tab, wrap, block maps | Built-in LSP client | `editor`, `text`, `rope`: GPL-3.0-or-later; `sum_tree`: Apache-2.0 but `publish = false`: ideas only |
| JetBrains | Swing | Immutable text tree | Own lexers + PSI parse trees | Folding / soft-wrap models | Own analysis; LSP for some languages | Community: Apache-2.0, Java |
| Sublime Text | Own C++ | Own | sublime-syntax (regex) | Own | LSP plugin | Proprietary |
| Helix | Terminal | ropey | tree-sitter | Own | Built-in LSP | MPL-2.0 (file-level copyleft) |
| Neovim | Terminal + GUI frontends | memline (B-tree of line blocks) | tree-sitter + regex syntax | Own | Built-in LSP | Apache-2.0 / Vim |
| Lapce | Floem (wgpu) | xi-rope | tree-sitter | Own | LSP + WASI plugins | Apache-2.0 |
| **gpui-kit `Editor`** | GPUI | ropey 2.0 beta (same as `carbon-core`) | tree-sitter, one cargo feature per language | Wrap + fold maps | Provider traits: completion, hover, definition, code actions, semantic tokens, document colors; diagnostics set; no LSP client | Apache-2.0, already a dependency |

Kit editor features found in source: multi-cursor (`add_selection`), search/replace, undo/redo, line numbers, folding, soft wrap, smart indent, auto-close pairs, whitespace display, inline completion (ghost text), read-only mode. Its doc comment says it "handles large text up to about 50K lines".

## 2. Can the kit editor scale later?

| Need | Kit 0.7.3 | Way around |
|---|---|---|
| Large files | Paints only visible lines; tree-sitter edits incrementally; files > 256 KB parse in the background | Measured, see `../project/built/code-editor.md` (speed gate) |
| Know what the user changed | `InputEvent::Change` has no edit details; the highlighter alone sees each `InputEdit` | Core takes the whole rope each change (same rope type, O(1) clone); LSP gets full text (Full sync) |
| Edit a range from outside (agents) | `insert` / `replace` act at the cursor only | Move the selection, replace, restore; or fork |
| Anchors (ED-11 ★), per-agent undo | Offsets only, one undo stack | Core keeps its own anchors, mapped through edits |
| Multibuffer, inline diff blocks, inlay hints | None | Stack editors (VS Code multi-diff style) or own element later |
| Folding | Works, but see findings in `../project/built/code-editor.md` (cost per keystroke, stack overflow on big broken files) | Off for now |

Recipe: the core owns the buffer (text, saving, disk changes, language servers); the editor panel is a thin adapter. If the kit blocks a feature, fork it (Apache-2.0) or replace the view only; core, tabs, save and LSP stay. Catch: the replaceable part is the big part (drawing, input, IME, selection).

## 3. Decision (user, 2026-10-17)

| Question | Options | Picked |
|---|---|---|
| Approach | Kit Editor + core buffer / kit Editor only / kit fork now / own Zed-style engine | **Kit Editor + core buffer** |
| Tier 1 scope | Edit + save + tabs, syntax colours, large-file speed check, language servers | **All four** |
| Language servers | Built in / installable language packages | Fixed table now (rust-analyzer); **installable language packages later** (user: "if people want to use rust they can install the rust packages later; or python etc."). 2026-10-19: none preinstalled, all via Manage Extensions (`language-tools.md`) |

## 4. Sources

- Zed crate manifests: `crates/editor`, `crates/text`, `crates/rope` (GPL-3.0-or-later), `crates/sum_tree` (Apache-2.0, `publish = false`) on github.com/zed-industries/zed.
- Zed rope / SumTree blog (summary on plushcap.com), DeepWiki "buffer architecture", zed.dev/blog/rainbow-brackets.
- VS Code 1.97 release notes (tree-sitter opt-in, via a hollo.social quote).
