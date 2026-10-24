# Agents extension: agent flowcharts (n8n-like), research and decisions

Written 2026-10-24. User request: an extension **Agents** (`carbon-extensions-agents/`) adding a new panel type, an interactive flowchart whose blocks are AI agents joined by arrows, to automate agent flows (like n8n). Research first, then decisions (s3), spike results (s4). Plan: `~/.claude/plans/now-i-am-thinking-sprightly-simon.md` (phases P0-P8). Related: `extensions.md` (extension system), `../project/archived/workflow-system.md` (shelved workflows; Q17-Q20), `../DESIGN.md` canvas rules (:100-110) and agent UX (:175-188).

## 1. What other products do

| Product (status) | Node types | Edges carry | Execution | Stored as | Canvas UX | Pain points | Ev |
|---|---|---|---|---|---|---|---|
| n8n (Sustainable Use licence) | Triggers, AI Agent root node + sub-nodes (model, memory, tools, parser), IF/Switch, Merge, Loop Over Items, Code, Wait, sub-workflow, Error Trigger | Arrays of `{json, binary}` items; special `ai_*` connections for sub-nodes | Trigger-driven DAG, branch/merge, wait/resume, error workflow, per-tool human approval | JSON (`nodes[]` + `connections` by name); git sync Enterprise only | Palette, inspector, per-node I/O, pinned test data, execution log | Agent silently skips tools; memory lost on restart; runaway loops/tokens; big JSON diffs | doc, A |
| Flowise Agentflow V2 (Workday since 2025-08-14) | 14: Start, LLM, Agent, Tool, Retriever, HTTP, Condition (+Agent), Iteration, Loop, Human Input, Direct Reply, Custom Function, Execute Flow | `{{ }}` refs + shared `$flow.state` | Cycles only via Loop (max, default 5); HITL checkpoints survive restart; SSE streaming | DB JSON, export (?) | Iteration as container | (?) | doc |
| Langflow | Agent, models, tools (Tool Mode), If-Else, Loop, MCP tools | **Typed colour ports** (Message, JSON, DataFrame) | Mostly DAG; Freeze caches upstream | JSON export (may include keys) | Playground | Spaghetti past ~50 nodes (?); upgrades break flows; CVE-2025-3248 | doc |
| Dify | Start, LLM, Agent, Knowledge, Classifier, If/Else, Code, Template, HTTP, Aggregator, Iteration, Loop, Human Input | Named variables | Parallel branches, fail branch per node, retries | **YAML DSL** (versions drift) | Polished, run logs | DSL compat; licence dispute | doc |
| LangGraph + Studio | Code functions; conditional edges; `Send` fan-out; `Command` | **One state object with reducers** | Pregel supersteps; cycles (limit 1000); checkpoints; `interrupt()` / resume | Code (Studio views it) | Trace, time-travel, fork from checkpoint | No visual editing; pre-interrupt code re-runs on resume | doc |
| OpenAI Agent Builder (**deprecated 2026-06-03, off 2026-11-30**) | Start, Agent, Note, File search, Guardrails, MCP, If/else, While (CEL), Human approval, Transform, Set state | Text + state vars | While loop; guardrail fail branch | Hosted; exports Agents SDK code | Drag-drop, preview | Lock-in, then shutdown | doc |
| Rivet (Ironclad, MIT) | Chat, Assemble Prompt, subgraphs, Loop Controller | Typed ports | DAG; Loop Controller only cycle | **YAML `.rivet-project` in repo** | Desktop app, remote debugger | Cadence (?) | doc |
| ComfyUI | Image/model nodes; subgraphs, blueprints | Typed ports | Topological; **outputs cached by input hash** | JSON (also in PNG metadata) | Node library, minimap, subgraphs | Huge graphs | doc |
| CrewAI | `@start/@listen/@router` flows; Crew Studio (hosted) | Return values, typed state | Parallel, `@persist` | Code; Studio exports code | Drag-drop + copilot | (?) | doc |
| MS Agent Framework (AutoGen maintenance mode) | Executors; direct/conditional/fan-out/fan-in edges; patterns | Typed messages | Supersteps, checkpoints, `RequestInfo` HITL | Code + declarative YAML (Power Fx) | Mermaid/Graphviz export only | Python < C# | doc |
| Claude Code subagents / VS Code 1.109 | Markdown agents (`tools`, `model`, `isolation`) | Prompt in, summary out | Parallel (20), nesting 3, resumable | `.claude/agents/*.md` in repo | **No graph view** | No visual composition | doc |

Standard everywhere: start/trigger, agent, tool/MCP, router, bounded loop, human approval, code, sub-flow; inspector side panel, per-node I/O, run trace, iteration caps. Differentiators: edge semantics (typed ports vs shared state), durability (checkpoint/resume, time-travel), caching unchanged nodes, **graph as a text file in the repo**. Pitfalls: >50-node spaghetti (plan grouping), JSON diffs (stable ids, sorted keys, layout apart), credentials in exports, upgrades breaking flows (version node types), runaway loops/cost (caps + budget), side effects re-run on resume, silent tool skipping (trace every tool call). **Gap: no IDE composes coding agents visually.**

