# Carbon features

Goal: catch features that became standardized across many apps AND not miss unique features that exist in only one app but should not be omitted. Sources and caveats: `RESEARCH.md`; project decisions: `PROJECT.md`.

## How to read the scores (Part 2)

**Adoption score (1-5):** how many products have the feature, counted from the "Seen in" column. Counts that are unclear are estimated and marked (?).


| Score | Meaning        | Products with it                |
| ------- | ---------------- | --------------------------------- |
| 5     | Near-universal | 10+ products, or all major IDEs |
| 4     | Widely used    | 6-9                             |
| 3     | Common         | 3-5                             |
| 2     | Few            | 2                               |
| 1     | Single product | 1                               |

**Unique flag ★:**

- Set on features found in only 1-2 products that are valuable enough not to omit.
- Such rows score 1-2, so ★ keeps them visible.
- The note column gives the reason and a keep priority: *must-consider* or *nice*.

**Row order:** by score descending, ★ rows last in each category.

**Evidence tags:**

- **M** measured
- **V** vendor claim
- **A** anecdotal
- **S** secondary source
- **doc** docs or spec
- **inf** analyst inference from the research files
- **(?)** unverified, or background knowledge not researched

**Caveats:**

- Scores and priorities are analyst judgment, not usage data.
- Old P0/P1/P2 judgments appear only in notes where they add information.

**Contents:** Part 1 Carbon features (1.1 pillars, 1.2 sub-features per pillar, 1.3 cross-cutting decisions, 1.4 coverage check) - Part 2 Catalog (summary, then 30 categories) - Part 3 Market gaps

# Part 1. Carbon features

## 1.1 Committed pillars (decided by the user)


| # | Pillar                                                                                                                                            |
| --- | --------------------------------------------------------------------------------------------------------------------------------------------------- |
| 1 | Plain code editor (like VS Code or Zed)                                                                                                           |
| 2 | Paradigm editor: diagrams used as AI context; AI or user creates "paradigms" (diagrams); the section is used for architecting systems             |
| 3 | Agents tab: one or more agent chats displayed in multiple windows/panes                                                                           |
| 4 | Multi-platform: Windows, macOS, Linux                                                                                                             |
| 5 | Lightweight (not first priority, but a design focus)                                                                                              |
| 6 | Modular extension sections: after building Carbon the user can easily add e.g. an SVG viewer or 3D model viewer; modular compatibility in general |

## 1.2 Sub-features per pillar

Status: **Committed** (inherent to the pillar or already a decision in PROJECT.md), **Proposed** (recommended here), **Open** (question). IDs refer to Part 2.

### Pillar 1: Plain code editor


| Sub-feature                                                                                       | Catalog IDs                       | Status                                |
| --------------------------------------------------------------------------------------------------- | ----------------------------------- | --------------------------------------- |
| Core editing: multi-cursor, find/replace, folding, format, auto save/hot exit, diff editor        | ED-01, ED-02, ED-03, ED-04        | Committed                             |
| Navigation and layout: command palette, tabs/splits, explorer, problems, themes                   | WB-01, WB-02, WB-03, WB-04, WB-07 | Committed                             |
| Settings, keymaps, VS Code settings/keymap/theme import, telemetry controls                       | ST-01, ST-04, ST-06               | Committed                             |
| Language intelligence: LSP client, navigation/refactor, tree-sitter highlighting, semantic tokens | LS-01, LS-02, LS-03, LS-04, LS-06 | Committed                             |
| Debugging: DAP client, launch.json, logpoints                                                     | DB-01, DB-02                      | Committed                             |
| Tasks, test explorer, problem matchers                                                            | TK-01, TS-01                      | Proposed                              |
| Workspace search (ripgrep-class)                                                                  | SR-01                             | Committed                             |
| Git: stage by hunk, blame, history, merge editor, PR integration                                  | SC-01, SC-03, SC-04, ED-06        | Committed                             |
| Integrated terminal with shell integration                                                        | TM-01, TM-02                      | Committed                             |
| Minimap, sticky scroll, breadcrumbs, markdown preview                                             | ED-05, ED-07                      | Proposed                              |
| Notebooks and HTTP client (must-keep 15)                                                          | ED-09, ED-10                      | Open (could ship as pillar-6 viewers) |
| Inline completion, next-edit, inline chat (full opt-out)                                          | IC-01, IC-02, IC-03               | Proposed                              |
| Rope + anchors buffer, agents as attributed edit peers                                            | ED-11                             | Proposed                              |
| Workspace Trust, multi-root                                                                       | WB-05, WB-06                      | Proposed                              |
| Accessibility                                                                                     | AX-01, AX-02                      | Proposed                              |

### Pillar 2: Paradigm editor (all rows (?): unresearched background knowledge)


| Sub-feature                                                                                                              | Catalog IDs / refs                | Status       |
| -------------------------------------------------------------------------------------------------------------------------- | ----------------------------------- | -------------- |
| PA-01 Diagram types: flow, architecture, C4, sequence, ER, state (?)                                                     | DA-02 (Mermaid render)            | Proposed (?) |
| PA-02 AI-generated and AI-edited diagrams from code or prompt (?)                                                        | AG-02, DA-01                      | Proposed (?) |
| PA-03 Diagram as AI context: export to Mermaid / JSON / text for prompts; agents read diagrams as structured context (?) | CX-01, MS-01                      | Proposed (?) |
| PA-04 Bidirectional code <-> diagram sync (?)                                                                            | WF-05 (YAML round-trip precedent) | Open (?)     |
| PA-05 Versioned diagrams stored in the repo as text, diffable in git (?)                                                 | SC-01, ED-04                      | Proposed (?) |
| PA-06 Node/graph canvas: reference React Flow, tldraw, Excalidraw, draw.io (licences unverified) (?)                     | WF-01                             | Open (?)     |
| PA-07 Human + agent co-editing of one diagram (?)                                                                        | CO-02, DA-03                      | Open (?)     |
| PA-08 Diagram nodes linked to files/symbols (click to open code) (?)                                                     | LS-02                             | Proposed (?) |

**Open question:** run a short follow-up research on diagram/architecture tools before designing this pillar: Excalidraw, tldraw, draw.io, Mermaid, PlantUML, Structurizr/C4, Lucidchart, Miro AI, Cursor/Claude canvases, VS Code Canvases; focus on file format, AI integration, round-trip with code, licences.

### Pillar 3: Agents tab


| Sub-feature                                                                  | Catalog IDs                       | Status                      |
| ------------------------------------------------------------------------------ | ----------------------------------- | ----------------------------- |
| Multiple sessions in panes/windows, parallel, grouped, searchable            | AG-01, SE-01, SE-03               | Committed                   |
| Worktree per session with setup scripts and port allocation                  | SC-02, SE-01                      | Proposed                    |
| Autonomous loop, subagents, custom agents with handoffs                      | AG-02, AG-03                      | Proposed                    |
| External agents via ACP; MCP for tools                                       | AG-06, MS-01, SE-04               | Committed (PROJECT.md)     |
| Approvals: per-action, graded modes, plan mode, unified policy engine, hooks | AP-01, AP-02, AP-03, AP-04, AP-06 | Committed (safety defaults) |
| Per-hunk accept/reject, one review model across harnesses                    | AP-08, AP-09                      | Proposed                    |
| Cross-session approval inbox; per-session FS/network policy                  | AP-07, AP-10                      | Proposed                    |
| OS sandbox default on; destructive-git guard                                 | AP-11, AP-12                      | Committed                   |
| Checkpoints and restore, incl. side effects                                  | CK-01, CK-02                      | Committed                   |
| Instruction/skill files: AGENTS.md, CLAUDE.md, SKILL.md, .mcp.json           | CX-01, MS-02                      | Committed                   |
| Model picker, BYOK/local, live cost meter, accept/rework metrics             | MD-01, MD-09, MD-10               | Committed (cost meter)      |
| Scheduled automations                                                        | AG-07                             | Proposed                    |
| Plan/spec artifacts; customization resolver                                  | SP-01, CX-03                      | Open                        |
| Browser tools, computer use                                                  | TR-04                             | Open                        |
| Agent-stream accessibility, chat debug view                                  | AX-03, PL-07                      | Proposed                    |

### Pillar 4: Multi-platform (Windows, macOS, Linux)


