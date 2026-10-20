# Modular workflow / mode / workspace / perspective systems (research for Carbon)

Research date 2026-10-20. Carbon context read first (PROJECT.md, section-bar.md, panel-layout.md): a workflow is a `Section` enum variant in `apps/carbon-gui/src/section_bar.rs`; its default layout is a `Preset` in `layouts.rs` via `section_preset` in `workbench.rs`; per-section `DockAreaState` is persisted in `layout.json`; Manage Workflows dialog is a placeholder; extensions are planned as declarative VSIX / LSP / WASM / MCP.

Labels: sourced facts carry a URL. [BG] = my background knowledge, not verified this session. "Inference" = my reasoning. Coverage is uneven: tool budget limited. Deeply sourced: Qt Creator IMode, Godot main screen, Eclipse perspectives, IntelliJ tool windows/dynamic plugins, VS Code contribution points/profiles, Unreal UEdMode, Obsidian Workspaces. Mostly [BG] or thin: Blender (manual pages would not load), Houdini, Adobe, Emacs, Zed, Theia, Pulsar, Figma, Cursor, Raycast, DAWs, KParts, NetBeans, Android Studio.

## 1. What is a workflow/mode/workspace made of, and how is it defined and registered (per product)?

### Takeaway
Products split into two families. (A) "Mode as code object": a plugin registers an object with id, name, icon, priority and a widget factory (Qt Creator IMode, Godot main-screen plugin, Unreal UEdMode). (B) "Mode as declarative layout template": a perspective/workspace is a saved arrangement plus visibility rules, defined by a factory class or manifest entry (Eclipse perspectives, JetBrains tool windows, VS Code view containers, Blender workspaces, Obsidian workspaces). Carbon needs both: a manifest-declared template plus an optional code-defined mode body.

