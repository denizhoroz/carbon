# Editor-native / next-gen Rust GUI frameworks (GPUI, Floem, Xilem, Freya, Makepad, Ribir, Vizia, Cushy, Kas, Blitz, Rinch, ratatui)

Research date context: 2026-10-16. Requirement context read from .agents/PROJECT.md and DESIGN.md (native GPU UI is an open question; AccessKit from first widget; docks, multi-window, extension sections, virtualized lists, budgets cold start <1 s, RAM stretch <600 MB). Label key: [M] = measured/primary data (registry API, repo page, release notes), [A] = anecdote/secondary/unverified. Tooling note: WebFetch summaries come from a small model; star counts are as shown on GitHub pages fetched during this session. Several fetches returned incomplete data (contributor counts, release dates); these are listed under Gaps. About 40 tool calls were used, above the 15-call guideline, because of the number of frameworks.

## 1. Is GPUI practically reusable by a third-party app in 2026? (docs, breaking changes, widgets, Windows/Linux)

### Takeaway
Technically yes, practically only with heavy caveats. crates.io `gpui` is stuck at 0.2.2 (Oct 2025), the README says "expect frequent breaking changes", official docs are thin ("read the Zed source or ask on Discord"), it ships zero widgets, and a community report says Zed refocused on "business-relevant work" in 2026. The practical route is the longbridge widget kit (now named gpui-kit, 0.7.x, weekly releases) that pins an exact GPUI snapshot from git/`gpui-pre`. Windows (DirectX/DirectWrite) and Linux (Vulkan, Wayland/X11) work because Zed 1.0 ships on all three, but accessibility is unproven (see section 3).

