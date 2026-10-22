# Carbon design

UI and interaction rulebook for Carbon. Read it before any UI build or fix. A rule asked for once ("never do X again") goes here so it holds for old and new screens. Scope lives in `FEATURE.md`, decisions in `PROJECT.md`, evidence in `RESEARCH.md`.

Tags: **[K]** adopted from `knowledge/DESIGN.md` (previous projects, read-only), **[R]** from `RESEARCH.md`, **[N]** new for Carbon, **(?)** proposal to validate. Carbon is a native desktop GUI (decided by the user; webview excluded); the framework is still pending (`PROJECT.md`, `documents/gui-framework.md`), so rules stay framework-neutral.

Contents: 1 Design stance - 2 Principles - 3 Foundations - 4 App shell - 5 Pillar surfaces - 6 Components - 7 Agent interaction - 8 Accessibility - 9 Lightweight budgets - 10 Cross-platform - 11 Text - 12 Checklist - 13 Open questions


# 1. Design stance: Zed's compactness, VS Code's approachability

Decided by the user: Carbon is compact and quiet like Zed, and as easy to learn and use as VS Code. Zed's UI details below are background knowledge, not researched (?). **[N]**

| Take from Zed | Take from VS Code | Leave out |
|---|---|---|
| Few visible parts: no activity bar by default, no toolbars, no banners | Everything findable: command palette, searchable settings, familiar default keymap | VS Code's many sidebars, badges, getting-started pages and notification prompts |
| Dense layout: small tabs, tight rows, one-line status bar | Familiar names and places: Explorer, Search, Source Control, Terminal, Problems | Zed's gaps: sparse settings UI, little discoverability for new users |
| Panels in docks (left, right, bottom) toggled from the status bar | Import of VS Code keymap, settings and themes | Heavy first-run flows |
| Fast, calm feel: no animation delays, instant panel toggles | Clear labels and tooltips with the shortcut shown | Features hidden only in JSON |

Rules that follow:

- **Quiet by default, discoverable on demand.** The first screen shows a file tree, the editor and a thin status bar. Every other surface is one shortcut or one palette command away, and every control shows its name and shortcut in a tooltip.
- **Simple means few visible choices, not few features.** Depth lives behind the palette, settings search and context menus; nothing is removed to look minimal.
- **Compact is the default density; comfortable is an option.** Dense rows and tabs, 4 px rhythm, small text steps; never cramped touch targets below the contrast and hit-size rules in 8.
- **One way to see everything:** the command palette lists every command with its shortcut; settings are one searchable list with a JSON view for power users.
- **Chrome budget:** at most three persistent regions besides the editor (left dock, right dock, bottom dock), each hidden until used or restored from the last session. No persistent toolbar rows.
- **No interruptions:** no modal popups for information, no "what's new" or tip prompts, no AI nudges (see 2).


# 2. Principles

- **One rulebook.** All design rules live here. **[K]**
- **Consistency over local cleverness.** Same meaning, same look: one list pattern, one picker, one colour per entity, one icon per meaning. Shared data (formats, colours, keybindings) comes from one source; local copies drift. **[K]**
- **Only offer what applies.** Hide options that cannot apply. Show one disabled, with the reason, only when the user can change something to make it apply. **[K]**
- **Say it once.** No repeated numbers; no status line next to a status that already says enough. **[K]**
- **Don't warn about yourself.** Never warn about a name, value or state the app just chose. **[K]**
- **Calm by default.** AI is opt-out and never nags; no unrequested panels or popups; telemetry off. **[R]**
- **Agent-native, human-in-control.** Every agent action is visible, interruptible and reversible; approvals and cost are first-class surfaces, not buried settings. **[R][N]**
- **Familiar first.** VS Code muscle memory works: command palette, default keymap, settings/keymap/theme import. Differ only where it buys something clear, and prefer Zed-style compact placement when both do the job (see 1). **[R]**
- **Plain words.** Short sentences that say what happens. **[K]**


# 3. Foundations (tokens)

