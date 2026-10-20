# Workflow system (design, not built)

Status: design accepted (user answers 2026-10-20 incl. Q3, see s6; workflow range widened the same day, s3.1). **Phase 0-3 built 2026-10-20** (s5.1-s5.3, `../built/workflows.md`); Phase 4+ not built. Evidence: `research_notes/Modular workflow systems/{workspace_mode_systems,plugin_architectures,agent_workflow_packages}.md`. Labels: sourced = URL in s7; **[BG]** = background knowledge, not verified; **(inf)** = my inference/design choice.

## 1. Goal, scope, non-goals

**Goal.** Replace the hard-coded `Section` enum with a registry of workflows so a user or third party can add, remove or modify one without touching Carbon's source, and without breaking the Editor (formerly "Code Editor") or any other system.

**What a workflow is in Carbon.** A switchable app mode = one Section Bar icon + a default dock layout + the panels it may use + its commands, menu items and key bindings (active only in it) + later agent behaviour (instructions, tools, MCP). Layout and shared state (workspace, buffers, terminals, chats in `carbon-core`) stay shared; only the *arrangement and contributions* switch. A workflow is not necessarily about code or a project folder (s3.1).

| Already decided (user) | Source |
|---|---|
| Section Bar = workflow switcher; Editor real, Paradigm Editor and Agent Automation as placeholder workflows now (decided 2026-10-20), users add custom workflows | `PROJECT.md`, `built/section-bar.md` |
| Manage Workflows dialog (bottom of the bar) adds/removes built-in + custom workflows; Manage Extensions is separate (compilers, linters, appearances, viewers) | `DESIGN.md` s4/5.4 |
| Extensions: WASM (capability grants) + MCP native, declarative tier first, no preinstalled language tools | `PROJECT.md`, `FEATURE.md` EX-01..09 |
| Installing from Manage Workflows is future; now: the modular foundation | this request |
| Workflows range from paradigm editor to automation, 3D graphics display and sound design; very modular (2026-10-20) | user (s3.1) |

**Non-goals now.** Installer/gallery UI, signing, WASM host, agent behaviour, webview panels (excluded by `PROJECT.md`), in-process third-party Rust plugins (no stable ABI), pop-out windows (T3, deferred).

## 2. Alternatives surveyed

Coverage is uneven. Deep, sourced: Qt Creator, Godot, Eclipse, IntelliJ, VS Code, Unreal, Obsidian, Claude Code, Roo, Windsurf, Kiro, VS Code custom agents. Thin or **[BG]**: Blender (manual page did not load; only API operators sourced), Zed (no mode concept found; extension model sourced), Cursor (anecdotal forum posts only), Houdini, Adobe, Emacs, Theia, Raycast.

### 2.1 Workspace / mode systems