### Cited Findings
- Qt Creator: `Core::IMode` is a mode shown as a tool button in the left mode selector; it controls most of the main window layout except left toolbar and status bar. Setters: id, display name, icon (34x34 and 68x68), priority (higher = nearer top; default -1; Welcome uses 100), enabled flag (signal `enabledStateChanged`), context (added when mode is selected), widget or lazy `setWidgetCreator`, optional per-mode context-menu functor. Modes register on creation and unregister on destruction. Docs say adding a mode should be done "sparingly, only as a last reserve"; prefer navigation widget factory, output pane, perspective, dialog or specialized editor first — [IMode](https://doc.qt.io/qtcreator-extending/core-imode.html)
- Qt Creator switching: `ModeManager` inserts a new mode into the tab stack by priority, gives it a `QtCreator.Mode.` shortcut; on tab change it removes the previous mode's additional contexts, adds the new mode's, emits `currentModeChanged` (older source snapshots, API may have moved) — search result summary over [git.qt.io modemanager.cpp](https://git.qt.io/tohunger/qt-creator/-/blob/3ab0a28cca0c20baa4b21a3d08223065a1e3be94/src/plugins/coreplugin/modemanager.cpp) and [IMode](https://doc.qt.io/qtcreator-extending/core-imode.html)
- Qt Creator plugin metadata: `DisabledByDefault` (default false) means plugin is not loaded until user enables it; dependency `Type` is Required (default), Optional or Test; plugins must be built with the same compiler/Qt as the host (forum claim) — [Plugin Meta Data](https://doc.qt.io/qtcreator-extending/plugin-meta-data.html); search summary of [Qt forum](https://forum.qt.io/post/185632)
- Godot main screen plugin: a script with `_has_main_screen()` returning true, `_make_visible(bool)` (called when its button is clicked, and with false by `_enter_tree` to hide at start), `_get_plugin_name()`, `_get_plugin_icon()`, optional `_handles()` to auto-focus the screen when a node type is selected. Panel is a Control scene added to `EditorInterface.get_editor_main_screen()`. Lifecycle: enable in Project Settings > Plugins, `_enter_tree`, then show/hide per click, `_exit_tree` frees it — [Godot main screen plugins](https://docs.godotengine.org/en/stable/tutorials/plugins/editor/making_main_screen_plugins.html)
- Godot packaging: an editor plugin is a folder in `addons/` with `plugin.cfg`; installed by extracting the zip (or AssetLib tab); merged without overwriting files; enable/disable works without restarting the editor — [Installing plugins](https://docs.godotengine.org/en/stable/tutorials/plugins/editor/installing_plugins.html)
- Eclipse perspective: extension point with `id` (req), `name` (req), `class` implementing `IPerspectiveFactory` (req; defines initial layout and visible action sets), `icon`, `fixed` (views cannot be closed/moved, default false), `defaultShowIn`; default perspective set via `defaultPerspectiveId` in `plugin_customization.ini`; "Open Perspective" menu lists perspectives from active perspective and `perspectiveExtensions` — [org.eclipse.ui.perspectives](https://help.eclipse.org/latest/topic/org.eclipse.platform.doc.isv/reference/extension-points/org_eclipse_ui_perspectives.html)
- IntelliJ tool window (declarative): `com.intellij.toolWindow` EP with `id`, `icon`, `anchor` (left default/right/bottom), `secondary`, `factoryClass`, `canCloseContents`; `createToolWindowContent()` runs only when user first clicks the button (no startup cost); availability via `isApplicable(Project)` evaluated once at project load; dynamic show/hide needs `ToolWindowManager.registerToolWindow` — [Tool windows](https://plugins.jetbrains.com/docs/intellij/tool-windows.html)
- VS Code: `contributes.viewsContainers` adds a container (id, title, icon) to Activity Bar or Panel; `views` attach to a container id or built-ins (explorer, scm, debug, test), with `when` visibility; `viewsWelcome`; `walkthroughs` (Getting Started steps with `completionEvents`); `customEditors` (viewType, selector globs, priority default/option); `keybindings` and `menus` with `when` clauses; `configurationDefaults`; activation events like `onView:${viewId}`, `onCommand` — [Contribution points](https://code.visualstudio.com/api/references/contribution-points)
- VS Code profile = bundle of settings, extensions, keybindings, MCP servers, snippets, tasks and UI layout; parts may be omitted and fall back to the Default Profile — [Profiles](https://code.visualstudio.com/docs/configure/profiles)
- Unreal: `UEdMode` is the base class of all editor modes; `RegisterTool(UICommand, ToolIdentifier, ToolBuilder, Scope)`; scopes Editor / EdMode / Default; EdMode-scoped tools are available only while the mode is active and unregistered on mode exit; `FModeToolkit` supplies the UI panel for tools; modes in plugins can modify asset editors of other modules — [UEdMode::RegisterTool](https://dev.epicgames.com/documentation/unreal-engine/API/Editor/UnrealEd/UEdMode/RegisterTool), [UEdMode](https://dev.epicgames.com/documentation/unreal-engine/API/Editor/UnrealEd/UEdMode), [FModeToolkit](https://docs.unrealengine.com/4.26/API/Editor/UnrealEd/Toolkits/FModeToolkit/index.html)
- Obsidian core Workspaces plugin: a workspace stores open files/tabs plus width and visibility of each sidebar; saved by name, re-saving with same name updates it; load/delete from ribbon or "Manage workspace layouts" command — [Obsidian Workspaces](https://obsidian.md/help/plugins/workspaces). Community plugins add per-workspace theme/font modes, hierarchy and hotkey switching — [Workspace Plus Group](https://community.obsidian.md/plugins/workspace-plus-group), [Workspace++](https://community.obsidian.md/plugins/workspace-plus-plus)
- Blender API: workspace operators include add (duplicate current OR append from user configuration), `append_activate(filepath, idname)` (append from a .blend library and activate in current window; `//` relative paths in 5.x), duplicate, `reorder_to_front`/`reorder_to_back`, `delete_all_others`, `scene_pin_toggle` (4.2+) — [bpy.ops.workspace 5.0](https://docs.blender.org/api/5.0/bpy.ops.workspace.html), [4.2](https://docs.blender.org/api/4.2/bpy.ops.workspace.html)
- Zed: settings at `~/.config/zed/settings.json`, project overrides in `.zed/` limited to editor/tooling options (not theme); panel dock placement via settings (e.g. `bottom_dock_layout`) — [Configuring Zed](https://zed.dev/docs/configuring-zed). No mode/workspace-template concept found in this search.

### Inferences
- Qt Creator's IMode is the closest analog of Carbon's Section: id + name + icon + priority + enabled flag + context + lazily created widget. Lazy widget creation (`setWidgetCreator`) and priority ordering map directly to Carbon's Section Bar order.
- Godot's contract is minimal (name, icon, show/hide callbacks, optional "handles" auto-switch); a good floor for a third-party workflow API. `_handles` suggests an auto-switch-by-context hook worth borrowing.
- Eclipse `fixed` and `defaultShowIn`, and JetBrains `anchor/secondary`, show a layout-template schema can be purely declarative: panels, slots, closable/movable flags, default focus.
- Unreal's scoped registration (tools registered at EdMode scope vanish on mode exit) is the cleanest model for "commands/menus belong to a workflow and are removed on switch".

### Gaps
- Blender manual workspace page (contents: mode, tool settings, add-on filter, "sync modes") did not load; only API operator docs sourced. [BG] A Blender workspace = a screen layout (areas/editors) + object interaction mode + active tool per mode + optional add-on filter + status bar text; stored in the .blend file; defaults come from `startup.blend`, templates from `scripts/startup/bl_app_templates_*`; app templates can ship whole workspace sets. Verify before relying on it.
- Houdini desktops, Adobe workspaces, Emacs modes, Pulsar, Theia, Figma/Affinity personas, Cursor/Windsurf modes, Raycast, Logic/Pro Tools, KParts, NetBeans, Android Studio: not sourced this pass. [BG] Houdini desktops are saved pane arrangements (`.desk` files) with named pane tabs and per-network-context behaviour; Photoshop workspaces save panel arrangement, keyboard shortcuts and menus and can be reset ("Reset Essentials") or exported; Emacs major modes are exclusive per buffer (hooks, keymap, syntax) while minor modes are stackable toggles; Theia is an extensible VS Code-compatible platform using DI modules and frontend/backend contributions; Android Studio is IntelliJ-based, so same tool-window model.

## 2. How does switching work and what state is kept per workflow?

### Takeaway
Best systems persist layout per mode automatically and separate it from the mode definition, and swap context (commands/keybindings/menus) on switch. Eager state kept per mode: layout, open documents, active context; shared state: project/workspace data.

### Cited Findings
- Qt Creator swaps key-binding contexts on mode change (remove previous mode contexts, add new, emit `currentModeChanged`); mode widgets are lazily created — [IMode](https://doc.qt.io/qtcreator-extending/core-imode.html), [modemanager.cpp](https://git.qt.io/tohunger/qt-creator/-/blob/3ab0a28cca0c20baa4b21a3d08223065a1e3be94/src/plugins/coreplugin/modemanager.cpp)
- Godot hides the previous main screen via `_make_visible(false)` and shows the new one; panel instance lives from enable to disable — [Godot](https://docs.godotengine.org/en/stable/tutorials/plugins/editor/making_main_screen_plugins.html)
- Eclipse saves a perspective's layout the first time it is opened and restores that saved copy later — [Eclipse forum summary](https://www.eclipse.org/forums/index.php/t/1078421) and related threads (search result; no formal doc)
- JetBrains tool-window content is created on first activation and cached by the platform [BG for persistence; the doc page did not cover state persistence] — [Tool windows](https://plugins.jetbrains.com/docs/intellij/tool-windows.html)
- VS Code profile switching changes settings, extensions, keybindings, UI layout etc. as a unit; a profile can be bound to a folder/workspace and activates automatically — [Profiles](https://code.visualstudio.com/docs/configure/profiles)
- VS Code lacks named layouts/perspectives (issue open since 2020) — [issue #95182](https://github.com/microsoft/vscode/issues/95182), as cited in Carbon `panel-layout.md`.
- Obsidian workspace stores tabs plus sidebar widths/visibility — [Obsidian](https://obsidian.md/help/plugins/workspaces)

### Inferences
- Carbon already has per-Section `DockAreaState`. Missing pieces by comparison: per-workflow context (commands/keybindings/menus) swap, open-document state, and lazy panel creation on first activation.
- Keep workflow state keyed by stable workflow id, not enum index, so uninstall/reinstall can restore state.

### Gaps
- Exact per-mode persisted state in Qt Creator (it saves the Debug mode's dock state via `QSettings` [BG]) not sourced.

## 3. How do users create, modify, clone, share, install, uninstall, version?

### Takeaway
Cloning a built-in then editing the clone is the common pattern; sharing is a file/gist/zip; versioning is mostly by host-API compat ranges. Few systems give diff-based overrides of built-ins.

### Cited Findings
- VS Code profiles: create from template, copy an existing, or empty; export to secret gist or `.code-profile`; import from URL/file; machine-specific settings not exported; Settings Sync can carry profiles — [Profiles](https://code.visualstudio.com/docs/configure/profiles)
- Eclipse: `perspectiveExtensions` lets other plugins add views to an existing perspective declaratively (silently) — [Eclipse forum](https://www.eclipse.org/forums/index.php/t/1078421); user customisation via Save Perspective As [BG, also in Carbon panel-layout.md]
- Blender: add workspace by duplicating current or appending from a .blend; append_activate API — [bpy.ops.workspace](https://docs.blender.org/api/5.0/bpy.ops.workspace.html)
- Godot: install by extracting `addons/` folder, merge-not-overwrite; AssetLib in editor — [Installing plugins](https://docs.godotengine.org/en/stable/tutorials/plugins/editor/installing_plugins.html)
- Qt Creator: plugin JSON metadata declares dependencies (Required/Optional), `DisabledByDefault`; users toggle plugins in About Plugins — [Plugin Meta Data](https://doc.qt.io/qtcreator-extending/plugin-meta-data.html)
- IntelliJ: dynamic plugins install/update/uninstall without restart if they obey restrictions (no components, only dynamic EPs, unique action group ids, no `overrides` services); plugins needing native libs set `require-restart="true"` — [Dynamic plugins](https://plugins.jetbrains.com/docs/intellij/dynamic-plugins.html)
- Obsidian: community plugins add workspace management (rename/hierarchy) the core plugin lacks — [Obsidian community](https://community.obsidian.md/plugins/supercharged-workspaces)

### Inferences
- Override-layer design for Carbon: workflow = base manifest (read-only, from package) + user overlay (JSON patch of panel set/layout/keybinding changes) + persisted dock state. Only the overlay is user-owned, so package updates replace the base without losing edits. No product surveyed documents exactly this; VS Code settings layering (default/user/workspace) and Eclipse `perspectiveExtensions` are the nearest precedents. This is design inference.
- "Clone" = copy base+overlay into a new user-owned workflow id with `derived_from` metadata, enabling later "diff against parent".

### Gaps
- Houdini/Adobe export formats, Blender startup-file override semantics, versioning schemes of shipped perspectives: not sourced.

## 4. Failure containment (broken/incompatible workflow cannot break the app)

### Takeaway
Hard isolation (separate process / sandbox) is rare in layout-mode systems; most rely on declarative manifests, lazy loading, dependency/compat metadata, disable switches, and reset. Dynamic unloading needs strict rules.

### Cited Findings
- Qt Creator: plugin loading depends on metadata validity; malformed JSON means plugin not loaded (bug reports QTCREATORBUG-22945/23057); plugins built against another Qt/compiler fail to load; disabled plugins are not loaded — [Plugin Meta Data](https://doc.qt.io/qtcreator-extending/plugin-meta-data.html), [QTCREATORBUG-22945](https://bugreports-test.qt.io/browse/QTCREATORBUG-22945). Plugins run in-process (native code), so a crash takes the app down [BG].
- IntelliJ: if unload fails on uninstall/update the IDE asks for restart; unload leaks diagnosed via `PluginClassLoader` heap dumps; services must be Disposable — [Dynamic plugins](https://plugins.jetbrains.com/docs/intellij/dynamic-plugins.html)
- VS Code: extensions run in an extension host (local Node, web worker, or remote) kept separate from UI so extensions do not slow startup or modify UI directly; lazy activation events — [Extension host](https://code.visualstudio.com/api/advanced-topics/extension-host). The page did not state crash behaviour; [BG] host is a separate process and is restarted on crash, with "Disable all extensions" and `--disable-extensions` as escape hatches.
- Eclipse: stale saved perspective hides plugin-added views; workarounds Reset Perspective and `eclipse -clean` — [Eclipse forum threads](https://www.eclipse.org/forums/index.php/t/1078421), [CDT dev list](https://www.eclipse.org/lists/cdt-dev/msg08996.html)
- Unreal: EdMode-scoped tools unregistered on mode exit — [RegisterTool](https://dev.epicgames.com/documentation/unreal-engine/API/Editor/UnrealEd/UEdMode/RegisterTool)
- Godot: plugins enable/disable at runtime without restart; plugin runs scripts in editor process [BG] so a script error disables only that plugin's callbacks but native/engine crashes are not contained — [Installing plugins](https://docs.godotengine.org/en/stable/tutorials/plugins/editor/installing_plugins.html)
- Obsidian Restricted Mode turns off community plugins [BG; search could not confirm] — core Workspaces plugin remains core.

### Inferences
- Carbon should treat a workflow as data first: a manifest + panel references + command ids. A manifest-only workflow cannot crash the app; worst case it fails validation and is skipped. Code-bearing parts (panels) should come via the planned WASM/MCP extension system, not in-process Rust dylibs. (Rust has no stable ABI; in-process third-party Rust plugins would repeat Qt Creator's same-compiler constraint.)
- Required safety set seen across products: validate manifest at load, per-workflow try/skip, "disabled" flag persisted before activation (crash-loop guard), safe mode that boots only built-in workflows, per-workflow Reset (Eclipse), dangling-reference repair (Carbon already has `repair_layout`).
- Switching must tolerate missing panels: layout referencing an uninstalled panel should drop that slot rather than fail.

### Gaps
- VS Code crash/restart semantics, Godot script-error handling, Obsidian Restricted Mode: not confirmed from primary docs this pass.

## 5. Common abstractions and what the best systems get right

### Takeaway
Common set: stable id, display metadata (name/icon/priority), layout template, contribution points (panels, commands, menus, keybindings with context/when conditions), lifecycle hooks (enter/exit, lazy create), persisted state, enable/disable.

### Cited Findings
- id/name/icon/priority/enabled/context/lazy widget — [Qt IMode](https://doc.qt.io/qtcreator-extending/core-imode.html)
- Contribution-point model with `when` conditions across views, menus, keybindings — [VS Code contribution points](https://code.visualstudio.com/api/references/contribution-points)
- Scope-bound registration with automatic unregistration — [Unreal RegisterTool](https://dev.epicgames.com/documentation/unreal-engine/API/Editor/UnrealEd/UEdMode/RegisterTool)
- Lazy content creation on first use — [IntelliJ](https://plugins.jetbrains.com/docs/intellij/tool-windows.html), [Qt IMode](https://doc.qt.io/qtcreator-extending/core-imode.html), [VS Code activation events](https://code.visualstudio.com/api/references/contribution-points)
- Layout template with flags (fixed, anchor, closable) — [Eclipse](https://help.eclipse.org/latest/topic/org.eclipse.platform.doc.isv/reference/extension-points/org_eclipse_ui_perspectives.html), [IntelliJ](https://plugins.jetbrains.com/docs/intellij/tool-windows.html)
- Qt docs advise against adding a mode unless needed (sprawl pressure): prefer panes/perspectives — [IMode](https://doc.qt.io/qtcreator-extending/core-imode.html)

### Inferences
- Proposed Carbon workflow manifest: `id`, `name`, `icon`, `order`, `api_version` range, `default_layout` (DockAreaState-like tree of panel ids), `panels` (ids from extensions or built-in), `commands`/`menus`/`keybindings` with `when: workflow == id`, `activation` (lazy), `agent` (prompt/tools, later), `settings_schema`, `derived_from`.
- Lifecycle: `register` (validate) -> `enter` (create panels lazily, register scoped commands) -> `exit` (unregister scoped commands, keep layout) -> `unregister`.
- Built-ins should use the same manifest path as third-party ones (dogfooding), replacing the `Section` enum with a registry keyed by string id.

### Gaps
- None of the surveyed docs show a mode API that also carries an agent-behaviour contribution; Carbon's "later own agent behaviour" has no direct precedent found (Cursor/Windsurf modes not researched).

## 6. Documented problems: sprawl, state leaks, layout corruption, conflicts, upgrade breakage

### Takeaway
Dominant documented problem is stale persisted layout vs updated definitions (Eclipse). Others: restart-required plugins, leaks on unload, ABI/toolchain mismatches, mode sprawl guidance.

### Cited Findings
- Eclipse: updated plugins' new views missing because saved perspective state is used; Reset Perspective or `-clean`; no prompt when views are added via `perspectiveExtensions` — [Eclipse forum](https://www.eclipse.org/forums/index.php/t/1078421), [e4-dev](https://www.eclipse.org/lists/e4-dev/msg05907.html) (asks how to force reset from code; answer not found)
- IntelliJ: class-loader leaks on dynamic unload must be hunted by heap dump; third-party paid plugins can't be dynamic — [Dynamic plugins](https://plugins.jetbrains.com/docs/intellij/dynamic-plugins.html)
- Qt Creator: ABI/toolchain mismatch; malformed metadata silently not loaded — [Plugin Meta Data search](https://doc.qt.io/qtcreator-extending/plugin-meta-data.html), [QTCREATORBUG-22945](https://bugreports-test.qt.io/browse/QTCREATORBUG-22945)
- Cursor layout resets in 2.0 (anecdotal) and VS Code lack of named layouts — cited in Carbon `panel-layout.md`: [Cursor forum](https://forum.cursor.com/t/ui-constantly-resets-layout-in-cursor-2-0/139511?page=3)
- Dear ImGui docking flags lost / sizes differ — [#6217](https://github.com/ocornut/imgui/issues/6217), [#3834](https://github.com/ocornut/imgui/issues/3834) (from Carbon notes)

### Inferences
- Carbon mitigation: persist layout with `workflow_version`; on version change, merge: keep user positions, add new default panels (opt-in prompt), drop missing ones; offer per-workflow Reset and "Reset all". Store overlay separate from base to avoid Eclipse-style stale copies.
- Cap or group workflows in Section Bar (overflow menu, hide/show) to limit sprawl; Qt's "last reserve" guidance suggests offering lightweight alternatives (presets within a workflow) first.

### Gaps
- Actual user-reported sprawl complaints (e.g. too many Eclipse perspectives, Blender workspaces) not found in sources this pass. Blender add-on/workspace breakage on version upgrade not researched.