- **Everything is a token.** Spacing, type, radii, motion, layout sizes, colours, fonts: named tokens in one place. Rules use tokens only. **[K]**
- **Spacing:** 4 px base, 2 px half step: 2, 4, 8, 12, 16, 20, 24, 32. Chrome (tabs, tree rows, status bar) uses the small steps (2-8); only content cards and forms use 12-32. **[K][N]**
- **Type scale:** few steps (xs, sm, base, xl, 2xl) multiplied by the user's text-size setting. Chrome text is `sm`; editor font size is a separate setting. Never a fixed size that ignores the user's scale. **[K]**
- **Fonts:** monospace first for the editor and terminal; bundled readable UI alternatives (e.g. Inter, IBM Plex Sans, Atkinson Hyperlegible Next). Bundle fonts, icons and styles; nothing loads from the internet. **[K]**
- **Themes:** built only from colour tokens against a written token contract; a new token means every theme gets a value. Half dark, half light, one dark default. Start with a few (own dark/light pair, Catppuccin, Tokyo Night, Gruvbox, Rosé Pine, Nord) and grow. **[K]**
- **Theme compatibility (?)** Import VS Code colour themes (token mapping to the contract) and keep syntax colours in a separate token group so editor themes and UI themes can differ. **[R][N]**
- **Contrast contract**, checked for every theme: body and muted text >= 4.5:1 on background and surface; text on accent >= 4.5:1; chart and diagram colours >= 3:1 on surface. **[K]**
- **Icons:** one line-icon set (Lucide style) through one icon component; new icons are added there, never as loose files. One icon per meaning. **[K]**
- **Motion:** short, ease-out, respects the OS reduce-motion setting; animations end on their own end event, not timers. **[K]**
- **Density:** compact (default) and comfortable as two token sets; agent transcripts may use comfortable line spacing for reading. **[N]**


# 4. App shell

Zed-style docks with VS Code's familiar panels. **[R][N]**

```
┌ title bar: logo, menus, command center (palette), window controls ─────┐
│ left dock          │ editor area (groups, tabs,      │ right dock       │
│ (files by default; │ splits; code + paradigm +       │ (agents tab by   │
│ search, git,       │ viewers)                        │ default)         │
│ outline on demand) ├─────────────────────────────────┤                  │
│                    │ bottom dock (terminal, problems,│                  │
│                    │ output, agent logs)             │                  │
├────────────────────┴─────────────────────────────────┴──────────────────┤
│ status bar: dock toggles | branch, diagnostics | agents, cost | language│
└─────────────────────────────────────────────────────────────────────────┘
```

