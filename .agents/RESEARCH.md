# Carbon research: landscape, VS Code reference, synthesis

Snapshot Oct 2026 (VS Code 1.142). Star counts drift. Evidence labels: **M** measured (method disclosed), **V** vendor claim, **A** anecdotal, **S** secondary, **doc** docs/spec, **inf** inferred, (?) unverified. Almost all comparative evidence is V or A; no independent same-hardware benchmark of VS Code vs Zed/Neovim/JetBrains exists. Priorities, adoption scores and fork verdicts are inference, not usage data.

**Contents:** 1 Bottom line - 2 Landscape - 3 Common stack - 4 VS Code reference - 5 Comparison and synthesis - 6 Community sentiment - 7 Patterns and gaps - 8 Evidence gaps and benchmarks - 9 Key sources

## 1. Bottom line

- No product matches Carbon. Closest: Odysseus (PIM + agents), Agent Zero (shared canvas), OpenHands Canvas / Zed / Warp / Devin Desktop (multi-agent hubs), Agno AgentOS (runtime + control plane), Sim (workspace-style canvas). Others are chat boxes, graph builders or code-only harnesses.
- Open space: neutral, secure-by-default workspace where human and agent co-edit visible state, with unified approvals, diffs and traces across heterogeneous agents.
- VS Code can be matched without forking: reuse open protocols (LSP, DAP, MCP, ACP, TextMate/theme JSON, `AGENTS.md`/`SKILL.md`/`.mcp.json`) and embeddable parts; rebuild agent-facing layers natively. A fork only buys the `vscode` extension API, which is also where licensing bites.
- VS Code weaknesses: structural to Electron + shared Node extension host (RAM, startup, one extension stalls all) and Microsoft's legal position (closed Remote-SSH/WSL/Tunnels, Live Share, Pylance, C# Dev Kit, C/C++). Still wins on ecosystem, debugging breadth, remote, accessibility (why users return). Agent stack is feature-rich but weak on security defaults and cross-harness consistency.
- Positioning: fast and calm by default beats VS Code; agent-native beats Zed and Sublime.
- Sequencing: Rust daemon (sessions, worktrees, PTY with OSC 633, LSP/DAP/MCP/ACP clients, policy engine) -> UI shell with VS Code importers -> Open VSX with declarative and LSP/DAP-launcher extensions -> then decide Node extension host or native UI.

## 2. Landscape (agent workspaces)

### A. Chat workspaces and personal agent hubs
| Product | License | Stack | Notable |
|---|---|---|---|
| Odysseus (~93k, May 2026) | AGPL-3.0-or-later | Python (FastAPI, S), SQLite (S), ChromaDB, SearXNG, ntfy (Compose-confirmed); Swift MLX *image* bridge | Chat+agents (opencode, MCP, shell, skills, memory), deep research, blind Compare, docs editor, IMAP/SMTP, notes/tasks/CalDAV, scheduled agents, 2FA. Breadth over depth; large privileged attack surface, no described sandbox; #187 (early Docker defaults exposed ChromaDB/ntfy, mostly fixed); 262 open PRs, no releases at 2 Jun. README: `AUTH_ENABLED=true`, `LOCALHOST_BYPASS=false` |
| Open WebUI (~156k) | custom, branding clause | Python, Svelte (inf), SQLite/Postgres, Redis | Most mature chat/RAG hub: plugins, MCP/OpenAPI, RBAC, memory, notes, calendar, automations, OTel |
| LibreChat, AnythingLLM | MIT | Node/Redis; Vite+React/LanceDB | Artifacts, no-code agents, SKILL.md, MCP, resumable streams; workspace per document set, citations, cron |
| LobeHub (~83k) | ambiguous | Next.js | Agent groups, white-box editable memory, shared workspaces |
| Onyx (~32k) | MIT CE / EE | Python, Next.js | Agentic RAG, 50+ connectors, MCP client **and server with ACLs** |
| Khoj (AGPL), Big-AGI (MIT), HF Chat UI (Apache), Perplexica/Vane (MIT), desktop: Jan (Apache, Tauri), Cherry (AGPL), Chatbox (GPL-3), Msty (closed) | | | Multi-surface; Beam merge; "Omni" routing; search-first; desktop rarely integrates email/calendar |