| Sub-feature                                                                                  | Catalog IDs         | Status    |
| ---------------------------------------------------------------------------------------------- | --------------------- | ----------- |
| Single codebase, per-OS quirks (ConPTY, WebKitGTK vs bundled Chromium, glibc vs static musl) | PL-12, TM-01, RM-06 | Committed |
| Screen-reader and AccessKit on all three                                                     | AX-01, AX-02        | Proposed  |
| Remote/WSL/containers for cross-OS workflows                                                 | RM-01, RM-02        | Proposed  |
| Native desktop GUI decided (webview excluded); framework choice by spikes                    | PL-10               | Decided, framework open |

### Pillar 5: Lightweight


| Sub-feature                                                           | Catalog IDs  | Status                   |
| ----------------------------------------------------------------------- | -------------- | -------------------------- |
| Fast startup and low idle RAM budgets, benchmarked vs VS Code and Zed | PL-11        | Committed (design focus) |
| Rust daemon, thin swappable UI client                                 | RM-06        | Committed                |
| In-process tree-sitter, priority-scheduled search                     | LS-06, SR-04 | Proposed                 |
| Write-ahead journal, per-agent processes, unresponsiveness detection  | PL-08        | Proposed                 |
| gitoxide reads, git CLI writes                                        | SC-06        | Proposed                 |
| Native GPU UI for hot surfaces only if profiling demands              | PL-10        | Open                     |

### Pillar 6: Modular extension sections


| Sub-feature                                                                                                               | Catalog IDs                | Status                  |
| --------------------------------------------------------------------------------------------------------------------------- | ---------------------------- | ------------------------- |
| WASM extensions (WIT, versioned API) with user-granted capability grants                                                  | EX-06                      | Committed (PROJECT.md) |
| MCP servers / skills / hooks as native extension types                                                                    | EX-02, MS-01, MS-02, MS-04 | Committed               |
| Custom editors/viewers and webview-like panels; MCP Apps (`ui://` sandboxed iframes)                                      | EX-05, EX-07, MS-03        | Proposed                |
| Viewer registry: file glob/MIME -> viewer (SVG, 3D model glTF, PDF, CSV, image) installable without rebuilding Carbon (?) | EX-05, DA-01               | Proposed (?)            |
| Declarative VSIX tier (themes, grammars, snippets, keymaps), LSP/DAP launchers                                            | EX-01, EX-03               | Committed               |
| Open VSX + own gallery, signed, no silent auto-update, allow-list                                                         | EX-08, EX-09               | Committed               |
| Optional VS Code extension host (phase 3+, no API parity)                                                                 | EX-04                      | Open                    |
| Extension bisect / process isolation so one extension cannot stall all                                                    | PL-08                      | Proposed                |
| Language tools (LSP, DAP, linters, formatters, compiler helpers) only via extensions; none preinstalled                   | LS-01, EX-03               | Committed (user 2026-10-19) |
| How extensions get tool binaries: config only / download on demand, PATH first / bundled per platform (`documents/language-tools.md`) | LS-01, EX-06               | Open                    |
| Narrow-API escape hatch (MCP) because Zed has no custom panels                                                            | EX-07                      | Proposed                |
| Manage Extensions dialog (Section Bar bottom): official + user-added extensions/plugins, VS Code-like (compilers, linters, appearances, ...) | EX-01, EX-03               | Committed (user 2026-10-19) |
| Workflows (= Section Bar sections): Code Editor now; Paradigm Editor, Agent Automation planned; users add custom-built workflows; Manage Workflows dialog adds/removes them | -                          | Committed (user 2026-10-19) |
| Manage Settings dialog (Section Bar bottom): every user option (system, appearance, agents, ...) | ST-01                      | Committed (user 2026-10-19) |

## 1.3 Cross-cutting decisions and constraints

- One static-musl Rust daemon owns workspace, buffers, terminals, LSP/DAP, agents, git; local = remote over loopback; UI is a swappable client. Do not fork VS Code; reuse open protocols (LSP, DAP, MCP, ACP, TextMate/theme JSON, AGENTS.md/SKILL.md/.mcp.json).
- Native desktop GUI (user decision; webview excluded); framework per `documents/gui-framework.md`. Needs own benchmark (published numbers conflict 2x-15x).
- Protocols: AHP-like state-first (snapshot, ordered actions, replay), ACP external, MCP (+ MCP Apps) tools. Wire formats unverified.
- Security defaults: OS sandbox on, loopback/auth-on defaults, tiered approvals + classifier + hooks, no silent extension auto-update, Workspace Trust gating. Licence watch: AGPL (PyMuPDF, SearXNG, Firecrawl), ELv2 (Phoenix).
- Calm defaults: one-switch AI opt-out, telemetry off, live cost meter, write-ahead journal, non-destructive restore.
- Accessibility: AccessKit from the first widget (hardest part of a native UI; audit per framework).
- Sequencing: daemon -> UI shell + importers -> extensions -> decide Node host / native UI.
- **Must-keep list** (inferred, not polled; reasons users return to VS Code): 1 extension ecosystem; 2 DAP debugger; 3 remote; 4 integrated terminal; 5 strong git diff/blame; 6 Pylance-class language intelligence; 7 settings/keymap import; 8 stability; 9 fast startup, low RAM; 10 honest AI opt-out; 11 per-hunk accept/reject; 12 collaboration; 13 tabs/sidebar navigation; 14 Vim mode quality; 15 notebooks and HTTP client.

## 1.4 Coverage check


| Set       | Total | In a pillar | Not assigned |
| ----------- | ------- | ------------- | -------------- |
| Score 5   | 38    | 34          | 4            |
| Score 4   | 40    | 31          | 9            |
| Score >=4 | 78    | 65          | 13           |
| ★ unique | 47    | 21          | 26           |

High-score (>=4) features not assigned to any pillar, for the user to decide: KM-01 Knowledge base / RAG pipeline (5); KM-03 Persistent memory (5); TR-01 Web search tool (5); SB-01 Sandboxed code execution (5); SR-03 Semantic/codebase index (4); RM-03 Tunnels / browser-hosted client (4); RM-04 Managed cloud workspaces/agents (4); CO-01 Live-Share-style co-editing (4); WF-02 Flow to API/JSON export (4); SB-02 Per-conversation container (4); PD-03 Multi-surface clients (4); PL-03 Observability/tracing (4); PL-02 Multi-tenant security: JWT (4).

Unassigned ★ features: ST-05, LS-07, DB-04, DB-05, SR-05, CO-05, CO-04, AG-13, AG-10, AG-08, AG-09, AG-11, MD-03, MD-04, MD-05, KM-04, KM-06, KM-07, TR-02, WF-04, WF-06, WF-08, SB-03, PD-02, PL-05, PL-09.

# Part 2. Catalog of features from other products

"Seen in" lists examples, not all products.


| Category                          | n   | 5  | 4  | 3  | 2  | 1  | ★ |
| ----------------------------------- | ----- | ---- | ---- | ---- | ---- | ---- | ---- |
| Editor                            | 11  | 4  | 4  | 1  | 1  | 1  | 1  |
| Workbench and layout              | 8   | 4  | 1  | 3  | 0  | 0  | 0  |
| Settings and profiles             | 6   | 1  | 2  | 2  | 1  | 0  | 1  |
| Language intelligence             | 7   | 3  | 1  | 2  | 0  | 1  | 1  |
| Debugging                         | 5   | 1  | 1  | 1  | 0  | 2  | 2  |
| Testing                           | 4   | 0  | 1  | 2  | 1  | 0  | 0  |
| Tasks                             | 2   | 0  | 1  | 0  | 0  | 1  | 0  |
| Search                            | 5   | 1  | 1  | 1  | 1  | 1  | 2  |
| Source control                    | 6   | 1  | 3  | 2  | 0  | 0  | 0  |
| Terminal                          | 4   | 1  | 1  | 2  | 0  | 0  | 0  |
| Extensions                        | 9   | 3  | 2  | 2  | 1  | 1  | 2  |
| Remote and web                    | 6   | 1  | 4  | 1  | 0  | 0  | 0  |
| Collaboration                     | 5   | 0  | 1  | 1  | 1  | 2  | 3  |
| Inline completion                 | 3   | 2  | 1  | 0  | 0  | 0  | 0  |
| Agents                            | 11  | 3  | 2  | 1  | 2  | 3  | 5  |
| Approvals and safety              | 12  | 2  | 3  | 1  | 1  | 5  | 5  |
| Checkpoints                       | 2   | 1  | 0  | 0  | 0  | 1  | 1  |
| Sessions                          | 3   | 0  | 2  | 0  | 1  | 0  | 1  |
| Context                           | 2   | 1  | 0  | 0  | 0  | 1  | 1  |
| MCP and skills                    | 6   | 2  | 0  | 4  | 0  | 0  | 0  |
| Specs                             | 1   | 0  | 0  | 0  | 1  | 0  | 1  |
| Models and chat                   | 9   | 1  | 0  | 3  | 1  | 4  | 5  |
| Knowledge, RAG and memory         | 8   | 2  | 0  | 3  | 1  | 2  | 3  |
| Tools, search and research        | 4   | 1  | 1  | 1  | 0  | 1  | 1  |
| Workflows and builders            | 8   | 1  | 1  | 2  | 3  | 1  | 4  |
| Sandboxing and execution          | 3   | 1  | 1  | 0  | 0  | 1  | 1  |
| Productivity, PIM and documents   | 3   | 0  | 1  | 1  | 1  | 0  | 1  |
| Platform, security and operations | 11  | 1  | 3  | 3  | 2  | 2  | 4  |
| Accessibility                     | 3   | 0  | 0  | 1  | 1  | 1  | 1  |
| Diagrams, visuals and artifacts   | 3   | 0  | 2  | 0  | 0  | 1  | 1  |
| **Total**                         | 170 | 38 | 40 | 40 | 20 | 32 | 47 |