- **No Section Bar, no sections** (user decision 2026-10-22, replacing the 2026-10-16 Section Bar; `project/archived/section-bar.md`): one layout. The title bar holds the **command center**, a search box centered on the window that opens the command palette (VS Code), so people see and click it; shortcuts stay. **[N]**
- **App menu** (user decision 2026-10-22): a three-line button at the right end of the status bar, after the language servers; opens upward; Extensions, Settings and other app-level places live there (also File > Preferences and the palette). **[N]**
- *(Superseded 2026-10-22.)* **Section Bar** (user decision 2026-10-16): a vertical icon strip at the far left, like VS Code's activity bar, but it switches **sections** (Code Editor first; later paradigm, agents), each with its own remembered dock layout. It does not toggle single panels. Sections were called **workflows** (2026-10-19); since 2026-10-22 there is no workflow system: panels and Explorer-section tabs come from extensions (`project/archived/workflow-system.md`). What a Section Bar icon is under that model is open (`PROJECT.md`). **[N]**
- *(Superseded 2026-10-22: the Manage dialogs open from the app menu.)* **Section Bar bottom group** (user decision 2026-10-19), top to bottom: **Manage Extensions** (puzzle icon), **Manage Workflows** (workflow icon), **Manage Settings** (gear). Each opens a dialog, never a dock panel; never shown as selected. Extensions = official and user-added plugins, VS Code-like (compilers, linters, appearances); Workflows = add/remove workflows (built-in and custom; dropped 2026-10-22, button still in code, its fate open); Settings = all user options (see Settings). **[N]**
- **Docks.** Left, right and bottom docks hold panels; each panel has a status-bar toggle button with a tooltip and shortcut. Panels can be dragged between docks. **[N]**
- **Default first screen:** left dock with file tree, editor area, thin status bar; right and bottom docks closed. **[N]**
- **Menus:** classic menu row (File, Edit, View, Help) inside the custom title bar on Windows/Linux, after the logo; system menu bar on macOS. Same commands as the palette; no in-window toolbar. (User decision 2026-10-16, replaces hamburger.) **[N]**
- **Tabs and rows are compact:** one-line tabs with close on hover, tree rows in small steps, breadcrumbs optional and off by default. **[N]**
- **Every region is movable and hideable**; layout persists per workspace and can be saved as a profile. Zen / focus mode hides all chrome. **[R]**
- **Command palette** reaches every command, setting and view; the single entry point for keyboard users. Shows each command's shortcut. **[R]** Built: `project/built/command-palette.md` (a menu item is a palette command automatically; others go in `commands::extras`).
- **Multi-window:** an editor, paradigm or agent chat can pop out to its own window (agents tab pillar); windows share one session state. Deferred to layout tier T3 with top dock and guide diamond; T1+T2 first (`project/built/panel-layout.md` s7). **[R][N]**
- **Narrow windows:** docks collapse to drawers; nothing scrolls sideways at the minimum window size. **[K]**
- **Panels and cards:** bordered surface, 16 px padding and gap, stretch with their column, never a fixed width. 20 px between stacked cards. Grid cells in one row share height. Titles live inside the card border. **[K]**
- **Empty states are exact:** nothing selected = one hint line with an arrow toward the panel; no workspace = Open and Create buttons. **[K]**
- **Status bar** shows running jobs and agents with a spinner and Stop, live cost, and diagnostics; no popup when a job starts. **[K][R]**
- **Crash safety visible:** unsaved state is restored after a crash; a save/dirty state is always visible. **[R]**


# 5. Pillar surfaces

## 5.1 Code editor
- Familiar editing model: tabs and groups, splits, multi-cursor, find/replace, folding, inline diagnostics, diff and merge views, quick open, go-to-symbol. Minimap and breadcrumbs exist but are off by default (compact). Full list and priorities: `FEATURE.md` Part 1.
- **Typing latency is a design constraint**, not a feature: no UI work on the keystroke path (no layout, no synchronous IPC). **[R]**
- Agent edits appear as attributed, reviewable changes in the same editor (per-hunk accept/reject), never silent. **[R]**

## 5.2 Paradigm editor (diagrams as AI context)
Unresearched (see `PROJECT.md` open questions). Design rules below come from the previous-project canvas editor and are the starting point. **[K](?)**

- Own route per diagram with the item id in the address, so reload/back keep the place. Leaving saves first.
- Layout: tools strip | canvas with top bar | resize handle | side list (items/nodes), sharing width with the right dock. The top bar and tools strip stay slim; secondary tools appear on demand.
- Top bar: diagram name, state pills, save state, `current / total`, filter, Previous, Next, Close.
- Tools strip: icon buttons with tooltip and single-key shortcut, one active tool: Select/move (V; 8 resize handles; Shift+click adds), Draw/connect, Pan (H or hold Space), Delete (Del). Below: Undo (Ctrl+Z), Redo (Ctrl+Y / Ctrl+Shift+Z), Zoom (+ / -, wheel), Fit (0), overlay visibility.
- Canvas: overlay in the content's own coordinates; zoom/pan by transform; arrow keys nudge 1 px (Shift: 10 px); copy/paste between diagrams; undo history per diagram.
- Side list: full table, rows keyed 1-9, search above, last row "+ Add", rename by double-click or F2, Remove confirms with the counts it will delete.
- **Autosave:** on switch, on leave, on Ctrl+S and 1.5 s after the last change. Save state always visible ("Saved" / "Saving..." / "Save failed - Retry - Discard"); navigation blocked until saved or discarded.
- **Diagram = context (?)** Every diagram has a text form (e.g. Mermaid/JSON) shown beside it; "Use as context" attaches that text to an agent chat; agent-created or edited diagrams show as a reviewable diff, like code. **[N]**
- Diagram colours are entity tokens with the same contrast rule as charts. **[K]**