### B. Autonomous / computer-use agents
| Product | License | Notable |
|---|---|---|
| Agent Zero (~19.5k) | not verified | Docker + Python. Linux desktop (XFCE, LibreOffice) in canvas, browser annotate, live doc editors, isolated projects, Plugin Hub (100+), MCP+A2A, Time Travel snapshots (diff/revert). **Closest to shared-canvas idea** |
| OpenHands | MIT | Python SDK + REST Agent Server, React "Agent Canvas", Docker per conversation; drives Claude Code, Codex, Gemini, OpenCode, any ACP agent |
| Manus (Meta) | closed | VM per task; sharing logs/artifacts separated from sandbox collaboration |
| OpenManus (~59k), Browser Use (~118k; cloud $0.02/browser-hour) | MIT | CLI/MCP browser agents; OpenManus multi-agent flow "unstable" |
| Anthropic computer use (beta), Cowork, OpenAI Operator (now ChatGPT agent) | closed | S only; connectors preferred over screen control |

### C. Coding agents and shells
| Product | License | Notable |
|---|---|---|
| opencode (~214k) | MIT | `opencode serve` OpenAPI 3.1 + SSE, multi-client, build/plan agents, ~30 plugin hooks, `/undo`; Kilo CLI is a fork |
| Claude Code | proprietary (?) | One engine across CLI/desktop/web/mobile/IDE; permission modes incl. classifier `auto`; teleport; routines |
| Codex, Gemini CLI, Cline, Goose, Aider, Kilo | Apache/MIT | Gemini: trusted folders, checkpoints, policy engine, ACP. Cline: per-edit approval, Plan/Act. Aider: git commit = undo. Goose: MCP recipes (Rust) |
| Zed (GPL-3+), Warp (AGPL), Cursor, Devin/Windsurf ("Devin Desktop") | mixed/closed | ACP host / BYO CLI agents in terminal workspace; Kanban-style command centers (low-authority sources) |
| Roo Code (shut down May 2026), Continue (archived) | | IDE-only agents dying |

### D. Visual / low-code builders
| Product | License | Notable |
|---|---|---|
| Dify (~158k) | Apache-2.0 + extra conditions | Canvas, prompt IDE, RAG, LLMOps, apps as API, MCP client. Stack from Railway templates (S); Zustand/React Flow unsupported |
| Langflow (~156k) | MIT | Python/uv, React Flow (topic); API/MCP server deploy; IBM via DataStax; CVE history unchecked |
| Flowise | Apache-2.0 | **Sunset** (freeze 29 Jul, archive 10 Aug, EOL 31 Aug 2026; Workday): coding agents outcompete rigid low-code |
| n8n (~208k), Coze Studio (~22k) | fair-code (not OSI); Apache-2.0 | 1,500+ integrations, HITL, MCP; Go + React microservices, Coze Loop AgentOps |
| Sim (~30k) | Apache-2.0 | Next.js, Bun, Postgres, ReactFlow, Socket.io, Trigger.dev, E2B. **Closest architectural analog**: realtime collab, tables/files/KB, visual/chat/code |
| Activepieces (MIT), Windmill (AGPL; Rust, nsjail), Kestra (Apache), AutoGPT Platform (Polyform Shield) | | pieces as MCP servers; scripts -> UI -> flows; YAML source of truth; NL->agent |
| Commercial: OpenAI AgentKit, Gemini Enterprise Agent Platform, AWS AgentCore, Copilot Studio, Pipedream (Workday), Lindy, Gumloop, Relevance, StackAI | closed | Credit-billing opacity main complaint |

### E. Code-first frameworks
- **Agno** (~43k, Apache-2.0): AgentOS, 50+ endpoints (REST/SSE/WS), JWT RBAC, scheduler, tracing, control-plane UI, Slack/Telegram/AG-UI/A2A. **Closest OSS workspace runtime.**
- LangGraph/Deep Agents, CrewAI, OpenAI Agents SDK, Google ADK, MS Agent Framework (DevUI), Strands, Pydantic AI (nine durable engines), Mastra, Vercel AI SDK: durable execution, HITL, handoffs, tracing; vendor-cloud gravity. Also smolagents (code-as-action), DSPy, Letta (memory-as-OS), ElizaOS, LlamaIndex, Haystack, CAMEL, MetaGPT.
- Claude Agent SDK (MIT repo, commercial terms): embeds Claude Code harness; hooks, permission modes, in-process MCP.

