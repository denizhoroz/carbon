# Workflow system: remaining work and design

Built (`../built/workflows.md`): Phase 0-3 (registry, manifests, built-in packages, user folder, overlays, `state.json`, crash guard, safe mode, Manage Workflows, per-workflow named layouts, Section Bar overflow) on 2026-10-20; package files, declarative views tier 1, experimental webview panels and Phase 4 install (folder/zip/git, capability prompt and grants, update diff, content hashes, catalog, `requires`) on 2026-10-21. This file holds what is left, the design for it, and the decisions. Evidence: `research_notes/Modular workflow systems/{workspace_mode_systems,plugin_architectures,agent_workflow_packages}.md`. Labels: **[BG]** = background knowledge, not verified; **(inf)** = my design choice.

## 1. To do

In rough order; each row points to its design.

| # | Item | Scope | Design |
|---|---|---|---|
| 1 | **Phase 4: what is left** | Built 2026-10-21: folder, zip and git install (gix), checks, capability prompt and grants, update diff, content hash and commit pins, catalog (git repo + `catalog.json`), `requires` with Install buttons (Q21), Fetching dialog with Cancel. Left: signatures and trusted keys; catalog URL in Settings (env meanwhile); re-check installed files against their hash (at load or a Verify button); byte progress; workspace trust for project-provided workflows; `requires` kinds extensions/tools/mcp once those systems exist | s4.5 |
| 2 | Capabilities | Built: names and scopes, grants in `state.json`, Permissions dialog (allow/revoke). Left: enforce in the webview page API and WASM; denied = typed error (Q13) | s4.5, Q13, Q19 |
| 3 | Webview, next steps | Page-to-Carbon API (open file, run command, events), gated by capabilities; network for pages via the `net` capability; decide whether the `webview` feature is on by default (measure the classic swap chain); a tab click should focus the panel; origin collision of rare ids (`a-.b` / `a.-b`); Linux (needs GPUI Fast or another path); not yet tested: auto-hide docks over a page, flicker during an edge drag, memory per page | s4.4 |
| 4 | Views tier 2 | Stateful widgets (input, toggle, select, form, table, tree) with data from a WASM component or from Carbon; refresh on events | s4.4 |
| 5 | Real Paradigm Editor and Agent Automation | Replace the placeholder panels (first-party Rust panels, views or webviews) | `README.md` row "More workflows" |
| 6 | Unbuilt manifest fields | `activation`, `file_types` (Q14), `settings` (needs the Settings system), `background` (Q20), `[agent]`; `keybindings` deferred (Q9) | s4.1, s4.3 |
| 7 | Robustness | Layout merge after a default changes (today a reset); Repair/Remove buttons on a "not available" placeholder; quarantine counters (N faults in M minutes); Bisect over workflows/extensions | s4.2, s4.3 |
| 8 | Locations and release | Change the three locations in the installer/Settings (Q16; env variables meanwhile); release packaging copies `apps/carbon-workflow/` next to the binary | Q15, Q16 |
| 9 | Contributor guide | A `workflow.toml` guide (fields, views, pages, suggested folders) | Q22 |
| 10 | Open questions | Q12 (device ownership, real-time audio), Q13 (denied or unsupported capability) | s5 |

## 2. Goal, scope, what we borrow

**Goal.** Workflows are data, so a user or third party adds, removes or changes one without touching Carbon's source and without breaking the Editor or other systems. A workflow is a switchable app mode: one Section Bar icon, a default layout, the panels it may use, its commands and menu items, later its key bindings, settings and agent behaviour. Workspace, buffers, terminals, chats and LSP stay shared; only the arrangement and contributions switch. A workflow is not tied to code or a folder.

**Non-goals.** In-process third-party Rust plugins (no stable ABI, no sandbox), pop-out windows (deferred).

**What we borrow** (survey in the research notes; thin evidence for Blender, Zed modes, Cursor):

| Idea | From |
|---|---|
| id/name/icon/order/enabled, lazy panels | Qt Creator `IMode`, IntelliJ tool windows |
| Declarative manifest, `when` clauses, contribution points | Eclipse, VS Code |
| Scoped registration removed on exit | Unreal `UEdMode` |
| Duplicate-to-customize, Reset; stale saved layout is the main pitfall | Blender, Eclipse perspectives, Obsidian |
| Small override layer over a read-only base | Roo, Windsurf, Claude `skillOverrides` |
| Versioned API, host keeps old versions, capability grants, WASM per extension | Zed |
| Bisect, safe mode | VS Code, Obsidian Restricted Mode |
| Namespaced ids, folder bundles, `SKILL.md`/`AGENTS.md`/`.mcp.json` reuse, hash pins | Claude Code, Agent Skills |