Score counts here include ★ rows. ★ rows are never counted in the standardized core.

**Standardized core (score 5):**

- Editor: ED-01 Multi-cursor; ED-02 Folding; ED-03 Format document/selection; ED-04 Diff editor
- Workbench and layout: WB-01 Command Palette; WB-02 Editor groups; WB-03 Explorer; WB-07 Themes
- Settings and profiles: ST-01 Settings UI + JSON
- Language intelligence: LS-01 Generic LSP 3.17 client: diagnostics; LS-02 Definition/type/implementation/references; LS-03 Built-in web languages
- Debugging: DB-01 Generic DAP client: launch/attach
- Search: SR-01 Workspace search: regex
- Source control: SC-01 Status
- Terminal: TM-01 PTY terminals
- Extensions: EX-01 Themes; EX-02 MCP servers; EX-03 Install/enable/disable UI
- Remote and web: RM-01 SSH remote with auto-installed server
- Inline completion: IC-01 Inline suggestions; IC-03 Inline chat
- Agents: AG-01 Agent sessions as primary surface; AG-02 Autonomous agent execution with tools; AG-03 Subagents / multi-agent teams
- Approvals and safety: AP-01 Per-action approval; AP-08 Per-hunk accept/reject of AI edits
- Checkpoints: CK-01 Checkpoints / snapshots / undo and restore
- Context: CX-01 Read `AGENTS.md`
- MCP and skills: MS-01 MCP client: stdio; MS-02 Skills
- Models and chat: MD-01 Multi-provider chat
- Knowledge, RAG and memory: KM-01 Knowledge base / RAG pipeline; KM-03 Persistent memory
- Tools, search and research: TR-01 Web search tool
- Workflows and builders: WF-01 Visual drag-drop canvas
- Sandboxing and execution: SB-01 Sandboxed code execution
- Platform, security and operations: PL-12 Windows/macOS/Linux parity

**Unique must-not-miss (★):**

- Editor: ED-11 Rope + anchors buffer
- Settings and profiles: ST-05 Layered config default < user < profile < workspace
- Language intelligence: LS-07 Carbon-owned rust-analyzer config
- Debugging: DB-04 Native DAP: cache-first data layer; DB-05 Language-specific debugger value renderers
- Search: SR-05 Local-first semantic index with Merkle sync; SR-04 Priority-scheduled search
- Extensions: EX-06 WASM extensions; EX-07 Declarative/sandboxed UI contributions
- Collaboration: CO-05 Take over the agent's IDE; CO-02 Multiplayer CRDT editing; CO-04 Share vs collaborate separation
- Agents: AG-13 Kanban-style agent command center; AG-10 NL-to-agent builder; AG-08 Code-as-action; AG-09 Automatic prompt optimization; AG-11 Self-evolving skills
- Approvals and safety: AP-04 Unified approval policy engine: tiers + reasoning-blind classifier + hard hooks; AP-07 Cross-session approval inbox; AP-09 One review model; AP-10 Per-session FS/network policy tied to worktree; AP-12 Destructive-git guard
- Checkpoints: CK-02 Side-effect-aware checkpoints: pre-turn FS snapshots incl. untracked/ignored files
- Sessions: SE-04 Agent-host protocol: state-first
- Context: CX-03 Customization resolver showing precedence across `.github/`
- Specs: SP-01 Plan/spec artifacts with comment-and-revise
- Models and chat: MD-10 Instrument accept rate; MD-03 Built-in model serving + hardware-aware picks; MD-04 Blind model comparison; MD-05 Beam multi-model compare/merge; MD-09 Live per-session cost meter
- Knowledge, RAG and memory: KM-04 User-editable white-box memory; KM-06 Memory-as-OS; KM-07 50+ enterprise connectors
- Tools, search and research: TR-02 Search modes
- Workflows and builders: WF-04 Source-code access in nodes; WF-06 Chat-first build with optional graph; WF-08 Agent-ops/evaluation loop; WF-05 YAML round-trip
- Sandboxing and execution: SB-03 Full Linux desktop in the workspace
- Productivity, PIM and documents: PD-02 Email triage/drafts
- Platform, security and operations: PL-05 Loopback-only / secure-by-default packaging; PL-08 Write-ahead journal of buffers and agent sessions; PL-07 Chat debug view; PL-09 Calm AI: one-switch full opt-out
- Accessibility: AX-03 Accessible linear view for agent streams
- Diagrams, visuals and artifacts: DA-03 Shared live canvas with browser annotate mode

Row columns: ID | Feature | Seen in | Sc (score) | ★ | Why keep / note | Ev.

## Editor


| ID    | Feature                                                                                                                             | Seen in                               | Sc | ★ | Note                                                                   | Ev      |
| ------- | ------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------- | ---- | ---- | ------------------------------------------------------------------------ | --------- |
| ED-01 | Multi-cursor, column select, expand/shrink selection, add-next-match; find/replace (case, word, regex, preserve case, in selection) | VS Code, JetBrains, Zed ...           | 5  |    | Monaco core; Helix/Neovim multi-selection; editor tier 1 built 2026-10-17 on the kit editor (`project/built/code-editor.md`) | inf     |
| ED-02 | Folding (regions), word wrap, rulers, indent detect, encodings, auto-close brackets                                                 | all major IDEs                        | 5  |    | folding off: kit folding costs ~25 ms per key and crashed on a big broken file (2026-10-17) | inf     |
| ED-03 | Format document/selection, on save/paste/type; Auto Save, Hot Exit                                                                  | all major IDEs                        | 5  |    | agents share the formatter; dirty-buffer conflicts                     | inf     |
| ED-04 | Diff editor (inline/side-by-side), compare with clipboard/saved                                                                     | VS Code, JetBrains, Zed ...           | 5  |    | core of agent change review                                            | inf     |
| ED-05 | Minimap, sticky scroll, breadcrumbs, indent guides, bracket colorization, snippets, Emmet (?)                                       | VS Code, JetBrains, Zed ...           | 4  |    |                                                                        | inf (?) |
| ED-06 | 3-way merge editor                                                                                                                  | VS Code, JetBrains, Sublime Merge ... | 4  |    | human approval view for agent-resolved conflicts                       | inf (?) |
| ED-07 | Markdown preview/editor, image preview                                                                                              | VS Code, JetBrains, Zed ...           | 4  |    | agent artifacts render as Markdown/HTML                                | inf     |
| ED-09 | Notebooks (kernel picker, variables, data viewer)                                                                                   | VS Code, JetBrains, Jupyter ...       | 4  |    | must-keep 15; agent runs cells                                         | inf     |
| ED-10 | HTTP/REST client                                                                                                                    | VS Code ext, JetBrains, Postman       | 3  |    | must-keep 15                                                           | inf (?) |
| ED-08 | Hex/binary view (?); overtype, manual folding ranges, wrapped tabs                                                                  | VS Code ext, others                   | 2  |    | nice                                                                   | inf (?) |
| ED-11 | Rope + anchors buffer, per-actor undo, agents as attributed edit peers                                                              | Zed                                   | 1  | ★ | must-consider: stable positions for diagnostics, comments, agent edits | V       |