## 5.3 Agents tab
- One or more agent chats as panes or windows; each pane shows session name, agent/model, state pill (running, waiting for approval, done, failed), cost, and worktree/branch. **[R][N]**
- A sessions list (all workspaces) with unread/needs-approval markers; a unified approvals inbox across sessions. **[R]**
- Compose area: send, queue, stop; context chips (files, diagrams, selections) that can be removed; `@` mentions. **[R]**
- Transcript: collapsible tool calls, diffs inline, plan view, and a linear accessible reading mode (8). **[R]**
- Logs and raw consoles are collapsible and closed by default; the status carries the summary. **[K]**
- Details: 7. The tab opens in the right dock by default and can be moved to the editor area or its own window.
- Built (v1, demo backend): `project/built/chat.md`.

## 5.4 Modular extension sections (viewers and panels)
- A section is a panel or editor type contributed by an extension (SVG viewer, 3D model viewer). It declares: id, title, icon (from the shared set), file types it opens, placement (editor area, a dock), and capabilities it needs. **[N]**
- Hosts give a section: theme tokens, spacing and type scale, focus/keyboard routing, accessibility hooks, and a standard toolbar/empty-state/error frame. Sections never draw their own chrome. **[N]**
- Sections run isolated (sandboxed); a crashing section shows an error card with Reload and never takes down the window. **[R]**
- Installing a section is one action with a visible capability list and a Remove button that confirms (6). **[R][K]**
- All extensions (official and user-added: compilers, linters, appearances, viewers) are listed, installed and removed in the **Manage Extensions** dialog, VS Code-like. Workflows are extensions too since 2026-10-22 (no separate Manage Workflows content; `project/archived/workflow-system.md`). **[N]**
- Image/media sets use one gallery grid with a viewer dialog (arrows step, Esc closes), never bare thumbnails or raw file links. **[K]**


# 6. Components and patterns

## Lists and tables (every one)
- Checkbox column; a row click selects only that row (again clears); a name button in the row for keyboard users. Selection bar: Select all / None / red **Remove** / "N selected". **[K]**
- `#` column; sortable headers (text A-Z first, counts largest first, ids lowest first); count and reload in the header; search above long lists. **[K]**
- Removing asks "Delete N ...? ... can't be undone" in a small popup under the button, never a box that pushes content down. Backdrop click and Esc cancel. **[K]**
- Fixed lists: the checkbox means "included", with Select all / None and count, no Remove; reload resets to defaults. Result tables: sortable, no checkboxes. **[K]**
- Rows with many settings put them on a line under the row so the table never scrolls sideways. A flex wrapper goes inside a cell; the cell itself never becomes flex. Special rows get a bordered badge. **[K]**
- Large lists are virtualized; rendering cost is independent of list length. **[R](?)**

## Explanations, warnings, feedback
- **Explanations are hover "i" icons** next to labels (shown on hover, kept on focus/click), not lines of small text. Small text stays for live values, errors and must-see safety notes. **[K]**
- **Warnings are icons**: a warning icon in the card header lists them on hover/focus. **[K]**
- **Popups escape containers**: placed against the window, never clipped or covered. **[K]**
- **Toasts** for finished actions; status bar for running work. **[K]**
- **Status pills and tags:** rounded border, matching soft background, always a word. Done/installed green, Failed red, Warning yellow, Skipped grey. Never colour alone. **[K]**