No surveyed product has a mode with both a dock layout and agent behaviour; Carbon's layering (read-only base + small overlay + separate dock state) is design inference.

## 3. Requirements still open

| # | Requirement | Where |
|---|---|---|
| R6 | Versioned API: `schema_version` + `carbon_api`; host keeps loaders for the last N schema versions | s4.5 |
| R7 | Trust: manifest-only workflows run nothing; capabilities declared and granted by the user; no auto-run on install; hash pins | s4.5 |
| R9 | Agent behaviour later without redesign (`[agent]` reserved; `[requires]` built for workflows, other kinds accepted) | s4.1 |
| W1 | Nothing assumes a folder or text buffer; `requires_workspace` shows an "open a folder" state | s4.3 |
| W2 | File-type handlers (glob/MIME to panel kind) | s4.1, Q14 |
| W4 | Background: suspend on exit by default; declared tasks visible and stoppable | s4.3, Q20 |
| W5 | Own settings namespace `workflow.<id>.*` | s4.1 |
| W6 | Workflows share data only through workspace files and MCP | s4.3 |
| W7 | Capabilities cover `fs` scopes, `net`, `process`, `agent.invoke`, `gpu`, `audio.*`, `midi` | s4.5 |

Range the design must carry (user, 2026-10-20; needs are **(inf)**):

| Workflow | Panels | Capabilities | Background |
|---|---|---|---|
| Editor | editor, explorer, terminal, chat | `fs`, `process`, LSP, `agent.invoke` | LSP, terminals (shared) |
| Paradigm Editor | canvas, inspector, chat | `fs`, `agent.invoke` | none |
| Agent Automation | run console, job list, flow view, chat | `process`, `net`, `fs`, MCP, `agent.invoke` | scheduled or long runs |
| 3D viewer | 3D view (webview WebGL/WebGPU), scene tree | `gpu`, `fs.read` | none |
| Sound design | timeline, mixer, meters | `audio.*`, `midi`, `fs` | playback may continue (Q12) |

## 4. Design for what is not built

### 4.1 Manifest fields not built

Built fields are in `../built/workflows.md`. Unknown top-level keys warn; the reserved ones below are accepted and ignored until built.

| Field | Design |
|---|---|
| `derived_from` | Written by Duplicate; later "diff against parent" |
| `keybindings` | `key`, `command`, optional `when`; deferred (Q9): global app bindings win, reserved ones (quit, palette, copy/paste) cannot be rebound |
| `activation` | `on = "enter"` (default) or `"startup"`; `startup` only with `background` |
| `requires_workspace` | Stored; to show an "open a folder" state (W1) |
| `file_types` | glob/MIME -> panel kind, `open = "primary"`/`"secondary"`; the active workflow wins, else the per-type user default ("Open With" asks once) (Q14) |
| `settings` | Typed keys with defaults under `workflow.<id>.*`, drawn by Manage Settings with no workflow code (W5) |
| `background` | Declared tasks (name, kind); default none = suspend on exit (Q20) |
| `[agent]` | Instruction files, tool allowlist, MCP servers, hooks; the same files as `AGENTS.md`/`SKILL.md`/`.mcp.json` |

### 4.2 Layers and updates

| Layer | Location | Writable | Contents |
|---|---|---|---|
| L0 shipped | `carbon-workflow/<name>/` next to the binary | no | Editor (not removable), Paradigm Editor, Agent Automation |
| L1 installed | `<local data>/Carbon/workflows/installed/<id>/<version>/` | no (replaced on update) | Package |
| L2 user | `<config>/Carbon/workflows/user/<folder>/` | yes | Whole workflow (by hand or Duplicate) |
| L3 overlay | `<config>/Carbon/workflows/overlay/<id>.toml` | yes | Patch of L0/L1; no id, capabilities or requires |
| L4 state | `<config>/Carbon/workflows/state/state.json` | app | Dock state, flags, named layouts, later grants |

Not built yet: on a base update, overlay ops that target a removed panel are dropped with a notice, and the saved layout is merged instead of reset: keep positions of panels still present, add new default panels where `closable` allows, drop missing ones (`base_fingerprint` and `base_panels` are already saved for this). Reset all (every workflow at once, with a confirm).

### 4.3 Lifecycle not built

- Background (W4, Q20): a workflow is suspended when the user leaves it (render loops, audio, timers stop). Declared `background` tasks keep running, show in the status bar jobs item and Manage Workflows with Stop; the user can deny background per workflow.
- `requires_workspace` without a folder: the workflow opens and shows an empty state.
- Quarantine counters (N faults in M minutes) and Bisect; today one unfinished opening quarantines.
- Workflows never hold references to each other's panels or state (W6).
- Later: `handles` auto-switch hook (Godot `_handles`).