| Product | What a "mode" is | Defined / contributed | Switching | User modification | Sharing / install | Failure containment | Lesson for Carbon |
|---|---|---|---|---|---|---|---|
| Qt Creator `IMode` | Left-bar mode: id, name, icon, priority, enabled flag, context, lazy widget | C++ plugin registers object; JSON plugin metadata (`DisabledByDefault`, Required/Optional deps) | `ModeManager` swaps key-binding contexts, emits `currentModeChanged` | Plugin toggle in About Plugins; no layout editing of a mode | Plugin binary; same compiler/Qt needed | Malformed metadata = plugin skipped; in-process, a crash kills app [BG] | Closest to `Section` (id/name/icon/order/enabled/lazy). Docs say add a mode "sparingly": cap Section Bar growth. Native in-process plugins rejected |
| Godot main-screen plugin | Top "main screen" tab (2D/3D/Script/custom) | `plugin.cfg` + script in `addons/`: `_has_main_screen`, `_make_visible`, name, icon, optional `_handles` | Hide old, show new; panel lives enable..disable | Enable/disable in Project Settings, no restart | Extract zip / AssetLib, merge without overwrite | Script in editor process; engine crash not contained [BG] | Minimal contract is a good floor; `_handles` = auto-switch hook (idea for later) |
| Blender workspaces | Screen layout + interaction mode + tool; stored in .blend | `startup.blend`, app templates; API: add (duplicate or append), `append_activate`, reorder, delete [API sourced; rest **BG**] | Tabs at top | Duplicate then edit; reorder | Append from a .blend library | Unverified | Duplicate-to-customize and append-from-library are the sharing model |
| Eclipse perspectives | Saved arrangement + visible action sets | Extension point: `id`, `name`, `class` (factory), `icon`, `fixed`; `perspectiveExtensions` lets other plug-ins add views | Open Perspective menu; layout saved at first open, restored later | Save As, Reset Perspective | Plug-in install | Stale saved state hides new views; fix = Reset / `-clean` | Biggest documented pitfall: persisted layout vs updated definition. Keep base and user state separate, version them, offer Reset |
| IntelliJ tool windows | Dockable panel (not a whole mode) | `plugin.xml` EP: `id`, `icon`, `anchor`, `factoryClass`; `isApplicable` | Content created on first click (no startup cost) | Drag/dock freely | Plugin repo; dynamic load/unload with strict rules | Unload leaks force restart | Lazy panel creation; declarative placement flags |
| VS Code | No workflow mode. View containers (Activity Bar/Panel) + views with `when`; Profiles bundle settings, extensions, keybindings, MCP servers, UI layout | `contributes.*` manifest, activation events; separate Extension Host | Profile switch changes all as a unit; can bind to folder | Profile copy/edit; no named layouts (issue #95182) | `.code-profile` file/gist, Settings Sync | Extension Host separate; Bisect; `--disable-extensions` [BG] | Profiles are the nearest "bundle"; contribution points with `when` clauses; Bisect for safe mode; layouts are a known gap |
| Unreal `UEdMode` | Editor mode with its own tools and toolkit UI | C++ class; `RegisterTool(cmd, id, builder, scope)` scopes Editor/EdMode/Default | Mode-scoped tools unregistered on exit | Limited | Plugin | In-process | Scoped registration: commands/menus belong to the active workflow and vanish on exit (adopt) |
| Obsidian Workspaces | Saved tabs + sidebar widths/visibility, by name | Core plugin; community plugins add hierarchy, hotkeys | Load by name from ribbon/command | Re-save same name updates | Per-vault file | Restricted Mode turns off community plugins [BG] | Named layouts are cheap and loved; Restricted Mode = safe mode |
| Zed (thin) | No mode concept found | `extension.toml` + WASM; declarative themes/languages | n/a | Settings JSON, `.zed/` overrides | Extension gallery | Wasmtime per extension; user-granted capabilities | Not a workflow precedent; strong precedent for WASM + capability grants + versioned WIT API (s4.9) |

### 2.2 Agent-era packages

| Product | What it is | Defined | Activation / switching | User modification | Sharing / install | Containment / trust | Lesson |
|---|---|---|---|---|---|---|---|
| Claude Code plugins + skills | Folder bundle: skills, agents, hooks, MCP/LSP, themes, workflows; skill = `SKILL.md` | `.claude-plugin/plugin.json` (only `name` required); components auto-discovered; unknown top-level keys stripped, strict objects error; `claude plugin validate` | Skills load name+description first (progressive disclosure); namespaced `plugin:component` | Shadow by name at higher scope; `skillOverrides` switch (on/off/name-only) | Git marketplaces, SHA/sha256 pins, scopes user/project/local/managed, dependencies with semver | "Can execute arbitrary code"; hooks/MCP outside sandbox; warning + allowlists only | Folder + optional manifest; namespacing makes collisions impossible; do **not** copy "warning only" trust |
| VS Code custom agents | `.agent.md`: name, description, `tools`, handoffs + prompt body | Workspace `.github/agents`, user dir, org, extension-contributed | User picks agent; instructions added to custom ones | Edit file | Commit to repo / org | Restrict `tools` (least privilege) | Markdown + frontmatter is the lowest-friction unit |
| Roo custom modes | Mode = slug, name, roleDefinition, whenToUse, `groups` (read/edit/command/mcp, `fileRegex`) | `.roomodes` (project) > global `custom_modes.yaml` > defaults | Mode picker | Same slug in higher layer **replaces whole mode**, no merge (incl. built-ins) | One-click marketplace; export/import YAML | Tool groups, path regex restrict edits | Closest to "mode with agent behaviour" but no layout; whole-file replace is simple and predictable |
| Windsurf workflows | Markdown steps, `/name`, manual only, 12k chars | Workspace, global, system dirs | Slash command | Edit file; order System > Workspace > Global > Built-in | Commit to repo | n/a | Layering order; manual-only keeps it safe |
| Kiro steering | Markdown rules in `.kiro/steering/`, modes always/fileMatch/manual/auto | Frontmatter | Per file inclusion mode | Workspace beats global | Repo / MDM | Custom agents opt in via `resources` | Opt-in context per agent |
| (not surveyed deeply) Cursor | Layout presets (agent/editor) from forum posts; reset regressions in 2.0 (anecdotal) | **BG** | | | | | Layout reset is a real user pain |

**Plain statements.** (1) No surveyed product has a mode that also carries agent behaviour *and* a dock layout: the agent products (Roo, VS Code agents, Kiro) have no layout; the layout products (Qt, Eclipse, Blender, VS Code profiles) have no agent behaviour. (2) None implements exactly Carbon's layering of read-only base + small user overlay + separately persisted dock state; nearest are VS Code settings layering, Eclipse `perspectiveExtensions`, Roo whole-slug override, Claude `skillOverrides`. The layering below is design inference.

### 2.3 What we borrow

| Idea | From |
|---|---|
| id/name/icon/order/enabled/lazy widget | Qt Creator |
| Declarative manifest, code loaded lazily, `when` clauses | Eclipse, VS Code |
| Scoped registration removed on exit | Unreal |
| Layout flags (`fixed`, anchor, closable) | Eclipse, IntelliJ |
| Duplicate-to-customize, Reset | Blender, Eclipse, Obsidian |
| Whole-file shadowing + small override table | Roo, Windsurf, Claude `skillOverrides` |
| Versioned API, host keeps last N, capability grants | Zed |
| Bisect + safe mode | VS Code |
| Namespaced ids, optional manifest, SKILL.md/AGENTS.md/.mcp.json reuse | Claude Code, Agent Skills |

## 3. Requirements

| # | Requirement | How met |
|---|---|---|
| R1 | Add / remove / modify workflows without editing Carbon | Workflow = data file(s) in a folder; registry scans built-ins + user dir; Manage Workflows later edits the same files (s4.2, 4.3) |
| R2 | Does not break other systems | Workflows only *reference* existing panel kinds and command ids; they cannot register Rust code; core UI-free types; every consumer (bar, menus, palette, workbench) reads the registry, not the enum (s4.5) |
| R3 | Keep Editor behaviour | Editor becomes an embedded built-in manifest equal to `Preset::Code`; Phase 1 has a golden test that the dumped layout is unchanged (s5) |
| R4 | Per-workflow layout persistence | `DockAreaState` keyed by workflow id string, survives restart, disable and reinstall (s4.7) |
| R5 | Safe mode | `--safe-mode`: built-ins only, user files untouched (s4.8) |
| R6 | Versioned API | `schema_version` (file format) + `carbon_api` range (host contract); host keeps loaders for last N schema versions (s4.1, 4.8) |
| R7 | Trust | Manifest-only workflows cannot execute anything; capabilities declared and granted per user; no auto-run scripts; hash pin on install (s4.8) |
| R8 | Lazy and light | Panels created on first enter; only manifests parsed at start; `PROJECT.md`: unused section costs no memory |
| R9 | Agent behaviour later without redesign | Reserved `[agent]` table and `requires` block; ignored now (s4.1) |
| R10 | Any domain (diagram, automation, 3D, audio, user-built), not tied to code or a folder | s3.1 requirements W1-W7; manifest fields `requires_workspace`, `file_types`, `settings`, `background`; surface panel kinds (s4.4) |

### 3.1 Workflow range and what it implies

User knowledge (2026-10-20): workflows range from a paradigm editor to automation, a 3D graphics displayer, sound design, and can be very modular. The table is my reading of what each needs **(inf)**; only the workflow names come from the user.

| Workflow | Panels / surfaces | File types opened | Resources / capabilities | Background when inactive | Own settings, shortcuts, menus |
|---|---|---|---|---|---|
| Editor | text editor, explorer, terminal, chat | source, text | `fs`, `process`, LSP, `agent.invoke` | LSP servers, terminals (shared, kept) | editor/terminal settings, edit and run menus |
| Paradigm Editor | canvas (nodes, edges), inspector, chat | diagram formats (open) | `fs`, `agent.invoke` | none | canvas keys (pan, snap), diagram menu |
| Agent Automation | run console, job list, flow/schedule view, chat | flow/schedule files | `process`, `net`, `fs`, MCP, `agent.invoke` | yes: scheduled or long runs | run/stop commands, schedule settings |
| 3D graphics viewer | wgpu 3D surface, scene tree, properties | glTF, OBJ, images | `gpu`, `fs.read` | none (render loop stops) | camera keys, render settings, view menu |
| Sound design | audio engine, timeline, mixer, meters, MIDI | WAV, FLAC, MIDI | `audio.output`, `audio.input`, `midi`, `fs`, optional `gpu` | playback or recording may continue | transport keys (space, record), audio device settings |
| User-built | any mix of the above | any | declared per workflow | declared | declared |

Requirements derived (folded into s4):

| # | Requirement | Where |
|---|---|---|
| W1 | The contract assumes no workspace folder and no text buffer: `requires_workspace = false` by default; a workflow that needs a folder declares it and shows an "open a folder" state otherwise | s4.1, s4.3 |
| W2 | A workflow may contribute file-type handlers / custom viewers (glob or MIME to panel kind) | s4.1, s4.4 |
| W3 | Panel kinds include `surface` kinds beyond host-rendered declarative UI (GPU surface, audio device, canvas) | s4.4 |
| W4 | Background lifecycle policy: suspend on exit by default; background tasks declared and visible | s4.3 |
| W5 | Per-workflow settings namespace (`workflow.<id>.*`), with own shortcuts/menus as already scoped | s4.1, s4.6 |
| W6 | Workflows share data through workspace files and MCP, never through direct references to each other | s4.3 |
| W7 | Capability list covers `gpu`, `audio.output`, `audio.input`, `midi`, `fs` scopes, `net`, `process` | s4.1, s4.8 |

## 4. Proposed design

### 4.1 Workflow definition (manifest)

One file `workflow.toml` per workflow, in a folder (TOML decided Q1: comments allowed, matches `Cargo.toml`/`extension.toml` habits; JSON accepted as the same schema). Unknown top-level keys are stripped with a warning; unknown keys in strict tables (`layout`, `requires`) are errors that reject only this workflow (Claude Code precedent).

| Field | Meaning |
|---|---|
| `id` | Namespaced string `publisher.name`, lowercase; `carbon.*` reserved for built-ins. Stable forever: it keys layout state |
| `schema_version` | Format version (integer) |
| `version` | Semver of the workflow itself |
| `carbon_api` | Semver range of host contract, e.g. `">=0.1, <0.2"` |
| `name`, `description`, `icon`, `order` | Section Bar label/tooltip, icon id (from the shared icon set; falls back to a generic one), sort key (Qt priority) |
| `derived_from` | Parent id + version when created by duplicate |
| `layout` | Template: `center`, `left`, `right`, `bottom` regions; each = ordered panel refs, `size`, `open`, optional flags `fixed`, `closable`, `auto_hide` |
| `panels` | Allowed panel kinds with instance rules (`single`, `multi`), default title |
| `commands` | Workflow-scoped palette commands: `id`, `label`, `run` (a host command id + args) |
| `menus` | Items placed in existing menus/groups, each referencing a command id |
| `keybindings` | `key`, `command`, optional `when` (small expression: `workflow == 'x'`, `panelFocus == 'terminal'`) |
| `activation` | `on = "enter"` (default) or `"startup"`; lazy by default. `startup` only for workflows that declare `background` (decided 2026-10-20) |
| `requires_workspace` | bool, default `false`. No part of the contract assumes a folder or text buffer (W1) |
| `file_types` | Handlers contributed: `glob`/MIME -> panel kind, `open = "primary"` or `"secondary"` (W2). Several claim a type: the active workflow wins, else the per-type user default (Q14) |
| `settings` | Own namespace `workflow.<id>.*`, typed keys with defaults declared here, so Manage Settings draws them without workflow code (W5; waits for the Settings system) |
| `background` | Optional declared tasks (name, kind); default none = suspend on exit (s4.3, W4, Q20) |
| `requires` | `extensions`, `tools`, `mcp`, other workflows: id + semver range; missing = workflow stays disabled with a visible reason and an Install button per missing item, never installed without a click (Q21) |
| `capabilities` | Requested grants: `fs.read`/`fs.write` with path scopes, `process.exec` patterns, `net` hosts, `agent.invoke`, `gpu`, `audio.output`, `audio.input`, `midi`; none needed for layout-only workflows (W7). Granted at install, update asks for new ones only (Q19, s4.8) |
| UI (reserved) | Declarative UI tree (tier 1, no code) or a WASM component / webview the manifest names by path (Q17, Q18, s4.4). Syntax not designed |
| `[agent]` (reserved) | Later: instruction files, tool allowlist, MCP servers, hooks; same file kinds as `AGENTS.md`/`SKILL.md`/`.mcp.json` |

**Sketch only** (not final syntax): the Editor workflow as a manifest. Values mirror `Preset::Code` and `fixed::*_DOCK_SIZE`. The placeholder workflows `carbon.paradigm-editor` and `carbon.agent-automation` use the same schema with one `placeholder` panel in `center` and their own icon/order (e.g. 200, 300).

```toml
id = "carbon.editor"           # legacy section id: "code"
schema_version = 1
version = "1.0.0"
carbon_api = ">=0.1, <0.2"
name = "Editor"
icon = "code"
order = 100
legacy_ids = ["code"]          # layout.json migration alias

[layout.center]
panels = ["editor"]
[layout.left]
panels = ["explorer"]
open = true                    # size = host default for the side
[layout.right]
panels = ["chat"]
open = true
[layout.bottom]
panels = ["terminal"]
open = true

[panels]
explorer = { multi = false, closable = false }   # as built today
editor   = { multi = true }
terminal = { multi = true }
chat     = { multi = true }

# Built-in menu items/shortcuts stay global in v1 (not scoped).
```

**Package folder (Q22, decided 2026-10-20).** Contributors arrange a package as they like. Only `workflow.toml` at the package root is fixed (discovery, `installed/<id>/<version>/` and hash pins depend on it); every other file (icon, WASM, webview HTML, assets) is named by path in the manifest, and Carbon never looks for files by folder name. Path rules, checked at load so a package works on every OS:

| Rule | Why |
|---|---|
| Relative, `/` separators; no `..`, no absolute paths, no symlinks leaving the package | Safety; same reference on every OS |
| Case matches the file exactly | Linux is case-sensitive, Windows/macOS are not |
| No Windows reserved names (`CON`, `NUL`, `AUX`, `COM1`, ...) or `<>:"|?*` | Package must install on Windows |
| Short paths | Windows 260-char limit; installed packages already sit 3-4 folders deep |

A `workflow.toml` guide for contributors (suggested folders like `assets/`, never enforced) comes later.

### 4.2 Layers and precedence

| Layer | Location | Owner | Writable by user | Contents |
|---|---|---|---|---|
| L0 built-in / shipped | package folders `apps/carbon-workflow/<name>/` shipped read-only next to the binary (built 2026-10-20); the Editor also compiled in as fallback | Carbon | no | Editor (real, not removable), Paradigm Editor and Agent Automation (placeholders) |
| L1 installed | `<local data>/Carbon/workflows/installed/<id>/<version>/` (loaded since 2026-10-20; installing is Phase 4) | package | no (replaced on update) | manifest + assets |
| L2 user-authored | `<user>/workflows/user/<id>/workflow.toml` | user | yes | whole workflow created by hand or Duplicate |
| L3 overlay (Q3 decided) | `<user>/workflows/overlay/<id>.toml` | user | yes | small patch of an L0/L1 base: `name`, `icon`, `order`, `hidden`, `layout` replace or add/remove panels, `keybindings` add/disable. Not allowed: id, capabilities, requires |
| L4 dock state | `<user>/workflows/state/state.json` | app | auto | last `DockAreaState` per id + `active` + `base_fingerprint` |

Effective workflow = L0 or L1 base, then L3 patch applied, then L4 dock state restores the arrangement. Rules:

- Id collision: L2 may not reuse a built-in id. A user *shadowing* a built-in is done with an overlay (L3), never a copy (avoids Roo-style silent whole-file replace) **(inf)**.
- Update of L0/L1: base replaced; overlay re-applied; ops that target a removed panel are dropped and listed in a notice. Dock state carries `base_fingerprint`; on mismatch run the merge: keep user positions of panels still present, open new default panels in their default region only when `closable` allows, drop missing ones (Eclipse lesson, no stale hidden views).
- **Reset to default** (per workflow): delete L3 and L4 for that id, apply base layout. **Reset all**: same for all. Both confirm.
- **Duplicate to customize**: copy effective workflow into L2 with a new id and `derived_from`; later "show diff vs parent" is possible. Duplicate of a built-in is the supported way to change its panel set heavily.
- Remove: L2 deleted (confirm), L1 uninstalled (keep L3/L4 for reinstall unless asked), L0 can only be *disabled/hidden*, never removed.

### 4.3 Registry and lifecycle

States: `discovered -> valid -> enabled -> active | faulted | quarantined | disabled`. Persisted enabled/quarantine flags live in `state.json`, separate from files, so disabling does not need deletion.

| Step | What happens | Failure |
|---|---|---|
| register(validate) | Parse, check schema, `carbon_api`, id rules, panel kinds known, command ids known, path rules (no `..`) | Workflow marked `invalid` with reasons; others unaffected |
| enter | Apply layout (restore L4 or build from template); create panels lazily; register this workflow's scoped commands/menu items/bindings; emit `MenusChanged` | Missing panel -> placeholder (s4.4); error -> fall back to previous workflow, mark `faulted` |
| exit | Dump layout to L4; unregister scoped contributions (Unreal); panels go away with the dock tree | Dump failure logged, old L4 kept |
| unregister | Remove from registry; Section Bar, menus, palette observe the change (Eclipse registry events) | If it was active, switch to the default workflow |

Kept per workflow: dock layout and sizes, open/closed docks, auto-hide flags, panel-local state that panels already persist. Shared across workflows (not switched): workspace, buffers, terminals, chats, LSP, settings. Open editor tabs: a tab belongs to the dock state of the workflow where it lives (today's behaviour); same buffer in two workflows is fine because buffers are in core.

Background policy (W4): on exit a workflow is suspended by default (render loops, audio streams, timers stop). A workflow may declare `background` tasks (e.g. automation runs); these keep running, show a visible entry in the status bar jobs item and Manage Workflows, and have a Stop control; the user can deny background for a workflow (Q20, decided 2026-10-20). Shared terminals/LSP keep today's behaviour. Data between workflows moves through workspace files and MCP (W6); a workflow never holds a reference to another workflow's panels or state. A workflow with `requires_workspace = true` and no folder open stays enterable but shows an empty state.

Active workflow = a `Global` readable by commands and `when` evaluation; `View > Section` stays but is generated from the registry. Optional later: `handles` auto-switch hook (Godot `_handles`).

### 4.4 Panel kinds

| Aspect | Design |
|---|---|
| Panel-kind registry | String id -> factory (`explorer`, `editor`, `terminal`, `chat`, `placeholder`). Today `layouts.rs::tabs()` matches on *title* and each panel calls the kit's `register_panel(PANEL_NAME)` for restoring dock state by name: the new registry replaces the title match and wraps the same factories; kit registration stays |
| Layout reference | Manifest names kinds, not Rust types; the GUI maps kind -> factory. `DockAreaState` keeps the kit's own panel names, so existing saved layouts still load |
| Missing kind | Layout loads with a placeholder panel ("Panel `x` is not available", Repair / Remove buttons), never fails the whole layout; repair = drop slot or install provider. The existing `repair_layout` (empty center refills) keeps running |
| Contributed panels (Q17, Q18, decided 2026-10-20) | Two kinds. **Declarative UI** for simple workflows: a UI tree (list, tree, text, form, table, button, markdown; later diagram canvas) drawn by host GPUI code; events return as messages. Logic in tiers: tier 1 = tree and actions written in the manifest, no code, actions call host commands only; tier 2 = a WASM component builds the tree and handles events (one Store per workflow, s4.8). **Webview** for advanced views: HTML/JS (WebGL/WebGPU for graphics), logic is JS inside the webview, talks to Carbon through a host bridge gated by `capabilities`. GPUI types never cross the boundary (kit breaks every minor, R-4). Native in-process plugins rejected for the same reason and for lack of a sandbox; imperative GPUI views stay first-party only |
| Webview panel | Embedding not built and unverified: GPUI has no webview; candidate `wry` (Tauri's library: WebView2 / WKWebView / WebKitGTK attached to a native window) **(?)**. Theme variables passed in so pages can match Carbon. Cost: memory per panel |
| Surface kinds (W3) | `surface.gpu`, `surface.audio`, `surface.canvas` as host-provided handles: **superseded by Q17** (2026-10-20): graphics go through the webview; native surface handles are not planned and are revisited only if a webview cannot meet a need (low-latency audio, Q12) |
| File-type handlers (W2) | Registry glob/MIME -> (workflow, panel kind); opening a file uses the handler of the active workflow, else the user's default. Related: FEATURE.md EX-05 (viewer registry) |
| Cost for third parties | Phase 2 workflows are layout over built-in kinds only; this is already useful (e.g. "Review": editor + chat + terminal rearranged, a second chat) |

### 4.5 Boundaries (sketch, not code to paste)

| Piece | Crate | Contents |
|---|---|---|
| `WorkflowDef`, `Layout` template, `Overlay`, schema versions, validation, `apply_overlay`, id/migration rules, `state.json` model, discovery of folders, fingerprints, fault counters | `carbon-core` (new module `workflow`, no UI deps, unit-testable, same pattern as `chat`) | pure data + `Result<_, WorkflowError>` |
| `WorkflowRegistry` (Entity/Global): list, `active`, subscribe | `carbon-gui` | wraps core registry, emits `WorkflowsChanged` |
| Panel-kind registry, placeholder panel, build layout from template | `carbon-gui` | extends `layouts.rs` |
| Section Bar, View > Section, scoped command/key registration, Manage Workflows dialog | `carbon-gui` | consume registry |

Pseudo-signatures, illustrative only: `registry.workflows() -> &[Effective]`; `registry.activate(id) -> Result<()>`; `validate(def, host_api) -> Vec<Problem>`; `PanelKinds::build(kind, ctx) -> Option<PanelHandle>`; `SwitchSection { id: WorkflowId }` (string newtype instead of enum). The core never sees GPUI; the layout template converts to `DockLayout` only in the GUI.

### 4.6 Scoped commands, menus, keys

- Today palette commands come from `menus::app_menus()` (`commands.rs::all`), keys from global `app_bindings()` (context `None`), and menus are rebuilt on `MenusChanged` via `refresh_menus`. Workflow contributions plug into the same paths: a scoped item = menu entry + palette command generated while that workflow is active, so palette parity tests keep holding.
- Scoped key bindings are skipped for now (Q9, decided): workflows ship without scoped keys, no spike; `keybindings` stays in the schema but is ignored. Later idea: a GPUI key context per active workflow **[BG: unverified kit/GPUI context API]**, or register on enter / clear on exit.
- Conflicts: global app bindings win; a workflow cannot rebind reserved ones (quit, palette, copy/paste); conflicts inside a workflow are validation warnings.
- A workflow command's `run` can only name an existing host command id (an action name), so it cannot invent behaviour.

### 4.7 Storage and migration of `layout.json`

**Storage locations (user decision 2026-10-20, Q15-Q16; built the same day except the installer/Settings controls, `../built/workflows.md`):**

| What | Where (default) | Writable | Notes |
|---|---|---|---|
| Shipped workflow packages (Carbon's own) | Repo `apps/carbon-workflow/<name>/` (`editor/`, `paradigm/`, `agent/`), installed read-only next to the binary (`<install dir>/carbon-workflow/<name>/`) | no | One folder per workflow: `workflow.toml` + assets, later its own code. Folder names are short; ids stay `carbon.editor`, `carbon.paradigm-editor`, `carbon.agent-automation` (they key saved state) |
| Installed packages (downloaded by the user) | `%LOCALAPPDATA%\Carbon\workflows\installed\<id>\<version>\` (OS local data dir) | app | Large and re-downloadable, so not in the roaming profile; survives app reinstall |
| Preferences and the workflow list (metadata) | `%APPDATA%\Carbon\workflows\` (OS config dir): `user/`, `overlay/`, `state/` | user/app | As built in Phase 2 |

- All three locations are user-changeable, in the installer and later in Settings (Manage Settings is not built: until then only `CARBON_WORKFLOWS_DIR` overrides the preferences root).
- Editor, Paradigm Editor and Agent Automation are packages; the Editor cannot be removed (it is the default and safe-mode workflow). A copy of the Editor manifest stays compiled in, so Carbon still opens if the shipped folder is missing or moved (recommended; to confirm at build time).
- Program folders are read-only for normal users (Windows `Program Files`, macOS signed `.app`, Linux `/usr`) and are replaced on update, so downloaded packages never go there.


| Today | New |
|---|---|
| `target/carbon-gui/layout.json` = `{ active: "code", sections: { "code": DockAreaState } }` (serde lowercase enum names; docks `version == LAYOUT_VERSION` or dropped) | `<user>/workflows/state/state.json` = `{ schema, active, enabled, quarantined, workflows: { <id>: { dock: DockAreaState, base_fingerprint } } }` |
| Named layouts + auto-hide flags in `LayoutStore` (separate file) | Per workflow (Q8, decided): each workflow has its own named layouts (max 9) and auto-hide flags, keyed by workflow id; the user can change them. Today's global `layouts.json` is read as the Editor's set. Scheduled for Phase 3 (Phase 0-2 keep the global store) |

Migration (built in Phase 1): `layout.json` stays readable by builds from before workflows, which parse `active` and `sections` with an enum key and fail on any unknown key. So workflows with a legacy id are written in `sections` under it (`"code"`), `active` always names one (the Editor when another workflow is active), and the other workflows go in `workflows` and `active_workflow`, which old builds ignore. Rollback keeps the Editor layout; only the placeholder workflows' layouts are not seen by the old build. Reading resolves `"code"` to `carbon.editor` through `legacy_ids`. Phase 2+ writes the new file and keeps `layout.json` untouched as a backup. `target/` is a dev location; the final user dir is the per-OS config dir `Carbon/workflows/` (Q6, decided). Unknown ids in old files are kept, not deleted (workflow may be reinstalled).

### 4.8 Failure containment, trust, versioning

| Mechanism | Detail |
|---|---|
| Validation isolates | One bad manifest = one `invalid` row in Manage Workflows with reasons; startup never blocked |
| Crash-loop guard | Before entering a workflow write `entering=<id>` to `state.json`; if next start finds it, quarantine that workflow (disabled, reason shown) and open the default |
| Quarantine counters | N faults in M minutes disables; user re-enables manually |
| Safe mode | `--safe-mode` (also offered after an unfinished start): only L0 built-ins, no overlays, no extensions; persisted files untouched. Later: Bisect over enabled workflows/extensions (VS Code Extension Bisect) |
| Layout repair | Missing panel kinds -> placeholder; corrupt `DockAreaState` -> base layout for that workflow only; `workflow_version` merge (s4.2) |
| Trust levels | built-in > signed/hash-pinned installed > unsigned installed > project-provided. Layout-only manifests need no prompt. Anything with capabilities, `requires.mcp`, scripts or `[agent].hooks` shows requested capabilities at install/enable; an update asks only for capabilities it adds (update diff); grants can be revoked in Manage Workflows (Q19, decided 2026-10-20; Claude Code + Zed lessons). Workflow found in a cloned repo needs workspace trust (WB-05) first |
| No auto-run | Install never executes scripts; no shell-injection syntax in agent files; invisible Unicode flagged in instruction files (prompt-injection findings: Snyk ToxicSkills, GlassWorm) |
| API negotiation | `carbon_api` range checked at register; newer-than-host = "needs newer Carbon", disabled not crashed. Host keeps loaders for the last N `schema_version`s; deprecation shown in Manage Workflows before removal (Zed keeps all old WIT versions: budget the cost) |
| Device capabilities | `gpu`, `audio.input` (microphone) and `audio.output` are granted per workflow and show an indicator while active; denied = typed error, workflow keeps running without that surface |
| Isolation of code | Only declarative data in-process now; WASM later: one Store per workflow, epoch deadline + memory limit, faults counted; MCP/LSP as subprocesses under the planned OS sandbox |

### 4.9 Interop

- Read `AGENTS.md`, `SKILL.md` folders and `.mcp.json` unchanged for the future `[agent]` part (no new format).
- Optional importer: a Claude-style plugin folder (`.claude-plugin/plugin.json`) becomes a workflow with no layout (agent-only), default hidden from the Section Bar. Low cost, wide ecosystem **(inf)**.
- WASM later uses a versioned WIT world (`carbon:workflow@x.y`) like Zed; the manifest stays the source for layout/commands.

### 4.10 Section Bar overflow (Q5, decided)

No fixed cap on entries. When there are more entries than fit the bar height, an up/down arrow appears at the ends of the workflow list; clicking scrolls the list, and the mouse wheel scrolls it too. The Manage buttons at the bottom stay fixed. Users can still hide workflows (overlay `hidden`).

## 5. Migration plan (app works after every step)

Touched code found by grep: `section_bar.rs` (enum `Section`, `label/icon/id`, `SectionBar`), `menus.rs` (`SwitchSection { section }`, `MenuState.section`, View > Section built from `Section::ALL`), `commands.rs` (icon for `"SwitchSection"`), `workbench.rs` (`LayoutFile`, `section_preset`, fields `section`/`section_layouts`, `switch_section`, `reset_layout`, `save`, `restore`, `menu_state`), `layouts.rs` (`Preset::Code`, `tabs()` title match), `main.rs` (`SectionBar::new`, `set_active`, `SwitchSection` handler), `manage.rs` (Workflows placeholder), panels' `register_panel`. Presets Paradigm/Agents stay as View > Layout presets and additionally get placeholder workflows (Q4).

| Phase | Change | Files / functions | Tests added | Rollback |
|---|---|---|---|---|
| 0 Id adapter (built 2026-10-20) | Add `WorkflowId(String)` newtype; `Section::id_str()`; `LayoutFile` keys become strings via an adapter; `SwitchSection` carries the id; behaviour identical | `section_bar.rs`, `menus.rs`, `workbench.rs` (`LayoutFile`, `switch_section`, `restore`), `main.rs` | old `layout.json` (`"code"`) loads; save round-trips byte-compatible keys; palette tests (`commands.rs`: every menu action once, bound actions have commands) stay green | revert commit; file format unchanged |
| 1 Built-in via registry (built 2026-10-20) | New `carbon-core::workflow` (types, TOML parse, validate, no UI); three embedded manifests: `carbon.editor` (real) plus placeholder `carbon.paradigm-editor` and `carbon.agent-automation` (placeholder panel, Section Bar icons); GUI registry feeds Section Bar, View > Section, `section_preset`; panel-kind registry replaces title match in `layouts.rs::tabs()` (title match kept as fallback); `legacy_ids` | core `workflow/`, `layouts.rs`, `workbench.rs`, `section_bar.rs`, `menus.rs` | golden test: layout built from manifest == `Preset::Code` layout dump; manifest validation unit tests (bad id, unknown kind, bad range); registry lists exactly the three built-ins in `order`; switching to a placeholder and back keeps each layout; old layout file restores; `--safe-mode` flag test | revert the commit; `layout.json` stays readable by the old build (s4.7); `Preset::Code` kept |
| 2 User workflows from folder (built 2026-10-20) | Scan `<user>/workflows/user/*`; overlay L3; dock state in `state.json`; placeholder panel; scoped commands/menus (no keys, Q9); crash-loop guard | core discovery + overlay + state; GUI placeholder panel, `refresh_menus` | invalid manifest skipped, others load; id collision; overlay apply/reset; missing panel -> placeholder; corrupt state -> base layout; entering-flag quarantine; two workflows keep separate layouts across restart | delete `workflows/` dir; Phase 1 behaviour remains |
| 3 Manage Workflows dialog (built 2026-10-20) | List (status, reason), enable/disable, hide, duplicate, reset, remove (user ones), open folder; in `manage.rs`; per-workflow named layouts (Q8); Section Bar scroll arrows + wheel (s4.10) | `manage.rs`, `layouts.rs` `LayoutStore`, `section_bar.rs`, registry events | dialog actions drive registry (core-level tests); reset deletes L3/L4 only; built-in cannot be removed | dialog is additive; hide the buttons' content |
| 4 Packages / install | Install from folder/zip/git (pinned hash/tag), `installed/<id>/<version>/`, catalog JSON, capability prompt (Q19), update diff, package path rules (Q22), Install button for missing `requires` (Q21), scoped keybindings (deferred, Q9; only if later wanted) | core `install`, GUI install flow | hash mismatch refused; path rule violations rejected; update keeps overlay; downgrade/`carbon_api` mismatch disabled; no script executed on install | uninstall removes `installed/`; user layers untouched |
| 5 Contributed panels, agent behaviour | Declarative UI vocabulary (tier 1 manifest, tier 2 WASM host) + webview panel with capability bridge (Q17, Q18); `[agent]` loads AGENTS.md/SKILL.md/.mcp.json; capability enforcement | new extension host crate, chat/agent code | fault -> quarantine; denied capability returns typed error; panel crash shows error card, window survives | disable extension feature |

Every phase: `cargo test --workspace`, clippy `-D warnings`, fmt; manual click test per `gui-click-test-method` (separate target dir, back up shared `layout.json`).

### 5.1 Built: Phase 0-1 (2026-10-20)

Phase 0 and 1 were built together (no separate id-adapter commit). What differs from the plan above:

| Plan | Built |
|---|---|
| `WorkflowId` in the GUI | In `carbon-core::workflow` (`WorkflowId(String)`, serde transparent); `SwitchSection { id }`, `MenuState.section`, workbench maps keyed by it; `Section` enum removed |
| Panel refs name kinds | Layout regions name entries of `[panels]`; each entry has `kind` (default: its name), `title`, `multi`, `closable`. Lets one workflow use two placeholders with different titles |
| Region flags `fixed`, `closable`, `auto_hide` | Not in schema 1: regions take `panels`, `size`, `open` only (strict table, unknown keys reject the workflow). `multi`/`closable` are parsed but not enforced |
| Reserved keys (`commands`, `menus`, `keybindings`, `requires`, `capabilities`, `[agent]`, ...) | Accepted and ignored silently; any other unknown top-level key is a warning printed at start |
| Panel-kind registry replaces the title match | `layouts::PanelKind` builds panels by kind for workflows; `Preset::Code` uses the same `LayoutSpec`; Paradigm/Agents presets still go through the title match (`tabs`) |
| Feature flag `workflows=embedded-only` | Not built: rollback = revert the commit; `layout.json` stays readable by the old build (s4.7) |
| `--safe-mode` | Built: defaults for every workflow; `layout.json` neither read nor written. No overlays or user workflows exist yet to skip |
| Missing dock in a manifest | The dock is set empty and closed, so nothing stays from the previous workflow's layout |
| Editor id and default | `carbon.editor` is the default workflow; an unknown saved `active` falls back to it |

Tests: core `tests/workflow.rs` (14), GUI `layouts.rs` (3, incl. golden Editor manifest == `Preset::Code`). Click test results in `../built/section-bar.md`.

### 5.3 Built: Phase 3 (2026-10-20)

Details: `../built/workflows.md`. Differences from the plan:

| Plan | Built |
|---|---|
| Enable/disable, hide | Turn On / Turn Off only (`state.json` `disabled`); hide stays an overlay/manifest key, not a dialog action |
| Reset deletes L3 and L4 | Deletes the overlay and the saved dock layout; named layouts (explicit saves) stay |
| Remove: L1 uninstalled keeps L3/L4 | As planned; a removed user workflow also loses its state entries and overlay |
| Per-workflow named layouts (Q8) | `state.json` `layouts` (id -> `LayoutStore` JSON: named layouts + auto-hide); the old `layouts.json` is the Editor's set once |
| Registry events | `workflows::reload` swaps an `Arc<Registry>` after each dialog action; the Section Bar and menus refresh through `MenusChanged`; Reload re-reads files without a restart |
| Section Bar overflow (s4.10) | Built as decided (arrows + wheel) |

### 5.2 Built: Phase 2 (2026-10-20)

Details and verification: `../built/workflows.md`. Differences from the plan and the review points settled:

| Plan / review point | Built |
|---|---|
| Presets Paradigm/Agents beside the workflows (review 5) | Removed (user, 2026-10-20); View > Layout > Preset keeps Code |
| State location (s4.7) | `<config>/Carbon/workflows/state/state.json` (or `CARBON_WORKFLOWS_DIR`); `layout.json` migrated once, never written again |
| Crash-loop guard clearing (review 2) | Mark file `state/entering`, deleted 2 s after a workflow opened. Unfinished user workflow -> quarantined; unfinished built-in -> next start in safe mode; built-ins never quarantined |
| Merge after a base update (review 3) | `base_fingerprint` (layout + panels only, not name/icon/order) and `base_panels` saved; on mismatch the layout resets with a notice. Merge itself not built |
| Safe mode and saved state (review 6) | Safe mode reads no user files and no `state.json` (base layouts), writes nothing |
| User ids (review 7) | Any `publisher.name` not under `carbon.*`; a duplicate of a built-in or earlier file is rejected. No installed layer yet, so no L1/L2 clash |
| Missing panel kind | Unknown kinds are a warning, not a rejection; they show as "Panel `x` is not available" (`TestPanel`). The kit's own `InvalidPanel` still covers unknown panel names inside saved dock state. No Repair/Remove buttons yet |
| Overlay ops | `name`, `icon`, `order`, `hidden`, per region `panels`/`add`/`remove`/`size`/`open`, new `[panels]`; only for built-ins. Key bindings not allowed (Q9) |
| Scoped commands/menus | `[[commands]]` (`id`, `label`, `run` = action name, optional `args`) and `[[menus]]` (`menu` = File/Edit/View/Help, `command`). Built at menu refresh with `build_action`; unknown actions left out and logged once |
| Quarantine counters (N faults in M minutes) | One unfinished opening quarantines; re-enable by editing `state.json` until Phase 3 |
| Registry events / live reload | Loaded once at start; changes need a restart |

## 6. Decisions, risks and open questions

| # | Question / risk | Status and answer |
|---|---|---|
| Q1 | Manifest format TOML or JSON? | Decided 2026-10-20: TOML, JSON accepted for the same schema |
| Q2 | Can a user change a built-in? | Decided 2026-10-20: only via overlay (rename/icon/order/hide/panel set) or Duplicate; never edit the embedded file |
| Q3 | Overlay patch vs whole-file replace (Roo style)? | Decided 2026-10-20: recommended default. Small patch overlay (L3) for tweaks of built-in workflows; whole-manifest copy for Duplicate; no deep merge beyond listed ops |
| Q4 | Should Paradigm/Agents become workflows now? | Decided 2026-10-20: yes (reverses the earlier default). Placeholder workflows `carbon.paradigm-editor`, `carbon.agent-automation` with placeholder panels and Section Bar icons, used to test and design the system; real content later |
| Q5 | Cap on Section Bar entries / overflow? | Decided 2026-10-20: no cap; scroll arrows (up/down) plus mouse wheel when entries exceed the bar (s4.10); replaces the overflow menu |
| Q6 | Where do user files live? | Decided 2026-10-20: OS config dir `Carbon/workflows/`; moving the whole app state dir is its own decision |
| Q15 | Where do workflow packages live? | Decided 2026-10-20: shipped ones in repo `apps/carbon-workflow/<name>/` (editor, paradigm, agent), installed read-only next to the binary; downloaded ones in the OS local data dir `Carbon/workflows/installed/`; preferences and the workflow list stay in the config dir (s4.7). Built 2026-10-20 |
| Q16 | Can users move these locations? | Decided 2026-10-20: yes, all three (shipped, installed, preferences), in the installer or in Settings. Settings is not built: environment variables `CARBON_SHIPPED_WORKFLOWS_DIR`, `CARBON_INSTALLED_WORKFLOWS_DIR`, `CARBON_WORKFLOWS_DIR` meanwhile |
| Q7 | Id for the Code Editor workflow? | Decided 2026-10-20: user-facing name is now "Editor", id `carbon.editor`, `legacy_ids = ["code"]` for the `layout.json` key; no other aliases. Follow-up: rename the UI label/docs from Code Editor to Editor (`README.md` in this folder) |
| Q8 | Per-workflow named layouts? | Decided 2026-10-20: yes (s4.7, Phase 3); built 2026-10-20 |
| Q9 | Scoped keybindings | Decided 2026-10-20: skip the spike for now; ship without scoped keys (s4.6) |
| Q10 | Per-workspace vs per-workflow layout (WB-09) | Decided 2026-10-20: per workflow; each workflow has its own layout config the user can change; per-folder later on top (key `(folder, workflow)`) |
| Q11 | How do extension-backed workflows get GPU/audio/canvas surfaces safely? | Superseded 2026-10-20 by Q17: through the webview (WebGL/WebGPU); native surface handles only if a webview cannot meet a need (s4.4) |
| Q12 | Device ownership: one shared audio/GPU manager in the host, or per workflow? Real-time audio thread vs GPUI main thread | **Open** (no evidence gathered; sound design needs a low-latency path) |
| Q13 | Behaviour when a capability is denied or unsupported (e.g. no GPU)? | **Open**, recommended: disable with a visible reason, like a missing `requires` |
| Q14 | Which workflow opens a file when several claim the glob? | Decided 2026-10-20 (recommended default accepted): active workflow, else per-type user default ("Open With" asks once) |
| Q17 | How do contributed workflows show their own UI? | Decided 2026-10-20: declarative UI for simple workflows + webview for advanced views; no native in-process plugins (s4.4) |
| Q18 | Where does workflow logic run? | Decided 2026-10-20: tier 1 no code (tree and actions in the manifest), tier 2 WASM component; webview logic is JS through a capability-gated bridge (s4.4) |
| Q19 | When are capabilities granted? | Decided 2026-10-20: at install; an update asks only for new ones; runtime denial = typed error; revoke in Manage Workflows (s4.8) |
| Q20 | Background work after switching away? | Decided 2026-10-20: suspended by default; declared `background` tasks visible in the status bar and Manage Workflows with Stop; user can deny per workflow (s4.3) |
| Q21 | A `requires` item is missing? | Decided 2026-10-20: workflow disabled with the reason and an Install button per item; nothing installed without a click (s4.1) |
| Q22 | Package folder structure? | Decided 2026-10-20: free; only `workflow.toml` at the root is fixed; other files named by path in the manifest; path rules checked at load (s4.1). Contributor guide later |
| Risk | Kit breaks every minor | Manifest never mentions GPUI; only the thin GUI adapter changes |
| Risk | Panel-kind registry duplicates kit `register_panel` | Registry wraps, does not replace it; verify with restore test |
| Risk | Over-design before a second workflow exists | Phases 0-1 are small and testable alone; Phase 1 already ships two placeholder workflows (Q4), so a second workflow exists; the range in s3.1 is design input only and adds no code before Phase 5 |
| Risk | Evidence gaps | Blender, Zed mode concepts, Cursor, VS Code crash semantics unverified; do not rely on them |

## 7. Sources (from the research notes)

- Qt Creator: https://doc.qt.io/qtcreator-extending/core-imode.html ; https://doc.qt.io/qtcreator-extending/plugin-meta-data.html ; https://git.qt.io/tohunger/qt-creator/-/blob/3ab0a28cca0c20baa4b21a3d08223065a1e3be94/src/plugins/coreplugin/modemanager.cpp
- Godot: https://docs.godotengine.org/en/stable/tutorials/plugins/editor/making_main_screen_plugins.html ; https://docs.godotengine.org/en/stable/tutorials/plugins/editor/installing_plugins.html
- Eclipse: https://help.eclipse.org/latest/topic/org.eclipse.platform.doc.isv/reference/extension-points/org_eclipse_ui_perspectives.html ; https://www.eclipse.org/forums/index.php/t/1078421 ; https://wiki.eclipse.org/FAQ_When_does_a_plug-in_get_started%3F
- IntelliJ: https://plugins.jetbrains.com/docs/intellij/tool-windows.html ; https://plugins.jetbrains.com/docs/intellij/dynamic-plugins.html ; https://plugins.jetbrains.com/docs/intellij/plugin-extensions.html
- VS Code: https://code.visualstudio.com/api/references/contribution-points ; https://code.visualstudio.com/docs/configure/profiles ; https://code.visualstudio.com/api/advanced-topics/extension-host ; https://code.visualstudio.com/blogs/2021/02/16/extension-bisect ; https://github.com/microsoft/vscode/issues/95182 ; https://code.visualstudio.com/docs/copilot/customization/custom-agents
- Unreal: https://dev.epicgames.com/documentation/unreal-engine/API/Editor/UnrealEd/UEdMode/RegisterTool
- Obsidian: https://obsidian.md/help/plugins/workspaces ; https://docs.obsidian.md/Reference/Manifest
- Blender API: https://docs.blender.org/api/5.0/bpy.ops.workspace.html
- Zed: https://zed.dev/blog/zed-decoded-extensions ; https://zed.dev/docs/extensions/developing-extensions ; https://github.com/zed-industries/zed/blob/main/crates/extension_api/README.md ; https://zed.dev/docs/configuring-zed
- Claude Code / Agent Skills: https://code.claude.com/docs/en/plugins-reference ; https://code.claude.com/docs/en/skills ; https://code.claude.com/docs/en/plugins/security ; https://code.claude.com/docs/en/plugins/org ; https://code.claude.com/docs/en/plugins/dependencies ; https://agentskills.io/specification
- Roo / Windsurf / Kiro / Cursor: https://roocodeinc.github.io/Roo-Code/features/custom-modes ; https://docs.devin.ai/desktop/cascade/workflows ; https://kiro.dev/docs/steering/ ; https://cursor.com/docs/context/rules ; https://forum.cursor.com/t/ui-constantly-resets-layout-in-cursor-2-0/139511?page=3
- Other: https://www.nushell.sh/contributor-book/plugins.html ; https://zellij.dev/documentation/plugins.html ; https://github.com/swc-project/swc/blob/main/docs/adr/00001-support-transformation-plugin-native.md ; https://grafana.com/developers/plugin-tools/publish-a-plugin/sign-a-plugin ; https://docs.rs/wasmtime/latest/wasmtime/struct.Config.html ; https://component-model.bytecodealliance.org/design/wit.html ; https://snyk.io/blog/toxicskills-malicious-ai-agent-skills-clawhub/ ; https://thehackernews.com/2025/10/self-spreading-glassworm-infects-vs.html ; https://modelcontextprotocol.io/registry/about ; https://github.com/modelcontextprotocol/mcpb/blob/main/MANIFEST.md
