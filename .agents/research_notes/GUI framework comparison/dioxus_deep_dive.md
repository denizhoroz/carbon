# Dioxus deep dive for Carbon (desktop 0.7.x stable and 0.8 alpha), as of 2026-10-16

Labels: [M] measured by a source, [A] anecdote or vendor claim, [D] official docs/source code, [API] GitHub/crates.io API read on 2026-10-16, [BG] background knowledge, NOT verified this session. Complements `web_family.md` (not repeated: versions, Blitz CSS table, Tauri installer sizes). Prior-project `DIOXUS.md` covers web only. Searches this session were weak on Reddit/HN/Discord, so community sentiment is thin (see Gaps).

## 1. Multi-window, per-window VirtualDom, shared state, pop-out windows

### Takeaway
In 0.7 every desktop window had its own VirtualDom, so signals/context/globals did not cross windows. On `main` (0.8 line) there is now a headless-root plus `Window` component model where one app root owns all native windows and shares one signal/store tree, merged via PR #5554 on 2026-08-20. Whether it is in 0.8.0-alpha.2 is unverified. This is directly relevant to Carbon's pop-out panes, but it is new, so budget for alpha churn.

### Cited Findings
- Issue #5160 "Multiple windows in the same runtime" (open, 2025-12-29): "desktop always requires a new virtual dom with a different signal + context for every new window"; proposed fix is letting a VirtualDom accept a foreign runtime so signals and globals work across windows — [#5160](https://github.com/DioxusLabs/dioxus/issues/5160) [API].
- Issue #1569 "Cross-virtual dom signals" (open since 2023-10-23, Low-Priority Backlog milestone): signals use the context API, so you cannot create a signal in one window and send it to another — [#1569](https://github.com/DioxusLabs/dioxus/issues/1569) [API].
- Merged PR #5554 (ealmloff, 2026-08-20) "Markerless placeholders/Portals/Multiwindow/const builder/optimized templates": "Portals/multirenderer support via a renderer id + routing" — [PR #5554](https://github.com/DioxusLabs/dioxus/pull/5554) [API].
- `main` examples `08-apis/multiwindow.rs` and `window_popup.rs`: `Config::new().with_headless_root(true)`, root component renders `Window { config: WindowConfig::new().with_window(WindowBuilder::new().with_title(..)), onclose: .. , AppWindow {..} }` per window, with a shared `use_store`/`Store` (`WindowState`, `MailState`) passed to children, "send data back to the parent window" — [multiwindow.rs](https://raw.githubusercontent.com/DioxusLabs/dioxus/main/examples/08-apis/multiwindow.rs), [window_popup.rs](https://raw.githubusercontent.com/DioxusLabs/dioxus/main/examples/08-apis/window_popup.rs) [D]. Config also has `with_close_behaviour`, `with_exits_when_last_window_closes`, `with_as_child_window` — [config.rs](https://raw.githubusercontent.com/DioxusLabs/dioxus/main/packages/desktop/src/config.rs) [D].
- 0.7 API: `use_window()`/`window()` returns `DesktopContext`, `new_window(VirtualDom, Config)` opens extra windows; `with_on_window(|window, dom| dom.provide_root_context(..))` injects a value into a window's VirtualDom — [docs.rs dioxus-desktop](https://docs.rs/dioxus-desktop/0.7.10/dioxus_desktop/), [wgpu_child_window.rs](https://raw.githubusercontent.com/DioxusLabs/dioxus/main/examples/08-apis/wgpu_child_window.rs) [D]. (`new_window` itself was not listed by the docs.rs fetch; treat exact signature as to-verify.)
- Old 0.5 roadmap: multi-window "requires support from Dioxus core and are not ready" — [0.5 roadmap](https://dioxuslabs.com/learn/0.5/contributing/roadmap) (dated).
- Issue #3588 "Fixed multiwindow closing issue again" (closed 2025-01-18) shows multi-window lifecycle bugs recurred — [API search](https://github.com/DioxusLabs/dioxus/pull/3588).

### Inferences
- Under 0.7 the robust pattern for Carbon is: Rust core/daemon owns state, each window's VirtualDom subscribes through channels (e.g. `use_coroutine` + tokio broadcast); do not rely on signals crossing windows. On 0.8 the headless-root pattern may make a single shared store workable. Pop-outs would be additional `Window` components, but each is still its own webview.

### Gaps
- Did not confirm the headless-root/`Window` API is in alpha.2 or stable-bound; check the 0.8.0-alpha.2 tag source.
- No measured per-window webview overhead found (see section 8).

## 2. Menus, tray, dialogs, clipboard, drag-drop, global shortcuts, titlebar

### Takeaway
Menus (muda), tray, and global shortcuts are first-class Dioxus hooks with examples. Drag-drop works via HTML DnD with `data_transfer()`. Custom titlebars work but undecorated-window resizing is an open gap, and file dialogs (rfd or input type=file) have Linux bugs.

### Cited Findings
- Menus: `Config::with_menu(muda::Menu)` and `use_muda_event_handler`; example `custom_menu.rs` builds a menu with `PredefinedMenuItem::undo/cut/copy/paste/select_all` and `MenuItem::with_id` — [custom_menu.rs](https://raw.githubusercontent.com/DioxusLabs/dioxus/main/examples/08-apis/custom_menu.rs) [D]; muda not available on Android/iOS — [docs.rs](https://docs.rs/dioxus-desktop/0.7.10/dioxus_desktop/).
- Tray: `trayicon` module, `use_tray_icon_event_handler`, `use_tray_menu_event_handler` (Linux/macOS/Windows), example `multiwindow_with_tray_icon.rs`, `Config::with_tray_icon_show_window_on_click` — docs.rs and config.rs above [D].
- Global shortcuts: `use_global_shortcut("ctrl+s", |state| ..)` with `HotKeyState`; works when app unfocused; returns `ShortcutHandle`/`ShortcutRegistryError` — [shortcut.rs](https://raw.githubusercontent.com/DioxusLabs/dioxus/main/examples/08-apis/shortcut.rs) [D]. In-app keybindings are DOM `onkeydown` [BG].
- Drag-drop: DOM drag events with `e.data_transfer().get_data(..)`; `Config::with_disable_drag_drop_handler` exists (wry file-drop handler off so HTML DnD works) — [drag_and_drop.rs](https://raw.githubusercontent.com/DioxusLabs/dioxus/main/examples/08-apis/drag_and_drop.rs), config.rs [D]. Issue #3100 "Dropping files doesn't work on Wayland" (open) — [API search](https://github.com/DioxusLabs/dioxus/issues/3100).
- Custom titlebar: issue #3128 "Undecorated window resizing" (open since 2025-07-01): disabling decorations in tao prevents resize, Dioxus lacks the API Tauri provides (drag-resize) — [#3128](https://github.com/DioxusLabs/dioxus/issues/3128) [API]. #4528 "Titlebar buttons unresponsive until window is maximized" (closed) — [API search](https://github.com/DioxusLabs/dioxus/issues/4528).
- File dialog: #3439 "File dialog issues on Linux desktop" (open) for `input type=file`; #3841/#3845 N/A — [#3439](https://github.com/DioxusLabs/dioxus/issues/3439). Dioxus has no first-party dialog/clipboard API in the sources checked; `rfd` and `arboard` are separate crates [BG].
- Other config: `with_disable_context_menu`, `with_custom_head`, `with_navigation_handler` (0.8 PR #5531 adds opt-in external-URL navigation handler) — config.rs, [v0.8.0-alpha.0 notes](https://github.com/DioxusLabs/dioxus/releases/tag/v0.8.0-alpha.0).

### Inferences
- Carbon would use rfd (native dialogs, off the webview) and arboard/clipboard via Rust; menus/tray/shortcuts are covered. A custom titlebar needs hand-rolled resize handles (via tao `drag_resize_window` [BG]) on Win/Linux.

### Gaps
- Clipboard and rfd integration not source-checked; macOS custom titlebar (traffic-light inset) not researched.

## 3. JS interop, embedding Monaco/CodeMirror/xterm.js/React Flow/Excalidraw, assets, IPC

### Takeaway
Desktop Dioxus has `document::eval` (two-way JSON channel), `asset!()` bundling (incl. folder assets), and custom protocol handlers. A wasm-bindgen-compatible desktop path is in-progress for 0.8. A working CodeMirror 6 wrapper crate exists; no Monaco/xterm.js/React Flow/Excalidraw Dioxus examples found. Virtual-DOM edits go over a binary websocket, so UI updates are fast; eval is JSON text and has open reliability bugs.

### Cited Findings
- Eval: `document::eval` with `dioxus.send/recv` in JS and `eval.send/recv::<T>()` in Rust — [desktop guide](https://dioxuslabs.com/learn/0.7/guides/platforms/desktop) [D]. Open eval issues: #5919 (2026-10-14: eval can run before `document::Script` from the same render), #4590 "Eval unexpected finish", #4064 eval in `use_drop` asserts, #3915 `try_recv` missing, #5037 getUserMedia fails on Linux (no wry permission handler) — [API search](https://github.com/DioxusLabs/dioxus/issues/5919).
- Transport: VDOM edits are sledgehammer batches sent through a websocket (random key via init script; formerly long-polling on wry custom protocol, changed due to Android bugs) — [edits.rs](https://raw.githubusercontent.com/DioxusLabs/dioxus/main/packages/desktop/src/edits.rs) [D].
- Custom protocol: `Config::with_custom_protocol(name, handler)` and `with_asynchronous_custom_protocol`; `use_asset_handler` + `RequestAsyncResponder`; assets served by the built-in protocol — config.rs, protocol.rs, docs.rs [D]. Heavy requests on mobile hit `AlreadyBorrowedError` (#4005, closed) — [API search](https://github.com/DioxusLabs/dioxus/issues/4005).
- Assets: `asset!()` extracts/optimizes at build, hashless assets and `/public` dir copied as-is in 0.7, new `asset!()` calls hot-patchable — [0.7 blog](https://dioxuslabs.com/blog/release-070/). CSS-referenced files must be declared with `asset!()` (prior project note, DIOXUS.md). Issue #3422 symlinked assets regression, #1756 cross-platform dynamic assets — [open list](https://api.github.com/repos/DioxusLabs/dioxus/issues?milestone=14).
- `dioxus_codemirror` crate: CodeMirror 6 wrapper driven by one long-lived `document::eval` channel (glue.js), CodeMirror vendored as a folder asset (no JS build step, no CDN), optional LspBridge — [docs.rs](https://docs.rs/dioxus_codemirror/). Only known editor integration found.
- wasm-bindgen on desktop: PR #5194 (open, 2026-06-15, in 0.8.0 milestone) forks wasm-bindgen/web-sys/js-sys as `*-x` crates so web and desktop share event/eval/mounted-data code, moves VirtualDom to a separate thread; breaking: `web-sys-x` types, `Send` bounds on wry event hooks; not compatible with low-level wasm access — [PR #5194](https://github.com/DioxusLabs/dioxus/pull/5194) [API].
- wgpu: `with_as_child_window()` lets Dioxus (webview) be a child window over a wgpu surface (example based on wry's) — wgpu_child_window.rs [D]; `10-integrations/wgpu-texture` example exists — [API listing](https://api.github.com/repos/DioxusLabs/dioxus/contents/examples/10-integrations).
- Rust-side desktop caveat: "browser APIs like WebGL and Canvas aren't available" is the official guide's wording for Rust code, meaning those must be driven through JS — [desktop guide](https://dioxuslabs.com/learn/0.7/guides/platforms/desktop).
- Monaco, xterm.js, React Flow, Excalidraw: no Dioxus-specific example, crate, or issue found in search [gap]. Generic comparison favors CodeMirror 6 for small custom embedding and Monaco for full IDE features — [pkgpulse 2026](https://www.pkgpulse.com/guides/monaco-editor-vs-codemirror-6-vs-sandpack-in-browser-2026) (secondary).

### Inferences
- Monaco/xterm.js/React Flow are plain JS bundles: bundle them (esbuild) into a folder asset and load via `asset!()` or a custom protocol; use eval for control messages, but for high-volume data (terminal bytes, large buffers) prefer a custom protocol URL, or a dedicated websocket the page opens to a Rust server, instead of JSON eval. [inference; same Tauri-class tradeoff]
- If JS-heavy panes dominate, Dioxus adds little over Tauri for those panes; Dioxus value is Rust-written chrome (tree views, chat, settings).

### Gaps
- No IPC latency/throughput measurements for eval or custom protocol. No Monaco-worker-under-custom-protocol report found.

## 4. Hosting a Rust terminal (portable-pty + alacritty_terminal) feeding xterm.js

### Takeaway
No Dioxus-specific implementation found. The architecture is generic: PTY in Rust, bytes to xterm.js. Everything below is [BG] and unverified.

### Cited Findings
- None sourced. Relevant sourced facts: eval is JSON text channel and has open reliability bugs (section 3); custom protocol and websocket transports exist in desktop (section 3).

### Inferences
- [BG] `portable-pty` (from WezTerm) spawns ConPTY/openpty; a Rust task reads PTY output and forwards chunks (base64 over eval or binary over a local websocket/custom protocol stream) to `xterm.write()`; keystrokes go back via xterm `onData`. `alacritty_terminal` is only needed if Rust must parse the grid itself (e.g. headless screen state for agents, scrollback search); xterm.js already parses VT in the webview, so using both duplicates parsing. A local websocket with a per-session token (as Dioxus does for edits) gives binary frames without JSON.

### Gaps
- Needs a prototype; xterm.js WebGL addon availability under WebKitGTK/WKWebView unverified.

## 5. Subsecond, dx tooling, bundling, auto-update

### Takeaway
Subsecond hot-patching is on by default in 0.8 and works on desktop on all three OSes. `dx bundle` produces NSIS/MSI, DMG/.app, AppImage/deb/rpm, and has an "Updater" package type and Windows WebView2 install modes, but no first-party update client was found in sources.

### Cited Findings
- Subsecond: hot-patches Rust code without losing state on web, desktop, mobile through `dx serve`; explicit integration points (`subsecond::call`); no struct-layout migration; Bevy and Iced adopted; `dx deploy` planned for 0.8 — [0.7 blog](https://dioxuslabs.com/blog/release-070/). 0.8.0-alpha.0: hotpatch enabled by default (#5506), wasm hotpatch with C deps fixed (#5556), `rust-lld` custom linker removed on Windows (#5242) — [alpha.0 notes](https://github.com/DioxusLabs/dioxus/releases/tag/v0.8.0-alpha.0).
- Bundle formats in CLI source: default Windows = NSIS, macOS = .app + DMG, Linux = AppImage; also selectable `Deb`, `Rpm`, `WindowsMsi`, and `Updater` — [bundle.rs](https://raw.githubusercontent.com/DioxusLabs/dioxus/main/packages/cli/src/cli/bundle.rs), [config/bundle.rs](https://raw.githubusercontent.com/DioxusLabs/dioxus/main/packages/cli/src/config/bundle.rs) [D]. Bundler is derived from Tauri's (tauri-bundler-style settings: `WixSettings`, `NsisSettings`, `sign_command`, `signing_identity`) [BG on lineage].
- WebView2 install modes (enum): `Skip`, `DownloadBootstrapper`, `EmbedBootstrapper`, `OfflineInstaller` (default), `FixedRuntime{path}`; NSIS `minimum_webview2_version` triggers WebView2 update — config/bundle.rs [D]. Offline installer default means the installer carries the WebView2 runtime (larger installer) [inference].
- 0.8 alpha.0 bundling fixes: macOS signing/notarization (#5527), aarch64 Alpine bundling (#5524) — alpha.0 notes. Issue #1967 "WebView data is not uninstalled with NSIS installer" (open).
- Auto-update: an `Updater` PackageType exists in the CLI enum, but no docs, update client, or updater issues except dated ones were found; issue #3855 "Dioxus Desktop & Mobile Native APIs" open — [API search](https://github.com/DioxusLabs/dioxus/issues/3855).
- dx CLI quirks (prior project): run from repo root, prebuilt dx needs glibc 2.39 — DIOXUS.md. 0.8: CLI from source needs `--locked`.
- Cross-compilation: Deno's 2026 comparison states Tauri and Dioxus need the target platform locally to build — [Deno docs](https://docs.deno.com/runtime/desktop/comparison.md) (competitor source).

### Inferences
- Carbon would likely use Velopack/self-made updater or the `self_update` crate [BG] since no Dioxus-blessed updater is documented; Tauri's updater plugin is a point in Tauri's favor.

### Gaps
- Actual `Updater` package behavior unknown. No Subsecond reliability reports on large Windows codebases found. No measured dx bundle sizes.

## 6. Dioxus Native / Blitz in 2026 (blocks for IDE use, 0.8 roadmap)

### Takeaway
0.8 puts a "huge quality upgrade" into dioxus-native, but there is no published roadmap or dated milestones. Blitz is "beta" (README) and the sources still show editor-blocking gaps; IME/a11y completeness is undocumented. Do not plan Carbon's shell on it.

### Cited Findings
- 0.8.0-alpha.0 (2026-05-19): "huge quality upgrade to dioxus-native" with more rendering capabilities, incremental rendering, custom elements; synced Blitz 0.3.0-alpha.4; points to a Rustweek 2026 Blitz talk — [alpha.0 notes](https://github.com/DioxusLabs/dioxus/releases/tag/v0.8.0-alpha.0). Blitz crates at 0.3.0-beta.4, 285,766 downloads for blitz-dom, updated 2026-10-16 — [crates.io](https://crates.io/api/v1/crates/blitz-dom) [API] (contrast with web_family's 44.5k for dioxus-native).
- README: "beta ... usable for making apps if you are an early adopter and willing to live on the bleeding edge ... many bugs and missing features"; plans AccessKit accessibility; `blitz-shell` integrates winit, AccessKit, muda — [Blitz README](https://raw.githubusercontent.com/DioxusLabs/blitz/main/README.md). Another listing calls Blitz "pre-alpha" — [lib.rs](https://lib.rs/crates/blitz-html) (older wording, conflicts with README "beta").
- Status page: `overflow:auto` unsupported, `overflow:scroll` supported, `user-select:text` supported, SVG renders but styling (fill/stroke) unsupported — [blitz.is/status](https://blitz.is/status). 0.7 blog lists text inputs, widgets, form handling, hover styling in the interactivity layer; no mention of IME — [0.7 blog](https://dioxuslabs.com/blog/release-070/).
- Open Dioxus Native issues: #5133 blank window on NixOS/Wayland (2025-12-23), #5191 "Failed to create GBM buffer" (2026-01-09) — [API search](https://github.com/DioxusLabs/dioxus/issues/5133).
- [A] LinkedIn post claims a dioxus-native (Vello/wgpu) chart app at 530 MB until one shared `vello::Renderer` saved 370 MB — [search summary of LinkedIn](https://il.linkedin.com/in/zabralex) (unverified, single post).
- No Blitz issue-tracker hits for IME/AccessKit via API search (the search returned nothing, which may reflect query limits, not absence).

### Inferences
- Blockers for an IDE on Blitz: no JS (so no Monaco/xterm.js; need a Rust editor like a custom text widget), no `overflow:auto`, unproven IME and screen-reader exposure, no multi-window story documented. A hybrid (webview shell, wgpu child window for canvas/3D via `with_as_child_window`) is the only practical Dioxus path now.

### Gaps
- No Rustweek 2026 talk content, no 0.8 roadmap doc, no text-editing/IME status of Blitz verified.

## 7. Accessibility and IME in webview mode

### Takeaway
Webview mode inherits platform browser accessibility and IME [BG], but Dioxus-specific IME bugs exist and Dioxus adds no a11y layer of its own beyond ARIA attributes in rsx.

### Cited Findings
- IME: #5246 "Inline IME stops working when moving window" (open, v0.7.3, Windows 11, Korean IME broken after move/resize until refocus) — [#5246](https://github.com/DioxusLabs/dioxus/issues/5246) [API]. #5581 "Implement support for beforeinput event" (closed 2026-05-23) — [API search](https://github.com/DioxusLabs/dioxus/issues/5581).
- 0.7 blog ships 28 Radix-style primitives (accessibility goals implied, unverified) — web_family notes cover this blog; alpha.0 notes mention no a11y changes — [alpha.0 notes](https://github.com/DioxusLabs/dioxus/releases/tag/v0.8.0-alpha.0).
- Monaco screen-reader mode details already in `web_family.md`.
- Aria attributes on `svg` need string names (DIOXUS.md).
- No Dioxus a11y issue-tracker hits found for "accessibility/a11y/screen reader" in desktop searches (search returned empty).

### Inferences
- A11y quality is the webview's, so it equals Tauri's. IME bug above is likely a wry/tao message-loop interaction on Windows [inference]; verify with Tauri parity.

### Gaps
- No NVDA/VoiceOver/Orca test reports for Dioxus desktop.

## 8. Bundle size, idle RAM, startup, per-window overhead

### Takeaway
No measured Dioxus-desktop numbers found. Because it uses wry/tao and system webview like Tauri, expect Tauri-like figures [inference]. One measured Tauri baseline exists.

### Cited Findings
- [D/A] "typically under 5MB" for desktop apps (old docs 0.3/0.4/0.6) — [desktop guide](https://dioxuslabs.com/learn/0.7/guides/platforms/desktop) (vendor claim).
- [M, secondary] Better Stack: Tauri idle ~109 MB vs Electron ~128 MB; process monitor main process ~36 MB hides webview children — [Better Stack](https://betterstack.com/community/guides/scaling-nodejs/tauri-vs-electron-vs-deno-vs-electrobun/).
- [A] dioxus-native with Vello chart app 530 MB (see section 6).
- Tauri installers 16-18 MB, AppImage 94 MB — in `web_family.md`.
- Dioxus 0.8.0-alpha.2 fixes leak of removed nodes never freed in Blitz — [releases](https://api.github.com/repos/DioxusLabs/dioxus/releases?per_page=8) (via web_family). #5716 "plug remaining per-render leaks in converted component props" (open 2026-07-29) — [API search](https://github.com/DioxusLabs/dioxus/issues/5716). #5885 webview that loses its page shows blank/frozen window (open 2026-10-03) — [#5885](https://github.com/DioxusLabs/dioxus/issues/5885).
- Per-window webview overhead: not found.

### Inferences
- On Windows each Dioxus window = one WebView2 controller (shared browser process per user-data dir, extra renderer process per window) [BG]; RAM per extra window likely tens of MB; must benchmark.

### Gaps
- Everything quantitative for Dioxus desktop. Carbon must benchmark hello-world, Monaco, and 1/3/5 windows on Windows and Linux.

## 9. Licensing, backing, cadence, breaking changes, stability

### Takeaway
MIT/Apache-2.0 dual; tiny venture-backed company (Dioxus Labs, YC); 0.x with a breaking minor roughly every 9-12 months and long alpha cycles. 0.7 to 0.8 is explicitly breaking.

### Cited Findings
- Licence: GitHub shows Apache-2.0; `dioxus-desktop` README badge says MIT; repo has LICENSE-MIT — [repo API](https://api.github.com/repos/DioxusLabs/dioxus), [desktop README](https://raw.githubusercontent.com/DioxusLabs/dioxus/main/packages/desktop/README.md). Dual MIT/Apache-2.0 [BG].
- Backing: aggregators say about $500K raised, pre-seed/seed 2023, investors listed variously as Y Combinator, Eight Capital, Parcel B, Pioneer Fund, GitHub Accelerator; founder Jonathan Kelley; FutureWei not confirmed — [CB Insights](https://www.cbinsights.com/company/dioxus-labs/financials), [VCBacked](https://www.vcbacked.co/company/dioxuslabs) (aggregators disagree; unverified). The "going full time" blog URL I tried returned 404.
- Contributors: about 419 pages at per_page=1, i.e. ~419 contributors [API]; 0.7 thanks 350+ contributors — [0.7 blog](https://dioxuslabs.com/blog/release-070/).
- Release cadence (GitHub releases [API]): 0.4.0 2023-08-02; 0.5.0 2024-03-28; 0.6.0 2024-12-09; 0.7.0 tag 2025-10-31 (alpha.0 2025-05-14; rc.0 2025-08-11) — conflicts with web_family's 0.7.0 "2025-09-08" from the blog date; 0.8.0-alpha.0 2026-05-19, alpha.1 2026-07-31, alpha.2 2026-10-14 — [releases](https://api.github.com/repos/DioxusLabs/dioxus/releases?per_page=100). No 0.8 stable yet. Also 0.7.10 patch 2026-07-30 (web_family).
- 0.8 breaking changes: internal API breaks, CLI and Signals behavior tweaks, non-exhaustive props, edition 2024, wasm-bindgen-x fork (open PR), const-builder/html-trait element rewrite (PR #5554: elements no longer namespaced in `dioxus_elements`) — [alpha.0 notes](https://github.com/DioxusLabs/dioxus/releases/tag/v0.8.0-alpha.0), [PR #5554](https://github.com/DioxusLabs/dioxus/pull/5554). 0.7 migration guide exists at /learn/0.7/migration/to_07.
- Open issue volume: 832 open issues (web_family); one anecdotal developer chose Tauri over Dioxus citing unanswered issues — [Lemmy comment/search summary](https://lemmy.linuxuserspace.show/comment/1340610) [A].
- Dioxus docs 0.4 say "no longer maintained"; Dioxus README: "not fully mature yet; APIs still shifting" appears on a mirror — [Gitee mirror](https://gitee.com/nur/dioxus) (non-official).

### Inferences
- A long-lived project must pin a minor version and budget a migration per release; desktop-only usage dodges fullstack churn but not rsx/signal/element-type churn. Bus-factor: one founder/company, funding unclear.

### Gaps
- No primary source on Dioxus Labs current funding or team size (Jan 2026 or later); FutureWei claim unverifiable.

## 10. Extension/plugin story (user-built UI modules at runtime)

### Takeaway
Dioxus has no dynamic UI-plugin system: components are compiled Rust. Runtime extensibility must come from webview-side mechanisms (sandboxed iframes, JS/WASM loaded into the page), out-of-process subprocess/RPC, or scripting; no Dioxus precedent found.

### Cited Findings
- Dioxus 0.8 PR #5554 adds user-defined HTML elements and portals/multi-renderer routing (compile-time) — [PR #5554](https://github.com/DioxusLabs/dioxus/pull/5554).
- Issue #1454 "How to handle Tauri sidecars and externalBin" (open) shows sidecar-style needs unmet — [API list](https://api.github.com/repos/DioxusLabs/dioxus/issues?milestone=14).
- Custom protocol handlers and `with_navigation_handler` let you serve extension assets from your own scheme and gate navigation — config.rs [D].
- Tauri precedent: plugin drivers as subprocesses speaking JSON-RPC over stdio — `web_family.md` (HackerNoon).
- No example of Dioxus loading WASM components at runtime found.

### Inferences
- Viable Carbon design (any webview shell): extension viewers as sandboxed `<iframe sandbox>` pages served from a custom protocol with postMessage bridging to the Rust host, or WASM (wasmtime component model in the Rust host, with a defined UI protocol) rendering to an SVG/DOM description. Dioxus gives no extra help; wasm-bindgen-on-desktop (#5194) is about Dioxus code sharing, not plugins.

### Gaps
- No sourced examples of runtime-loaded UI modules in Dioxus.

## 11. Linux and Windows platform quirks

### Takeaway
Linux (WebKitGTK/Wayland/NVIDIA) is the weakest platform, with multiple open Dioxus issues; Windows needs WebView2 distribution choices handled by `dx bundle`.

### Cited Findings
- #4505 "Lots of different failing issues under Wayland" (open): dies with Gdk "Error 71 (Protocol error)"; works with `WAYLAND_DISPLAY=""`; GTK init panic when launched from VS Code on Wayland — [#4505](https://github.com/DioxusLabs/dioxus/issues/4505) [API].
- #3845 "Desktop Build Opens Empty Window" (open); #3453 AcceleratedSurfaceDMABuf framebuffer failure on X11/openSUSE (open); #3427 NixOS WebKitGTK small text; #3354 SIGSEGV on WSL2; #5805 gdk-3.0 compile issue (2026-09-02) — [API results](https://github.com/DioxusLabs/dioxus/issues/3845).
- Config knob `with_disable_dma_buf_on_wayland(bool)` exists — config.rs [D]. 0.8.0-alpha.2: libxdo no longer linked by default on Linux — web_family.
- #3100 file drop fails on Wayland (open); #3439 file dialog problems on Linux (open) — links above. #5037 getUserMedia fails on Linux (no wry permission handler).
- Windows: WebView2 Evergreen is on Windows 11 and updated Windows 10 [BG]; Dioxus bundler offers five install modes (section 5); #3154 command line unresponsive after hibernation (open); `with_windows_browser_args` passes Chromium flags — config.rs [D].
- macOS uses WKWebView; 0.8 ports macOS/iOS code to objc2 (#5101); wry bumped to 0.54.4 (#5544) — alpha.0 notes.

### Inferences
- Same WebKitGTK QA burden as Tauri, plus Dioxus-specific workarounds; Carbon should test Wayland/X11/NVIDIA and set DMA-BUF/compositing env workarounds [BG on WEBKIT_DISABLE_* variables].

### Gaps
- No NVIDIA-specific Dioxus reports beyond #3845/#5191.

## 12. Community complaints, limits, and Dioxus vs Tauri + JS frontend for this IDE

### Takeaway
Dioxus desktop = Tauri's wry/tao with a Rust-written UI and no IPC for UI logic, but weaker shell features, immature multi-window, and 0.x churn. Tauri + JS is the safer base for Monaco/xterm.js/React Flow-class components; Dioxus is attractive if Carbon wants all-Rust UI and accepts JS only for editor/terminal/canvas panes.

### Cited Findings
- Dioxus claim: Tauri confines the UI to JS/WASM and requires IPC, Dioxus runs Rust natively with no bridge — Dioxus README comparison, quoted by [Lemmy thread](https://lemmy.linuxuserspace.show/comment/1340610) (promotional).
- Dioxus shares tao/wry with Tauri — same source. Dioxus main-branch dependency: wry 0.54.4 (alpha.0 notes).
- Complaints, issue-level: eval reliability (#4590, #5919), undecorated resize (#3128), Wayland (#4505), IME (#5246), multi-window state (#5160/#1569), `dx` toolchain (glibc, run-from-root, DIOXUS.md), Windows `aws-lc-sys` install failure for 0.6 CLI (#3309), 832 open issues (web_family).
- Tauri's side (from web_family): IPC serialisation cost, webview divergence, Linux packaging, large ecosystem (112k stars vs Dioxus 39k), updater plugin, capabilities security model.
- Missing from Dioxus vs Tauri in sources checked: first-party updater, dialog/fs/shell plugins, capability-based IPC permissions, sidecar (#1454), documented Linux packaging (Flatpak/snap) [BG for Tauri features].
- Strengths of Dioxus: hooks for muda menus, tray, global shortcuts; Subsecond hot patching of Rust UI (default-on in 0.8); `with_as_child_window` for wgpu; headless-root multi-window on main; one-language UI.

### Inferences
- For Carbon (editor, terminal, canvas, 3D, SVG are JS-ecosystem-heavy; sandboxed extension viewers), most UI value is inside JS widgets, so Dioxus's Rust-UI advantage applies mainly to chrome (explorer, chat panes, settings). Tauri's capability model, updater, and larger community outweigh that unless the team strongly prefers Rust-only chrome. If Dioxus is chosen, isolate it behind a thin shell so wry/tao could be swapped to Tauri and keep all state in the Rust daemon, since cross-window signals are in flux.

### Gaps
- No Reddit/HN/Discord threads were retrievable (search tool returned docs and aggregators only); no production IDE-scale Dioxus desktop app identified (known examples in repo: file-explorer, hotdog, calculator).
- No direct apples-to-apples Dioxus vs Tauri benchmark exists in sources.