### 4.4 Contributed panels

Decided (Q17, Q18): **declarative views** for simple workflows, **webview pages** for advanced ones; GPUI types never cross the boundary (the kit breaks every minor), no native in-process plugins.

| Part | Built | Not built |
|---|---|---|
| Views tier 1 (manifest, no code) | Widgets heading, text, markdown, button, row, column, divider, list; buttons run `[[commands]]` | Icons beyond the embedded set |
| Views tier 2 | none | A WASM component (one Store per workflow, epoch deadline, memory limit) builds the tree and handles events; stateful widgets (input, toggle, select, form, table, tree); versioned WIT world `carbon:workflow@x.y` like Zed |
| Webview pages | Package page over `carbon-page`, one origin per workflow, CSP without network, navigation kept inside, IPC bridge (test messages), theme variables, Carbon shortcuts from the page, hidden under overlays/zoom/background tabs | Page API gated by capabilities (open file, run command, subscribe to events); `net` for pages; error card for a crashed page; feature on by default (decision) |
| Surface kinds (GPU, audio, canvas handles) | none | Superseded by Q17: graphics through the webview; native handles only if a webview cannot meet a need (Q12) |
| File-type handlers (W2) | none | Registry glob/MIME -> (workflow, panel kind); related `FEATURE.md` EX-05 |

Webview library (research 2026-10-20, sources s6): the kit's `gpui-wry` 0.7.3 (same version line as gpui-kit, on `gpui-pre 0.3.9` like Carbon) over `lb-wry`. Rejected: `web-view` / `tauri-web-view` (MSHTML/EdgeHTML, no WebView2). The page is a native child window above GPUI; overlay composition fixes (Zed PR #62379, GPUI Fast on Windows) are unmerged or experimental; CEF off-screen rendering would avoid it but ships Chromium. Linux exists only in the newer `gpui-webview` (GPUI Fast, X11 only).

### 4.5 Install, trust, versioning (Phase 4)

Built 2026-10-21 (folder/zip/git sources, checks, dialog, grants, update diff, uninstall drops grants, content hash, commit and catalog pins, catalog, `requires`): `../built/workflows.md`. Still to build:

| Topic | Design |
|---|---|
| Signatures | Signed `catalog.json` (or per-package signature over the content hash) with keys the user trusts; key list in Settings |
| Catalog | URL in Settings (env `CARBON_WORKFLOW_CATALOG` meanwhile); several catalogs; search; update notices for installed packages |
| Integrity | Re-check installed files against `installs.hash` (at load or a Verify button); offer Reinstall on a mismatch |
| Workspace trust | A workflow found in a cloned repo needs workspace trust first (WB-05) |
| Update | Saved layout merged instead of replaced (s4.2); diff of views/pages beyond the layout fingerprint |
| Trust levels | built-in > hash-pinned installed > unpinned installed > project-provided (the `pinned` flag is recorded and shown; nothing behaves differently by level yet) |
| API | `carbon_api` checked at load (newer-than-host = "needs newer Carbon", disabled); host keeps loaders for the last N `schema_version`s with a deprecation notice |
| Agent files | No shell-injection syntax; invisible Unicode flagged (Snyk ToxicSkills, GlassWorm findings) |
| Devices | `gpu`, `audio.input`, `audio.output` granted per workflow, indicator while in use; denied = typed error, the workflow keeps running |

### 4.6 Interop

- `[agent]` reads `AGENTS.md`, `SKILL.md` folders and `.mcp.json` unchanged.
- Optional importer: a Claude-style plugin folder (`.claude-plugin/plugin.json`) becomes a workflow with no layout (agent-only), hidden from the Section Bar **(inf)**.

## 5. Decisions and open questions

