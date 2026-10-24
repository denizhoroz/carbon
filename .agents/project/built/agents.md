# Agents extension and graph panels (built 2026-10-24)

User request (2026-10-24): an extension **Agents** that adds a new panel type, an interactive flowchart of AI agents connected by arrows, to automate agent flows (like n8n). Research, decisions and spike results: `../../documents/agents.md`. Built in phases P0-P7 (spikes, graph model, manifest + API versions, graph panel, ACP client, extension API 0.2, run surfaces, the extension); P8 leftovers are in `../not_built/README.md` ("Agents leftovers"). Start with "Status at a glance" and "Working on it" below.

Built as generic Carbon pieces (any extension can use them) plus the extension:

| Piece | Where | What |
|---|---|---|
| Graph model | core `graph.rs` (+ `graph/{doc,spec,yaml,validate,edit,history,run_state}.rs`) | YAML graph files (`carbon_graph: 1`, nodes by stable id, `node.port` edges, layout last; canonical bytes; logic hash ignores layout; unknown keys kept); node type specs from manifests (ports, `loop_back`, routes as ports, inspector fields); generic checks (types, ports, single-arrow inputs, `max_count`, required fields, cycles only into `loop_back`, only the loop's own arrows flagged); invertible edits with undo/redo (typing merges within 1 s); run state per node |
| Manifest | core `extension/manifest.rs` | `[[agents]]` (ACP programs: command, args, env, install hint, `[agents.npm]` package/version/bin/npx fallback), `[[graphs]]` (dir, suffix, template, `check`, actions with `when`, graph-level fields), `[[node_types]]`, panel kind `graph`; need `carbon_api >= 0.2`; `SUPPORTED_APIS` 0.1 + 0.2, `ExtensionDef::api()` |
| ACP client | core `acp.rs` (+ `acp/{session,manager,resolve,transcript}.rs`), `src/bin/carbon-fake-acp-agent.rs` | One thread + process per session, newline JSON-RPC; `initialize`, `session/new`, mode via `session/set_mode` (ask = `default`, allow-edits = `acceptEdits`, allow-all = `bypassPermissions`/`auto`, capped by the user's setting), prompts queued, `session/cancel`; permission requests answered only by Carbon; usage + cost from `usage_update`; failures classified (auth-required, missing-program, crashed, protocol); program found on PATH (PATHEXT), else npm-installed into `<tools>/npm/<pkg>@<ver>` and run with `node`, else `npx -y`; stdin close then `taskkill /T` on Windows; transcripts kept after the end (200 ended sessions) |
| Extension API 0.2 | core `wit/since_v0.2.0/extension.wit`, `extension/wasm.rs`, `extension/storage.rs`; SDK `carbon-extension-api` 0.2 | Both API versions live side by side (two bindgens, shared host functions). New imports: `write-file` (`fs.write:workspace`, atomic), storage (per extension + folder, 1 MiB/16 MiB), `now-ms`, `set-timer`; `jobs`; `acp` (`agent.invoke`); `graphs` (own kinds only: list, read, run state); `approvals`. New exports `on-event`, `check-graph`. Host calls return at once; work reports back as `HostEvent`s (GUI) and `ExtensionEvent`s (to the extension) |
| GUI host | gui `extensions/host.rs`, `extensions/code.rs` | `AgentsHost` global: jobs into the status bar, runs, approvals, permission requests, timers; ACP events routed to their owner; per-extension ordered event pump into `on-event`; Stop cancels a job's sessions and tells the extension; turned-off extensions' sessions close; setting `agents.max_permission_mode` (default allow-edits) |
| Graph panel | gui `graph_panel.rs` (+ `graph_panel/{canvas,geometry,inspector,files,run_view}.rs`) | One file per tab (from the Explorer, or the panel's file list + New from template or a start block); canvas: dot grid, bezier arrows with heads (dashed into loop-back inputs, red with problems), blocks as divs with port dots, drag to move (8 px snap, Alt free), drag output→input to connect (single-arrow inputs replace), marquee and Shift-select, arrow select, Space/middle-drag pan, wheel zoom at pointer (trackpad pans), +/-/0/1 keys, Del, Ctrl+D, Ctrl+A, Ctrl+Z/Y, Esc; inspector Edit tab (graph name + fields, block title/fields from its type incl. multi-line text, numbers, switches, choices, agents, route lists; turned off; problems; Duplicate/Delete) and Run tab (run state + Stop, approval and permission cards, block states, output, agent transcript, log); autosave 1.5 s, atomic write, reload on outside change or "Reload / Keep mine"; toolbar Add (grouped), undo/redo, zoom, problems count, save state, the kind's actions (Run/Stop by `when`); blocks show state word + badge + coloured border; status bar Stop per extension job and "N waiting" (opens the flow); quit asks to stop running flows |
| Agents extension | repo `carbon-extensions-agents/` (`agents/`: `extension.toml`, `src/{lib,adapter,runner,flow,template}.rs`, `build.py`, `agents.wasm`; `catalog.py`, README) | `carbon.agents` 0.1.0: Claude Code agent (`@agentclientprotocol/claude-agent-acp` 0.95.0); graph kind `flow` in `.carbon/flows/*.flow.yaml` (task, cost limit); blocks Start, Agent, Router, Loop, Approval, Parallel, Merge, Output; runner: state in storage after every event, sessions reused per block within a run, templates `{{input}}`/`{{task}}`/`{{nodes.<id>.output}}`, merge waits for all (or first; partial merges flush when nothing else can happen), loop until text or max, fail fast, cost limit, Stop, one run per file at a time; `check-graph` (one Start, empty routes, output path inside the folder, agents nothing leads to) |

## Status at a glance (2026-10-24)

Not placeholder: every part below is working code with tests. It is a solid v1, proven end to end with a fake agent, not yet with real Claude Code from the GUI.

| | What |
|---|---|
| **Works, verified** | Flow files (YAML, byte-stable); canvas editing (add, connect, move, undo/redo, autosave, outside-change reload, problems); running flows (states, costs, transcripts, approvals, Stop, cost limit) with the fake agent; ACP client against the real Claude Code adapter (npm install, prompt, cost); extension API 0.2 with a compiled guest |
| **Real, not yet seen with real Claude** | A real Claude Code agent inside a flow run from the GUI (each piece works, the chain works with the fake agent); permission cards with real Claude requests (option names/kinds may display poorly) |
| **Thin (works, basic)** | Blocks pass plain text only (no shared state, no structured data); Router is "answer contains text" (no model decides); agents use Claude Code's own tools, not Carbon's editors (edits not reviewable in Carbon); canvas has no minimap, copy/paste, right-click add, groups; Claude Code is the only agent |
| **Missing** | Runs survive a restart; MCP tools for agents; Chat panel on ACP (still the demo agent); `carbon.agents` in the catalog. Full list: `../not_built/README.md` "Agents leftovers" |

## How it fits together

A run, step by step (who does what):

1. User presses **Run** in a graph panel (`graph_panel.rs` `run_action`): the file is saved, and `extensions/host.rs` `deliver` queues `ExtensionEvent::GraphAction { path, action: "run" }` for the extension.
2. The event pump calls the extension's `on-event` (WASM, `extensions/code.rs` `on_event` → `extension/wasm.rs`). The adapter (`carbon-extensions-agents/agents/src/adapter.rs`) turns it into a runner event; `runner.rs` `handle` reads the flow (`graphs::read-graph`), starts a job and a run, and walks from Start.
3. An Agent block calls `acp::start-session` → host `AcpManager::start_session` (`acp/manager.rs`), which spawns the agent on its own thread (`acp/session.rs`) and returns an id at once. The runner stores the prompt as pending and saves its state to storage.
4. Session events come back: `AcpEvent` → `host.rs` `on_acp` → `ExtensionEvent` (`SessionReady`, `PromptFinished`, ...) → `on-event` again. The runner sends the prompt, then on the answer marks the block done and moves on.
5. Everything the runner shows (block states, outputs, log, approvals, job) goes out as host calls → `HostEvent`s → `host.rs` `apply` → `AgentsHost` global → the panel and status bar redraw.
6. Permission requests from the agent never reach the extension: `on_acp` keeps them in `AgentsHost.permissions`, `run_view.rs` shows cards, `answer_permission` replies to the agent.

Rule kept throughout: Carbon owns long-running, interactive and dangerous things (processes, files, the graph document, permission answers); the extension owns what blocks mean and reacts in short calls (5 s each), keeping state in storage because Carbon may reload its code between events.

## Working on it

**Build and test**
- `cargo test -j 4 --workspace` (Carbon; includes `tests/acp.rs` against the fake agent `carbon-fake-acp-agent`, `tests/graph.rs`).
- `cargo test -p carbon-core --test acp -- --ignored --nocapture`: one real Claude Code prompt (needs Node 22+, a Claude Code sign-in).
- `cargo test -p carbon-core --test extension_wasm -- --ignored`: builds the example guest for API 0.2 (`rustup target add wasm32-wasip2`).
- Extension: `cd carbon-extensions-agents/agents`, `cargo test` (runner, native), `python build.py` (tests + `agents.wasm`). The repo must sit at `carbon-extensions-agents/` in the Carbon root (SDK path dependency; ignored by Carbon's git).
- Use in Carbon: Extensions > Install > Link Development Folder > `carbon-extensions-agents/agents`; Refresh after rebuilding `agents.wasm`.
- Click test without spending Claude usage: copy the package, set its `[[agents]]` `command = "carbon-fake-acp-agent"`, `env = { FAKE_ACP = "ok" }` (scenarios in `src/bin/carbon-fake-acp-agent.rs`: ok, permission, hang, crash, auth), add `process.exec:carbon-fake-acp-agent`, and start Carbon with `target\debug` first on PATH.

**Where to change what**

| To | Change |
|---|---|
| Add a block type | `agents/extension.toml` `[[node_types]]` (ports, fields: the canvas and inspector follow) + `runner.rs` `arrive` (what it does) + a runner test |
| Add an agent (Codex, Gemini CLI) | `[[agents]]` in the manifest (command, npm package, capabilities); ACP mapping of modes is in `acp.rs` `PermissionMode::agent_modes` |
| Add a host function for extensions | WIT `wit/since_v0.2.0/extension.wit` (or a new `since_v0.3.0` with its own bindgen in `wasm.rs`), the `impl v2::... for HostState` in `wasm.rs`, a `HostEvent` + `host.rs` `apply` if the GUI must act, SDK re-export in `apps/carbon-extension-api/src/lib.rs` |
| Canvas look or interaction | `graph_panel/canvas.rs` (drawing, mouse), `geometry.rs` (sizes, hit tests; unit-tested), tokens `style/fixed.rs` `GRAPH_*` |
| Inspector fields | `graph_panel/inspector.rs` (`field_control` by `FieldKind`) |
| Run tab and cards | `graph_panel/run_view.rs` |
| Graph file format or checks | `apps/carbon-core/src/graph/` (`doc.rs`, `validate.rs`; `tests/graph.rs`) |

**Suggested next steps, in order**
1. Run a real Claude Code flow from the GUI and fix what shows (permission card labels, slow first start, errors in the Run tab).
2. Give blocks more than text: a shared run state (key/value) blocks can read and write, and a model-decided Router.
3. Runs that survive a restart (`session/load`, mark stored runs interrupted on `init`).
4. ACP `fs/*` through Carbon's buffers so agent edits are reviewable diffs; MCP servers for agents.
5. Chat panel on the ACP client (`ChatStore::set_backend`).
6. Canvas polish: minimap, copy/paste blocks, right-click add, groups; publish `carbon.agents` in the catalog.

## Verified

- Tests: `cargo test --workspace -j 4` 191 passed, 0 failed, 9 ignored (new: `tests/graph.rs` 8, `tests/extension.rs` +4 incl. the real Agents manifest, `tests/acp.rs` 5 against the fake agent, gui `graph_panel` geometry/inspector units); ignored ones by hand: `extension_wasm` (v0.2 guest: storage across reloads, write grant, timer → event, check-graph), `acp::claude_code_through_a_managed_npm_install` (real adapter npm-installed into a temp tools folder, answered "pong", cost reported USD 0.12, no processes left). Agents crate: 9 runner tests (straight flow with file output, parallel + merge order, loop in one session until matched and at its limit, approve/reject, Stop, cost limit, job Stop, agent failure, turned-off pass-through, no second run). Clippy and fmt clean (workspace and the agents crate).
- Click-tested (test build, scratch config, window on the second monitor): open a flow from the Explorer (fit to view); select, zoom, drag a block (arrows follow, Unsaved → Saved), connect Rejected → Plan (cycle flagged), Undo/Redo, Add menu (grouped), Router with a route (port appears), typing into a route (autosaved), file edited outside (reloaded). Run against the fake agent through the real `agents.wasm`: Start/Plan Done with $0.25, approval card + "Waiting for you" + status bar job with Stop and "1 waiting", Approve → `report.md` written, run Finished ($0.25), transcript (start note, rendered prompt, answer).
- Not tested by hand: a real Claude Code flow from the GUI (the adapter is tested from core), permission cards with a real agent, Stop/cost limit from the GUI, quit with a running flow, marquee/keys (posted keys can't send modifiers), restart restoring a graph tab, extension `check-graph` problems showing.

## Limits

- Runs don't survive a restart (state stays in storage; Carbon marks nothing interrupted yet). One run per flow file at a time.
- Agent blocks start a new session per block per run; every new Claude Code session loads the user's whole Claude setup (~41 k cached tokens), so the first prompt costs more.
- No ACP `fs/*` / `terminal/*` (the agent uses its own tools); no `session/load` resume; no MCP servers passed; agent text is not streamed into the canvas (transcripts are, in the Run tab).
- Canvas: no minimap, no copy/paste as YAML fragments, no double-click add, no grouping/subflows, no moving dashes on active arrows; text is scaled, not re-laid out.
- Agent permission cards show option names as the agent gives them; "always" options are passed on, Carbon keeps no rules of its own.
- The agents repo builds against the Carbon SDK by path (repo checked out inside Carbon).
