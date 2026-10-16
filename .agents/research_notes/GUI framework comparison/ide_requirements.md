# IDE-like app cross-cutting requirements vs Rust GUI approaches (Carbon)

Method note: ~25 web searches/fetches; search tool returned many mirrors/old pages. Items marked **(bg)** are background knowledge NOT verified in this session (no URL) and must be re-checked before scoring. Evidence labels: M measured, V vendor claim, A anecdotal, doc docs/spec. Carbon context read from PROJECT.md / FEATURE.md Part 1: pillars = code editor, paradigm (diagram) editor, agents tab with multi-pane chats, Win/mac/Linux, lightweight, modular post-build extension sections (SVG/3D viewers), AccessKit "from first widget", terminal, WASM (WIT) extensions + MCP Apps iframes; current plan = Rust daemon + Tauri webview (Monaco, xterm.js), native GPU UI only for hot surfaces, benchmark pending ("published numbers conflict 2x-15x"). Approaches abbreviated: WV = webview (Tauri/wry, Dioxus desktop), GPUI, egui, Slint, Iced, Floem, Xilem, DxN = Dioxus Native (Blitz+Vello).

## 1. Code-editor building blocks usable from Rust

### Takeaway
Rope/tree-sitter/text-shaping are framework-agnostic crates, so every native approach can assemble an editor, but only GPUI (via gpui-component, and Zed's own non-reusable crates) and Floem (Lapce-core) ship an editor widget; egui/Slint/Iced/Xilem need a build-your-own. A webview gets Monaco/CodeMirror 6 for free (IME, a11y, folding, minimap, diff) at the cost of the JS bridge.

### Cited Findings
- Zed's buffer is a SumTree (thread-safe copy-on-write B+ tree), used beyond the rope; documented in Zed's "Rope & SumTree" post (Apr 2024, authors Ball/Sobo/Scandurra/Brunsfeld). [Plushcap summary](https://www.plushcap.com/content/zed/blog/rope-sumtree) (secondary summary of the Zed post; original on zed.dev/blog).
- gpui-component (longbridge) advertises: dock layout (resizable panels, draggable tabs, nested splits, edge docks, serializable), code editor "stable performance at 200K lines" with tree-sitter highlighting and LSP diagnostics/completion/hover, native Markdown/HTML rendering, AccessKit roles/names/states "built into the interaction layer and covered by tests", macOS/Windows/Linux + wasm, v0.7, 17.7k stars, used to build Longbridge Pro (V, README claims). [GitHub](https://github.com/longbridge/gpui-component)
- gpui-component tree-sitter is one all-or-nothing feature flag; enabling all grammars added ~35 MiB to an app's bundle; open issue asks for per-language flags (A, single user report). [issue #2295](https://upd.dev/longbridge/gpui-component/issues/2295)
- Lapce: pure Rust, UI in Floem, uses Xi-editor rope science, wgpu rendering (V, repo README). [mirror](https://gitee.com/bemy/lapce) No source found for Lapce IME/a11y status.
- Zed vs VS Code latency/RAM numbers: Zed's own compare page claims <10 ms typing latency, ~600 MB RAM vs VS Code 3-4 GB+ with many extensions (V). [zed.dev](https://zed.dev/compare/vscode). A third-party 2026 blog gives much closer numbers: keystroke ~18 ms VS Code vs ~8 ms Zed, idle RAM ~730 MB vs ~142 MB (A, method unknown). [tech-insider](https://tech-insider.org/?p=19695). Search found no rigorous independent benchmark and no Lapce numbers.
- Lapce design claim: less memory than Zed, CPU rendering for old machines (design claim, not measured). [devday.kr](https://devday.kr/article/lapce-rust-code-editor-vs-vscode-zed)
- **(bg, unverified)**: ropey (stable, widely used: Helix, Lapce-derived), xi-rope, crop; tree-sitter + tree-sitter-highlight (queries per language, incremental); syntect/TextMate grammars as alternative (VS Code theme/grammar import needs TextMate: `syntect` or `tm-grammar` crates); text shaping: cosmic-text (used by Iced, Bevy, COSMIC, gpui-component?), parley+swash/fontique (Linebender, used by Xilem/Vello/Blitz), GPUI has its own platform text system (CoreText/DirectWrite/cosmic-text on Linux); cosmic-edit is a COSMIC app (iced-based) not a widget library; helix-core/helix-view are GPL-incompatible concerns (MPL-2.0; check); Zed `editor`/`text`/`rope` crates are GPL-3/Apache mixed and tied to GPUI (not reusable standalone; check licence). Monaco/CodeMirror 6 are MIT/BSD-ish and provide IME, bidi, a11y (screen-reader mode), minimap (Monaco), merge/diff view (Monaco diff editor; CM6 @codemirror/merge).
- **(bg)** Large files: Monaco/VS Code struggle past ~tens of MB (piece-tree buffer, 50 MB warning); rope + lazy line index + viewport-only layout is the native approach; for CM6 viewport virtualisation is built in.

### Inferences
- Only WV and GPUI(-component) give an editor with LSP-ready features today; for egui/Slint/Iced/Xilem the editor is the single biggest build-from-scratch item (selection, IME composition, multi-cursor, folding, wrap, bidi, a11y text model).
- Because Carbon plans "rope + anchors, agents as attributed edit peers" (ED-11), a Rust-owned buffer in the daemon with a thin renderer is required either way; Monaco would then be a view over daemon state (sync cost: every keystroke crosses the IPC boundary), while a native view reads the same rope in-process.
- Bundle-size note from gpui-component (35 MiB for all grammars) is relevant to the "lightweight" pillar; grammars should be loadable at runtime/WASM (Zed does this per extension).

### Gaps
- No measured IME quality comparison across GPUI/egui/Slint/Iced editors found (only general knowledge that egui/Iced IME has historically been weak on Linux/Windows composition; unverified).
- No independent Monaco-vs-native-editor latency benchmark found; Carbon's own benchmark remains necessary.
- Licences of Zed crates, helix-core, lapce-core not verified.

## 2. Terminal embedding

### Takeaway
alacritty_terminal (VT state + grid) + portable-pty (ConPTY on Windows) is the native standard (Zed uses alacritty-based terminal); a webview uses xterm.js (what VS Code uses). Ready-made widgets exist for egui (egui_term, immature) and GPUI (gpui-component docs do not mention a terminal); libghostty-vt is promising but young.

### Cited Findings
- egui_term: terminal widget on egui + alacritty backend; v0.1.0 depends on alacritty_terminal ^0.25 and egui ^0.31; "under development, does not provide full terminal features"; tested macOS/Linux, not Windows. [docs.rs egui_term](https://docs.rs/crate/egui_term/latest)
- gpui-component README capture: terminal not mentioned (could not confirm one exists). [GitHub](https://github.com/longbridge/gpui-component)
- libghostty-vt: zero-dependency lib for parsing terminal sequences and maintaining state; Rust crate `libghostty-vt` with `Terminal` and `RenderState` types, not thread-safe, custom allocators. [docs.rs](https://docs.rs/libghostty-vt). Hashimoto (Sep 2025) said Zig/C APIs were "early testing quality, not ready for general usage" (may be out of date). [mitchellh.com](https://mitchellh.com/writing/libghostty-is-coming). Community list shows Rust port of Ghostling using macroquad; cmux uses libghostty for panes. [awesome-libghostty](https://github.com/Uzaaft/awesome-libghostty)
- Third-party guide says Zed's terminal is built on Alacritty's approach (secondary, does not name the crate). [deployhq](https://www.deployhq.com/guides/zed)
- Carbon FEATURE.md TM-02 already notes: "`alacritty_terminal` + own renderer; libghostty-vt alpha"; shell integration OSC 633/133 needed.
- **(bg, unverified)**: wezterm-term is not published as a stable independent crate (used via git); portable-pty is on crates.io (wezterm); xterm.js has WebGL/canvas renderer addon, a11y screen-reader mode, ligatures, and is used by VS Code; Tauri terminals commonly pair xterm.js with portable-pty; Zed's terminal crate lives in the Zed repo (GPUI-bound); Dioxus/Iced/Slint have no mature terminal widget (iced_term exists, immature).

### Inferences
- Common backend for all approaches: PTY + VT state in the Rust daemon (matches Carbon's daemon design); only the renderer differs. WV renderer = xterm.js (ready); native renderer = draw cell grid yourself (glyph atlas, selection, scrollback, IME, links) in GPUI/egui/Iced/Xilem.
- Terminal a11y natively is a build-from-scratch item (screen-reader text model); xterm.js ships a screen-reader mode.

### Gaps
- No verified performance numbers (throughput, latency) for xterm.js vs alacritty-based renderers in this session.
- Whether gpui-component has a terminal widget unconfirmed; wezterm-term crate status unverified.

## 3. Accessibility

### Takeaway
AccessKit lists Bevy, egui, Freya, Slint, Xilem as integrators; gpui-component claims AccessKit in its interaction layer; Dioxus Native has an `accessibility` feature. Iced and core GPUI/Zed have no confirmed status. Webviews inherit platform accessibility from the browser engine, which is the least-risk path for a text-heavy IDE, but text-editing a11y in AccessKit-native stacks remains the weak spot.

### Cited Findings
- AccessKit site lists Rust GUI projects that already integrate it: Bevy, egui, Freya, Slint, Xilem. [accesskit.dev](https://accesskit.dev/) (no maturity detail)
- Slint's first accessibility (v0.2.5, 2022) used Qt as backend, text inputs not yet accessible; plan to add non-Qt backends. [Slint 0.2.5](https://slint.dev/blog/slint-0.2.5-released). Later AccessKit integration not verified in this session (accesskit.dev list says Slint integrates).
- Old Iced issue #552 discusses AccessKit as a route; no evidence of shipped support; thread noted AccessKit text-editing was then unsupported (2022 era). [iced #552](https://git.tdem.in/iced-rs/iced/issues/552)
- Dioxus Native: `accessibility` feature enables AccessKit; also `accesskit_xplat` adapter (no winit dependency) with extra boilerplate and implementation risk per AccessKit maintainers. Blitz described as pre-alpha in a May 2026 listing (another mirror says beta; conflicting). [docs.rs dioxus-native](https://docs.rs/dioxus-native/0.8.0-alpha.1/dioxus_native/), [lib.rs blitz-html](https://lib.rs/crates/blitz-html)
- gpui-component: AccessKit roles/names/states/relationships/actions "covered by tests" (V). [GitHub](https://github.com/longbridge/gpui-component)
- A March 2024 W3C RQTF thread raised concern that GPU-rendered UIs like Zed bypass the accessibility APIs (concern, not a bug report). [W3C list](https://lists.w3.org/Archives/Public/public-rqtf/2024Mar/0022.html). I found no Zed screen-reader issue/status.
- Tauri docs record WebKitGTK/driver issues on Linux (graphics, not a11y). [Tauri Linux graphics](https://v2.tauri.app/develop/debug/linux-graphics/). No source found on AT-SPI behaviour of WebKitGTK/child webviews.
- **(bg, unverified)**: VS Code/Monaco has a dedicated screen-reader mode and extensive ARIA, tested with NVDA/JAWS/VoiceOver/Orca; webviews (WebView2, WKWebView) expose UIA/NSAccessibility natively; AccessKit adapters: Windows UIA, macOS NSAccessibility, Linux AT-SPI (zbus), plus Android; AccessKit text-edit support has improved since 2022 but IDE-grade (multi-cursor, code folding semantics) is unproven.

### Inferences
- For "accessible" as a stated requirement, WV is lowest risk; among native, egui/Slint/Xilem/gpui-component have plausible AccessKit paths but no one has shown an accessible code editor with screen-reader parity in these sources.
- Screen-reader testing (NVDA/JAWS on Windows, VoiceOver macOS, Orca Linux) is manual everywhere; no automated screen-reader harness found. AccessKit exposes a tree that can be asserted in tests (egui/gpui-component claim tests) but not found documented here.

### Gaps
- 2026 status of AccessKit text editing, Slint's current backend, Iced/GPUI/Floem a11y: not confirmed (search results old/empty).
- WebKitGTK + Orca behaviour not found.

## 4. Diagram / canvas editing (paradigm editor)

### Takeaway
Mature diagram editors (React Flow/xyflow, tldraw, Excalidraw, draw.io) are JS and run only in a webview (or inside Dioxus-web/Tauri); native Rust has node-graph crates for egui (egui-snarl; egui_node_graph) and low-level canvases (Vello, lyon, femtovg), but nothing near tldraw/Excalidraw feature depth. Excalidraw's JSON is plain and easy to emit/parse from Rust.

### Cited Findings
- Excalidraw format: plaintext JSON, top-level `type`, `version`, `source`, `elements`, `appState`, `files`; clipboard variant has a different `type`. [Excalidraw docs](https://docs.excalidraw.com/docs/codebase/json-schema)
- tldraw SDK: free use requires keeping "Made with tldraw" watermark visible; removal needs a paid business license; license key verified client-side; docs on commercial-use wording differ between pages (check current terms). [tldraw license](https://tldraw.dev/legal/tldraw-license), [license key](https://tldraw.dev/sdk-features/license-key)
- egui-snarl: node-graph container for egui, "early development", generic over node data, serde support, viewer trait for node/pin UI. [docs.rs](https://docs.rs/egui-snarl/)
- egui_node_graph: no result returned in this session (**bg**: archived/unmaintained, forks exist such as egui_node_graph2).
- Dioxus Native pairs Blitz (layout) with Vello (GPU vector) on wgpu. [docs.rs dioxus-native](https://docs.rs/dioxus-native/0.8.0-alpha.1/dioxus_native/)
- **(bg, unverified)**: React Flow (xyflow) is MIT; Excalidraw MIT; draw.io Apache-2.0 (iframe embed); Mermaid MIT; Vello (Linebender) GPU 2D renderer, used by Xilem/Blitz; lyon tessellation (used by egui-like/iced canvas), femtovg (OpenGL); Iced has a `canvas` widget; GPUI has custom paint elements/paths; Slint has Path elements and a Skia/femtovg renderer; Floem supports canvas-style painting. Hit-testing, text editing in nodes, edge routing, zoom/pan, undo, selection are all hand-built in native.

### Inferences
- Paradigm editor rows in FEATURE.md are all unresearched (?); a webview diagram surface (React Flow for graph-style, Excalidraw/tldraw for freeform) is the fastest, with JSON as the AI-context format. Native option is viable only for a constrained node-graph (egui-snarl-class), not freeform sketching.
- A hybrid is feasible in any native framework only if a webview can be embedded as a pane (see item 5/7).

### Gaps
- No measured performance (nodes count at 60 fps) for any native vs JS diagram solution.
- xyflow/Excalidraw licences, egui_node_graph status and Vello-based diagram projects not verified with URLs.
- No existing Rust crate found that parses `.excalidraw` with full fidelity (not searched deeply).

## 5. 3D / SVG / media viewers

### Takeaway
resvg/usvg (SVG) and wgpu (3D) are native crates available to every Rust approach; hosting a live wgpu surface inside a GUI differs per framework (egui and Iced have first-class wgpu callbacks; Slint/GPUI/Floem/Xilem need more work). three.js/model-viewer and `<img>/<svg>` in a webview are the zero-effort path.

### Cited Findings
- Dioxus Native renders to GPU via Vello/wgpu with winit; desktop renderer uses a webview. [docs.rs dioxus-native](https://docs.rs/dioxus-native/0.8.0-alpha.1/dioxus_native/)
- bevy_egui and bevy_iced exist as third-party bridges (embedding UI in Bevy rather than Bevy in UI). [bevy_egui](https://docs.rs/crate/bevy_egui/0.40.1/source/README.md), [bevy_iced](https://github.com/tasgon/bevy_iced)
- Tauri child webviews (multiwebview) are behind an unstable flag and built on wry `build_as_child`, which on Linux works only under X11 (Wayland needs the GTK path). [wry docs](https://docs.rs/crate/wry/0.57.0)
- Tauri docs: WebGL context creation can silently fall back to a slow path on Linux WebKitGTK, showing as high latency in map/chart/editor views. [Tauri Linux graphics](https://v2.tauri.app/develop/debug/linux-graphics/). A 2026 GeoLibre how-to claims >500 ms tile loads on WebKitGTK vs <50 ms on Chromium (single project, A). [instagit](https://instagit.com/opengeos/GeoLibre/how-to-debug-webkitgtk-performance-linux/)
- **(bg, unverified)**: resvg is a pure-Rust SVG renderer (CPU, tiny-skia) with usvg tree; egui `egui_extras` has SVG image loader via resvg; Slint has built-in SVG support (own resvg-based) and Skia renderer; wgpu embedding: egui `egui_wgpu::Callback`, Iced `shader` widget, GPUI `surface`/paint custom (macOS only for video surfaces?), Slint has wgpu interop (`slint::wgpu` unstable); glTF viewers: `three-d`, `bevy`, `gltf` crate, `rend3`; `<model-viewer>`/three.js work in any webview.

### Inferences
- Pillar 6 "viewer registry" is easiest as sandboxed webview iframes (SVG/3D/PDF/CSV all exist as web components); native-extension viewers need a WASM-to-texture or shared-surface interface that no framework provides out of the box.
- Linux WebKitGTK WebGL slow-path is the main risk for in-webview 3D viewers; bundling Chromium (CEF) avoids it at large size cost (conflicts with "lightweight").

### Gaps
- No verified per-framework wgpu-embedding examples with URLs; Slint/GPUI/Floem/Xilem integration status unconfirmed.

## 6. Extension / plugin hosting for UI modules

### Takeaway
Real editors split in two: Zed and Lapce use WASM(+WIT/WASI) for logic only (no custom UI panels), VS Code/Obsidian use JS webviews/DOM for UI. Carbon's "user adds an SVG/3D viewer after build" needs UI contributions, which only a webview-iframe host (or a declarative UI manifest rendered natively) delivers without building a UI-ABI.

### Cited Findings
- Zed extensions: Rust compiled to Wasm via Component Model, run in Wasmtime, WIT + wit_bindgen; WASI permission-scoped access; `zed_extension_api` crate generated from WIT, versioned per Zed release; extensions add languages, themes, snippets, slash commands, debuggers, MCP servers; UI customisation "planned in the future". [Zed blog](https://zed.dev/blog/zed-decoded-extensions), [Plushcap](https://www.plushcap.com/content/zed/blog/life-of-a-zed-extension-rust-wit-wasm), [WIT](https://zed.dev/extensions/wit)
- Carbon PROJECT.md: WASM (WIT, capability grants) + MCP native; FEATURE.md EX-07: "Zed has no custom panels", MCP Apps `ui://` sandboxed iframes as escape hatch.
- Zed's earlier plugin runtime (plugin_runtime crate) was replaced by the component-model approach (history visible in repo docs). [secluded mirror](https://git.secluded.site/zed/blob/v0.53.0/crates/plugin_runtime/README.md)
- **(bg, unverified)**: Lapce plugins = WASI (any language), LSP-centric, no custom UI widgets; VS Code webviews = sandboxed iframes with postMessage; Obsidian plugins = full JS in-process DOM access (no sandbox); abi_stable for dylib plugins (Rust-only, unsafe, per-OS binaries); scripting options: Rhai, mlua (Lua), rquickjs/boa/deno_core (JS); Extism for WASM plugins; Slint offers a runtime interpreter (`slint-interpreter`) to load `.slint` UI files at run time (enables declarative UI modules natively); egui is immediate-mode so a WASM guest can issue draw commands per frame (some projects do this).

### Inferences
- Ranking for "viewer added after build": (1) webview iframe + postMessage/MCP Apps (any framework that can host a webview; native-only frameworks cannot without embedding wry); (2) Slint interpreter or declarative manifest (native, limited widget set); (3) WASM with a custom WIT UI-command interface (must be designed from scratch; egui/Iced easiest); (4) dylib/abi_stable (not recommended: unsafe, per-platform).
- A webview host framework (Tauri/Dioxus desktop) gets pillar 6 viewers nearly free; native-only frameworks need to embed a webview anyway.

### Gaps
- Lapce plugin docs and Obsidian security model not fetched. No example found of a Rust GUI app that renders WASM-guest-defined widgets in GPUI/Iced.

## 7. Multi-window and docking

### Takeaway
egui has egui_dock and egui_tiles; gpui-component has a serializable dock; webview apps use HTML/CSS docking libs (dockview, golden-layout) and Tauri windows; Slint/Iced/Floem have no turnkey docking.

### Cited Findings
- egui_dock 0.21 (egui 0.36): tab open/close, move tabs between nodes, drag tabs into new windows; egui_tiles is an alternative, more flexible but earlier-stage. [docs.rs egui_dock](https://docs.rs/crate/egui_dock/0.21.0)
- Version coupling: egui_term targets egui 0.31 while egui_dock 0.21 targets egui 0.36 (ecosystem crates lag the core). [egui_term](https://docs.rs/crate/egui_term/latest), [egui_dock](https://docs.rs/crate/egui_dock/0.21.0)
- gpui-component dock: resizable panels, draggable tabs, nested splits, edge docks, serializable layouts/Tiles (V). [GitHub](https://github.com/longbridge/gpui-component)
- Tauri multi-window is stable; multiwebview inside one window is unstable (wry child webview, X11-only on Linux). [wry](https://docs.rs/crate/wry/0.57.0)
- **(bg, unverified)**: Slint supports multiple windows but docking is DIY; Iced multi-window via `iced::daemon` (0.13+), no docking crate comparable to egui_dock (iced_aw has some tabs); Floem/Xilem: multi-window supported to varying degree; Dioxus desktop multi-window works with shared signals/`Context`, state sharing across windows needs a global store in the same process; with Carbon's daemon, state sharing between windows is by protocol (snapshot/actions) regardless of GUI.

### Inferences
- Carbon's daemon+client protocol (state-first, replay) makes cross-window state sharing a protocol concern, reducing the GUI's burden; pop-out windows then just open another client view.
- Agents tab with multiple chat panes: docking is needed (egui_dock, gpui-component dock, or web dock lib).

### Gaps
- No verified information on Slint/Iced/Floem/Xilem docking or pop-out; no pop-out + a11y interplay tests.

## 8. Theming/design tokens, hot reload, i18n, GUI testing, packaging, auto-update

### Takeaway
Webview gets CSS variables (design tokens), HMR, and Playwright-style tests for free; native frameworks have theme structs (egui Visuals, GPUI theme, Slint globals) but token pipelines and screenshot/a11y-tree testing are mostly DIY. Packaging: cargo-packager (Tauri org-adjacent, CrabNebula) covers dmg/app/deb/AppImage/pacman/NSIS/MSI plus an updater; MSIX is not listed.

### Cited Findings
- cargo-packager outputs: macOS DMG and .app; Linux deb, AppImage, Pacman; Windows NSIS (.exe) and MSI (WiX); also an updater; can package Tauri apps. [CrabNebula docs](https://docs.crabnebula.dev/packager/tauri/), [docs index](https://docsearch.algolia.com/mcp/docs/repo/crabnebula-dev/cargo-packager). No MSIX seen.
- Tauri bundler (fork of cargo-bundle): DMG/App, AppImage/deb/rpm, MSI/NSIS, with built-in updater. [docs.rs tauri-bundler](https://docs.rs/tauri-bundler)
- Dioxus: 0.8.0-alpha.0 release notes mention bundling fixes incl. macOS signing/notarization; current `dx bundle` formats/updater not documented in results (old 0.3 docs used cargo-bundle: .app, .deb, .msi). [Dioxus releases](https://github.com/DioxusLabs/dioxus/releases); prior user notes: `dx bundle --release --platform web -p <crate>` and `dx` needs glibc 2.39 on Linux (knowledge/rust/DIOXUS.md, local).
- Local prior-experience notes: Dioxus hashes asset names and only `asset!()` files are bundled; CSS-referenced fonts need `asset!()`; `dx` must run from repo root (DIOXUS.md, local).
- gpui-component: tree-sitter all-or-nothing flag adds ~35 MiB (A). [issue](https://upd.dev/longbridge/gpui-component/issues/2295)
- **(bg, unverified)**: hot reload: Dioxus has RSX/CSS hot reload (subsecond hot patching in 0.7), Slint has live-preview (`slint-viewer`, LSP preview), egui/GPUI/Iced require recompile (Iced/hotpatch tools optional); i18n: Slint has gettext-based `@tr()`, Fluent (`fluent-rs`) generally for others, ICU4X; testing: egui_kittest (snapshot + AccessKit-based queries), Slint testing backend, Dioxus/Tauri webview tests via WebDriver (tauri-driver) or Playwright against the web build, GPUI has headless test platform with simulated input; auto-update: Tauri updater plugin, cargo-packager updater, `self_update`/Velopack (Rust support) etc.

### Inferences
- Design tokens: W3C-style JSON -> CSS variables is trivial for WV; native needs a codegen step to a theme struct (Slint globals, egui Visuals) and VS Code theme import (Carbon requirement) maps naturally to CSS variables/Monaco themes.
- MSIX likely needs separate tooling for any option; verify.

### Gaps
- No verified GUI-testing/screenshot-test or hot-reload docs fetched; auto-update comparison unverified; MSIX support not confirmed anywhere.

## 9. Evidence: webview vs native for editor-heavy apps

### Takeaway
Strongest evidence is first-party, not independent: Zed claims (V) <10 ms/~600 MB vs VS Code 3-4 GB+; third-party blogs give conflicting magnitudes (A); no independent controlled benchmark found, consistent with Carbon's own note that numbers conflict 2x-15x. Webview risk evidence is concrete for Linux (documented WebKitGTK/driver/WebGL issues in Tauri's docs, doc-level).

### Cited Findings
- Zed first-party compare page: <10 ms typing latency, ~600 MB RAM vs VS Code 3-4 GB+ with many extensions (V). [zed.dev](https://zed.dev/compare/vscode)
- Third-party 2026 numbers: ~18 ms (VS Code) vs ~8 ms (Zed) keystroke; idle RAM ~730 MB vs ~142 MB (A; method unknown). [tech-insider](https://tech-insider.org/?p=19695); another guide: Zed <10 ms vs VS Code >50 ms under load (A). [dev.to](https://dev.to/truongandev/zed-vs-vs-code-in-2026-is-it-worth-switching-17gm)
- Tauri documents Linux graphics breakage (NVIDIA + WebKitGTK DMABUF: blank window, flicker, crashes on resize) and silent WebGL slow-path; recommends against blanket workarounds. [Tauri docs](https://v2.tauri.app/develop/debug/linux-graphics/) (doc)
- GeoLibre (Tauri) write-up: WebKitGTK tile processing on main thread, >500 ms vs <50 ms on Chromium (A, single project). [instagit](https://instagit.com/opengeos/GeoLibre/how-to-debug-webkitgtk-performance-linux/)
- Webview child-window limits on Linux (wry X11-only). [wry docs](https://docs.rs/crate/wry/0.57.0)
- Search found no "why we left Tauri/Electron" posts and no HN/Reddit threads in this session; Lapce numbers absent. gpui-component's "production ready" and Longbridge Pro usage are vendor claims. [GitHub](https://github.com/longbridge/gpui-component)
- Blitz/Dioxus Native maturity: pre-alpha per a May 2026 listing vs beta per older mirror (conflict). [lib.rs](https://lib.rs/crates/blitz-html)

### Inferences
- Measured evidence in sources found: none that is independent and controlled. Strongest "documented" evidence is Tauri's own Linux caveats (doc). Everything on latency/RAM is V or A.
- Carbon's plan (daemon + swappable client, webview first) is consistent with the evidence: it defers the contested decision and makes a native renderer for the hot surface (editor/terminal) possible later; the daemon already carries the heavy state, so memory comparisons would be driven by the renderer (webview process tree vs GPU app).

### Gaps
- Need: own benchmark (keystroke-to-photon, idle RSS with N open files, startup, 1 GB log file open, terminal throughput) on Win/mac/Linux for Monaco-in-Tauri vs one native candidate (GPUI-component most complete).
- Not found/searched: HN/Reddit threads, Lapce/Floem maintainers' posts on performance or IME, Xilem/Floem status 2026, Slint 1.x AccessKit status, Iced 2026 a11y status, Zed accessibility status, xterm.js benchmarks, React Flow/xyflow licence, per-framework wgpu embedding, GUI test tooling.

## Quick ready-made vs build-from-scratch matrix (from the above; (bg) cells unverified)

| Requirement | WV (Tauri/Dioxus desktop) | GPUI (+gpui-component) | egui | Slint | Iced | Floem/Xilem/DxN |
|---|---|---|---|---|---|---|
| Code editor widget | Monaco/CM6 ready (bg) | gpui-component editor w/ tree-sitter, LSP (V) | build | build | build (text_editor basic, bg) | Floem: lapce editor (bg); Xilem/DxN: build |
| Terminal | xterm.js (bg) | Zed has one (bg); gpui-component unconfirmed | egui_term (immature) | build | iced_term (bg) | build |
| AccessKit | browser a11y | claimed in gpui-component (V) | yes (accesskit.dev) | yes (accesskit.dev) | not confirmed | Xilem yes; DxN feature flag |
| Diagram canvas | React Flow/tldraw/Excalidraw (licence check) | custom paint | egui-snarl (early) | Path elements, build | canvas widget (bg) | Vello (Xilem/DxN) |
| Docking/multi-window | JS libs; Tauri windows | dock (V) | egui_dock (+new windows) | DIY | multi-window, no dock | unconfirmed |
| Plugin UI hosting | iframe/webview | WASM logic only (Zed model) | WASM draw-command DIY | slint-interpreter (bg) | DIY | DIY |