| # | Question | Answer |
|---|---|---|
| Q1 | Manifest format | TOML (JSON accepted for the same schema) |
| Q2 | Changing a built-in | Only via overlay or Duplicate; the shipped file is never edited |
| Q3 | Overlay patch or whole-file replace | Small patch overlay for tweaks; whole manifest for Duplicate; no deep merge |
| Q4 | Paradigm/Agents as workflows now | Yes, as placeholders to build the system; real content later |
| Q5 | Section Bar overflow | No cap; up/down arrows plus wheel (built) |
| Q6 | User files | OS config dir `Carbon/workflows/` (built) |
| Q7 | Editor id | Name "Editor", id `carbon.editor`, `legacy_ids = ["code"]` (built) |
| Q8, Q10 | Named layouts / layout key | Per workflow, user-changeable; per-folder later on top (built) |
| Q9 | Scoped key bindings | Deferred; no spike |
| Q11 | Safe GPU/audio/canvas surfaces | Superseded by Q17 (webview) |
| Q12 | Device ownership, real-time audio vs GPUI main thread | **Open** |
| Q13 | Denied or unsupported capability | **Open**; recommended: disabled with a visible reason, like a missing `requires` |
| Q14 | File claimed by several workflows | Active workflow, else per-type user default |
| Q15 | Package locations | Shipped next to the binary, downloads in the local data dir, preferences in the config dir (built) |
| Q16 | Moving those locations | Yes, all three, in the installer or Settings; env variables meanwhile |
| Q17 | Contributed UI | Declarative views + webview pages; no native plugins (views tier 1 and webview pages built) |
| Q18 | Where logic runs | Tier 1 manifest (built), tier 2 WASM; webview logic is JS through a capability-gated bridge |
| Q19 | Capability grants | At install; an update asks only for new ones; revocable (built, not enforced yet) |
| Q20 | Background work | Suspended by default; declared tasks visible with Stop; deniable |
| Q21 | Missing `requires` | Disabled with the reason and an Install button; nothing installs without a click (built for workflows) |
| Q23 | Git transport | gix built in (HTTPS via reqwest + rustls), not the user's git (user, 2026-10-21) (built) |
| Q24 | Catalog source | A git repo with `catalog.json`, every version pinned by commit + content hash (user, 2026-10-21) (built) |
| Q25 | Hash pins | Commit id for git; SHA-256 content hash for every install, checked against catalog pins; signatures later (user, 2026-10-21) (built) |
| Q22 | Package folder | Free; only root `workflow.toml` fixed; path rules (built) |

Risks: the kit breaks every minor (only the GUI adapter touches GPUI types); the webview is a native window above GPUI (every new overlay must hide pages; kept in `Workbench::sync_webviews`); thin evidence for Blender, Zed modes, Cursor, VS Code crash semantics.

## 6. Sources

- Qt Creator: https://doc.qt.io/qtcreator-extending/core-imode.html ; https://doc.qt.io/qtcreator-extending/plugin-meta-data.html
- Godot: https://docs.godotengine.org/en/stable/tutorials/plugins/editor/making_main_screen_plugins.html
- Eclipse: https://help.eclipse.org/latest/topic/org.eclipse.platform.doc.isv/reference/extension-points/org_eclipse_ui_perspectives.html ; https://www.eclipse.org/forums/index.php/t/1078421
- IntelliJ: https://plugins.jetbrains.com/docs/intellij/tool-windows.html ; https://plugins.jetbrains.com/docs/intellij/dynamic-plugins.html
- VS Code: https://code.visualstudio.com/api/references/contribution-points ; https://code.visualstudio.com/docs/configure/profiles ; https://code.visualstudio.com/blogs/2021/02/16/extension-bisect ; https://github.com/microsoft/vscode/issues/95182
- Unreal: https://dev.epicgames.com/documentation/unreal-engine/API/Editor/UnrealEd/UEdMode/RegisterTool
- Obsidian: https://obsidian.md/help/plugins/workspaces
- Blender API: https://docs.blender.org/api/5.0/bpy.ops.workspace.html
- Zed: https://zed.dev/blog/zed-decoded-extensions ; https://zed.dev/docs/extensions/developing-extensions
- Claude Code / Agent Skills: https://code.claude.com/docs/en/plugins-reference ; https://code.claude.com/docs/en/plugins/security ; https://agentskills.io/specification
- Roo / Windsurf / Kiro: https://roocodeinc.github.io/Roo-Code/features/custom-modes ; https://docs.devin.ai/desktop/cascade/workflows ; https://kiro.dev/docs/steering/
- Trust and WASM: https://docs.rs/wasmtime/latest/wasmtime/struct.Config.html ; https://component-model.bytecodealliance.org/design/wit.html ; https://snyk.io/blog/toxicskills-malicious-ai-agent-skills-clawhub/ ; https://thehackernews.com/2025/10/self-spreading-glassworm-infects-vs.html ; https://modelcontextprotocol.io/registry/about
- Webview: https://gpui-kit.com/docs/webview/ ; https://lib.rs/crates/gpui-wry ; https://github.com/longbridge/gpui-kit/pull/2626 ; https://github.com/longbridge/gpui-kit/pull/3395 ; https://github.com/longbridge/gpui-kit/pull/3398 ; https://github.com/zed-industries/zed/pull/62379 ; https://github.com/bokuweb/gpui-cef ; https://github.com/Boscop/web-view/issues/133 ; https://learn.microsoft.com/en-us/microsoft-edge/webview2/reference/win32/icorewebview2controlleroptions3 ; local source `gpui-wry-0.7.3/src/lib.rs`, `gpui-pre-windows-0.3.9/src/window.rs`