References: canvas gpui-flow (GPUI, React-Flow style, 2026-03), egui-snarl (viewer trait); format Oracle Open Agent Spec (arXiv 2510.04173); A2A for remote agents (protocol, not a graph format).

## 2. Carbon today (2026-10-24, M)

- Agent backend: demo only (`carbon-core/src/chat/demo.rs`; `ChatBackend` trait `chat/backend.rs:69`, `ChatStore::set_backend`). `agent.rs` is a data model. No ACP/MCP/provider code.
- Extension UI: views (static widgets, `view.rs`), webview behind the `webview` feature (off; native window above GPUI; no page API). No canvas.
- Extension code: one call at a time, 5 s (`wasm.rs` `CALL_LIMIT`), 256 MB; no jobs, events, storage, file writes; `agent.invoke` and `fs.write` capabilities parse but have no host functions.
- GPUI has the primitives: `canvas()`, `PathBuilder::cubic_bezier_to`, `paint_path`, hitboxes, drag/scroll events; kit has no node editor (sankey link path `vendor/gpui-base/src/plot/shape/sankey.rs:905` is a model).
- `lsp::find_program` tries only `.exe` on Windows (npm/npx are `.cmd`).

## 3. Decisions (user, 2026-10-24)

| Question | Options | Picked |
|---|---|---|
| Agent block | ACP agent programs / direct model API / both | **External agent programs over ACP**, Claude Code first |
| Canvas | native graph panel kind / webview (React Flow) / built-in panel | **New generic native `graph` panel kind** drawn by Carbon (GPUI); extension declares block types; reusable for the paradigm editor |
| Storage | YAML in project / app data / both | **YAML in the project** (`.carbon/flows/*.flow.yaml`), stable ids, layout apart, no secrets |
| Execution | steps + bounded loops / free graph with cycles | **Steps in order, parallel branches, bounded Loop the only cycle**, approval pauses, trace, budget |
| Runner | logic in extension + generic host / runner in core | **Logic in the extension's WASM**; Carbon adds generic capabilities (ACP sessions, jobs + events, storage, file writes, graph I/O, approvals) |
| Blocks v1 | | Start, Agent, Output, Router, Loop (max N), Approval, Parallel + Merge |

## 4. Spike results (M, 2026-10-24, Windows 11)

Claude Code adapter `@agentclientprotocol/claude-agent-acp` **0.95.0** (bin `dist/index.js`, node >= 22; this machine node 24.18, npm 11.16), run with `npx -y ...@0.95.0`, scratch folder, user signed in to Claude Code:

| Check | Result |
|---|---|
| Framing | One JSON-RPC message per line on stdout; adapter logs on stderr (`[session/create] ... phase=...`) |
| Start time | `initialize` answered after 11.6 s (npx resolve + node start; first run) |
| `initialize` | protocolVersion 1; `loadSession`, session list/resume/fork/close, prompt image + embedded context, MCP http/sse; `agentInfo` title "Claude Agent" |
| Auth | Uses the Claude Code login (subscription): `_auth/status_update` notifications report `{kind: "account", label: "Claude Pro"}`; no API key needed; no `authenticate` call |
| `session/new` | 0.8 s; returns `modes`: `default` (Manual), `acceptEdits`, `plan`, `auto`, `bypassPermissions` -> Carbon permission modes map onto these via `session/set_mode` |
| Updates | `available_commands_update` (the user's own plugins and skills), `session_info_update` (title), `usage_update {used, size, cost {amount, currency}}`, `agent_message_chunk` |
| Prompt result | `{stopReason: "end_turn", usage {inputTokens, outputTokens, cachedReadTokens, cachedWriteTokens, totalTokens}}` plus per-model `_meta.quota` |
| Cost | **Reported** (`usage_update.cost`): a one-word reply cost USD 0.33 notional, because each new session writes ~41.6 k tokens of context to cache (the user's Claude setup, skills and plugins load into every session) |
| Shutdown | Adapter exits when stdin closes; no orphan `node` left after killing the `npx.cmd` process (a Job Object is still worth having) |
| Rust crates | `agent-client-protocol-schema` 1.12.0 (wire types); `serde_yaml_ng` 0.10.0 (YAML); `serde-saphyr` 2.0.0 is an alternative |

Consequences: per-session context is expensive, so flows should reuse a session across steps where it makes sense (`session: reuse`) and the budget must count cache writes; the user's personal Claude plugins load into flow agents (show this; later an option to start clean if the adapter allows).

## 5. Sources

n8n docs (flow logic, data structure, HITL tools, source control); Flowise Agentflow V2 docs, Workday newsroom 2025-08-14; Langflow docs (data types, loop, components, export, release notes); Dify docs (key concepts, agent node, error handling, licence); LangGraph docs (graph API, interrupts, Studio); OpenAI developer docs (node reference, Agent Builder, deprecations); Rivet site and docs; ComfyUI blog/docs; CrewAI docs; Microsoft Learn agent-framework workflows; Claude Code subagents docs; VS Code blog 2026-02-05; docs.rs egui-snarl; users.rust-lang.org gpui-flow post (2026-03-23); github.com/oracle/agent-spec; agentclientprotocol.com protocol overview; docs.rs agent-client-protocol(-schema); npm `@agentclientprotocol/claude-agent-acp`.