## Inputs and forms
- Numbers with a sensible range are sliders (range and value shown; fixed value width; optional special text such as 0 = "Automatic"). Free-range numbers stay number boxes. **[K]**
- Form grids: one field per column; checkbox rows share the columns; an option that modifies an input sits right of it. **[K]**
- Choices are radio cards (whole card clickable, actions below it). Several names: toggle chips, none picked = all. **[K]**
- Form actions: primary button first, then ghost buttons; notes beside them vertically centred. Optional helper sections open by default. **[K]**
- Entity colours change in one place with a swatch button opening presets (4x4 grid, current outlined) plus "Custom colour...". Never a bare system colour input. **[K]**
- Output pickers: one pattern everywhere ("Save as new" default / "Overwrite source" with an "I understand" checkbox); suggested names are `<input>-<suffix>`. **[K]**

## Settings
- Opened from **Settings** in the status bar's app menu (also File > Preferences, the palette). Holds every option the user can change (system, appearance, agents, ...).
- One searchable settings view in plain language, plus a JSON view for power users; both edit the same data. Sections by concern: Appearance, Editor, Agents (models, keys, approvals, sandbox), Extensions, System. Common settings first, advanced ones behind "Show advanced". **[R][N]**
- Layered scopes (user, workspace, folder) shown as a badge on each changed setting. **[R]**
- API keys show "Saved" only, never any part of the key; the line under the field is the "Where to find the token" link; purpose goes in the label's info tip. **[K]**
- System limits sized to the machine (concurrent jobs up to cores and memory, with an info tip). **[K]**

## Jobs and history
- A page shows the job it is running or just ran; finished results then live in a History overlay. **[K]**
- **History overlay:** every job and one-step action as category > section > process with start/end, outcome and details; Esc or outside click closes. Applies to agent sessions: every run, tool call and approval is recorded. **[K][N]**
- Jobs: one per tool at a time; refused when another uses the same data or the concurrency limit is reached; every job is cancellable; per-item outcome (done / failed / warning / skipped / cancelled) with re-run of failed items. **[K]**


# 7. Agent interaction design

Weaknesses found in competitors drive these rules (`RESEARCH.md` s4-7). **[R]**

- **Approvals are tiered and uniform** across every agent and harness: Manual, Assisted (classifier or judge), Allow rules, Autopilot. One policy model for rules, hooks, classifier and sandbox tier. The approval card shows exactly what will run (command, path, URL, diff) and the sandbox it runs in. Approval fatigue is the central problem: offer scoped "always allow this" rules, with a visible list to revoke.
- **Sandbox on by default**; "run outside the sandbox" is an explicit, separately approved request. Terminal auto-approve is never presented as a security boundary.
- **Checkpoints cover side effects**: file snapshots including untracked files, plus a guard on destructive git commands. Restore shows what will change before doing it, and states what it cannot undo (network calls, external effects).
- **Review is one model** across all agents: per-hunk accept/reject in the editor, a Changes view per session, comments sent back to the agent.
- **Parallel sessions are isolated** per worktree with copy-files and setup scripts and per-session port allocation; the pane shows branch and isolation state.
- **Cost and trace transparency:** live cost per session in the pane and status bar; failed requests are not billed to the user; run traces exportable (OpenTelemetry). Rate-limit state is visible, not hidden.
- **Plan before act:** a read-only plan mode is one toggle away; a plan can be edited before it runs.
- **Interruptibility:** Stop works at any time and states what was left half-done.
- **Context is inspectable:** the user sees what is in an agent's context (files, diagrams, rules files) and can remove items. Reads `AGENTS.md`, `SKILL.md` and `.mcp.json` conventions.
- **Handoff:** a session can move between local, worktree and remote without losing transcript or state.


# 8. Accessibility