## 3. Common stack

| Layer | Shared choices | Note |
|---|---|---|
| Frontend | React/Next.js dominant; Svelte (Open WebUI, HF); canvas: React Flow (Langflow, Sim), Svelte (Windmill), Vue (Kestra). Desktop: Electron (Cherry, Chatbox), Tauri (Jan, Cline), Rust native (Zed, Warp) | React Flow licence unverified |
| Backend / data | Python most common; Node/TS (n8n, LibreChat, Sim); Rust (Goose, Codex, Windmill); Go (Coze). Postgres + Redis servers, SQLite local | |
| Vector / memory | Chroma, LanceDB, Qdrant, pgvector, Weaviate, Milvus; fastembed local. Memory: Mem0, Letta, Zep/Graphiti (dropped self-hosted CE), Cognee | Benchmarks vendor-authored, conflicting (Mem0 49% vs Zep 63.8% / Letta 83.2% LongMemEval): ignore |
| Protocols | **MCP near-universal**; MCP Apps (SEP-1865, `ui://` sandboxed iframes; official ~Jan 2026, one source); A2A (agent-agent); ACP (editor-agent); AG-UI (agent-frontend); A2UI; AGENTS.md + SKILL.md | A2A/A2UI/ACP/AG-UI maturity unverified |
| MCP infra | Registry (preview), Docker MCP Gateway, MetaMCP, Smithery | Schema bloat -> deferred/semantic tool loading |
| Models | Ollama, llama.cpp, vLLM, MLX; OpenAI-compatible endpoint; LiteLLM/OpenRouter | Ollama ~10-14% slower than llama.cpp; vLLM 2.8-6x Ollama under concurrency (V) |
| Sandbox | Docker, E2B (Firecracker, no GPU), Modal (gVisor, GPU), nsjail, Pyodide, VM per task | Least standardized; Daytona reportedly closed core dev Jun 2026 (?); no independent benchmark |
| Search / research | SearXNG + Crawl4AI local; Tavily/Brave/Exa/Firecrawl hosted (AIMultiple Dec 2025: Brave 14.89, Firecrawl 14.58, Exa 14.39, Tavily 13.67, within noise). Deep research: Tongyi, GPT-Researcher, open_deep_research, STORM | No head-to-head |
| Doc parsing | Docling (best layout, heavy), MarkItDown (light), PyMuPDF (AGPL), pypdf, PaddleOCR, Unstructured | READoc 2024: Docling 58.8, PyMuPDF4LLM 40.6, MarkItDown 35.2 |
| Observability / packaging | Langfuse (MIT, ClickHouse-owned), Opik, Phoenix (ELv2), LangSmith (closed), OTel GenAI (?); Docker Compose default, desktop + CLI second | |

Licence watch when bundling: PyMuPDF/SearXNG/Firecrawl (AGPL), Phoenix (ELv2), Claude Agent SDK (commercial), Open WebUI branding, Dify extra conditions, n8n fair-code, AutoGPT Polyform.

## 4. VS Code reference

**Layering.** `src/vs/`: base, platform, editor, workbench, code (desktop), server (remote), each split by runtime (common/browser/node/electron). `vs/editor` has no Node/Electron deps, so Monaco runs standalone. Rust crates: `base`, `platform` (traits), `editor-core`, `workbench`, `app-shell`, `server`.
**Processes.** Main, sandboxed renderer per window, extension host(s), shared process, pty host (watcher/PTY placement differs by source); extensions have full permissions.
**Agent Host (1.133, Aug 2026; the part to imitate).** Separate process owns harnesses and sessions; UIs render. Protocol AHP: JSON-RPC over WebSocket, immutable state, snapshot + ordered actions, catch-up on reconnect; sessions survive window close; client-contributed tools work only while connected. Wire format/licence unverified.

**Reusable.** Monaco (MIT (?), heavy), xterm.js (MIT (?)) + OSC 633 scripts + Rust PTY, generic LSP 3.17/DAP clients (adapters js-debug, codelldb, debugpy, delve; debug UI bespoke), ripgrep (?), git CLI, tree-sitter (?), ACP, MCP + Apps, Agent Skills/AGENTS.md, TextMate/theme JSON. Copilot Chat source (MIT, archived 20 May 2026): design reference only. Bespoke: testing API, tasks/problem matchers (regex data, cheap), SCM UI, merge editor. TS 7 (Go port) speaks LSP; replace MS-only servers with basedpyright, Roslyn/OmniSharp, clangd.

