# Own-renderer Rust GUI frameworks: Slint, Iced, egui, libcosmic, Floem, Relm4/Qt (as of 2026-10-16)

Scope note: research budget was ~20 tool calls. Fetches were summarised by a small model, so treat details as "per fetched page". Much of the requested detail (bundle size, startup, IME per framework, Slint LSP/designer, Relm4, cxx-qt) was NOT found and is listed under Gaps. Evidence labels: [doc] official page, [M] measured, [A] anecdote.

## 1. Versions, cadence, licence, backing, maturity

### Takeaway
All three main frameworks are actively released as of Oct 2026. Slint is the only one with a company-run, dual-licensed (GPLv3 / royalty-free with disclosure / commercial) model and a stable-ish 1.x API. Iced (0.14, 0.x) and egui (0.37) still make breaking changes. Floem is pre-1.0 and described as maturing.

### Cited Findings
- Slint latest 1.18.1 (21 Sep 2026); 1.18.0 16 Sep 2026; 1.17.0 24 Jun; 1.16.0 16 Apr; 1.15.0 4 Feb 2026; 1.14.0 21 Oct 2025. Minor release roughly every 8-12 weeks, patch within 1-3 weeks. [doc] — [Slint releases](https://github.com/slint-ui/slint/releases)
- Slint GitHub: 24.2k stars, 1.0k forks; maintained by SixtyFPS GmbH (remote team in Germany, Finland, US). Targets embedded, desktop, mobile, web (WASM). Bindings: Rust, C++, JS/Node (Beta), Python (Beta). [doc] — [slint repo](https://github.com/slint-ui/slint)
- Slint licence: choice of three. (1) Royalty-Free License for proprietary desktop, mobile, web apps, free but requires disclosure that you use Slint (e.g. `AboutSlint` widget or Slint badge), otherwise Commercial licence is needed; intended for apps installed on a general-purpose computer/phone as one app among many; embedded systems excluded. (2) GPLv3 for open source incl. embedded. (3) Commercial for proprietary incl. embedded. [doc] — [LICENSE.md](https://github.com/slint-ui/slint/blob/master/LICENSE.md)
- Slint pricing page: plans Royalty-Free, Enterprise, Small Enterprise (up to 50 employees, up to 10M EUR turnover), Startup & Individual (up to 10 employees, up to 2M EUR, under 5 years). Embedded royalty free for non-commercial/personal/OSS; commercial embedded one-time per-device royalty from $1.00 with volume discounts. Perpetual Fallback License after 12 consecutive months of payments. Anyone designing/developing/testing the app needs a licence. Page lists "attribution optional / no source obligation" in a comparison table without saying which plan (conflicts in nuance with the disclosure condition above; read the full T&Cs). [doc] — [slint.dev/pricing](https://slint.dev/pricing)
- Iced latest 0.14.0 (7 Dec, year not shown, presumably 2025); earlier 0.13.0 (18 Sep), 0.12.0 (15 Feb), 0.10.0 (28 Jul), irregular, roughly 1-2 minors per year. No 0.15 seen as of fetch. [doc] — [iced releases](https://github.com/iced-rs/iced/releases)
- Iced repo: 31.8k stars, 1.7k forks; development sponsored by the Cryptowatch team at Kraken.com. [doc] — [iced repo](https://github.com/iced-rs/iced)
- egui latest 0.37.0 (15 Oct 2026); 0.36.0 5 Aug; 0.35.0 25 Jun; 0.34.0 26 Mar 2026; frequent patch releases; roughly a minor every 2-3 months. Interfaces still changing with breaking changes each release. [doc] — [egui releases](https://github.com/emilk/egui/releases), [egui repo](https://github.com/emilk/egui)
- egui repo: ~31.5k stars, 933 open issues; sponsored by Rerun (Rerun Viewer is the flagship app). Dual MIT/Apache (repo licence not shown on fetched page; widely known, unverified here). — [egui repo](https://github.com/emilk/egui)
- Floem: 4.3k stars, pre-1.0 ("occasional breaking changes ... on our way to v1"), under the lapce org; renders via wgpu with vger or vello, optional Skia via AnyRender, tiny-skia CPU fallback. [doc] — [floem repo](https://github.com/lapce/floem)
- cosmic-edit: 349 stars, GPL-3.0, README says incomplete pre-alpha. [doc] — [cosmic-edit](https://github.com/pop-os/cosmic-edit)

### Inferences
- Slint's licence is workable for a proprietary Carbon only with a visible Slint disclosure; if Carbon is open source, GPLv3 applies and contaminates the app licence. Carbon's licence stance (PROJECT.md open question) must be decided first.
- Slint has the most predictable release cadence and 1.x stability; iced and egui require budgeting for upgrade churn.

### Gaps
- Contributor counts for each repo not captured. Iced 0.14 year and any newer iced release not confirmed. libcosmic version/cadence and Relm4/gtk4-rs, cxx-qt, qmetaobject were not researched.

## 2. Architecture, rendering backends, DSL

### Takeaway
Slint: declarative `.slint` DSL compiled to Rust/C++, with femtovg (OpenGL ES 2), Skia, software, and (new in 1.18) experimental Vello renderer. Iced: Elm-architecture retained-ish, wgpu (Vulkan/Metal/DX12) plus tiny-skia software fallback. egui: immediate mode, glow (OpenGL) or wgpu backends via eframe.

### Cited Findings
- Slint backends: femtovg (OpenGL ES 2.0), skia, software (CPU, no deps); `qt` style when Qt installed. [doc] — [slint repo](https://github.com/slint-ui/slint)
- Slint 1.18 (16 Sep 2026): experimental Vello-based renderer behind `renderer-vello` feature; leaner generated code, faster compile, smaller binaries; much faster rendering/editing of large text; screen readers can see text-input content and selection; `WindowMoveArea` for custom title bars with `no-frame: true`; LSP rename propagates to Rust/C++ accessors; live preview exposes `Platform.uses-mock-data`. [doc] — [Slint 1.18 blog](https://slint.dev/blog/slint-1.18-released)
- Slint March 2026 blog: "Deprecating Native-Looking Styles" (change of default style); title only seen. — [slint blog](https://slint.dev/blog)
- Iced: renderers `iced_wgpu` (Vulkan, Metal, DX12) and `iced_tiny_skia` (software fallback); type-safe reactive Elm-style model; async actions; debug tooling with time travel and perf metrics; custom widgets; modular runtime/windowing shell. [doc] — [iced repo](https://github.com/iced-rs/iced)
- Iced multi-window added in 0.12.0 (PR #1964). [doc] — [iced releases](https://github.com/iced-rs/iced/releases)
- egui: eframe official framework for web, Linux, Mac, Windows, Android; backends egui_glow and egui-wgpu; winit integration; third-party bevy_egui. Immediate-mode limits: one-frame layout lag, very large scroll areas costly because content is laid out every frame, explicit IDs for persistent state. [doc] — [egui repo](https://github.com/emilk/egui)
- egui 0.34 (26 Mar 2026): font rendering moved to Skrifa with hinting/variations; `App::update` replaced by `ui`; unified `Panel`; viewports get `&mut Ui`. 0.35: `egui_inspection` protocol and `egui_mcp` server, CSS-like classes on Ui. 0.37: coloured emoji, `CompletionPopup`, experimental theme/`StyleProvider` system. [doc] — [egui releases](https://github.com/emilk/egui/releases)

### Inferences
- For Carbon's docking shell + canvas editor, none of the three ships a docking system in the fetched sources; docking would be custom or via third-party crates (not verified).
- Embedding 3D viewer: iced_wgpu and egui-wgpu share wgpu, so custom wgpu paint is plausible; not verified from sources here.

### Gaps
- Docking/panel libraries (egui_dock, etc.), native menus/tray/dialogs, wgpu embedding specifics, Slint multi-window and menu support per-version: not found.

## 3. Accessibility and IME (2026)

### Takeaway
egui has the most concrete, active AccessKit integration. Slint has a11y properties and improving screen-reader text-input support. Iced has no confirmed AccessKit support (tracking issue open since 2020). libcosmic a11y status is unclear.

### Cited Findings
- egui: AccessKit always a dependency since 0.34 (feature removed), AccessKit 0.24; 0.37 replaced `WidgetType` by AccessKit `Role`, TextEdit supports AccessKit `SetValue`/`ReplaceSelectedText`, panel resize handles get AccessKit support, kittest fails when input widgets lack accessible names. README: AccessKit native on Windows and macOS (default in eframe); web has experimental screen reader, AccessKit not yet on web. [doc] — [egui releases](https://github.com/emilk/egui/releases), [egui repo](https://github.com/emilk/egui)
- egui IME: 0.36 improved IME/autocomplete on iOS/Android web; 0.35 better IME composition visuals; contributor credit for mobile web input and IME. [doc] — [egui releases](https://github.com/emilk/egui/releases)
- Slint: 1.6 (May 2024) announced enhanced accessibility; 1.18 lets screen readers see text input content and selection. [doc] — [Slint 1.6](https://slint.dev/blog/slint-1.6-released), [Slint 1.18](https://slint.dev/blog/slint-1.18-released)
- Iced: tracking issue #552 "Implement accessibility support" opened Oct 2020, labelled help wanted, appears still open; no release note or PR found confirming AccessKit. [doc/mirror] — [iced #552 mirror](https://git.tdem.in/iced-rs/iced/issues/552)
- Iced 0.14.0 "Added" list includes input method support (PR #2777). [doc] — [iced 0.14.0](https://github.com/iced-rs/iced/releases/tag/0.14.0)
- libcosmic: Nov 2024 commit "feat: better a11y support" and an iced fix "a11y_nodes fix"; a May 2024 snapshot removed an accessibility feature from docs "until it works"; no AccessKit mention for 2026 found. [mirror, weak] — [libcosmic mirror](https://git.tfcconnection.org/chris/libcosmic/src/testing-libcosmic-pr1135/)
- Floem: no accessibility info found on README. — [floem](https://github.com/lapce/floem)

### Inferences
- DESIGN.md demands AccessKit from the first widget: egui satisfies this best of the three; Slint partially (needs verification of Linux/AT-SPI and focus behaviour); iced fails today.

### Gaps
- CJK IME quality on desktop (Windows/macOS/Linux) per framework; Slint IME status; libcosmic/iced fork a11y actual state. Needs hands-on test.

## 4. Text editing widgets and real editors

### Takeaway
No framework in this family ships a code-editor-grade widget. Iced has `text_editor` (+ `iced_highlighter`), used for basic editing; real editor-grade work is done in custom stacks (Lapce/Floem, cosmic-edit on cosmic-text). Slint's TextEdit is basic. egui TextEdit is multi-line but layout-per-frame limits large files.

### Cited Findings
- Iced `TextEditor` introduced 0.12.0; 0.13 added custom key bindings, text_size/line_height, Ctrl+A; 0.14 added LineEnding, indent/unindent, id, expandable word selection on double click, IME, more `iced_highlighter` grammars, `move_to`. [doc] — [iced releases](https://github.com/iced-rs/iced/releases), [0.14.0](https://github.com/iced-rs/iced/releases/tag/0.14.0)
- Slint TextEdit (docs v1.2.0, old): "very few basic shortcuts", no syntax highlighting listed; 1.18 states large text now much faster. Current TextEdit docs not retrieved. — [Slint 1.2 TextEdit docs](https://releases.slint.dev/1.2.0/docs/slint/src/language/widgets/textedit), [1.18](https://slint.dev/blog/slint-1.18-released)
- Lapce uses its own GPU-accelerated rendering layer built on Floem (aggregator source; Lapce performance claims unbenchmarked); Floem repo has an `editor-core` directory. [A/weak] — [rustutils Lapce](https://rustutils.com/tools/lapce/), [floem](https://github.com/lapce/floem)
- cosmic-edit: pre-alpha per README (text engine cosmic-text is known from outside knowledge, not confirmed on page). — [cosmic-edit](https://github.com/pop-os/cosmic-edit)
- Iced production apps named on README: Halloy (IRC) and Icebreaker. — [iced repo](https://github.com/iced-rs/iced)
- egui: Rerun Viewer is the showcase professional app; multiline TextEdit with undo; very large scroll areas can be slow as everything is laid out every frame. — [egui repo](https://github.com/emilk/egui)

### Inferences
- Carbon would need a custom editor surface (rope + own shaping/virtualised paint) in any of these; leverage cosmic-text or Floem editor-core as references. This supports the PROJECT.md plan of Monaco in a webview initially.

### Gaps
- Code-editor-grade comparisons, Halloy's text input quality, Slint syntax-highlight/custom text engine options, egui_code_editor: not researched.

## 5. Verified numbers (size, RAM, startup, frame time)

### Takeaway
No controlled benchmark found. Only anecdotes, mixed platforms/backends. Carbon must run its own benchmark (as PROJECT.md already states).

### Cited Findings
- [A] Slint app launch under 50 ms and memory under 80 MB (blogger informal test); hello-world about 45 MB via btop (Reddit); Slint's 300 KiB claim is runtime only. A system monitor moved from egui (130 MB) to Slint (~30 MB, 50 MB with GPU monitor). egui dashboard ~102 MB. Tauri build 600-650 MB. LTO may cut binary 30-50% (Reddit comment). — summarised via [search results](https://bgenc.net/state-of-rust-GUIs/) and [Reddit thread](https://nyc1.lr.ggtyler.dev/r/rust/comments/1jzawxo/small_footprint_gui_library) (secondary; not opened)
- [M, 2023, old] Tauri vs iced vs egui startup/resize speed comparison; concluded all fast enough; no memory/size. — [Kalbertodt](https://lukaskalbertodt.github.io/2023/02/03/tauri-iced-egui-performance-comparison.html)

### Gaps
- No verified binary sizes, idle RAM, or frame times for iced; none for Carbon-like workloads.

## 6. Extension/plugin story, tooling, platform reach

### Cited Findings
- Slint live preview and LSP exist (1.18 notes mention LSP rename and live preview flag). [doc] — [Slint 1.18](https://slint.dev/blog/slint-1.18-released)
- Slint also published a Servo-with-Slint embedding series (Jan and May 2026, Windows support), relevant to embedding web content. [title only] — [slint blog](https://slint.dev/blog)
- egui 0.35 added `egui_inspection` protocol and an MCP server for inspecting UIs. [doc] — [egui releases](https://github.com/emilk/egui/releases)

### Inferences
- Runtime-loaded UI modules: Slint ships an interpreter (known from outside knowledge; not confirmed in fetched sources) which could suit declarative sections; iced and egui would need WASM or process isolation with a custom UI protocol.

### Gaps
- Interpreter/dynamic loading, hot reload for iced/egui, theming systems (design tokens), native menus, tray, dialogs, bundle sizes, packaging, Relm4/gtk4-rs, cxx-qt/qmetaobject, complaints sentiment: not researched due to budget.
