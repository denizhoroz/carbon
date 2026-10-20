# Carbon

## Overview

Carbon is a multi-tool workspace for agents that improves user interactibility. It is a Rust project. Goal: an IDE-like workspace with VS Code feature parity (so users never need to return to VS Code) plus agent-focused differentiators, synthesized from prior products (VS Code, Zed, Cursor, Claude Code, Agent Zero, OpenHands, Odysseus and others).

Positioning: fast and calm by default (beats VS Code); agent-native (beats Zed and Sublime); neutral and secure-by-default over heterogeneous agents (open market gap).

## Key decisions

**Committed pillars (decided by the user):**
1. Plain code editor (like VS Code or Zed).
2. Paradigm editor: diagrams used as AI context; AI or user creates "paradigms" (diagrams); used for architecting systems.
3. Agents tab: one or more agent chats in multiple windows/panes.
4. Multi-platform: Windows, macOS, Linux.
5. Lightweight (not first priority, but a design focus).
6. Modular extension sections: e.g. SVG or 3D model viewers addable after build; modular compatibility in general.

**Technical decisions** (recommended, from RESEARCH.md section 5; those marked built are in `project/built/`; the rest are not yet validated by benchmarks):

- **Architecture:** one static-musl Rust daemon owns workspace, buffers, terminals, LSP/DAP, agents, git; local = remote over loopback. UI is a swappable client.
- **Do not fork VS Code.** Reuse open protocols and parts (LSP, DAP, MCP, ACP, TextMate/theme JSON, `AGENTS.md`/`SKILL.md`/`.mcp.json`).
- **UI: native desktop GUI with GPUI + gpui-kit (gpui-component, renamed at v0.6; decided by the user, 2026-10-16); webview excluded.** Depend on `gpui-kit` with an exact version (0.7.3 read) on crates.io `gpui-pre` `=0.3.9`, Rust 1.92+; API breaks every minor. Usage notes: `project/ui/ui-framework.md`; alternatives and risks: `documents/gui-framework.md` (API churn, accessibility, editor build cost).
- **Protocol:** AHP-like state-first protocol (snapshot, ordered actions, replay); ACP for external agents; MCP (+ MCP Apps) for tools.
- **Buffer:** rope + anchors, per-actor undo, agents as attributed edit peers; CRDT deferred.
- **Extensions:** no full `vscode` API parity. Declarative VSIX and LSP/DAP launchers first, WASM (WIT, capability grants) + MCP native, Open VSX + own gallery, no Microsoft-only dependencies. **No preinstalled language tools** (servers, linters, formatters, compilers): users install them via Manage Extensions (user decision, 2026-10-19; `documents/language-tools.md`).
- **Safety defaults:** OS sandbox on, tiered approvals + classifier + hooks, side-effect-aware checkpoints, destructive-git guard.
- **Defaults:** calm opt-out AI, telemetry off, live cost meter, VS Code settings/keymap/theme import, AccessKit from the first widget, write-ahead journal.
- **Panel layout (user decision, 2026-10-16):** T1 (kit docking: drag/snap/split/resize/close/zoom/persist) + T2 (named layouts on hotkeys, task presets, keyboard move/resize, auto-hide) now; T3 (pop-out OS windows, top dock, guide diamond, float inside window) later. `project/built/panel-layout.md`.
- **Section Bar (user decision, 2026-10-16):** left vertical bar of section icons; each section has its own layout; first section Editor (`< >` icon). `project/built/section-bar.md`.
- **Manage dialogs (user decision, 2026-10-19):** bottom of the Section Bar, top to bottom: Manage Extensions, Manage Workflows, Manage Settings; each opens a dialog (placeholders now).
  - **Workflows:** a workflow is a workspace mode with its own layout; the code calls it a `Section` (one Section Bar icon each). Now: Editor (renamed from Editor 2026-10-20, id `carbon.editor`). Paradigm Editor (pillar 2) and Agent Automation are placeholder workflows now (empty panels, Section Bar icons) used to test and design the workflow system (user decision 2026-10-20). Users can add their own custom-built workflows. Workflows span any domain (user knowledge 2026-10-20): editor, paradigm editor, automation, 3D graphics viewer, sound design, user-built; the contract is not tied to code or a workspace folder (`requires_workspace` is optional). Manage Workflows adds and removes them. Workflows are manifest-defined (TOML) and modular, each with its own layout and named layouts, per `project/not_built/workflow-system.md` (design accepted incl. Q3: patch overlay for built-in tweaks, whole manifest for Duplicate; scoped keys deferred).
  - **Extensions:** works like VS Code. Official (bundled) and user-added extensions/plugins: compilers, linters, appearances (themes), and more (pillar 6). Manage Extensions lists, adds and removes them.
  - **Settings:** every user-changeable option: system, appearance, agents, etc. (`DESIGN.md` Settings).