## Workbench and layout


| ID    | Feature                                                                                                             | Seen in                            | Sc | ★ | Note                                   | Ev      |
| ------- | --------------------------------------------------------------------------------------------------------------------- | ------------------------------------ | ---- | ---- | ---------------------------------------- | --------- |
| WB-01 | Command Palette, Quick Open (prefixes `>` `@` `:` `#` (?))                                                          | VS Code, JetBrains, Zed ...        | 5  |    | every agent action also a command; built 2026-10-19: files, `>`, `:`, `?` (`@` `#` wait for LSP symbols) | inf     |
| WB-02 | Editor groups, tabs, split/grid, preview tabs; six-area layout (editor, side bars, activity bar, panel, status bar) | all major IDEs                     | 5  |    | secondary side bar hosts chat          | inf     |
| WB-03 | Explorer, Open Editors, Outline, Timeline                                                                           | VS Code, JetBrains, Zed ...        | 5  |    | Timeline = per-file agent edit history; explorer tree T1 built 2026-10-17 (`project/built/file-tree.md`) | inf     |
| WB-07 | Themes (color, file icon, product icon), `colorCustomizations` (?)                                                  | all major IDEs                     | 5  |    | import VS Code theme JSON              | inf     |
| WB-04 | Problems panel, Output channels, Notifications + Do Not Disturb                                                     | VS Code, JetBrains, Zed            | 4  |    | Problems = structured feed for agents  | inf     |
| WB-05 | Workspace Trust / Restricted Mode, trusted folders                                                                  | VS Code, Gemini CLI, JetBrains ... | 3  |    | gates agents, terminal, tasks, debug   | doc     |
| WB-06 | Multi-root workspaces (?)                                                                                           | VS Code, JetBrains, Sublime        | 3  |    |                                        | inf (?) |
| WB-09 | Flexible panel layout: drag/snap to edges, tab group and split by drop, resize, close, zoom, auto-hide, pop-out windows, named and task layouts | Visual Studio, JetBrains, VS Code, Eclipse, Blender | 5  |    | T1+T2 committed, T3 deferred; `project/built/panel-layout.md` | doc     |
| WB-08 | Zen mode, centered layout, modal editors, welcome/walkthroughs (floating windows moved to WB-09)                    | VS Code, JetBrains, Zed            | 3  |    | nice                                   | inf (?) |

## Settings and profiles


| ID    | Feature                                                                                                                              | Seen in                      | Sc | ★ | Note                                          | Ev      |
| ------- | -------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------ | ---- | ---- | ----------------------------------------------- | --------- |
| ST-01 | Settings UI + JSON, scopes (default/user/remote/workspace/folder), keybindings with `when` and chords, keymap presets (Vim, Sublime) | all major IDEs               | 5  |    | must-keep 7, 14                               | doc     |
| ST-04 | Telemetry/update controls; localization packs                                                                                        | VS Code, JetBrains, Zed      | 4  |    | telemetry off or opt-in is P0                 | doc     |
| ST-06 | Import VS Code settings/keymaps/themes                                                                                               | Cursor, Windsurf, Zed ...    | 4  |    | highest migration value per effort (?)        | inf (?) |
| ST-02 | Profiles (settings, keybindings, MCP servers, snippets, tasks, extensions), templates, `.code-profile` export                        | VS Code, Zed, JetBrains (?)  | 3  |    | natural unit for agent configs                | doc     |
| ST-03 | Settings Sync                                                                                                                        | VS Code, JetBrains, Zed (?)  | 3  |    | own backend or git; Microsoft's is closed (?) | inf (?) |
| ST-05 | Layered config default < user < profile < workspace; trust-gated workspace layer; git-syncable, agent-editable                       | Zed (VS Code scopes partial) | 2  | ★ | nice: agent-editable config as normal files   | doc     |

## Language intelligence