- Every control has a visible label or an accessible name; tips, warnings, sliders and chart marks carry text for screen readers. **[K]**
- Visible focus (`:focus-visible` style); full keyboard use (lists, split handles, sliders, canvas tools, editor shortcuts); live regions for status and alerts; sort, pressed and checked states exposed. **[K]**
- Contrast >= 4.5:1 in every theme; never colour alone (pills and levels also say it in words). **[K]**
- **Accessibility API from the first widget** (AccessKit): custom-rendered UIs lose screen-reader support otherwise (Zed had none as of Oct 2025). **[R]**
- **Accessible linear view** of agent output and diffs: a plain, ordered, keyboard-navigable reading mode, modelled on VS Code's Accessible View. **[R]**
- **Compact never means small targets:** dense rows keep a usable hit area and visible focus; the comfortable density and the text scale stay available. **[N]**
- Audio/visual signals for agent events (done, needs approval, failed) are optional and configurable. **[R]**
- Reduce-motion honoured; text scale honoured everywhere including the editor chrome. **[K]**


# 9. Lightweight budgets (?)

Proposed targets to validate by our own benchmark; published numbers conflict 2x-15x (`RESEARCH.md` s8). **[R][N]**

- Cold start to a usable editor: target under 1 s (Zed claims under 1 s; Sublime under 1 s).
- Idle memory with one small project: target well under VS Code (about 1.6 GB reported with 3 projects); stretch goal under 600 MB.
- Typing latency: no work beyond buffer update, shaping and paint on the keystroke path; measure key-to-photon on all three OSes.
- Ship small: measure the release bundle, not the dev build; one HTTP/TLS stack; download large optional parts on demand with checksums. **[K]**
- Extensions and sections load lazily; an unused section costs no memory. **[R]**
- Budget regressions fail CI once baselines exist.


# 10. Cross-platform

Windows, macOS and Linux are equal targets. Lessons from previous projects: **[K]**

- Native dialogs must open from the right thread: on macOS they need the main thread and an event loop that keeps pumping; on Windows a server without a window needs a topmost owner window to bring dialogs to the front.
- Windows file operations can fail with "Access is denied" while scanners or indexers hold files: retry rename/delete for up to 10 s, then name the busy path.
- Write only to the user data folder (`%LOCALAPPDATA%\Carbon`, `~/Library/Application Support/Carbon`, `~/.local/share/carbon`); ship read-only resources next to the executable and find them relative to it.
- Pin shell scripts and config to LF line endings.
- Linux needs early testing (Vulkan/Wayland, NVIDIA); prebuilt tools may need a newer glibc than old distributions have. **[R]**
- Same keymap logic with platform modifiers (Cmd vs Ctrl); system fonts for UI fallbacks only, bundled fonts first.


# 11. Text

- Sentence case for labels and titles. Buttons say the action ("Add file", "Run", "Remove"); while working: "Running...", "Working...". **[K]**
- Destructive confirmations name the count and say it can't be undone. **[K]**
- Counts and plurals through shared helpers, never by hand. **[K]**
- Errors show the message only; stack and wrappers stay in logs. **[K]**


# 12. Before finishing a UI change

- Build every target; look at the screen running when possible.
- Check spacing, alignment and equal heights in a dark and a light theme, in compact and comfortable density, and at a large text size.
- Check the chrome budget (1): no new persistent region, toolbar row or prompt without a rule change.
- Check discoverability: the feature has a palette command, a tooltip with its shortcut and a settings entry where relevant.
- Test with keyboard only and with a screen reader on at least one platform.
- Check latency and memory against the budgets in 9 when the change touches the editor or shell.
- Log the change in `STATUS.md`; if it adds or changes a rule, update this file.


# 13. Open questions

- Zed's UI details used in 1 and 4 come from background knowledge; check the current Zed UI (docks, menus, settings UI) before finalizing the shell.
- Which native framework (pending, `documents/gui-framework.md`) decides how tokens, accessibility and virtualization are implemented; the rules above stay valid for any of them.
- Whether the optional activity bar ships in v1 or is a later setting.
- Paradigm editor: diagram formats, AI round-trip and canvas library need research before 5.2 is more than a starting point.
- Extension section API: declarative manifest vs WASM component with UI surface; how much layout freedom sections get.
- Default keymap and which VS Code themes can be imported without loss.
- Density, theme count and default theme.
