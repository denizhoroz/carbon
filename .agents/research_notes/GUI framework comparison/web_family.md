# Rust GUI frameworks, web-tech family (Dioxus, Tauri, Leptos, Blitz, wry/tao, Sycamore, Yew) for Carbon

Context read first (project files): Carbon needs code editor, diagram canvas, agent chat panes, terminal, sandboxed extension viewers (SVG, 3D), multi-window, Win/mac/Linux, lightweight, AccessKit/ARIA from first widget. PROJECT.md already recommends "Rust core + Tauri webview (Monaco, xterm.js)", with Linux webview strategy and native-vs-webview latency still open. Prior-project DIOXUS.md covers Dioxus 0.7 fullstack web only (not desktop): dx 0.7.10 crashes if run inside crate folder, prebuilt dx needs glibc 2.39, assets must use `asset!()`, `aria-hidden` on svg needs string attr names. Evidence labels below: [M] measured by a cited source, [A] anecdote/vendor claim, [D] official docs claim, [API] GitHub/crates.io API on 2026-10-16, [BG] my background knowledge, not verified this session (treat as unverified).

## 1. Versions, cadence, licence, maturity (as of 2026-10-16)

### Takeaway
Tauri 2.12.x is stable and shipping weekly-ish patch releases, with 3.0 in alpha. Dioxus 0.7.10 is stable but 0.8.0-alpha.2 (with Blitz 0.3 beta) is already out. Leptos is near 0.9.0 (rc0). Yew and Sycamore are slow-moving. All are permissively licensed. Dioxus has frequent minor-version breaking changes (0.x).