| ID    | Feature                                                                                                       | Seen in                     | Sc | ★ | Note                                                                                                    | Ev   |
| ------- | --------------------------------------------------------------------------------------------------------------- | ----------------------------- | ---- | ---- | --------------------------------------------------------------------------------------------------------- | ------ |
| LS-01 | Generic LSP 3.17 client: diagnostics, completion, hover, signature help                                       | VS Code, Zed, Neovim ...    | 5  |    | closed MS servers (Pylance, C# Dev Kit, C/C++) not reusable: use basedpyright, Roslyn/OmniSharp, clangd; rust-analyzer client built 2026-10-17: diagnostics, hover, completion; servers to become installable packages | doc  |
| LS-02 | Definition/type/implementation/references, rename, symbols; code actions, refactor, organize imports          | all major IDEs              | 5  |    | symbol-aware agent tools; deterministic code actions; go to definition built 2026-10-17 (F12) | doc  |
| LS-03 | Built-in web languages (JSON/HTML/CSS/Markdown), TypeScript                                                   | VS Code, JetBrains, Zed ... | 5  |    | TS 7 speaks LSP                                                                                         | doc  |
| LS-04 | Semantic tokens, inlay hints, call/type hierarchy, code lens, folding ranges                                  | VS Code, JetBrains, Zed ... | 4  |    | syntax-aware chunking for agents                                                                        | doc  |
| LS-06 | In-process tree-sitter + built-in async LSP client                                                            | Zed, Neovim, Helix          | 3  |    | highlight/outline/inline values without server                                                          | V    |
| LS-05 | Inline values, linked editing, moniker, document color                                                        | VS Code, JetBrains, Zed     | 3  |    | nice                                                                                                    | doc  |
| LS-07 | Carbon-owned rust-analyzer config (scoped check, priming off for huge repos), server state shown to UI/agents | Zed docs                    | 1  | ★ | nice: RA ~12-16 GB on Zed repo                                                                          | A, V |

## Debugging


| ID    | Feature                                                                                                                                                                               | Seen in                               | Sc | ★ | Note                                        | Ev       |
| ------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------- | ---- | ---- | --------------------------------------------- | ---------- |
| DB-01 | Generic DAP client: launch/attach, line/conditional/hit-count breakpoints, step, stack, variables, watch, console; `launch.json` with substitution, `preLaunchTask`, compound configs | VS Code, Zed, Neovim ...              | 5  |    | must-keep 2; VS Code wins adapter breadth   | doc, inf |
| DB-02 | Logpoints, function/data/exception breakpoints, `runInTerminal`, child sessions; JS debug terminal/auto-attach, source maps                                                           | VS Code, JetBrains, Visual Studio ... | 4  |    | agent sets logpoints instead of print edits | inf      |
| DB-03 | Memory/disassembly, step-back, instruction breakpoints                                                                                                                                | VS Code, Visual Studio, CLion ...     | 3  |    | nice; Zed has only planned                  | inf (?)  |
| DB-04 | Native DAP: cache-first data layer, tree-sitter inline values, structured stop/stack/variable stream to agents; bundled Rust-aware adapter                                            | Zed                                   | 1  | ★ | nice: debugger as agent feedback source     | V        |
| DB-05 | Language-specific debugger value renderers                                                                                                                                            | JetBrains (RustRover)                 | 1  | ★ | nice                                        | inf      |

## Testing


| ID    | Feature                                                                                       | Seen in                               | Sc | ★ | Note                                         | Ev      |
| ------- | ----------------------------------------------------------------------------------------------- | --------------------------------------- | ---- | ---- | ---------------------------------------------- | --------- |
| TS-01 | Test Explorer tree, run/debug per node, filters, gutter results, inline errors; coverage view | VS Code, JetBrains, Visual Studio ... | 4  |    | structured results = best agent feedback     | inf (?) |
| TS-03 | Result ingestion (JUnit XML, TAP, cargo test JSON); own adapter format                        | CI tools, JetBrains                   | 3  |    | do not copy VS Code Testing API (unverified) | inf (?) |
| TS-04 | Continuous run, test profiles                                                                 | VS Code, JetBrains, Wallaby (?)       | 3  |    | nice                                         | inf (?) |
| TS-02 | Fix Test Failure (AI)                                                                         | VS Code Copilot, JetBrains AI (?)     | 2  |    |                                              | inf (?) |

## Tasks


| ID    | Feature                                                                                                                                                     | Seen in                                       | Sc | ★ | Note                                                 | Ev  |
| ------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------- | ---- | ---- | ------------------------------------------------------ | ----- |
| TK-01 | `tasks.json` (shell/process, dependsOn, groups, presentation, inputs, OS overrides); problem matchers (`$tsc`, `$eslint-stylish`, `$go`, regex, background) | VS Code, JetBrains run configs, Zed tasks ... | 4  |    | projects work as-is; task output becomes diagnostics | doc |
| TK-02 | `runOn: folderOpen`                                                                                                                                         | VS Code                                       | 1  |    | gate by Workspace Trust                              | doc |

## Search


| ID    | Feature                                                                                    | Seen in                       | Sc | ★ | Note                                                          | Ev            |
| ------- | -------------------------------------------------------------------------------------------- | ------------------------------- | ---- | ---- | --------------------------------------------------------------- | --------------- |
| SR-01 | Workspace search: regex, globs, gitignore-aware, replace with diff preview, case modifiers | all major IDEs                | 5  |    | same engine backs agent grep (ripgrep (?))                    | doc           |
| SR-03 | Semantic/codebase index                                                                    | Cursor, Copilot, Windsurf ... | 4  |    | design unverified                                             | S (?)         |
| SR-02 | Quick Search; Search Editor                                                                | VS Code, JetBrains, Sublime   | 3  |    |                                                               | inf           |
| SR-05 | Local-first semantic index with Merkle sync; agentic grep first                            | Cursor, Claude Code (grep)    | 2  | ★ | nice: p99 time-to-first-query 4.03 h -> 21 s, +12.5% accuracy | V             |
| SR-04 | Priority-scheduled search (ripgrep as library), path and symbol indexes                    | Zed                           | 1  | ★ | nice: first match 16.8 s -> 32 ms, total +18-33%              | M first-party |

## Source control


| ID    | Feature                                                                                                              | Seen in                             | Sc | ★ | Note                                                                                | Ev      |
| ------- | ---------------------------------------------------------------------------------------------------------------------- | ------------------------------------- | ---- | ---- | ------------------------------------------------------------------------------------- | --------- |
| SC-01 | Status, diff, stage by line/hunk, commit, branch, push/pull, stash; merge conflict UI; blame, inline gutter, history | all major IDEs                      | 5  |    | must-keep 5; git CLI shell-out likely (?)                                           | inf     |
| SC-02 | Worktrees first-class (create, list, per-session), setup scripts, port allocation                                    | Cursor, VS Code, Codex ...          | 4  |    | one worktree per agent session                                                      | V       |
| SC-03 | Source Control Graph, Timeline                                                                                       | VS Code, JetBrains, GitLens ...     | 4  |    |                                                                                     | inf     |
| SC-04 | GitHub PR/issue integration, create-PR form, stacked PRs                                                             | VS Code, JetBrains, Cursor ...      | 4  |    | agent monitors PR, fixes review comments                                            | inf     |
| SC-06 | Keyboard hunk/line staging; SCM provider abstraction; gitoxide reads + git CLI writes                                | Magit, lazygit, JetBrains; gitoxide | 3  |    | gitoxide 1.8-2.4x faster than git2 (M single author) but lacks rebase/push/checkout; gitoxide status reads in use since 2026-10-17 (Explorer markers) | A, M    |
| SC-05 | Interactive rebase editor (?), SVN/Hg                                                                                | JetBrains, GitLens, VS Code ext     | 3  |    | nice                                                                                | inf (?) |

## Terminal


| ID    | Feature                                                                                                                    | Seen in                    | Sc | ★ | Note                                                                                                 | Ev      |
| ------- | ---------------------------------------------------------------------------------------------------------------------------- | ---------------------------- | ---- | ---- | ------------------------------------------------------------------------------------------------------ | --------- |
| TM-01 | PTY terminals, tabs, split, profiles, ConPTY on Windows, links, find, scrollback                                           | all major IDEs             | 5  |    | agent-controllable objects; ConPTY quirks; tier 1 built 2026-10-18: alacritty_terminal in core, profiles, find, links, scrollback (`project/built/terminal.md`) | inf     |
| TM-02 | Shell integration OSC 633/133 (command start/end, exit code, cwd) for bash/zsh/fish/pwsh; command blocks exposed to agents | VS Code, Warp, kitty ...   | 4  |    | command boundaries without screen scraping; `alacritty_terminal` + own renderer; libghostty-vt alpha; OSC 633 for PowerShell/bash/zsh built 2026-10-18: exit-code marks, cwd, command records | doc     |
| TM-03 | Persistent sessions, reconnect, detach/attach; decorations, sticky scroll, run recent command, quick fixes, IntelliSense   | VS Code, tmux, Warp ...    | 3  |    | pairs with background agent shells                                                                   | doc     |
| TM-04 | Sixel/iTerm inline images                                                                                                  | VS Code, iTerm2, kitty ... | 3  |    | nice                                                                                                 | inf (?) |

## Extensions


| ID    | Feature                                                                | Seen in                                | Sc | ★ | Note                                                                                  | Ev     |
| ------- | ------------------------------------------------------------------------ | ---------------------------------------- | ---- | ---- | --------------------------------------------------------------------------------------- | -------- |
| EX-01 | Themes, TextMate grammars, snippets, keymaps (declarative extensions)  | VS Code, Zed, Sublime ...              | 5  |    | highest migration value per effort                                                    | doc    |
| EX-02 | MCP servers, tool contributions, agent/skill packs as extensions       | VS Code, Cursor, Claude Code ...       | 5  |    | native, not via VS Code API                                                           | doc    |
| EX-03 | Install/enable/disable UI, recommendations (`.vscode/extensions.json`) | all major IDEs                         | 5  |    | no silent auto-update                                                                 | inf    |
| EX-04 | Run unmodified VS Code extensions (Node host + `vscode` API)           | Cursor, Windsurf, Theia ...            | 4  |    | phase 3 earliest; no API parity promise                                               | S      |
| EX-08 | Open VSX + own gallery, signed, no silent auto-update                  | Cursor (?), Windsurf (?), VSCodium ... | 4  |    | malware detections 27 -> 105 (M via press); replaces MS-only Remote, Pylance, C#, C++ | M, S   |
| EX-05 | Webviews, custom editors, notebook renderers                           | VS Code, Theia, JetBrains (JCEF) (?)   | 3  |    | MCP Apps (`ui://`) cover agent UI                                                     | doc    |
| EX-09 | Agent marketplaces / plugin hubs                                       | Dify, Kilo, Agent Zero (100+) ...      | 3  |    |                                                                                       | S      |
| EX-06 | WASM extensions (WIT, versioned API), user-granted capabilities        | Zed, Lapce (?), Zellij (?)             | 2  | ★ | must-consider (pillar 6): no shared host; narrow API needs MCP escape hatch           | V, doc |
| EX-07 | Declarative/sandboxed UI contributions                                 | MCP Apps, Zed (no custom panels)       | 1  | ★ | must-consider (pillar 6)                                                              | doc    |

## Remote and web


| ID    | Feature                                                                                                                                                        | Seen in                             | Sc | ★ | Note                                                                               | Ev     |
| ------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------- | ---- | ---- | ------------------------------------------------------------------------------------ | -------- |
| RM-01 | SSH remote with auto-installed server                                                                                                                          | VS Code, Zed, JetBrains ...         | 5  |    | must-keep 3; MS Remote-SSH closed; Carbon daemon over SSH                          | doc    |
| RM-02 | WSL, Dev Containers (`devcontainer.json`, Features)                                                                                                            | VS Code, JetBrains, Cursor ...      | 4  |    | container per agent session                                                        | doc    |
| RM-03 | Tunnels / browser-hosted client; port forwarding; remote FS provider                                                                                           | VS Code, Codespaces, Gitpod ...     | 4  |    | cf. `code agent host --tunnel`; discoverable tunnel + auto-approve is a known risk | doc, S |
| RM-04 | Managed cloud workspaces/agents (Codespaces-style, VM per task)                                                                                                | Codespaces, Gitpod, Devin ...       | 4  |    |                                                                                    | inf    |
| RM-06 | Headless Rust daemon (static musl) owning workspace, buffers, terminals, LSP/DAP, agents, git; local = remote over loopback; many clients; remote-drivable TUI | Zed remote, opencode, OpenHands ... | 4  |    | one backend for local/SSH/tunnel; musl avoids glibc breakage (?)                   | V, doc |
| RM-05 | Remote reattach (SSH ControlMaster, proxy survives dropped link)                                                                                               | Zed, VS Code Server (?), tmux       | 3  |    | Zed beta, ~100k-file limit (?)                                                     | V, doc |

## Collaboration


| ID    | Feature                                                            | Seen in                                                                      | Sc | ★ | Note                                                                 | Ev  |
| ------- | -------------------------------------------------------------------- | ------------------------------------------------------------------------------ | ---- | ---- | ---------------------------------------------------------------------- | ----- |
| CO-01 | Live-Share-style co-editing, shared workspaces                     | VS Code Live Share (closed), Zed channels, Code With Me (sunset Q1 2027) ... | 4  |    | must-keep 12; human+agent co-edit is the relevant case               | doc |
| CO-03 | Share session transcripts/artifacts; GitHub PR review/comments     | Claude Code, Cursor, Copilot ...                                             | 3  |    |                                                                      | inf |
| CO-05 | Take over the agent's IDE; local-to-cloud `/handoff` (teleport)    | Devin, Claude Code                                                           | 2  | ★ | nice                                                                 | S   |
| CO-02 | Multiplayer CRDT editing (human + agent)                           | Zed channels                                                                 | 1  | ★ | nice: defer; pick CRDT by own benchmark; Xi author doubted value (S) | doc |
| CO-04 | Share vs collaborate separation (logs/artifacts vs sandbox access) | Manus                                                                        | 1  | ★ | nice                                                                 | S   |

## Inline completion


| ID    | Feature                                                                                    | Seen in                               | Sc | ★ | Note                                                                         | Ev  |
| ------- | -------------------------------------------------------------------------------------------- | --------------------------------------- | ---- | ---- | ------------------------------------------------------------------------------ | ----- |
| IC-01 | Inline suggestions (ghost text)                                                            | Copilot, Cursor, Zed ...              | 5  |    | low-latency completion model                                                 | inf |
| IC-03 | Inline chat, smart actions (commit message, explain)                                       | Copilot, Cursor, JetBrains ...        | 5  |    | easy full disable (must-keep 10)                                             | doc |
| IC-02 | Next Edit Suggestions / next-edit prediction (local Zeta-class model, accept-rate logging) | Cursor Tab, Copilot NES, Zed Zeta ... | 4  |    | Cursor Tab RL: +28% accept, 21% fewer suggestions; separate low-latency path | V   |

## Agents


| ID    | Feature                                                                                                                           | Seen in                                 | Sc | ★ | Note                                             | Ev     |
| ------- | ----------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------- | ---- | ---- | -------------------------------------------------- | -------- |
| AG-01 | Agent sessions as primary surface; parallel, grouped, searchable                                                                  | VS Code, Cursor, Zed ...                | 5  |    | chat panel v1 with demo backend built 2026-10-19 (`project/built/chat.md`) | doc, V |
| AG-02 | Autonomous agent execution with tools                                                                                             | Claude Code, Cursor, Copilot ...        | 5  |    |                                                  | inf    |
| AG-03 | Subagents / multi-agent teams, agent groups, custom agents with handoffs, agent-as-tool                                           | Claude Code, Copilot, Agno ...          | 5  |    |                                                  | doc    |
| AG-06 | Hub over third-party agents (ACP); harness plurality (Copilot SDK, Claude Agent SDK, Codex, cloud agents) via separate agent host | OpenHands, Zed, Warp ...                | 4  |    | Carbon: ACP client/host; ACP maturity unverified | doc, S |
| AG-07 | Scheduled/cron agents and automations (`.automation.md`, routines)                                                                | Odysseus, AnythingLLM, LobeHub ...      | 4  |    |                                                  | doc    |
| AG-05 | Durable execution, checkpoints, HITL                                                                                              | LangGraph, Pydantic AI (9 engines), n8n | 3  |    |                                                  | doc    |
| AG-13 | Kanban-style agent command center                                                                                                 | Devin Desktop, Vibe Kanban (?)          | 2  | ★ | nice: low-authority sources                      | S      |
| AG-10 | NL-to-agent builder ("AutoPilot")                                                                                                 | AutoGPT Platform, Sim                   | 2  | ★ | nice                                             | S      |
| AG-08 | Code-as-action                                                                                                                    | smolagents                              | 1  | ★ | nice                                             | doc    |
| AG-09 | Automatic prompt optimization                                                                                                     | DSPy                                    | 1  | ★ | nice                                             | doc    |
| AG-11 | Self-evolving skills                                                                                                              | Odysseus                                | 1  | ★ | nice                                             | S      |

## Approvals and safety


| ID    | Feature                                                                                                                                                            | Seen in                                      | Sc | ★ | Note                                                                        | Ev     |
| ------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------- | ---- | ---- | ----------------------------------------------------------------------------- | -------- |
| AP-01 | Per-action approval; per-tool/per-command auto-approve                                                                                                             | Claude Code, Cline, Cursor ...               | 5  |    |                                                                             | doc    |
| AP-08 | Per-hunk accept/reject of AI edits; Changes view with inline review comments                                                                                       | Cursor, Copilot, Windsurf ...                | 5  |    | must-keep 11; VS Code Keep/Undo only in Local harness                       | doc, A |
| AP-02 | Plan mode (read-only) / Plan-Act split                                                                                                                             | Claude Code, Cline, opencode ...             | 4  |    |                                                                             | doc    |
| AP-03 | Graded permission modes (default, acceptEdits, plan, auto, dontAsk, bypass; manual, LLM judge, allow all, autopilot)                                               | Claude Code, VS Code, Cursor ...             | 4  |    | approval fatigue is the central UX problem                                  | doc    |
| AP-06 | Hooks on tool calls/prompts (SessionStart, UserPromptSubmit, PreToolUse, PostToolUse, PreCompact, SubagentStart/Stop, Stop; Claude-format; ~30 events in opencode) | Claude Code, opencode, Claude Agent SDK ...  | 4  |    | normalize schema (gap 6)                                                    | doc    |
| AP-11 | OS sandbox of terminal, MCP and LSP servers                                                                                                                        | Claude Code, Gemini CLI, Codex ...           | 3  |    | Carbon: on by default                                                       | V, doc |
| AP-05 | Policy engine                                                                                                                                                      | Gemini CLI, Claude Code managed settings (?) | 2  |    |                                                                             | doc    |
| AP-04 | Unified approval policy engine: tiers + reasoning-blind classifier + hard hooks, escalate after repeated denials                                                   | Claude Code (`auto` classifier)              | 1  | ★ | must-consider: 93% of manual prompts approved; classifier FPR 0.4%, FNR 17% | V      |
| AP-07 | Cross-session approval inbox                                                                                                                                       | none (VS Code gap)                           | 1  | ★ | must-consider: approvals are per session today                              | doc    |
| AP-09 | One review model (diff-first, revertible) across all harnesses                                                                                                     | none (VS Code gap)                           | 1  | ★ | must-consider                                                               | doc    |
| AP-10 | Per-session FS/network policy tied to worktree                                                                                                                     | none (VS Code gap)                           | 1  | ★ | must-consider: worktrees do not restrict commands/network                   | doc    |
| AP-12 | Destructive-git guard (`reset --hard`, `clean`, `checkout .` outside agent's own worktree)                                                                         | none; documented losses                      | 1  | ★ | must-consider; Copilot CLI `git clean -fd` (issue #1675, single user) (?)   | A      |

## Checkpoints


| ID    | Feature                                                                                                         | Seen in                                   | Sc | ★ | Note                                       | Ev  |
| ------- | ----------------------------------------------------------------------------------------------------------------- | ------------------------------------------- | ---- | ---- | -------------------------------------------- | ----- |
| CK-01 | Checkpoints / snapshots / undo and restore                                                                      | Cline, Gemini CLI, opencode (`/undo`) ... | 5  |    | file-tool-only everywhere                  | doc |
| CK-02 | Side-effect-aware checkpoints: pre-turn FS snapshots incl. untracked/ignored files, irreversible-effects ledger | none (gap)                                | 1  | ★ | must-consider: documented bash work losses | A   |

## Sessions


| ID    | Feature                                                                                   | Seen in                                       | Sc | ★ | Note                                                                           | Ev  |
| ------- | ------------------------------------------------------------------------------------------- | ----------------------------------------------- | ---- | ---- | -------------------------------------------------------------------------------- | ----- |
| SE-01 | Per-session worktree/folder isolation, commit/PR                                          | Cursor, VS Code, Codex ...                    | 4  |    |                                                                                | doc |
| SE-03 | Sessions survive window close; reconnect and catch-up                                     | VS Code Agent Host, opencode, Claude Code ... | 4  |    |                                                                                | doc |
| SE-04 | Agent-host protocol: state-first (snapshot, ordered actions, replay); REST/SSE agent APIs | VS Code AHP, opencode (OpenAPI 3.1 + SSE)     | 2  | ★ | must-consider: ACP at external edge, MCP for tools; wire format unverified (?) | doc |

## Context


| ID    | Feature                                                                                                                                             | Seen in                              | Sc | ★ | Note                            | Ev  |
| ------- | ----------------------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------- | ---- | ---- | --------------------------------- | ----- |
| CX-01 | Read `AGENTS.md`, `CLAUDE.md`, `.github/copilot-instructions.md`, `*.instructions.md`, `*.prompt.md`, `*.agent.md`, `.mcp.json`, `.vscode/mcp.json` | Claude Code, opencode, LibreChat ... | 5  |    | instant ecosystem compatibility | doc |
| CX-03 | Customization resolver showing precedence across `.github/`, `.claude/`, `.agents/`, `~/.copilot`                                                   | none (VS Code gap)                   | 1  | ★ | nice                            | doc |

## MCP and skills


| ID    | Feature                                                                                                                  | Seen in                                                                    | Sc | ★ | Note                                     | Ev     |
| ------- | -------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------- | ---- | ---- | ------------------------------------------ | -------- |
| MS-01 | MCP client: stdio, Streamable HTTP, resources, prompts, elicitation, sampling, OAuth; OpenAPI tool import; 100+ toolkits | near-universal (Claude Code, Cursor, VS Code, Zed, Open WebUI, Dify, Agno) | 5  |    |                                          | doc    |
| MS-02 | Skills (`SKILL.md` in `.agents/`, `.claude/`, `.github/skills`)                                                          | Claude Code, opencode, LibreChat ...                                       | 5  |    |                                          | doc    |
| MS-04 | Plugin system (filters, pipes, tools, skills); typed hooks                                                               | Open WebUI, Dify, Agent Zero ...                                           | 3  |    |                                          | doc    |
| MS-03 | MCP Apps (`ui://` sandboxed iframes), AG-UI streams, generative UI                                                       | MCP Apps spec, CopilotKit, VS Code (?)                                     | 3  |    | client support and spec dates unverified | doc, S |
| MS-05 | Flow deployed as MCP server; pieces as MCP servers                                                                       | Langflow, Activepieces, Kestra                                             | 3  |    |                                          | doc    |
| MS-07 | MCP registry/gateway (Docker MCP Gateway, MetaMCP, Smithery); deferred/semantic tool loading against schema bloat        | MCP Registry, Docker, Smithery                                             | 3  |    |                                          | S      |

## Specs


| ID    | Feature                                     | Seen in                        | Sc | ★ | Note                                | Ev |
| ------- | --------------------------------------------- | -------------------------------- | ---- | ---- | ------------------------------------- | ---- |
| SP-01 | Plan/spec artifacts with comment-and-revise | Kiro specs, Antigravity (thin) | 2  | ★ | nice: spec-driven evidence weak (?) | S  |

## Models and chat


| ID    | Feature                                                                                        | Seen in                                 | Sc | ★ | Note                                                             | Ev   |
| ------- | ------------------------------------------------------------------------------------------------ | ----------------------------------------- | ---- | ---- | ------------------------------------------------------------------ | ------ |
| MD-01 | Multi-provider chat, local + cloud; OpenAI-compatible endpoint; BYOK, model picker             | Open WebUI, LibreChat, Jan ...          | 5  |    |                                                                  | inf  |
| MD-02 | Model hot swap/management; gateways/routing (LiteLLM, OpenRouter, "Omni" router by input type) | Dify, HF Chat UI, Jan                   | 3  |    |                                                                  | S    |
| MD-06 | Personas/assistant library (300+)                                                              | Big-AGI, Cherry, LobeHub                | 3  |    |                                                                  | S    |
| MD-07 | Voice, multimodal input; resumable streams                                                     | Big-AGI, Cherry, Agno ...               | 3  |    |                                                                  | S    |
| MD-10 | Instrument accept rate, approval rate, rework, cost per task                                   | Cursor, Claude Code                     | 2  | ★ | nice: METR n=16 (M, small): self-perceived speed-up can be wrong | V, M |
| MD-03 | Built-in model serving + hardware-aware picks ("Cookbook")                                     | Odysseus (Ollama, llama.cpp, vLLM, MLX) | 1  | ★ | nice                                                             | S    |
| MD-04 | Blind model comparison                                                                         | Odysseus                                | 1  | ★ | nice                                                             | S    |
| MD-05 | Beam multi-model compare/merge                                                                 | Big-AGI                                 | 1  | ★ | nice                                                             | S    |
| MD-09 | Live per-session cost meter; no billing for failed requests                                    | none (Copilot quota opacity)            | 1  | ★ | must-consider: cost transparency (gap 4)                         | A    |

## Knowledge, RAG and memory


| ID    | Feature                                                                                | Seen in                                                                  | Sc | ★ | Note                                            | Ev   |
| ------- | ---------------------------------------------------------------------------------------- | -------------------------------------------------------------------------- | ---- | ---- | ------------------------------------------------- | ------ |
| KM-01 | Knowledge base / RAG pipeline; agentic RAG                                             | Dify, AnythingLLM, Open WebUI ...                                        | 5  |    |                                                 | S    |
| KM-03 | Persistent memory                                                                      | Odysseus (Chroma + fastembed), Open WebUI, Letta ...                     | 5  |    |                                                 | S    |
| KM-02 | Workspace-per-document-set with cited answers; multi-vector-DB support                 | AnythingLLM; Dify, Open WebUI                                            | 3  |    |                                                 | S    |
| KM-05 | Temporal/graph memory layers                                                           | Zep/Graphiti, Cognee, Mem0                                               | 3  |    | benchmarks vendor-authored, conflicting: ignore | S    |
| KM-08 | Document parsing (PDF, OCR, Office -> Markdown)                                        | Odysseus (pypdf, markitdown, pymupdf (?)), Dify (paddleocr (?)), Docling | 3  |    | Docling best layout but heavy; PyMuPDF AGPL     | M, S |
| KM-04 | User-editable white-box memory; per-project isolated memory and secrets                | LobeHub; Agent Zero                                                      | 2  | ★ | nice                                            | S    |
| KM-06 | Memory-as-OS (agent-managed memory)                                                    | Letta                                                                    | 1  | ★ | nice                                            | S    |
| KM-07 | 50+ enterprise connectors, ACL-aware retrieval; MCP server exposing context under ACLs | Onyx                                                                     | 1  | ★ | must-consider (gap 5)                           | S    |

## Tools, search and research


| ID    | Feature                                                                     | Seen in                                                      | Sc | ★ | Note                                                | Ev        |
| ------- | ----------------------------------------------------------------------------- | -------------------------------------------------------------- | ---- | ---- | ----------------------------------------------------- | ----------- |
| TR-01 | Web search tool, incl. private (SearXNG)                                    | Odysseus, Open WebUI, LibreChat ...                          | 5  |    | hosted: Tavily/Brave/Exa/Firecrawl, no head-to-head | M (noise) |
| TR-04 | Browser agent/automation, integrated browser, computer use (screen control) | Browser Use, OpenManus, Agent Zero ...                       | 4  |    | connectors preferred over screen control (S)        | S         |
| TR-03 | Deep research engine                                                        | Odysseus (Tongyi claim unverified), Onyx, GPT-Researcher ... | 3  |    |                                                     | S         |
| TR-02 | Search modes (speed/balanced/quality), academic sources                     | Perplexica/Vane                                              | 1  | ★ | nice                                                | S         |

## Workflows and builders


| ID    | Feature                                                                                            | Seen in                      | Sc | ★ | Note                                                          | Ev     |
| ------- | ---------------------------------------------------------------------------------------------------- | ------------------------------ | ---- | ---- | --------------------------------------------------------------- | -------- |
| WF-01 | Visual drag-drop canvas, conditionals/loops, custom tool nodes; interactive playground; prompt IDE | Dify, Langflow, Sim ...      | 5  |    | Flowise sunset: rigid low-code outcompeted by coding agents   | doc, S |
| WF-02 | Flow to API/JSON export                                                                            | Langflow, Dify, n8n ...      | 4  |    |                                                               | doc    |
| WF-03 | Integrations library (1,500+)                                                                      | n8n, Pipedream, Activepieces | 3  |    |                                                               | S      |
| WF-09 | Task queues/schedulers (Celery, Trigger.dev, Temporal, Inngest)                                    | Dify, Sim, Agno              | 3  |    |                                                               | S      |
| WF-04 | Source-code access in nodes; scripts -> auto UI -> flows -> apps                                   | Langflow, Windmill           | 2  | ★ | nice                                                          | doc    |
| WF-06 | Chat-first build with optional graph                                                               | Sim, AutoGPT AutoPilot       | 2  | ★ | must-consider (gap 7)                                         | S      |
| WF-08 | Agent-ops/evaluation loop                                                                          | Coze Loop, Dify LLMOps       | 2  | ★ | nice                                                          | S      |
| WF-05 | YAML round-trip (YAML source of truth + no-code editor)                                            | Kestra                       | 1  | ★ | must-consider: precedent for diagram <-> text sync (pillar 2) | doc    |

## Sandboxing and execution


| ID    | Feature                                                                | Seen in                                   | Sc | ★ | Note                                               | Ev |
| ------- | ------------------------------------------------------------------------ | ------------------------------------------- | ---- | ---- | ---------------------------------------------------- | ---- |
| SB-01 | Sandboxed code execution                                               | Dify, LibreChat, Onyx ...                 | 5  |    | least standardized layer; no independent benchmark | S  |
| SB-02 | Per-conversation container; per-task cloud VM; plugin daemon isolation | OpenHands; Manus, Devin; Dify; Gemini CLI | 4  |    |                                                    | S  |
| SB-03 | Full Linux desktop in the workspace (XFCE, LibreOffice)                | Agent Zero                                | 1  | ★ | nice (unique)                                      | S  |

## Productivity, PIM and documents


| ID    | Feature                                                                                                                                                 | Seen in                                                               | Sc | ★ | Note                                                 | Ev |
| ------- | --------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------- | ---- | ---- | ------------------------------------------------------ | ---- |
| PD-03 | Multi-surface clients (browser, Obsidian, Emacs, phone, WhatsApp); messaging/channel interfaces (Slack, Telegram); embeddable widget; desktop app + CLI | Khoj; Agno, LobeHub, OpenHands automations; AnythingLLM; Langflow ... | 4  |    |                                                      | S  |
| PD-01 | AI-assisted document workspace/editor                                                                                                                   | Odysseus, Agent Zero, ChatGPT canvas (?)                              | 3  |    |                                                      | S  |
| PD-02 | Email triage/drafts (IMAP/SMTP), notes, tasks and calendar (CalDAV)                                                                                     | Odysseus; Open WebUI (notes, calendar)                                | 2  | ★ | must-consider (gap 5); scope question in PROJECT.md | S  |

## Platform, security and operations


| ID    | Feature                                                                                                                                                                                           | Seen in                                                        | Sc | ★ | Note                                                                   | Ev      |
| ------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------- | ---- | ---- | ------------------------------------------------------------------------ | --------- |
| PL-12 | Windows/macOS/Linux parity                                                                                                                                                                        | VS Code, JetBrains, Zed ...                                    | 5  |    | Zed Windows/Linux late (?)                                             | inf (?) |
| PL-03 | Observability/tracing (OTel; Langfuse, Opik, Phoenix, LangSmith); OpenTelemetry in IDE                                                                                                            | Dify, Langflow, Agno ...                                       | 4  |    | Phoenix ELv2                                                           | doc     |
| PL-02 | Multi-tenant security: JWT, RBAC, SSO; 2FA                                                                                                                                                        | Agno, Open WebUI, Dify ...                                     | 4  |    |                                                                        | S       |
| PL-11 | Fast startup, low RAM as product feature                                                                                                                                                          | Zed, Sublime (<1 s, <200 MB), Neovim ...                       | 4  |    | structural weakness of Electron                                        | A, V    |
| PL-04 | Docker Compose packaging, Kubernetes; push notifications (ntfy)                                                                                                                                   | Odysseus, Dify, Open WebUI                                     | 3  |    |                                                                        | S       |
| PL-01 | REST/SSE/WebSocket APIs (50+ endpoints); control-plane UI                                                                                                                                         | Agno AgentOS, opencode, MS Agent Framework DevUI               | 3  |    |                                                                        | doc     |
| PL-10 | Native GPU UI (Rust) vs webview                                                                                                                                                                   | Zed GPUI, Warp, Lapce (?)                                      | 3  |    | benchmark first (2x-15x conflicting); Tauri gain small; WebKitGTK risk | V, A    |
| PL-05 | Loopback-only / secure-by-default packaging                                                                                                                                                       | OpenHands, Odysseus (after early exposed defaults, #187)       | 2  | ★ | must-consider                                                          | S       |
| PL-08 | Write-ahead journal of buffers and agent sessions; non-destructive restore (never open empty); per-agent processes; unresponsiveness detection with profiling; extension bisect, Process Explorer | VS Code (hot exit, bisect), Zed (buffer loss #52038 shows gap) | 2  | ★ | must-consider: stability is must-keep 8                                | doc, A  |
| PL-07 | Chat debug view, prompt-cache explorer                                                                                                                                                            | VS Code                                                        | 1  | ★ | nice                                                                   | doc     |
| PL-09 | Calm AI: one-switch full opt-out, no nagging, telemetry off or opt-in                                                                                                                             | none (top user complaint)                                      | 1  | ★ | must-consider: fixable by Carbon                                       | A       |

## Accessibility


| ID    | Feature                                                                                                                                                                                                  | Seen in                                    | Sc | ★ | Note                                | Ev      |
| ------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------- | ---- | ---- | ------------------------------------- | --------- |
| AX-01 | Screen readers (NVDA, JAWS, VoiceOver, Orca), Accessibility Help (Alt+F1), Accessible View (Alt+F2), Accessible Diff Viewer, audio/announcement signals, F6 region navigation, high-contrast theme, zoom | VS Code, JetBrains, Visual Studio          | 3  |    | VS Code wins clearly; Zed paid late | doc     |
| AX-02 | AccessKit from the first widget                                                                                                                                                                          | Zed (~May 2026, PR status unverified)      | 2  |    | hardest for Rust-native UI          | inf (?) |
| AX-03 | Accessible linear view for agent streams, diffs, tool calls; announcements for agent events                                                                                                              | VS Code Accessible View (extension needed) | 1  | ★ | must-consider                       | doc     |

## Diagrams, visuals and artifacts


| ID    | Feature                                                     | Seen in                             | Sc | ★ | Note                                           | Ev |
| ------- | ------------------------------------------------------------- | ------------------------------------- | ---- | ---- | ------------------------------------------------ | ---- |
| DA-01 | Artifacts / generative UI / canvases (React, HTML, Mermaid) | LibreChat, Open WebUI, Onyx ...     | 4  |    |                                                | S  |
| DA-02 | Math/diagram rendering (KaTeX, Mermaid)                     | Odysseus, Open WebUI, LibreChat ... | 4  |    |                                                | S  |
| DA-03 | Shared live canvas with browser annotate mode               | Agent Zero                          | 1  | ★ | must-consider: closest to shared-artifact idea | S  |

# Part 3. Market gaps (inference, no user research)

1. Unified sessions, diffs, plans and approvals across heterogeneous agents (ACP + MCP + A2A): AG-06, AP-07, AP-09, SE-04.
2. Human and agent co-editing the same live artifact beyond Agent Zero's canvas: DA-03, CO-02, ED-11, PA-07.
3. One approval policy model unifying rules, classifier review and sandbox tier, with audit trail: AP-04, AP-10, AP-11.
4. Cost and run-trace transparency for non-developers: MD-09, PL-03.
5. ACL-aware unified context (mail, calendar, docs), secure by default: KM-07, PD-02, PL-05.
6. Normalized hook/event schema across vendors: AP-06, CX-03.
7. Chat-first agents with optional visual flows: WF-06, WF-05.
