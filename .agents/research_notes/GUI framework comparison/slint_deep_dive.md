# Slint deep dive for Carbon (as of 2026-10-16, Slint 1.18.x)

Method note: ~30 tool calls; WebFetch summaries come from a small model, so treat details as "per fetched page". Search engine surfaced no Reddit/HN threads, so community sentiment, real-app numbers, and production-app lists are mostly Gaps. Labels: [doc] official docs/changelog, [M] measured, [A] anecdote, [BG] unsourced background knowledge. Not repeated from own_renderer_family.md: version cadence, basic licence tiers, 1.18 headline list.

## 1. Multi-window, pop-out windows, state sharing

### Takeaway
Multiple top-level windows work (several exported `Window`/`Dialog` components since 1.7, each `show()`n independently), but Slint globals are per component instance and NOT shared between windows. Cross-window state must live in Rust (shared model / Rc state) and be pushed into each window's globals/properties.

### Cited Findings
- Several exported Window/Dialog components supported from 1.7.0 (2024-07-18); multiple simultaneous PopupWindows from 1.9.0 (2024-12-18). [doc] — [CHANGELOG](https://raw.githubusercontent.com/slint-ui/slint/master/CHANGELOG.md)
- To show several components at once call `show()` on each instance; `run()` is for a single one. Globals are instantiated per component, so each instance gets its own copy. [doc] — [slint crate docs](https://docs.rs/slint/latest/slint/)
- "Global singletons aren't shared between separate windows"; you must initialise global callbacks/properties per window instance. [doc] — [Globals guide](https://docs.slint.dev/latest/docs/slint/guide/language/coding/globals/)
- Strong component handles are not `Clone`; use `clone_strong()`/`as_weak()`; capturing a strong handle in a callback leaks (reference loop). All components must be created on the thread running the event loop. [doc] — [slint docs search result](https://docs.rs/slint)
- Window API: `set_position`, `set_size` (physical/logical), `on_close_requested` returning `CloseRequestResponse` (lets you intercept close, e.g. re-dock a pop-out). `run_event_loop_until_quit` keeps the loop alive with zero windows (tray-only apps). [doc] — [slint docs](https://docs.rs/slint/latest/slint/)
- `Window.x/y` setters were deprecated in 1.18.0. [doc] — [CHANGELOG](https://raw.githubusercontent.com/slint-ui/slint/master/CHANGELOG.md)
- `WindowMoveArea` + `no-frame: true` for custom title bars (1.18). [doc] — [1.18 blog](https://slint.dev/blog/slint-1.18-released)

### Inferences
- Pop-out agent chat panes = one `.slint` Window component per pane type, each instantiated N times; one Rust-side store (`Rc<RefCell<..>>` or `Arc` + `invoke_from_event_loop`) with `VecModel`/`ModelRc` shared among instances (models are shareable by reference; this is [BG] but consistent with Model API docs). Design tokens in a global must be re-applied per window.
- Re-docking a pop-out means destroying a window and recreating a component in the main window; no native reparenting.

### Gaps
- No official multi-window example or performance/RAM cost per extra window found. Per-window cost of renderer contexts (femtovg GL context per window?) unmeasured.

## 2. Docking / panels / tabs / split panes

### Takeaway
No built-in docking system. Upstream issue "Docking widgets" #1723 (feature, roadmap label, open since Oct 2022). Carbon would build its own dock (layout tree in Rust + `for` repeaters + draggable splitters via `TouchArea`/`DragArea`/`DropArea`) or adopt a community component set.

### Cited Findings
- GitHub issue #1723 "Docking widgets": open, Feature, roadmap label, opened 2022-10-10. [doc] — [issues search](https://github.com/slint-ui/slint/issues?q=docking%20OR%20dock%20OR%20%22split%20view%22)
- The SlintPad "online_editor" (Slint's own web IDE) has save/restore/reset dock layouts (PR #1703, merged Oct 2022) — i.e. Slint's own editor uses a dock layout, but in the TypeScript/web layer, not as a Slint widget. [doc] — same link (PR title only; implementation detail [BG])
- In-window `DragArea`/`DropArea` introduced 1.17.0 (2026-06-24); native OS drop support (Qt) in 1.18.0; opaque `data-transfer` payload converted by host-language callbacks; actions copy/move/link; Ctrl/Shift modifiers. [doc] — [Drag and Drop guide](https://docs.slint.dev/latest/docs/slint/guide/development/drag-and-drop/), [CHANGELOG](https://raw.githubusercontent.com/slint-ui/slint/master/CHANGELOG.md)
- Issue #1967 "better Drag 'n' Drop handling" closed 2026-07-19. [doc] — issues link above
- Std-widgets include TabWidget (cosmic style update in 1.6); 1.18 adds `FlexboxLayout` and run-time z-order change. [doc] — [CHANGELOG](https://raw.githubusercontent.com/slint-ui/slint/master/CHANGELOG.md), [1.18 blog](https://slint.dev/blog/slint-1.18-released)
- Community component sets: Material (in-repo `ui-libraries/material`), SurrealismUI, Vivi (desktop-app oriented), sleek-ui (Ant Design based), slintcn (shadcn-style CLI copy-in registry). None advertised as docking. [doc] — [Third party libraries](https://docs.slint.dev/latest/docs/slint/guide/development/third-party-libraries/)

### Inferences
- Custom dock is feasible but is a real chunk of work: splitters, tab strips with reorder, drag-to-dock overlays, layout serialization. Drag-to-dock needs in-window DnD (1.17+), which is brand new (4 months old as of Oct 2026) — risk.
- `Window` cannot be dragged between native windows with DragArea unless native OS drag support is mature (only Qt mentioned for 1.18) — verify on winit backend.

### Gaps
- No community docking crate for Slint found. Whether `DragArea` works across two Slint windows on winit unverified.

## 3. Theming, styles, tokens, fonts

### Takeaway
Styles: fluent (default on all platforms since 1.16), material, cupertino, cosmic, each with -light/-dark variants; `qt` needs Qt installed. Style is chosen at COMPILE time (not runtime switchable). Runtime dark/light follows OS or can be forced; custom tokens go in a global; Figma variable export generates token globals with mode switching.

### Cited Findings
- Styles `fluent`, `material`, `cupertino`, `cosmic` with `-light`/`-dark`; `qt` no variants and requires Qt; `native` maps per platform; `fluent` default. Style set at compile time via `SLINT_STYLE`, `slint_build::compile_with_config`, interpreter `set_style()`. By default styles follow system light/dark. `Palette` and `StyleMetrics` globals expose current style values; `Platform.style-name` allows branching. [doc] — [Style docs](https://docs.slint.dev/latest/docs/slint/reference/std-widgets/style/)
- Fluent made the default on all platforms in 1.16.0 (2026-04-16); a Slint blog post "Deprecating Native-Looking Styles" exists (March 2026; page fetch 404, title only). [doc] — [CHANGELOG](https://raw.githubusercontent.com/slint-ui/slint/master/CHANGELOG.md), [blog index](https://slint.dev/blog)
- `color-scheme` property introduced 1.6.0 (2024-05-13). [doc] — CHANGELOG link above
- Globals as design tokens: `global Palette { in-out property<color> primary; }` then bind `background: Palette.primary`; export via `export global` and set from Rust via `app.global::<T>()`. [doc] — [Globals guide](https://docs.slint.dev/latest/docs/slint/guide/language/coding/globals/)
- Figma variable export plugin: Color->brush, Float->length, String->string, Boolean->bool; multi-mode collections (light/dark) produce a mode enum, `Scheme` struct, and a global with `current-mode` (in-out) and `current` (out); names like `colors/background/primary` become nested structs. Mode switching is manual. [doc] — [Figma guide](https://docs.slint.dev/latest/docs/slint/guide/tooling/figma-inspector/)
- Fonts: system fonts picked up automatically; bundle `.ttf/.ttc/.otf` via `import "./file.ttf";`; on embedded/software renderer must embed; `with_sdf_fonts()` shrinks binaries; font fallback, OpenType features, colour emoji not covered by page. [doc] — [Fonts guide](https://docs.slint.dev/latest/docs/slint/guide/development/fonts/)
- Fontique (font backend) feature moved to 010 in 1.17 and 011 in 1.18 (breaking for those who pin it). [doc] — CHANGELOG link

### Inferences
- Runtime theme switching of Carbon's own tokens is straightforward (own global with a `dark` bool/struct, set from Rust, all bindings update reactively). Switching the built-in std-widgets style at runtime is not supported; use own widgets, or `-light/-dark` follow-system, or ship custom components built from tokens.
- User-added themes (JSON -> global struct) are viable without recompiling since tokens are plain properties set from Rust.

### Gaps
- Whether std-widgets can be re-skinned via Palette overrides at runtime (setting `Palette.color-scheme` from code) — `Palette` has `color-scheme` per [BG]; not verified here.

## 4. slint-interpreter, runtime-loaded UI, plugin patterns

### Takeaway
`slint-interpreter` (1.18.1) compiles `.slint` source/files at runtime into dynamic `ComponentInstance`s with `Value`-typed property/callback access. That enables declarative extension UI loaded from disk, with a dynamic (stringly-typed) bridge. No official plugin system (WASM/dylib) exists.

### Cited Findings
- `Compiler` (replaces deprecated `ComponentCompiler`, deprecated 1.7) offers `build_from_source` / `build_from_path` -> `CompilationResult` -> `component("Name")` -> `ComponentDefinition::create()` -> `ComponentInstance` (implements `ComponentHandle`); `set_property`/`get_property`, `set_callback`, `invoke`, globals via `Global` trait; `Value` dynamic type (Struct, SharedString, Image, Color...). Async compilation with optional `set_file_loader`. Needs the same backend/renderer features; `mcp` debug feature not for production. [doc] — [slint-interpreter docs](https://docs.rs/slint-interpreter/latest/slint_interpreter/)
- Interpreter exports `spawn_local` too. [doc] — [spawn_local search result](https://docs.slint.dev/latest/docs/rust/slint_interpreter/fn.spawn_local)
- Live preview in Rust uses the interpreter: `SLINT_LIVE_PREVIEW=1 cargo run --features slint/live-preview`; app keeps running, properties/callbacks/models preserved, previous good version stays on syntax error; renaming API used by Rust may terminate app; dev only. [doc] — [Live preview](https://docs.slint.dev/latest/docs/slint/guide/tooling/live-preview/)
- Interpreter compile-time style via `set_style()` (style still fixed per compile). [doc] — Style docs link

### Inferences
- Extension viewers (SVG/3D) as `.slint` files: the extension's UI is declarative, Rust host exposes a fixed API via globals/callbacks. Extensions can import host-provided design-token globals only if loaded in the same compiler session with the host's `.slint` library (needs the compiler to resolve imports from a shipped library path; plausible, unverified).
- Cost: interpreter pulls the whole compiler into the binary (larger size; [BG]); and each load incurs compile time. Untrusted extensions would run only declarative expressions (Slint cannot call arbitrary code itself) — a sandboxing advantage, but they cannot have logic beyond expressions/callbacks, so logic needs WASM or out-of-process.
- Rust-side plugin patterns (WASM via wasmtime/extism, dyn libs via abi_stable, rhai/mlua scripting, JSON-RPC subprocess) are generic [BG]; no Slint-specific official pattern found.

### Gaps
- Interpreter load time for large files, binary size delta, and dynamic model/`ModelRc` ergonomics unmeasured. No real-world "Slint + WASM plugin" project found.

## 5. LSP, live preview, VS Code, SlintPad, hot reload, Figma

### Takeaway
Tooling is strong for a UI toolkit: LSP + VS Code extension + live preview + standalone viewer + SlintPad web editor + Figma inspector/variable export, with editor support for many IDEs.

### Cited Findings
- Docs list tooling: VS Code, manual LSP setup, Kate, Qt Creator, Helix, (Neo)Vim, Sublime, JetBrains, Zed; Live Preview for Rust/C++; Slint Viewer; Figma Variable Export; AI coding assistants page. [doc] — [docs home](https://docs.slint.dev/latest/docs/slint/)
- 1.18 LSP: rename of property/callback also renames Rust/C++ accessors; import actions and completion for structs/enums; `Platform.uses-mock-data` true in live preview/viewer. [doc] — [1.18 blog](https://slint.dev/blog/slint-1.18-released)
- Tree-sitter grammar maintained in the repo (`editors/tree-sitter-slint`), used by nvim-treesitter and Helix. [doc] — [tree-sitter-slint](https://de.github.com/slint-ui/tree-sitter-slint)
- Hot reload details: see section 4. Figma plugin: see section 3.
- SlintPad (web) exists [BG]; not re-fetched.

### Gaps
- VS Code extension feature details (property editor/designer) not fetched; no known state of the "Slint designer drag-drop" status.

## 6. Native menus, tray, file dialogs, clipboard, drag and drop

### Takeaway
Native `MenuBar` and `ContextMenu` exist since 1.10 (2025-02-28); `SystemTrayIcon` since 1.17 (2026-06). File dialogs are not part of Slint (use `rfd` from Rust). Clipboard via TextInput/TextEdit `copy/cut/paste` plus Rust-side crates. DnD is new (1.17/1.18).

### Cited Findings
- MenuBar/ContextMenu added 1.10.0 (2025-02-28); SystemTrayIcon 1.17.0 (2026-06-24). [doc] — [CHANGELOG](https://raw.githubusercontent.com/slint-ui/slint/master/CHANGELOG.md)
- MenuBar is declared inside `Window`; native or Slint-rendered depending on platform. [doc] — [MenuBar search result](https://docs.slint.dev/latest/docs/slint/reference/window/menubar) (page itself 404'd on fetch; per search summary only)
- SystemTrayIcon: props `icon`, `tooltip`, `visible`, `title`; `clicked()` semantics differ (macOS only when no menu attached); needs exactly one `Menu` child; shortcut bindings ignored; Linux uses StatusNotifierItem/AppIndicator over D-Bus ("plain X11 system trays are not supported"; GNOME needs an extension); macOS NSStatusItem; Windows Shell_NotifyIcon; tray-only app possible; globals not shared with windows. [doc] — [SystemTrayIcon](https://docs.slint.dev/latest/docs/slint/reference/window/systemtrayicon/)
- TextEdit exposes `copy()`, `cut()`, `paste()`, `select-all()`. [doc] — [TextEdit](https://docs.slint.dev/latest/docs/slint/reference/std-widgets/views/textedit)
- Drag and drop: see section 2; OS-level drops depend on platform support, per-backend limits undocumented. [doc] — [DnD guide](https://docs.slint.dev/latest/docs/slint/guide/development/drag-and-drop/)

### Inferences
- Whether the native menu is implemented via muda internally was not confirmed [BG: I believe it is on winit backend]; verify before relying on it. `rfd` is independent and usable from Rust callbacks (use async + `spawn_local`).

### Gaps
- muda use, macOS global menu bar behaviour, Windows/Linux menu rendering (native vs drawn) per platform, clipboard API for non-text data, file drop from OS on winit/Windows: not confirmed.

## 7. Text editing, code editor feasibility, ListView

### Takeaway
Built-in `TextEdit` is a plain-text multi-line widget with no syntax highlighting, no documented undo/redo or IME-specific API; 1.18 made large text much faster. A code editor needs a custom approach (own Rust text model + virtualized line rendering, or embed another editor); no Slint-based code editor project found.

### Cited Findings
- TextEdit props: text, font-*, wrap (`no-wrap`, `word-wrap`, `char-wrap` — char-wrap only Qt & software renderers), read-only, `input-method-hints`, `content-*` scroll props (renamed from viewport-* in 1.18), `placeholder-text`; callbacks `edited`, `key-pressed`, `key-released` (can intercept keys, return accept/reject); functions `set-selection-offsets(anchor, focus)` (UTF-8 offsets), `select-all`, `clear-selection`, `copy/cut/paste`, focus control. Plain `string` text; no rich text, no per-range styling, no line numbers, no documented undo API. [doc] — [TextEdit](https://docs.slint.dev/latest/docs/slint/reference/std-widgets/views/textedit)
- 1.18: rendering/editing large text much faster; screen readers read text input content/selection. [doc] — [1.18 blog](https://slint.dev/blog/slint-1.18-released)
- `ListView` `scrolled` callback added 1.8.0 (2024-09-23). [doc] — CHANGELOG link. ListView virtualizes rows (only visible instantiated) [BG].
- Slint's own editors use external grammars (tree-sitter-slint) — not a Slint widget. — see section 5.
- No Slint-based code-editor project or syntax-highlighting widget found in searches.

### Inferences
- Viable code-editor design: custom component drawing visible lines as `Text` elements inside a `Flickable`/ListView with Rust-side rope + tree-sitter producing per-line styled spans (needs a Text element supporting colored runs: not found in docs; likely one `Text` per span or `StyledText`/markdown element — unverified). Cursor/selection/IME handled by a hidden `TextInput`/`FocusScope` — substantial effort. Realistic alternative per PROJECT.md: webview Monaco/CodeMirror initially.

### Gaps
- IME composition details (CJK) in Slint desktop; undo/redo behaviour of TextEdit; styled-run text API; large-file benchmarks.

## 8. Custom drawing, wgpu, 3D, SVG

### Takeaway
Slint can import externally rendered `wgpu::Texture` into an `Image` (unstable feature flag tied to wgpu major version) and supports rendering notifiers for underlay/overlay — good for a 3D viewer. Diagram canvas can use `Path` or a custom Rust-rendered image.

### Cited Findings
- `slint::Image::try_from(wgpu::Texture)` with feature `unstable-wgpu-NN`; device/queue via `window.set_rendering_notifier`; official `examples/wgpu_texture` renders an effect during BeforeRendering and imports it; call `request_redraw()` for animation. [doc] — [wgpu_24 docs / example search](https://docs.slint.dev/latest/docs/rust/slint/wgpu_24), [wgpu_29](https://docs.slint.dev/latest/docs/rust/slint/wgpu_29/)
- wgpu feature churn: `renderer-femtovg-wgpu` in 1.12.0; wgpu 27/28 replaced by `unstable-wgpu-29` in 1.17.0; `unstable-wgpu-28` removed in 1.18.0; docs.rs lists 29 and 30. [doc] — [CHANGELOG](https://raw.githubusercontent.com/slint-ui/slint/master/CHANGELOG.md), [slint docs](https://docs.rs/slint/latest/slint/)
- Winit-backend renderers: FemtoVG (OpenGL), FemtoVG-wgpu (Metal/D3D/Vulkan), Skia (GL/Metal/D3D/software), Skia Software, Software. [doc] — [winit backend](https://docs.slint.dev/latest/docs/slint/guide/backends-and-renderers/backend_winit/)
- Renderer tradeoffs: Software: lightweight, no rotation/scaling/drop-shadow/text outline, western scripts only; FemtoVG: text/path quality "can be suboptimal"; Vello: experimental, needs compute-capable GPU, no software fallback, never selected automatically; Skia: strong GPU acceleration, "large disk footprint"; Qt: QPainter software, Qt backend only. [doc] — [Backends and renderers](https://docs.slint.dev/latest/docs/slint/guide/backends-and-renderers/backends_and_renderers/)
- `raw-window-handle` (06) and `unstable-winit-030` modules exist, enabling native-child/custom-surface experiments. [doc] — [slint docs](https://docs.rs/slint/latest/slint/)
- 1.18 deprecated `SkiaWGPURenderer`; breaking wgpu feature rename every ~2 minors. [doc] — CHANGELOG

### Inferences
- A 3D extension viewer renders offscreen with the same wgpu version as Slint (version coupling: Carbon's wgpu must match Slint's pinned `unstable-wgpu-NN`) and shows the result as an Image per frame. Needs the femtovg-wgpu or Skia/Vello-wgpu renderer; plain femtovg-OpenGL path is a different interop story.

### Gaps
- SVG: Slint supports `.svg` in `Image` (via resvg, [BG]) but nothing fetched on SVG limits, dynamic SVG string loading (`Image::load_from_svg_data` [BG]), or filter/text support. Path element docs not fetched.

## 9. Bundle size, RAM, startup, renderers

### Takeaway
No reliable measured hello-world numbers found. Only an old embedded runtime claim; Skia has a large footprint per docs.

### Cited Findings
- Runtime "less than 300KiB RAM" claim is for embedded (old README 1.3.1). [doc] — [README 1.3.1](https://cdn.jsdelivr.net/gh/slint-ui/slint@1.3.1/README.md)
- Docs state Skia "large disk footprint", Software lightweight. [doc] — Backends link above
- 1.18 blog: leaner generated code, faster compile, smaller binaries (no numbers); `with_sdf_fonts()` reduces bundled-font size. [doc] — [1.18 blog](https://slint.dev/blog/slint-1.18-released), [Fonts](https://docs.slint.dev/latest/docs/slint/guide/development/fonts/)
- Anecdotes from earlier notes (launch <50 ms, ~45 MB hello world, egui->Slint 130->30 MB) remain unverified — see [own_renderer_family.md](own_renderer_family.md).

### Gaps
- No measured binary size / idle RAM / startup for hello world or real apps with femtovg vs skia vs software vs vello; Carbon needs its own benchmark.

## 10. Accessibility

### Takeaway
Slint has a role/label/action API, AccessKit-based desktop backend (AccessKit 0.16 upgrade noted in 1.7), and 1.18 exposes text input content/selection to screen readers; official docs are modest ("more work needed").

### Cited Findings
- AccessKit 0.16 upgrade listed in 1.7.0 (2024-07-18). [doc] — [CHANGELOG](https://raw.githubusercontent.com/slint-ui/slint/master/CHANGELOG.md)
- `accessible-role` enum has ~29 values incl. Button, Checkbox, Slider, Table, Switch and landmark roles (Banner, Main, Navigation). [doc] — [AccessibleRole](https://docs.slint.dev/latest/docs/rust/slint/language/enum.AccessibleRole)
- Custom components should set `accessible-role`, `accessible-label`, `accessible-action-default`; testing tools suggested: Accessibility Insights (Windows), Accessibility Inspector (macOS, app must be a bundle). [doc] — [Best practices](https://docs.slint.dev/latest/docs/slint/guide/development/best-practices/)
- Docs say basic infrastructure for assistive tech is in place; more work needed for best-in-class (via search summary). — [search result](https://github.com/slint-ui/slint/issues/32)
- 1.18: screen readers can see text-input content and selection. [doc] — [1.18 blog](https://slint.dev/blog/slint-1.18-released)

### Inferences
- Focus-order, custom canvas (diagram) accessibility requires hand-annotating every custom element with roles/labels; the code editor widget accessibility would hinge on the TextInput path only.

### Gaps
- Screen-reader test results (NVDA/Narrator/VoiceOver/Orca) not found; Linux AT-SPI status unconfirmed; tracking issue #32 only partially seen.

## 11. Licensing

### Takeaway
Royalty-free licence requires either AboutSlint widget in an About screen reachable from the top-level menu, or the Slint attribution badge on a public web page (near download). No embedded; may not expose Slint APIs to others. GPLv3 option lets you keep your own files MIT/Apache but the combined work is GPLv3.

### Cited Findings
- Section 2 Attribution, exact: "Display the AboutSlint widget in an "About" screen or dialog that is accessible from the top level menu of the Application" (or Splash Screen if no About screen); OR "Display the Slint attribution badge on a public webpage, preferably where the binaries of your Application" can be downloaded; must be easy to find. [doc] — [Slint T&Cs](https://slint.dev/terms-and-conditions)
- Section 3: cannot distribute the Software alone (that is GPLv3 territory); "does not permit the use of the Software within Embedded Systems"; "does not permit the distribution of Application that exposes the APIs, in part or in total, of the Software"; may not remove licence notices. No termination section in this licence. [doc] — same link
- Scope: proprietary desktop/mobile/web apps at no cost; "Without that disclosure, use the Commercial license". [doc] — [LICENSE.md](https://github.com/slint-ui/slint/blob/master/LICENSE.md)
- GPLv3: open source with GPL-compatible terms free; you may keep your own files under MIT/Apache-2.0. [doc] — LICENSE.md link

### Inferences
- "Exposes the APIs of the Software" clause matters for Carbon: an extension SDK that lets third-party extensions author `.slint` (interpreter) UIs arguably exposes Slint's language/API to users — likely needs legal clarification or commercial licence if Carbon is proprietary. Under GPLv3, Carbon would be GPLv3 as a whole (extensions loaded into the process likely become GPL-covered [BG, not legal advice]).

### Gaps
- Whether the exposing-APIs clause covers interpreter-loaded extension UIs: no source; ask Slint.

## 12. Maturity, breaking changes, production apps, async, Wayland/X11

### Takeaway
1.x promise is general ("maintaining backwards compatibility"), but unstable/feature-flagged parts (wgpu, winit-030, fontique, vello) churn every release; desktop is officially a target and Fluent became default in 1.16.

### Cited Findings
- 1.0 post: "We'll keep adding new features in future 1.x releases and also commit to maintaining backwards compatibility"; no formal semver/exclusion policy on that page. [doc] — [Announcing 1.0](https://slint.dev/blog/announcing-slint-1.0)
- Breaking/deprecation timeline: 1.7 ComponentCompiler deprecated; 1.9 `close-on-click`->`close-policy`; 1.11 global callback aliases; 1.15 Android safe-area; 1.16.1 `KeyEvent`/`KeyboardModifiers` non_exhaustive; 1.17 wgpu 27/28 -> 29, Python int type change, `init()` deprecated; 1.18 `unstable-wgpu-28` removed, fontique 011, iOS UIScene plist changes, `Window` x/y setters and `viewport-*` deprecated. [doc] — [CHANGELOG](https://raw.githubusercontent.com/slint-ui/slint/master/CHANGELOG.md) (only 1.6+ read; read truncated at 100k of 161k chars)
- Async: `spawn_local` runs futures on the Slint loop; `invoke_from_event_loop` for worker-to-UI; Tokio futures need a runtime context: Tokio's current-thread scheduler cannot run on the main thread; docs recommend `async_compat::Compat` over a shared multi-thread Tokio runtime. [doc] — [spawn_local](https://docs.slint.dev/latest/docs/rust/slint/fn.spawn_local), [invoke_from_event_loop](https://docs.slint.dev/latest/docs/rust/slint/fn.invoke_from_event_loop)
- Winit backend supports macOS, Windows, Linux Wayland and X11; features `backend-winit-x11` / `backend-winit-wayland`; needs X11 libs listed. [doc] — [Winit backend](https://docs.slint.dev/latest/docs/slint/guide/backends-and-renderers/backend_winit/)
- Default backend order: qt -> winit -> linuxkms; with winit default renderer order Skia (if compiled) -> FemtoVG -> software. [doc] — [Backends](https://docs.slint.dev/latest/docs/slint/guide/backends-and-renderers/backends_and_renderers/), winit link
- Older README mirrors list embedded "Ready" and desktop "In Progress" (2024 era) — dated. [A/old] — [README 1.3.1](https://cdn.jsdelivr.net/gh/slint-ui/slint@1.3.1/README.md)
- Showcase examples in old README: Chiptrack (sequencer). [old] — same link

### Gaps
- No notable production desktop app list, Reddit/HN complaints, Wayland client-side-decoration/IME issues, or 1.0-1.5 changelog read. Known community themes [BG, unverified]: no docking, limited rich text, DSL learning curve, single-thread UI model, tooling versus ecosystem size.