- **Workflow storage (user decision, 2026-10-20):** Carbon's own workflows are packages in `apps/carbon-workflow/<name>/` (editor, paradigm, agent), shipped read-only next to the binary; the Editor cannot be removed. Downloaded workflow packages go to the OS local data dir; preferences and the workflow list stay in the OS config dir. Users can change all three locations in the installer or Settings (Settings not built: documented only). `project/not_built/workflow-system.md` s4.7.
- **Core first, in-process (user decision, 2026-10-16, option a):** `apps/carbon-core` holds workspace, file tree, buffers, terminals, agent sessions as plain Rust; the GUI calls it directly; daemon split later keeps the same API. `project/built/carbon-core.md`.
- **Sequencing (revised 2026-10-20):** in-process core + native UI shell and core panels (done) -> agent backend (ACP/MCP) -> extensions and Manage dialogs -> paradigm editor (after its research) -> daemon split -> macOS/Linux + CI.

## Open questions

- Paradigm editor (pillar 2) is unresearched: run a short follow-up on diagram/architecture tools (Excalidraw, tldraw, draw.io, Mermaid, PlantUML, Structurizr/C4, Lucidchart, Miro AI, Cursor/Claude canvases, VS Code Canvases): file format, AI integration, code round-trip, licences.
- Still to verify on GPUI Kit (editor speed gate and terminal embed passed on Windows; see `project/built/`): accessibility (Narrator, VoiceOver, Orca), IME in the editor, macOS and Linux behaviour, RAM on all three OSes (published numbers conflict 2x-15x), docking across windows (panel layout T3).
- CRDT choice (Zed-style, Diamond Types, Loro) and whether multi-user is needed early.
- How much of the PIM/agent-hub scope (email, calendar, notes, shared canvas) is in v1 vs IDE parity.
- AHP/ACP wire-format maturity and licences; MCP Apps client support.
- Licence stance for bundled components (AGPL: PyMuPDF, SearXNG, Firecrawl; ELv2: Phoenix).
- How extensions get language-tool binaries: config only / download on demand, PATH first / bundled per platform; compilers detected or installed on request (`documents/language-tools.md` s3; user: decide later).
- Workflow webview embedding: GPUI has no webview; how to host one (candidate `wry`, unverified) for the decided declarative UI + webview model (Q17-Q18, `project/not_built/workflow-system.md` s4.4, s6). Low-latency audio path still open (Q12).
- Full list of unverified items and benchmarks to run: RESEARCH.md section 8.

## Status

Build phase (Oct 2026). Working on Windows: native shell on gpui-kit with title bar and menus, Section Bar, docking layout, Explorer, code editor tabs with rust-analyzer, terminal, command palette, and a GUI-only chat panel (demo agent). Not built: real agent backend (ACP/MCP), paradigm editor, extension system, daemon split, debugging, whole-project search; macOS and Linux untested. What exists: `project/built/README.md`; what is left: `project/not_built/README.md`. Adoption scores, priorities and verdicts in `FEATURE.md`/`RESEARCH.md` are analyst inference, not user data. Live progress, decision log and error/solution log: `STATUS.md`.

## Docs map (`.agents/`)
Routing for agents (which file for which task): `README.md`.

| File | Contents | Use it for |
|---|---|---|
| `PROJECT.md` | Overview, decisions, open questions | Start here |
| `STATUS.md` | Progress tracker, decision log, error and solution log, session notes | What is done, what is next, known problems |
| `DESIGN.md` | UI and interaction rulebook for Carbon (tokens, shell, pillar surfaces, components, agent UX, accessibility, budgets) | Before any UI build or fix |
| `documents/` | Decision documents: `gui-framework.md` (native GUI comparison), `code-editor.md`, `terminal.md`, `language-tools.md` (how editors ship language servers, linters, compilers) | Why a technology was chosen; revisiting it |
| `project/built/` | One file per built feature + index (shell, theming, panel layout, Explorer, editor, terminal, command palette, chat, core) | What exists in code |
| `project/not_built/` | Index of committed features not built yet (FEATURE.md IDs) | Picking the next feature |
| `project/ui/ui-framework.md` | GPUI Kit knowledge base: versions, setup, components, theming, docking, editor, accessibility, gotchas | Building UI with GPUI Kit |
| `knowledge/` | Read-only knowledge from previous projects (design, architecture, security, experience, Rust/Dioxus/CSS/HTML/Python/uv/shell) | Reuse proven rules and avoid known problems; never edit |
| `RESEARCH.md` | Landscape, common stack, VS Code reference, comparison and synthesis, sentiment, gaps, evidence caveats, key source URLs | Why a decision was made; checking evidence strength |
| `FEATURE.md` | Part 1 Carbon features (pillars, sub-features, coverage check); Part 2 catalog of features from other products with adoption scores 1-5 and unique flags; Part 3 market gaps | Scoping and prioritizing work |

Conventions: evidence labels (M/V/A/S/doc) and (?) for unverified; update RESEARCH.md when decisions change evidence, FEATURE.md when scope changes.