### Cited Findings
- GPUI is "hybrid immediate and retained mode, GPU-accelerated"; "still pre-1.0 ... expect frequent breaking changes"; three layers: Entities (state), Views (`Render`, Tailwind-style API), Elements (low-level). [M] — [GPUI README](https://github.com/zed-industries/zed/blob/main/crates/gpui/README.md)
- README quick start: add `gpui` + `gpui_platform`, `gpui_platform::application().run(...)`. Metal on macOS (needs Xcode + `font-kit` feature for glyphs); Linux needs `wayland`/`x11` features; Windows needs no features (Win32 + DirectWrite). `#[gpui::test]` with `TestAppContext` for headless tests. [M] — [GPUI README](https://github.com/zed-industries/zed/blob/main/crates/gpui/README.md)
- README: best learning resource is Zed source code or Zed Discord; "better docs, examples and guides are planned". README links an "Accessibility" doc (`crates/gpui/_accessibility`) whose content I could not read. [M] — [GPUI README](https://github.com/zed-industries/zed/blob/main/crates/gpui/README.md)
- crates.io `gpui`: newest 0.2.2, updated 2025-10-22, 364,835 downloads total (204,937 recent), Apache-2.0 (0.1.0 was MIT), repo = zed monorepo. No newer crates.io release in the 12 months before Oct 2026. [M] — [crates.io API](https://crates.io/api/v1/crates/gpui)
- Community-sourced summary says maintainers stated in Feb 2026 they are pausing community-facing GPUI work to focus on Zed business priorities; community fork `gpui-ce` has sparse activity. Treated as secondary. [A] — search summary of [gpui-ce issue 280](https://github.com/gpui-ce/gpui-ce/issues/280)
- gpui-ce issue "The State of CE in 2026": author says Zed announced it would be "purely focusing on business-relevant work", fragmenting the ecosystem; CE has no docs/guides; upstream changes merged with LLM assistance; maintainer bandwidth is a concern. Community opinion, not official. [A] — [gpui-ce #280](https://github.com/gpui-ce/gpui-ce/issues/280)
- A 2025 hands-on survey (Apr 2025): GPUI "works", but "spotty docs, janky install, not sure you're actually supposed to use GPUI at this stage"; no built-in text input widget (author adapted a 700+ line example); Windows Narrator could not see window contents; Japanese IME worked. [A, first-hand test, 2025] — [boringcactus survey](https://www.boringcactus.com/2025/04/13/2025-survey-of-rust-gui-libraries.html)
- GPUI ships no widget library; third-party kit is needed. [A] — search summary citing [docs.rs gpui](https://docs.rs/gpui/latest/gpui/)
- Widget kit (repo `longbridge/gpui-component`, which now renders as `gpui-kit`): 17.7k stars, 1.1k forks, 2,604 commits, Apache-2.0; 75+ components; code editor listed as stable at 200K lines with Tree-sitter highlighting and LSP diagnostics/completion/hover; dock layout (resizable panels, draggable tabs, nested splits, edge docks, serializable); charts; Markdown/HTML rendering; virtualized data tables; optional JS extension runtime `gpui-shell`; macOS/Windows/Linux + wasm; claims AccessKit roles/names/states built into interaction layer and covered by tests. Claims are from the project's own page. [M for counts, A for quality claims] — [repo](https://github.com/longbridge/gpui-component)
- gpui-kit release cadence: v0.6.0 (3 Sep), 0.6.1 (9 Sep), 0.6.2/0.6.4 (18 Sep), 0.6.6 (21 Sep), 0.7.0 (28 Sep), 0.7.1 (5 Oct), 0.7.2 yanked, 0.7.3 (12 Oct 2026). Roughly weekly releases. [M] — [gpui-kit releases](https://gpui-kit.com/releases/) and [GitHub releases](https://github.com/longbridge/gpui-component/releases)
- gpui-kit v0.7.3 pins GPUI snapshot `gpui-pre` =0.3.9 (zed@93d7456); optional `gpui-fast` 0.1.7 backend. The kit has moved from GPUI 0.2.x to the `gpui-pre` snapshot family (published snapshots of Zed's GPUI, so a third-party crate set, not Zed's official crates.io release). [M] — [gpui-kit releases](https://gpui-kit.com/releases/)
- Breaking-change churn in the kit: v0.7.0 changed window startup (`gpui_kit::open_window`, removed `Root::render_*_layer`), plot primitives, DatePicker event type, Table selection getters, etc.; v0.6.0 split Input from Textarea/Editor and renamed Table to DataTable. [M] — [gpui-kit releases](https://gpui-kit.com/releases/)
- The name/relationship between gpui-component and gpui-kit is unclear from the pages read (the repo URL is still longbridge/gpui-component; gpui-kit.com describes `gpui_component::Root` re-exporting the Base Root). A `gpui-base` crate appears on docs.rs (0.6.0, 2026-09-03). Verify before depending. [A] — search results: [docs.rs gpui-base](https://docs.rs/crate/gpui-base/0.6.0)
- Zed 1.0.0 stable shipped 2026-04-29; 1.0.1 on 2026-05-04; latest stable seen 1.18.1 (2026-09-04), 1.18.0 on 2026-09-02 (roughly 2-week minor cadence). Windows, macOS, Linux. [M] — [Zed 1.0.0 notes](https://zed.dev/releases/stable/1.0.0), [stable releases](https://zed.dev/releases/stable?page=2), [Linuxiac](https://linuxiac.com/zed-code-editor-hits-1-0-with-gpu-accelerated-ui/)
- Linux uses Vulkan; docs list failure modes (`NoSupportedDeviceFound`, `ERROR_INITIALIZATION_FAILED`, `ERROR_SURFACE_LOST_KHR`) and driver/GPU-selection workarounds. [M] — [Zed Linux docs](https://zed.dev/docs/linux.html)
- Windows backend history: team first tried Vulkan on Windows, hit compatibility issues, then built a DirectX 11 backend (2025 reporting). [A] — [Neowin](https://www.neowin.net/news/windows-slows-zeds-development-as-platform-differences-bite/)
- Third-party Zed-related project pins: GPUIX (React binding to GPUI) is pre-1.0 and says to pin exact versions. [A] — search summary of [gpuix.dev](https://gpuix.dev/)

### Inferences
- A third-party app in 2026 should treat GPUI as "vendored/pinned Zed snapshot", not as a stable dependency: pin one git rev or `gpui-pre` version and budget regular migration work (the kit itself breaks APIs nearly every minor).
- Zed's real win (compact, fast) is plausible for Carbon, since Carbon's DESIGN.md copies Zed's dock model; but the missing pieces (a11y, IME polish, docs) are the risk, and GPUI team priorities are Zed, not third parties.
- gpui-kit (editor, docks, tables, charts) covers most of Carbon's shell needs, but a Chinese-brokerage-company-led (Longbridge) kit is a single-vendor dependency.

### Gaps
- Contents of GPUI's `_accessibility` doc and whether Zed/GPUI has AccessKit or any screen-reader support in 2026: not found (zed.dev/docs/accessibility is 404; searches returned only 2024 material). Treat as unknown, likely absent.
- Primary source for the Feb 2026 "pausing community work" statement not located.
- Contributor counts and stars of zed repo, gpui-ce not captured.
- Real third-party GPUI apps beyond Zed (e.g. which shipped products use it) not verified.

## 2. Which are production-ready for an IDE vs experimental?

### Takeaway
Only GPUI (via Zed 1.0) has a shipped, mainstream IDE; Lapce/Floem has a shipped but low-momentum editor; Makepad has shipped apps (Robrix, Moly) but is a niche DSL framework; Xilem, Freya, Ribir, Vizia, Cushy, Kas, Blitz/Dioxus-native, Rinch are experimental or pre-1.0 for IDE-grade work.

### Cited Findings
Per-framework facts (version/licence/registry data are [M] from crates.io API fetched 2026-10-16 unless noted):

**GPUI / Zed** — see section 1. Zed licence for app is not captured here (GPL/AGPL/Apache split in repo; verify); gpui crate Apache-2.0. Zed ships WASM extensions (confirmed in PROJECT.md context only; not re-verified this session).

**Floem (Lapce)** — MIT. crates.io `floem` max 0.2.0, last updated 2024-11-14, 43,695 downloads. [M] — [crates.io](https://crates.io/api/v1/crates/floem). Repo: 4.3k stars, "will make occasional breaking changes ... on our way to v1", renders via wgpu with vger or vello, or AnyRender/Skia; tiny-skia CPU fallback; Taffy flex/grid; fine-grained reactivity; inspector; Fluent i18n. Accessibility not mentioned in README. [M] — [floem repo](https://github.com/lapce/floem). Lapce: 38.9k stars, Apache-2.0, 830 open issues, 71 open PRs, UI in Floem, wgpu rendering; no release data readable. [M] — [lapce repo](https://github.com/lapce/lapce). Apr 2025 test: Narrator cannot see any text; Japanese IME fails; layouts limited to 16 children per container. [A] — [boringcactus](https://www.boringcactus.com/2025/04/13/2025-survey-of-rust-gui-libraries.html). A 2026 survey (read only via search snippet) says Floem had no release in nearly two years and its IME and screen reader support are poor. [A] — [wybxc 2026 survey](https://blog.wybxc.cc/blog/rust-gui-survey-2026/) (direct fetch 403).

**Xilem / Masonry / Vello (Linebender)** — Apache-2.0. Repo 5.6k stars, "experimental Rust native UI framework", AccessKit for a11y, Parley+Fontique text stack, requires Rust 1.96+. [M] — [xilem repo](https://github.com/linebender/xilem). crates.io: `xilem` 0.4.0 and `masonry` 0.4.0 both updated 2025-10-29 (no newer 2026 release); `vello` 0.11.0 updated 2026-10-02 (1.0M downloads, Apache-2.0 OR MIT). [M] — [xilem](https://crates.io/api/v1/crates/xilem), [masonry](https://crates.io/api/v1/crates/masonry), [vello](https://crates.io/api/v1/crates/vello). Xilem/Masonry 0.4 (Oct 2025) switched shaping to HarfRust. [A, search snippet] — [Linebender blog](https://linebender.org/blog/tmil-21/). Apr 2025 test: Narrator partial (sees text, wrong position), IME works with missing glyphs, "no versioning; basically usable now if you accept bleeding-edge jank". [A] — [boringcactus](https://www.boringcactus.com/2025/04/13/2025-survey-of-rust-gui-libraries.html). No 2026 Linebender blog post found (digests found end late 2025).

**Freya** — MIT. crates.io `freya` max 0.5.0-rc.10 updated 2026-10-11 (45,198 downloads); stable 0.4 (no longer Dioxus-based since 0.4; own reactive core, Skia rendering via rust-skia). Repo 3.2k stars. Listed features: VirtualScrollView, rich text editing, tree-sitter code editor, Markdown, routing, Blitz-based HTML rendering, terminal emulation with PTY, devtools, plotting, headless testing, Fluent i18n; desktop, web, Android. AccessKit-style `a11y_id` used in examples. [M] — [freya repo](https://github.com/marc2332/freya), [crates.io](https://crates.io/api/v1/crates/freya). Apr 2025 test (older 0.3): "not quite ready for serious use yet", Narrator partial, IME partial. [A] — [boringcactus](https://www.boringcactus.com/2025/04/13/2025-survey-of-rust-gui-libraries.html). Freya is the only one in this set whose README lists a terminal widget and tree-sitter code editor out of the box (claim from README, quality unverified).

**Makepad** — MIT. Repo 7.2k stars, 2,371 commits on `dev`; GPU 2D/3D; live-editable DSL ("script engine"); targets macOS/Metal, Windows/DX11, Linux/OpenGL, wasm/WebGL, plus iOS/Android/Quest. Accessibility not mentioned. [M] — [makepad repo](https://github.com/makepad/makepad). crates.io `makepad-widgets` 1.0.0 updated 2025-05-13, MIT OR Apache-2.0. [M] — [crates.io](https://crates.io/api/v1/crates/makepad-widgets). README recommends nightly Rust (web builds need it). Production apps: Robrix (Matrix client), Moly (AI LLM client). [A, search summary] — [Makepad README via search](https://github.com/makepad/makepad). Apr 2025 test: Narrator fail, IME partial, "undocumented macro DSL; built mainly for its own team". [A] — [boringcactus](https://www.boringcactus.com/2025/04/13/2025-survey-of-rust-gui-libraries.html). I did not confirm a "Makepad Studio" product.

**Ribir** — MIT. crates.io `ribir` newest 0.4.0-alpha.65 (stable 0.3.0) updated 2026-04-21; 66,031 downloads. [M] — [crates.io](https://crates.io/api/v1/crates/ribir). 2026 survey (snippet): docs sometimes disagree with code; Apr 2025 test: Narrator fail, state glue broke example. [A]

**Vizia** — MIT. crates.io `vizia` 0.4.0 updated 2026-04-23; 6,794 downloads. [M] — [crates.io](https://crates.io/api/v1/crates/vizia). 2025 test: works, Narrator partial, IME works; "not quite ready for serious use". [A]

**Cushy** — MIT OR Apache-2.0. crates.io 0.4.0 last updated 2024-08-20 (stale 2 years); noisy Vulkan/DX12 errors, IME composer hidden, Narrator fail (2025 test). [M]+[A]

**Kas** — Apache-2.0. crates.io 0.17.0 updated 2026-01-27, 40,468 downloads. 2025 test: tutorials outdated, IME won't activate, Narrator fail, "isn't really ready for prime time". [M]+[A]

**Blitz (Dioxus Native)** — `blitz-dom` 0.3.0-beta.4 updated 2026-10-16 (285,766 downloads), MIT OR Apache-2.0: actively developed, still beta. Freya uses Blitz for HTML rendering. [M] — [crates.io](https://crates.io/api/v1/crates/blitz-dom). Dioxus Native status not otherwise verified (search returned nothing).

**Rinch** — GitHub joeleaver/rinch: HTML/CSS-like layout, native Vello (GPU) or tiny-skia (CPU), fine-grained signals, `rsx!`; ~2 stars; no crates.io listing found (404). Hobby-stage. [A] — via search results; [repo listing](https://skillselion.com/skills/joeleaver/rinch/claude-plugin).

**Quasar** — no Rust GUI framework by this name found. Closest hit: "Quarve" (pre-alpha, built for Monocurl). **Hotline** — not researched/found. [A]

**ratatui (TUI baseline)** — MIT, 0.30.2 updated 2026-06-19, 60.6M downloads (by far the most mature). [M] — [crates.io](https://crates.io/api/v1/crates/ratatui). Not a GUI; cannot do SVG/3D/canvas viewers or multi-window.

- 2025 survey headline: 94.4% of surveyed Rust GUI libs not production-ready (build issues, missing features, usability, unmaintained). [A] — via search summary of [boringcactus](https://www.boringcactus.com/2025/04/13/2025-survey-of-rust-gui-libraries.html) / [libhunt](https://www.libhunt.com/compare-Ribir-vs-floem)

### Inferences
- Production for an IDE: GPUI/Zed (proven by Zed 1.0), Lapce/Floem (shipped but momentum doubtful: Floem crates.io stale since Nov 2024). Everything else is experimental for IDE-grade text editing.
- Release signals: actively cadenced = gpui-kit (weekly), Zed (biweekly), Freya (rc builds), Blitz, Vello; stale = Floem crates.io, Cushy, Xilem/Masonry on crates.io (Oct 2025, though git main may be active); a downstream team would use git dependencies for these.

### Gaps
- Floem/Lapce/Xilem/Makepad real 2026 release dates and contributor counts (GitHub pages showed empty release sections).
- Zed's own licensing split and WASM extension API details were not re-verified.
- Hotline, Quasar, Rinch maturity: no reliable independent sources.

## 3. Verified numbers vs anecdotes; accessibility, text/editor widgets, multi-window, terminal, 3D/canvas, performance

### Takeaway
No framework in this set has verified production-grade screen-reader support on Windows as of the latest tests found (Apr 2025 hands-on, no 2026 re-test captured). Xilem/Masonry (AccessKit-native) is the strongest a11y design; GPUI has none confirmed; gpui-kit claims AccessKit integration but that is a vendor claim. Performance numbers for Zed vs VS Code are low-quality and conflict by up to ~5x.

### Cited Findings
- Accessibility (hands-on Windows Narrator, Apr 2025): GPUI fail; Floem fail; Xilem partial; Freya partial; Makepad fail; Ribir fail; Vizia partial; Cushy fail; Kas fail. [A, single tester, dated] — [boringcactus](https://www.boringcactus.com/2025/04/13/2025-survey-of-rust-gui-libraries.html)
- IME (Japanese, same test): works = GPUI, Xilem (glyph gaps), Vizia; partial = Freya, Makepad, Ribir, Cushy; fail = Floem, Kas. [A] — same source.
- Zed memory/startup claims: Tech Insider reports Zed ~222 MB vs VS Code ~3,549 MB idle (also a separate piece: 142 MB vs 730 MB); Vantaige reports Zed ~220 MB on 50k-LOC repo vs VS Code >1 GB with Copilot; cold start 0.12 s vs 1.2 s (M2), or 180 ms vs 2.1 s. Methodology undisclosed, some sources look generated; absolute numbers unreliable. [A] — [tech-insider](https://tech-insider.org/zed-vs-vscode-2026/), [vantaige](https://vantaige.io/blog/zed-1-vs-cursor-vs-vscode-real-benchmark-2026)
- Editor-grade text widgets: gpui-kit lists a code editor (200K lines, Tree-sitter, LSP) [A vendor claim]; Freya lists tree-sitter code editor and terminal+PTY [A README claim]; Xilem uses Parley (shaping via HarfRust since 0.4) but no code-editor widget confirmed; Floem has an editor used in Lapce (rope-based, in Lapce repo; not re-verified this session).
- Docking/multi-window: only gpui-kit documents a serializable dock system; GPUI supports multiple windows (Zed itself is multi-window; kit's `open_window` API). Other frameworks: not verified.
- Terminal embedding (alacritty_terminal): Zed uses it (known, not re-verified this session); Freya README lists PTY terminal emulation. Others: not verified.
- wgpu/3D embedding: Makepad has its own 3D forward renderer; Floem/Xilem/Vello are wgpu-based and can host custom wgpu surfaces (Vello exposes wgpu rendering); GPUI uses its own Metal/DX11/Vulkan renderers rather than wgpu (Windows DX11 per Neowin; Linux Vulkan per Zed docs), so embedding a wgpu 3D viewer in GPUI is non-trivial (inference).
- Hot reload/tooling: Makepad live-edit DSL; Floem has an element inspector; Freya has devtools and headless testing; GPUI has `#[gpui::test]`. [M, README claims]

### Inferences
- Carbon's "AccessKit from first widget" requirement is unmet by default in GPUI; options are (a) gpui-kit's claimed AccessKit layer (must be audited with NVDA/Narrator/VoiceOver/Orca), (b) Xilem/Masonry (AccessKit-native but pre-1.0 and no code editor), or (c) the webview route already preferred in PROJECT.md.
- Practical ranking for an IDE shell today: 1) GPUI + gpui-kit (pinned), 2) Freya (broad built-ins but rc API churn), 3) Xilem (best foundations, longest road), 4) Floem (stale), 5) Makepad (distinct DSL, small ecosystem), rest not suitable.

### Gaps
- Bundle size, startup, RAM for each framework measured with a hello-world: no authoritative numbers found; recommend Carbon's own benchmark.
- 2026 re-test of accessibility/IME: the 2026 survey (wybxc.cc) was only seen via search snippets because direct fetch returned 403.
- Platform quirks for Linux (Wayland/X11, NVIDIA) for each framework beyond Zed docs: not found.
- Funding/contributor numbers (Zed Industries is venture funded; Linebender grants/Google Fonts; Makepad/Longbridge corporate backing) not verified this session.