### Cited Findings
- Tauri: stable `tauri-v2.12.4` and `3.0.0-alpha.5` both published 2026-10-14; 3.0 alpha release notes are only dependency bumps, no goals or breaking changes stated — [Tauri releases API](https://api.github.com/repos/tauri-apps/tauri/releases?per_page=8), [alpha.5 notes](https://github.com/tauri-apps/tauri/releases/tag/tauri-v3.0.0-alpha.5). 111,948 stars, 1,483 open issues, licence reported Apache-2.0 by API (Tauri is Apache-2.0/MIT dual [BG]) — [API](https://api.github.com/repos/tauri-apps/tauri). The 3.0 alpha CI shows cargo-audit flags five unmaintained/yanked deps (fxhash, paste, rustybuzz, ttf-parser, num-modular) — same alpha.5 page.
- Dioxus: stable v0.7.10 (2026-07-30); v0.8.0-alpha.0/.1 (2026-07-31)/.2 (2026-10-14) — [releases](https://api.github.com/repos/DioxusLabs/dioxus/releases?per_page=8). 0.7.0 shipped 2025-09-08 — [0.7 blog](https://dioxuslabs.com/blog/release-070/). 39,413 stars, 832 open issues, MIT-or-Apache style licence shown as Apache-2.0 by API — [API](https://api.github.com/repos/DioxusLabs/dioxus). 0.7 blog: 350+ PRs merged for 0.7. Cadence: ~1 minor per ~6-9 months with long alpha periods; 0.x means breaking changes each minor [BG].
- 0.8.0-alpha.2 desktop change: Linux no longer links `libxdo` by default (opt-in feature `linux-libxdo`) because prebuilt binaries failed to start on some distros; boolean attributes like `disabled: false` now removed instead of written as "false"; Blitz bump fixes a leak where removed nodes were never freed — [releases](https://api.github.com/repos/DioxusLabs/dioxus/releases?per_page=8).
- Leptos: v0.9.0-rc0 (2026-10-15), v0.8.22 (2026-10-05), v0.8.21 (2026-09-26), v0.9.0-beta (2026-07-18), v0.9.0-alpha (2026-05-19) — [releases](https://api.github.com/repos/leptos-rs/leptos/releases?per_page=6). 21,395 stars, MIT, 116 open issues — [API](https://api.github.com/repos/leptos-rs/leptos). 0.9 is "mostly compatible" with 0.8 app code; minor breaks: Signal passed as `impl Fn() -> T` may need deref, `on:toggle` event type, serde_qs 1.0 form encoding, lazy loading behind `lazy` feature; signal calls `foo()` now work on stable; lead author gbj; 9 first-time contributors in rc0 — [rc0 notes](https://github.com/leptos-rs/leptos/releases/tag/v0.9.0-rc0).
- Yew: v0.23.0 released 2026-03-10 — [API](https://api.github.com/repos/yewstack/yew/releases?per_page=3). Sycamore: 0.9.4 (2026-10-06), 0.9.3 (2026-08-31), 0.9.2 (2025-09-23) — [API](https://api.github.com/repos/sycamore-rs/sycamore/releases?per_page=3). Both are web (WASM) frameworks only; no desktop story of their own beyond running inside a webview (Tauri) [BG].
- Blitz: README says "beta"; renders many no-JS sites (Wikipedia, Old Reddit) with "many bugs and missing features"; 4.3k stars; Apache-2.0/MIT; no WebSockets, WebRTC, localStorage; GitHub Releases page empty (versioned via crates.io) — [repo](https://github.com/DioxusLabs/blitz). `dioxus-native` on crates.io: 0.8.0-alpha.2, only 44,544 total downloads (low adoption signal) — [crates.io](https://crates.io/api/v1/crates?q=dioxus-native&per_page=3). Dioxus alpha.2 pins Blitz 0.3.0-beta.3 — dioxus releases above.
- wry/tao: released as part of Tauri (tauri-runtime-wry 2.12.2 / 3.0.0-alpha.5 on 2026-10-14) — [Tauri releases](https://api.github.com/repos/tauri-apps/tauri/releases?per_page=8). wry is the cross-platform webview crate, tao the window/event-loop fork of winit [BG].

### Inferences
- Tauri 3 alpha is not a decision blocker; build on 2.x. Dioxus 0.8 will break 0.7 code; any Dioxus adoption must budget for migration churn.
- Active maintenance of all three main candidates is clearly healthy (releases within days of today).

### Gaps
- Contributor counts and funding not retrieved (Dioxus: FutureSDR/Huawei/other backing and Dioxus Labs company [BG]; Tauri: Tauri Foundation within Commons Conservancy, CrabNebula sponsor [BG]; Leptos: mostly gbj, sponsor-funded [BG]).
- Tauri 3 contents unknown (no source found).

## 2. How Dioxus 0.7 desktop actually renders (webview vs Blitz) and native-renderer maturity

### Takeaway
Default `dioxus-desktop` is a system webview via wry+tao. Dioxus Native (Blitz + wgpu/Vello, no webview) is opt-in and officially "work in progress"/beta; it is not suitable as Carbon's primary shell in 2026 but worth watching for isolated surfaces.

### Cited Findings
- `dioxus-desktop` 0.7.10 is a webview renderer built on wry and tao and needs a system webview installed — [docs.rs README](https://docs.rs/crate/dioxus-desktop/0.7.10/source/README.md).
- 0.7 blog: Dioxus Native paints on GPU with WGPU, no webview; Blitz often "indistinguishable from Chrome/Safari" for supported content but "still a work in progress", "still very young", not all CSS supported, performance not yet a focus; no size numbers given — [0.7 blog](https://dioxuslabs.com/blog/release-070/). 0.7 docs call the native renderer experimental and say TUI renderer was deprecated until Blitz is more stable — [search result summarising docs](https://dioxuslabs.com/learn/0.7/beyond/project_structure).
- Blitz CSS status: supported block, inline-block, flex, grid, positions (sticky in progress), `@font-face`, 2D transforms, transitions, animations, box-shadow, background/border; partial `display: table`, filter/backdrop-filter; NOT supported: 3D transforms, mix-blend-mode, multicolumn, text-shadow, hyphens, line clamp, `overflow: auto`, scroll snapping, `overscroll-behavior`, `scrollbar-gutter`; SVG renders but `fill`/`stroke` styling not supported — [blitz.is/status](https://blitz.is/status). That page says nothing on IME or accessibility completeness.
- Blitz lists AccessKit accessibility as a goal; `blitz-shell` uses AccessKit; completeness not documented; text layout by Parley; no mention of IME in README — [Blitz repo](https://github.com/DioxusLabs/blitz).
- Dioxus 0.8.0-alpha.1 added synthetic click events "for testing native apps" — [releases](https://api.github.com/repos/DioxusLabs/dioxus/releases?per_page=8).

### Inferences
- Monaco/xterm.js (need DOM, JS, canvas/WebGL, contenteditable) cannot run in Blitz (no JS engine; README excludes JS-dependent features). So Dioxus Native means writing your own editor and terminal widgets in Rust, or using a different stack for them. With the webview renderer, Dioxus is essentially "Tauri-like with Rust-side Dioxus UI".
- Missing `overflow: auto`, SVG fill/stroke, and unverified IME make Blitz a poor fit for an IDE shell in late 2026.

### Gaps
- No independent test of Blitz text input/IME, a11y tree quality, or perf found. Blitz current crate version and latest feature list (editing, selection) not verified.

## 3. Verified numbers (binary size, RAM, startup) vs anecdotes

### Takeaway
Only installer-size claims are well supported. RAM and startup numbers are sparse and contradictory; no measured Dioxus-desktop or Leptos-in-Tauri figures were found. Carbon must run its own benchmark (matches DESIGN.md s9).

### Cited Findings
- [M] Installers, same-category apps (Tabularis 0.16.0 on Tauri vs Beekeeper Studio 5.9.2 on Electron): .deb 17 vs 207 MB, .dmg 18 vs 290, Windows 12 vs 201, AppImage 94 vs 282 MB; Tauri .msi/.deb/.rpm/.dmg 15.8-18.3 MB; AppImage 93.7 MB of which ~76 MB bundled WebKitGTK deps — [HackerNoon](https://hackernoon.com/six-months-with-tauri-the-benefits-and-the-bill). No RAM data there.
- [M, one source] Better Stack controlled test: startup Deno Desktop 242 ms, Electron 273 ms, Tauri 311 ms (Tauri slower than Electron); idle RAM Tauri ~109 MB vs Electron ~128 MB — [Better Stack](https://betterstack.com/community/guides/scaling-nodejs/tauri-vs-electron-vs-deno-vs-electrobun/) (via search summary, page not fully fetched). Another source concurs RAM gap is far smaller than disk gap — [Stork](https://www.stork.ai/blog/electrons-reign-is-over-heres-why).
- [A] Claims of Tauri 30-40 MB idle vs Electron 300-500 MB, and sub-500 ms vs 1-2 s startup, lack methodology — [DEV Community](https://dev.to/devtoolpicks/tauri-vs-electron-for-indie-hackers-in-2026-honest-comparison-o0e). Other bundle claim 57 MB vs 323 MB — Stork.
- [D] Dioxus docs claim webview desktop apps "typically under 5MB" — [Dioxus 0.6 desktop docs](https://dioxuslabs.com/learn/0.6/guides/desktop/llms.txt). No measured hello-world figure found. The "under 100kb" figure is web/WASM only — [optimizing guide](https://dioxuslabs.com/learn/0.6/cookbook/optimizing).
- Leptos: no desktop numbers; `lazy` feature off by default in 0.9 gives smaller binary — rc0 notes.

### Inferences
- Webview-based RAM is dominated by the web engine process tree (WebView2 msedgewebview2 processes, WebKit helpers) plus whatever the editor (Monaco) loads; expect the framework choice (Dioxus vs Tauri vs Leptos) to matter little for RAM. Any IDE-sized app will be well above 100 MB hello-world figures [inference].

### Gaps
- No verified idle RAM/startup for Dioxus desktop, Dioxus Native, or Leptos+Tauri. No Monaco-in-Tauri benchmark.

## 4. Architecture, DSL, state, hot reload

### Takeaway
Dioxus = React-like component model with `rsx!` and signals, fullstack, Subsecond Rust hot-patching. Tauri = Rust backend + any JS/WASM frontend with IPC commands/events. Leptos = fine-grained signals, `view!` macro, WASM CSR/SSR; on desktop it is used as the frontend inside Tauri.

### Cited Findings
- Dioxus 0.7: Subsecond hot-patches Rust code at runtime without losing state on web, desktop (mac/Linux/Windows) and mobile; no automatic migration when struct fields change; also 28 Radix-style primitive components, integrated debugger; `dx serve` enables it — [0.7 blog](https://dioxuslabs.com/blog/release-070/). 0.7.10 fixed hot-patching when binary crate name differs from package name — releases API. Known issue from prior project: `dx` must be run from repo root — DIOXUS.md.
- Tauri: IPC serialises JSON between Rust and webview; Tabularis reports results must cross the bridge via serde_json, mitigated by paging (500 rows) and streaming exports to file; plugin drivers as subprocesses speaking JSON-RPC over stdio — [HackerNoon](https://hackernoon.com/six-months-with-tauri-the-benefits-and-the-bill).
- Leptos 0.9: signal calls now work on stable; reactive_stores; islands; server functions; Axum/Actix integration — rc0 notes.
- Dioxus state: signals, context, stores [BG]; Tauri: Rust `State<T>` managed state + frontend framework's own [BG]; Leptos: RwSignal/Store [BG].

### Inferences
- Two-process-ish split (Rust core + webview UI) with large payloads (buffers, terminal output, LSP data) must use binary/streaming channels. This matches PROJECT.md's daemon-owns-state design; webview UI as thin client suits it.

### Gaps
- Official Tauri 2 frontend hot reload specifics (Vite HMR [BG]) not sourced; Dioxus CSS hot reload [BG].

## 5. Multi-window, menus, dialogs, tray

### Takeaway
Tauri has the most complete documented desktop shell API (multi-window, native menus, tray, dialogs plugin) [BG]; Dioxus desktop supports multiple windows and menu/tray via tao/muda/tray-icon [BG]; Blitz/Dioxus Native multi-window and native menus are unverified.

### Cited Findings
- Dioxus 0.7 blog does not mention multi-window — [0.7 blog](https://dioxuslabs.com/blog/release-070/). 0.8.0-alpha.2 desktop notes only mention libxdo change — releases API.
- Tauri 2 plugin ecosystem exists for dialog, fs, shell, updater etc. [BG]; Flathub packaging for a Tauri app required offline-vendored deps (open issue in Tabularis #326) — HackerNoon.
- macOS quirk: a straight quote typed in a webview filter became a curly quote (WKWebView smart-quote behaviour) — HackerNoon.

### Inferences
- DESIGN.md requires pop-out windows sharing session state: with webviews each window is a separate webview (RAM cost per window), state must live in the Rust core.

### Gaps
- No primary-source fetch of Tauri/Dioxus window, menu, tray docs this session; all feature claims here are [BG]. A report writer should verify against v2.tauri.app and docs.rs/dioxus-desktop before relying.

## 6. Accessibility, IME and text input

### Takeaway
Webview frameworks inherit platform web accessibility (UIA via WebView2, NSAccessibility via WKWebView, ATK via WebKitGTK) and browser IME, which is a major advantage over Blitz or custom GPU UIs. Monaco has a documented screen-reader mode.

### Cited Findings
- Monaco ships screen-reader mode (paged text strategy), F1 command palette, Tab-trapping toggle, `ariaLabel`; it disables word wrap when `accessibilitySupport` is "on"; NVDA caret timeout tuning recommended — [Monaco Accessibility Guide](https://github.com/Microsoft/monaco-editor/wiki/Accessibility-Guide-for-Integrators), [GitLab MR](https://gitlab.com/gitlab-org/gitlab/-/merge_requests/217065/pipelines).
- Blitz: AccessKit is goal; Parley text layout; IME/selection support not documented — [Blitz repo](https://github.com/DioxusLabs/blitz), [status](https://blitz.is/status) (`caret-color` supported, `user-select` partial).
- Per-platform webview divergence in CSS, input and GPU compositing (WKWebView, WebKitGTK, WebView2) — HackerNoon.

### Inferences
- If accessibility is a hard day-one requirement, webview + ARIA is the lowest-risk path; native Blitz would need own a11y verification.

### Gaps
- No Tauri-specific a11y test results found; WebKitGTK screen reader (Orca) quality in Tauri unverified.

## 7. Embedding Monaco, CodeMirror, xterm.js; canvas/WebGL/wgpu

### Takeaway
All webview options can embed Monaco/CodeMirror/xterm.js as ordinary JS (Tauri via bundled frontend; Dioxus desktop via `asset!()` + `document::eval`; Leptos via wasm-bindgen interop). Blitz cannot.

### Cited Findings
- Dioxus: CSS/fonts referenced from CSS are not bundled unless declared with `asset!()`; `document::eval` runs JS in async fn; string attribute names needed for aria on svg — DIOXUS.md (prior project).
- Blitz excludes JS-dependent features — [Blitz repo](https://github.com/DioxusLabs/blitz); SVG fill/stroke unsupported, so SVG viewer would be poor — [status](https://blitz.is/status).
- Dioxus Native uses wgpu so custom wgpu surfaces (3D viewer) are plausible in-process [BG].
- Webview WebGL/WebGPU availability varies by engine (WebView2 Chromium has WebGPU; WKWebView/WebKitGTK lag) [BG, unverified].

### Inferences
- Carbon's SVG/3D viewers and diagram canvas fit webview (inline SVG, three.js, canvas) with sandboxed iframes; terminal via xterm.js with PTY in Rust daemon.

### Gaps
- No measured Monaco/xterm.js latency in Tauri/WebView2/WebKitGTK found.

## 8. Platform quirks (WebKitGTK, WebView2, WKWebView)

### Takeaway
Linux is the main risk: WebKitGTK crashes/blank windows on some GPU/Wayland setups, glibc linkage and AppImage size issues. Alternative engines (Servo/Verso/CEF) are not production-ready for Tauri.

### Cited Findings
- Tabularis (Tauri) issues: Wayland error on Arch (#9), `libEGL ... DRI_Mesa` failure with Snap (works in .deb) (#45), blank AppImage window fixed by preloading host libwayland-client (#54), WebKit abort on Solus AppImage (#423), release built on ubuntu-24.04 needed glibc 2.39 and failed on Ubuntu 22.04 (fix: build in ubuntu:22.04 container) — [HackerNoon](https://hackernoon.com/six-months-with-tauri-the-benefits-and-the-bill). Same glibc trap hit prebuilt `dx` — DIOXUS.md.
- Verso (Servo-based Tauri runtime) reportedly archived Oct 2025 with tauri-runtime-verso without commits since; Servo ships monthly releases and published to crates.io April 2026 — [youngju.dev blog](https://www.youngju.dev/blog/2026-07-17-servo-embedding-crates-io-verso-archived.en) (single personal blog, unverified).
- Dioxus 0.8.0-alpha.2: libxdo no longer linked by default on Linux — releases API.

### Inferences
- PROJECT.md open question "WebKitGTK QA vs bundled Chromium": no bundled-Chromium option in Tauri is production-ready in the sources found (CEF runtime exists as experiment [BG]); budget Linux QA across Wayland/X11/NVIDIA.

### Gaps
- No WebKitGTK performance benchmark vs Chromium for editor workloads found.

## 9. Production apps, reach, extension story, packaging, community, complaints

### Takeaway
Tauri has the largest ecosystem (112k stars). Dioxus is mid-size (39k) with fewer large production desktop apps. Leptos is web-first.

### Cited Findings
- Tauri production example in sources: Tabularis (DB client, 64 releases in 6 months, 10 languages) — [HackerNoon](https://hackernoon.com/six-months-with-tauri-the-benefits-and-the-bill). Other known Tauri apps (Spacedrive, Hoppscotch desktop, Pake, etc.) [BG].
- Tauri extension pattern: plugin drivers as subprocesses with JSON-RPC over stdio — same article. Tauri packaging: .msi/.exe, .dmg, .deb/.rpm, AppImage, Flatpak needs offline build — same.
- Dioxus: web, desktop, mobile (iOS/Android), fullstack, `dx bundle` — [0.7 blog](https://dioxuslabs.com/blog/release-070/), DIOXUS.md. Tauri 2 also mobile [search summary of Tauri blog](https://v2.tauri.app/blog/create-tauri-app-version-3-released/) (alpha mobile support mention).
- Complaints: mismatched webviews across OSes, IPC serialisation cost, Linux packaging, long-running version churn — HackerNoon; Dioxus 0.x breaking changes and tooling (`dx` glibc, run-from-root) — DIOXUS.md.
- Production apps for Dioxus desktop: not verified this session [BG: Dioxus-based apps exist but none IDE-scale].

### Inferences
- For modular user-built extensions: webview iframes + WASM components or subprocess/JSON-RPC (Tauri pattern) fit Carbon's WASM/MCP plan. Dioxus Native has no sandboxed extension UI story.

### Gaps
- Funding, contributor counts, Reddit/HN sentiment threads not retrieved; Dioxus desktop RAM/startup complaints not found; mobile/web reach for Leptos/Yew/Sycamore not detailed (web-only [BG]).

## 10. Pain points for an editor-heavy IDE, per framework (synthesis, mostly inference)

- Tauri: IPC overhead for large buffers; WebKitGTK divergence; per-window webview RAM; need JS toolchain (Monaco, xterm.js) so project is bilingual; security model (capabilities) is a plus for sandboxing.
- Dioxus (webview): all Tauri webview issues plus 0.x churn; Rust-only UI but editor still JS via eval interop; `asset!()` pipeline and `dx` tooling quirks (DIOXUS.md).
- Dioxus Native/Blitz: no JS so no Monaco/xterm.js; CSS gaps (overflow:auto, SVG styling); IME/a11y unverified; beta; 44.5k downloads.
- Leptos/Sycamore/Yew: only the UI layer inside a webview (usually Tauri); WASM-to-JS interop for Monaco adds glue; Leptos has best reactivity model and fastest release cadence of the three.
- wry/tao directly: maximal control and minimal bundle but you reimplement menus, updater, bundling; Tauri is the batteries-included layer on top.