**Fork vs rebuild.**
| Option | Verdict |
|---|---|
| A. Fork Code-OSS (Cursor, Windsurf, Antigravity, Void, Kiro (?)) | Only if arbitrary VS Code extensions are top priority: merge treadmill, no Marketplace/MS extensions, TS/Electron |
| B. Eclipse Theia (1.74, EPL) | Lower merge burden, VS Code extension compatible, AI-first; TS/Node, slower startup, not Rust |
| C. Rust core + Tauri webview (Monaco, xterm.js) | **Recommended start**; budget Linux WebKitGTK QA or fall back to bundled Chromium |
| D. Native Rust UI (GPUI/egui/iced/Slint/Dioxus) | Later, hot surfaces only if profiling demands; editor, IME, accessibility rebuilt; GPUI stability unverified, Zed editor crates GPL; other toolkits' text input not evaluated |
| E. Rust daemon + swappable frontends | **Rule for C and D** (mirrors VS Code Server, Theia, `opencode serve`) |

Primary surface is threads, diffs, plans, terminals, so C+E is lowest risk. ("Cursor 3 dropped the fork" single-source (?).)

**Extension strategy.**
1. No full `vscode` API parity (Node host + RPC, ~25 activation events, ~38 contribution points, webview/notebook hosts: multi-year). Optional layer, phase 3+.
2. Tiers: (a) declarative VSIX (themes, grammars, snippets, keymaps); (b) thin LSP/DAP launchers; (c) code extensions via Node host subset; (d) webview-heavy, proposed-API, MS-proprietary: unsupported.
3. Registry: Open VSX + mirror, allow-list, signatures, block list, no silent auto-update. Open VSX ~3,000 vs 48,000+ Marketplace (dated); GlassWorm worm first seen 17 Oct 2025; scanner fail-open bug fixed 11 Feb 2026; Nx Console breach via auto-update (AI-assisted CSA note, cautious). Do not recommend extensions absent from your registry (namespace squat, S).
4. Licensing: Code-OSS MIT; Marketplace terms limit use to MS products (C/C++ v1.24.5, 3 Apr 2025 blocks forks); Remote-SSH/WSL/Tunnels, server binaries, Live Share, C# debugger restricted; Dev Containers spec open, extension likely proprietary (?).
5. Agent-native points first: MCP servers, skills, hooks, custom agents, MCP Apps.

**VS Code agent weaknesses -> Carbon response.** Sandbox off by default, network open, Windows experimental -> secure by default, OS sandbox. Auto-approve "not a security boundary", deny needs hook -> one policy engine where deny denies. Worktrees do not restrict commands/network -> per-session FS/network policy. Checkpoints files only -> include side effects. Keep/Undo review only in Local harness -> one review model. Approvals per session -> cross-session inbox. Local harness tied to window -> daemon-first. Spec-driven flow only in Kiro (S) -> plan/spec artifacts with comment-and-revise. Hook schemas differ (`chat.useClaudeHooks` maps Claude hooks) -> normalized schema. Customization sprawl (`.github/`, `.claude/`, `.agents/`, `~/.copilot`) -> one resolver showing precedence.

## 5. Comparison and synthesis

"Best" = best-evidenced direction, not proven measurement.
| Area | VS Code | Best alternative | Evidence | Carbon adopts |
|---|---|---|---|---|
| Shell/UI | Electron | Zed GPUI; Tauri middle | V/A, 2x-15x conflicting (Zed 0.6 s/222 MB vs VS Code 1.3 s/3,549 MB; 180 ms vs 2.1 s; input ~2 vs ~12 ms, below many users' perception). Tauri vs Electron idle 109 vs 128 MB, startup 311 vs 273 ms (small). Fleet killed | Rust core as local server; native GPU UI if latency/RAM central; benchmark first |
| Editor | Monaco | Zed rope + CRDT anchors; Helix | None (Fatin 2015 typing study stale; one blog A; Xi author doubted CRDT (S)) | Rope + anchors; defer CRDT; Monaco/CM6 interim |
| Language | LSP in ext host; TextMate | Zed in-process tree-sitter + LSP | V reasoning. rust-analyzer ~12-16 GB on Zed repo (A); Zed docs blame `checkOnSave --workspace --all-targets`; TS7 ~11.9x (S) | tree-sitter + async LSP; own RA config |
| Debug | DAP, broad | Zed DAP (cache-first); RustRover renderers | VS Code wins breadth (inf); Zed DAP Jun 2025, watch/memory planned | Rust DAP client, Rust-aware adapter, agent stream |
| Search | ripgrep | Cursor Merkle index; Zoekt | M first-party: Zed search total +18-33%, first match 16.8 s -> 32 ms. V: Cursor p99 4.03 h -> 21 s, +12.5% accuracy | ripgrep lib + priority scheduling; optional embeddings; agentic grep first |
| Terminal | xterm.js, OSC 633 | `alacritty_terminal`; Warp blocks; libghostty-vt (alpha) | M old/low rigor: macOS VS Code editor input 62.5 ms vs kitty 29-38; Windows conhost 33-41, Alacritty 62.5, Windows Terminal 66.7; VS Code untested; VS Code wins features (inf) | `alacritty_terminal` + OSC 633/133 blocks; ConPTY quirks |
| Git | SCM, GitLens | Magit/lazygit; JetBrains | A. gitoxide 1.8-2.4x faster than git2 (M single author); no rebase/push/checkout | gitoxide reads, CLI writes; worktree-per-agent (setup scripts, port allocation) |
| Extensions | Shared Node host | Zed WASM + WIT; MCP | V design; malware detections 27 -> 105 (M via press); Zed API narrow, no custom panels | WASM + MCP; Open VSX + own gallery; replace MS-only Remote, Pylance, C#, C++ |
| Settings | JSON, Profiles, Sync, Trust | Zed layered, trust-gated | doc; Sync not compared | Layered, trust-gated, git-syncable |
| Accessibility | Chromium DOM, Accessible View | none; Zed inaccessible through 2025, AccessKit ~May 2026 (inf) | doc/issues: VS Code wins | AccessKit from first widget; linear agent-stream surface |
| Remote | VS Code Server (glibc >= 2.28) | Zed headless server + reattach; JetBrains thin client | None measured; Zed beta, ~100k-file limit (?); glibc breakage hits Cursor too (A) | One daemon, loopback = remote; ControlMaster; static musl (inf); devcontainers; token-auth tunnels (discoverable tunnel + auto-approve is a known risk) |
| Collaboration | Live Share (closed) | Zed channels (CRDT) | doc; Code With Me sunset (final Q1 2027) | Daemon attach for humans + agents; CRDT later |
| AI/agent | Agent Host, Copilot | Cursor (Tab RL, index, Bugbot); Claude Code approvals; ACP | V: Tab +28% accept, 21% fewer suggestions; 93% manual prompts approved, classifier FPR 0.4%/FNR 17%; Bugbot precision 68.9% (V/S). METR RCT: AI tasks 19% slower, n=16 (M, small). Copilot weakness: quota opacity | AHP-like + ACP + MCP; tiered approvals + classifier + hooks; sandbox; snapshots; cost meter |
| Stability | Hot exit, separate ext host | Zed native | A/issues; no data-loss rates; ext-host stalls from agent extensions; Zed buffer loss #52038; Live Share broke in 1.102 | Write-ahead journal, non-destructive restore, per-agent processes |

### Proposed Carbon stack (beyond table picks)
1. **Daemon:** one static-musl Rust binary (workspace, buffers, terminals, LSP/DAP, agents, git); clients via Unix socket, SSH ControlMaster reattach, or WebSocket/tunnel with token auth. AHP-like state-first protocol, ACP external agents, MCP tools. Rope + anchors, per-actor undo, agents as attributed edit peers; native GPU UI if latency/RAM goal (webview fallback), AccessKit day one.
2. **Agent safety:** OS sandbox; tiered approvals + reasoning-blind classifier + deterministic hooks, escalate after repeated denials; pre-turn FS snapshots incl. untracked/ignored files + irreversible-effects ledger; destructive-git guard (`reset --hard`, `clean`, `checkout .` outside own worktree). Checkpoints are file-tool-only everywhere today.
3. **Defaults:** calm opt-out AI, telemetry off, live cost meter, no billing for failed requests, BYOK, VS Code import, write-ahead journal.
4. **Local-first components (inf):** MCP + MCP Apps host; AG-UI streaming; Langfuse/OTel; Ollama/llama.cpp via LiteLLM; SQLite + Chroma/LanceDB + fastembed; Docling + MarkItDown; SearXNG + Crawl4AI (Tavily/Brave optional); Docker/gVisor or Pyodide sandbox (E2B/Modal optional).

**Take / avoid.** VS Code: Agent Host protocol, OSC 633, accessibility, DAP breadth, devcontainers, SCM API, extension bisect/Process Explorer, Workspace Trust / shared Node host, licence handshake. Zed: rope+anchors, tree-sitter+LSP, WASM capabilities, reattach, channels, ACP / late accessibility, no UI escape hatch, restore bugs. Cursor: accept-rate loop, Merkle index, worktree agents, precision review / proprietary harness, opaque pricing. JetBrains: persisted indexes, debugger renderers, thin client / JVM UI. Claude Code: graded approvals + measured classifier / tool-level-only rewind. Neovim/Helix: multi-selection, modal / unsandboxed Lua. Warp: command blocks / proprietary. Theia: not researched.

## 6. Community sentiment

All A or S unless marked; no reaction counts retrievable; rankings qualitative.
- **Top complaints:** (1) AI/Copilot push and bloat (8 of 9 headline 1.102 features Copilot; users re-disable AI settings); (2) slowness/RAM/crashes (one 2019 MacBook: 4-6 s cold start, ~1.6 GB; ~300-500 MB Electron baseline is vendor-blog); (3) Microsoft control (C/C++, Pylance, Remote restricted; telemetry; legal facts); (4) release regressions (Live Share 1.102, vscode#255805); (5) Copilot billing/quota opacity, long-chat 413/400 errors, lost chat (vscode#252030).
- **Why leave:** speed, low RAM, stability, less AI nagging (Zed, Sublime <1 s start and <200 MB, Neovim); agent-first workflows (Cursor, Claude Code, which runs alongside editors (S)).
- **Why return:** Cursor "bloated, slow to start" while VS Code + Copilot reached parity; Zed gaps in 2024-25 posts (debugger, git integration, extensions, per-hunk AI accept/reject, side-by-side diff), partly stale since Zed 1.0 (29 Apr 2026); Sublime leavers miss Live Share, REST client, first-class terminal.
- **Surveys (M):** SO 2025 VS Code ~75.9%, Cursor ~18%, Claude Code ~10%. SO 2026 figures (VS Code 70.9%, Cursor 14.9%) unverified (403, inconsistent groupings), so "VS Code fell ~5 points" is weak. JetBrains Jan 2026: 90% use an AI tool at work; JetBrains 2025 (n=24,534): ChatGPT 41%, Copilot 30%, Cursor 13%.
- **Must-keep (inferred, not polled):** see `FEATURE.md` Part 1.3.
- **Structural vs fixable:** RAM, startup, shared ext host (Electron), closed MS extensions are structural; AI opt-out, telemetry defaults, git UX, regressions are fixable.

## 7. Patterns and gaps

**Patterns.** Headless core + many clients (opencode, OpenHands, Claude Code, Agno, Letta; ACP standardizes editor half). Graded autonomy (plan mode, approvals, policy engines, classifier, sandbox to relax approvals); approval fatigue is the central UX problem. Undo as safety net (git, checkpoints, snapshots, `/undo`). Source of truth: JSON graph vs YAML vs scripts vs code; Flowise's sunset suggests chat-first build plus optional graph. Isolation: plugin daemon, VM per task, container per conversation, loopback defaults. Share vs collaborate separation (Manus). Memory: white-box (LobeHub) vs agent-managed (Letta) vs per-project (Agent Zero). Consolidation: Flowise/Pipedream -> Workday, Manus -> Meta, Langflow -> IBM, Langfuse -> ClickHouse. Desktop clients rarely do PIM; web hubs are adding it.

**Gaps (analyst inference; no user research).** (1) Neutral workspace over heterogeneous agents (ACP + MCP + A2A): unified sessions, diffs, plans, approvals. (2) Live shared artifacts: human + agent co-edit beyond Agent Zero; MCP Apps host, AG-UI. (3) Approval UX and audit trail as product surface, one policy model. (4) Cost and run-trace transparency for non-developers; Langfuse/OTel export. (5) ACL-aware unified context (mail, calendar, docs), secure by default (loopback, auth on, no exposed vector DB/notification services). (6) Normalized hook/event schema. (7) Chat-first agents with optional visual flows.

## 8. Evidence gaps, unverified items, benchmarks to run

**Benchmarks to run ourselves** (numbers conflict or are vendor-only): Typometer-style keystroke-to-screen (Windows/macOS/Linux); idle/loaded RSS vs VS Code and Zed on identical hardware; RSS after N agent sessions; 100 MB file/long-line open; search first-match/total vs ripgrep; rust-analyzer scoped vs default config; remote reconnect time and server RSS; gitoxide vs git CLI on monorepos; CRDT (Zed-style, Diamond Types, Loro) on Carbon edit traces; AI loop (accept rate, approval rate, rework, cost per task; METR: self-perception unreliable); screen-reader pass (NVDA, JAWS, VoiceOver, Orca); `microsoft/vscode` open issues by thumbs-up for real complaint counts.

**Corrections still useful.** Odysseus: "Tongyi DeepResearch" absent from README (one secondary listing); multi-user support, Flask, Node frontend, llmfit, pypdf/markitdown/pymupdf, radicale, katex/mermaid unverified (opencode agent and CalDAV sync confirmed); security flaws known from a reposted article; MLX bridge is image-only, LLM MLX via mlx-lm/oMLX over OpenAI-compatible API; licence AGPL-3.0-or-later (some say MIT). Dify: Apache-2.0 plus additional conditions; observability list and paddleocr unverified. Langflow: React/TS/Vite, SQLAlchemy, "LangChain base" unverified.

**Unverified, landscape.** Dify/Langflow/Coze vector DB, queue, sandbox specifics; licences of Agent Zero, LobeChat, Claude Code, Vercel AI SDK, AutoGPT; Msty MCP; ChatGPT agent mode; Cursor/Devin/Windsurf/Manus internals and dates; Codex/Gemini sandbox details; MCP Apps client support and spec dates (client list S; "2026-07-28 spec" uncorroborated); A2A/A2UI/ACP/AG-UI maturity; assistant-ui, mcp-use, LiteLLM, Helicone, Pyodide (MPL-2.0 (?)), React Flow/Rete/tldraw licences; Daytona openness; resource footprints. Not researched: Vellum, Zapier/Make, BuildShip, Voiceflow, Retool, deepset Studio.

**Unverified, VS Code and comparison.**
- Background knowledge only: Monaco/xterm.js licences, ripgrep as engine, PTY/IPC details, Testing API, Settings Sync backend, Emmet, multi-root, Quick Open prefixes, AccessKit, `@devcontainers/cli` licence.
- Single/secondary: Cursor 3 dropping fork, Cursor ownership change (Jun 2026), Windsurf rename (2 Jun 2026)/"Devin Desktop", Windsurf $2.4B figure, Kiro as Code-OSS fork, Antigravity features, Void "no security fixes", Open VSX counts, Nx Console breach, GlassWorm totals, Vibe Kanban.
- Conflicts: TS 7 GA (July vs Aug 2026); JetBrains joining ACP (Oct 2025 vs Jan 2026); Claude Code approval rate 93% vs 97% and auto-mode default date 14 Aug 2026; Agents window preview date.
- Unverified claims: all Zed vs VS Code latency/startup/RAM numbers ("<10 ms vs >50 ms" S); TS7 memory figure; Cursor "475 -> 260 ms", Composer "4x", SWE-grep "10x", Bugbot 80%; Zed AccessKit merge/PR #54224/stable status; Copilot CLI `git clean -fd` (issue #1675, single user); Zed remote extension propagation and 100k-file limit; static musl fixing glibc (inf); Warp architecture; Theia, Neovim, JetBrains plugin isolation; Gitpod/Codespaces pricing.
- Not read: LSP 3.18, full DAP list, release notes 1.108-1.133/1.143+, Marketplace Terms and enforcement, AHP/Copilot SDK licences, MCP registry, hook exit codes, codebase index internals, Agent HQ. No evidence on: Windsurf/Junie/Cline/Amp/Codex comparisons, why users chose Windsurf/Kiro/Antigravity/Lapce/Helix, large-file/monorepo/WSL complaints.

## 9. Key sources

Labels as above. Domains abbreviated; search the title.

**Landscape.** Repos: odysseus-dev/odysseus, agent0ai/agent-zero, All-Hands-AI/OpenHands, agno-agi/agno, simstudioai/sim, FoundationAgents/OpenManus, browser-use/browser-use. implicator.ai Odysseus article (S; Compose services, #187, maturity). opencode.ai/docs, code.claude.com/docs (doc). flowiseai.com/sunset (doc); newsroom.workday.com 2025-11-19 (Pipedream). blog.modelcontextprotocol.io/posts/2025-11-21-mcp-apps, github.com/modelcontextprotocol/ext-apps (doc); docs.showcase.copilotkit.ai/agentic-protocols (V). research.aimultiple.com/agentic-search (M); arxiv.org/pdf/2409.05137 READoc (M); arxiv.org/pdf/2510.24701 Tongyi report; mcp.directory memory comparison, morphllm.com/ai-agent-sandbox (S/V, conflicting).

**VS Code.** code.visualstudio.com/docs/agents/concepts/agent-host, /agents/run/agent-sandboxing, /agents/run/approvals, /agent-customization/hooks, /terminal/shell-integration, /configure/accessibility/accessibility; /blogs/2026/08/26/agent-host-architecture (doc). github.com/microsoft/vscode/wiki/Source-Code-Organization, /wiki/Differences-between-the-repository-and-Visual-Studio-Code (doc); DAP spec microsoft.github.io/debug-adapter-protocol; LSP 3.17 spec. theregister.com/2025/04/24/microsoft_vs_code_subtracts_cc_extension; devclass.com/2025/04/08 marketplace wars (press). thehackernews.com/2025/10/self-spreading-glassworm-infects-vs, /2026/02/eclipse-foundation-mandates-pre-publish (press). research.contrary.com/company/anysphere (S); adtmag.com Theia 1.74 (S).

**Performance and comparison.** zed.dev/compare/vscode, /blog/videogame, tech-insider.org/zed-vs-vscode-2026, dev.to truongandev, vantaige.io Zed benchmark (V/A, conflicting). betterstack.com tauri-vs-electron (M); v2.tauri.app/develop/debug/linux-graphics (doc). pavelfatin.com/typing-with-pleasure (M stale); chadaustin.me/2024/02/windows-terminal-latency (M); dev.to/lkhrs/measuring-terminal-latency (M low rigor). zed.dev/blog/nerd-sniped-project-search (M first-party); cursor.com/blog/secure-codebase-indexing, /semsearch, /tab-rl (V). rust-lang/rust-analyzer discussions/20871 (A); zed.dev/docs/languages/rust.html; zed.dev/blog/debugger; zed.dev/releases/stable/1.0.0, 1.18.0; zed.dev/docs/remote-development.md, /collaboration/channels, /extensions/capabilities; zed.dev/blog/crdts (doc). github.com/o2sh/onefetch/pull/635 (M single author), Byron/gitoxide crate-status.md (doc). anthropic.com/engineering/claude-code-auto-mode (V); eweek METR coverage (M, n=16). zed-industries/zed issues #41138, #6576 (accessibility), #52038 (buffer loss). blog.jetbrains.com/platform/2026/03/sunsetting-code-with-me; mitchellh.com/writing/libghostty-is-coming; agentpatterns.ai remote-agent-host-sessions (S, tunnel risk); esecurityplanet malicious VS Code extensions (M via press).

**Sentiment and incidents.** devclass.com/2025/07/14 (1.102 AI bloat); tenthousandmeters.com/blog/i-switched-from-vscode-to-zed; pivotlog.net/switched-away-from-vs-code; forum.cursor.com/t/cursor-is-getting-worse-i-moved-back-to-vscode/159845; towardsdatascience.com/vscode-is-the-best-ai-powered-ide (A). microsoft/vscode issues #252030 (billing), #255805 (Live Share regression). claudeissues.com/issue/33850, eon.io claude-code-rewind-bash (A; destructive-git/checkpoint losses); claudeissues.com/issue/69473, helgeklein.com Copilot unresponsiveness (A). visualstudiomagazine.com 2025/08/01 (SO 2025, M); blog.jetbrains.com/research/2026/04/which-ai-coding-tools-do-developers-actually-use-at-work (M).
