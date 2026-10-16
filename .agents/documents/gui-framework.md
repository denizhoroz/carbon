# Carbon GUI framework comparison (native desktop)

Scope: native desktop GUI in Rust only; webview/web excluded by user decision (2026-10-16). Data as of 2026-10-16.
Evidence tags: **M** measured, **A** anecdotal/vendor/secondary, **?** unverified. Scores are judgement, not measurement. No independent benchmark exists for any candidate.

## 1. Basics

| Framework | Version / status | Licence | Renderer | Language / DSL | Backing / stars | Maturity / API stability |
|---|---|---|---|---|---|---|
| Slint | 1.18.1 (2026-09) | GPLv3, Royalty-Free*, or paid | FemtoVG, Skia, software | `.slint` DSL + Rust | SixtyFPS GmbH; 24.2k | 1.x; wgpu/winit features churn |
| Dioxus Native (Blitz) | 0.8 alpha; Blitz 0.3 beta | MIT/Apache-2.0 | Vello on wgpu, own HTML/CSS | Rust `rsx!` | Dioxus Labs | Beta, "many bugs"; no JS engine |
| GPUI (+ gpui-component) | gpui-kit 0.7.3 (2026-10, ex gpui-component) on crates.io `gpui-pre` =0.3.9; plain `gpui` 0.2.2 is stale | Apache-2.0 | Own GPU: Metal, DX11, Vulkan | Rust builders, no DSL | Zed; kit by Longbridge; 17.7k | Pre-1.0; pinned snapshots, weekly kit breaks |
| Iced | 0.14.0 (2025-12) | MIT | wgpu, tiny-skia fallback | Rust, Elm model | Kraken sponsor; 31.8k | 0.x; 1-2 breaking minors/year |
| egui | 0.37.0 (2026-10-15) | MIT/Apache-2.0 | glow or wgpu, immediate mode | Rust | Rerun; ~31.5k | Breaking every 2-3 months |
| Floem | 0.2.0 (2024-11) stale | MIT | wgpu (vger/Vello), Skia opt. | Rust, reactive | Lapce; 4.3k | Stale crates.io; momentum doubtful |
| Xilem / Masonry | 0.4.0 (2025-10); main newer | Apache-2.0 | Vello on wgpu; Parley text | Rust, reactive | Linebender; 5.6k | "Experimental", pre-1.0 |
| Freya | 0.5.0-rc.10; stable 0.4 | MIT | Skia | Rust, own reactive core | Mostly one maintainer; 3.2k | rc churn; "not ready" (A, v0.3) |
| Makepad | 1.0.0 (2025-05) | MIT/Apache-2.0 | Own GPU, 2D+3D | Live DSL; nightly Rust | Makepad team; 7.2k | Niche; undocumented DSL (A) |

*Slint Royalty-Free does not permit apps that "expose the APIs" of Slint (risk for an extension SDK); needs AboutSlint badge.

## 2. IDE fit

| Framework | Code-editor widget | Terminal embed | Docking | Multi-window | Diagram canvas | 3D / wgpu | Extension UI path |
|---|---|---|---|---|---|---|---|
| Slint | Plain `TextEdit`, no highlighting; build own | None | None (issue open since 2022) | Yes; globals per instance | `Path`, SVG `Image` | wgpu texture import (unstable) | `slint-interpreter` loads `.slint`; licence risk |
| Dioxus Native | None; no JS for Monaco | None | None | Undocumented | Vello; SVG styling gaps | wgpu possible (?) | None |
| GPUI + kit | Kit: tree-sitter, LSP, "200K lines" (A, unaudited) | Zed has one; kit none | Kit dock, serializable (A) | Yes (Zed) | Custom paint elements | Hard: GPUI is not wgpu | Zed: WASM logic only, no UI panels |
| Iced | Basic `text_editor`, highlighter | `iced_term`, immature (?) | None | Yes (0.12+) | `canvas` widget | `shader` (wgpu) | DIY |
| egui | Multi-line `TextEdit`; not editor-grade | `egui_term` v0.1, incomplete | `egui_dock`, `egui_tiles` | Viewports | `egui-snarl` (early) | `egui_wgpu::Callback` | DIY (WASM guests, ?) |
| Floem | Lapce rope editor core; unbenchmarked (A) | Lapce has one (?) | Unverified | Unverified | Unverified | wgpu | Lapce WASI plugins, no UI |
| Xilem / Masonry | None; Parley is strong text base | None | Unverified | Unverified | Vello plausible | wgpu surfaces | None |
| Freya | Claims tree-sitter editor (A) | Claims PTY terminal (A) | Unverified | Unverified | Plotting widgets | Skia, not wgpu | None |
| Makepad | Internal editor (?) | Not found | Unverified | Unverified | 2D/3D renderer | Own GPU, not wgpu | Live DSL (?) |

No framework ships a verified code-editor-grade text widget; GPUI's kit editor is a vendor claim. Zed's own app crates may be GPL/AGPL (?).

## 3. Platform and quality

| Framework | Accessibility (AccessKit) | IME / text input | Theming / tokens | Hot reload / tooling | Size / RAM / startup | Win / Linux / macOS notes | Production apps |
|---|---|---|---|---|---|---|---|
| Slint | AccessKit; "more work needed" | IME not found | Style fixed at compile time; global tokens | Best: LSP, live preview, Figma | ~45 MB hello world (A) | Fine; tray/menus built in | Embedded mostly |
| Dioxus Native | AccessKit feature, completeness unknown | Undocumented | Partial CSS; no `overflow: auto` | Subsecond (0.8) | Vello app 530 MB (A) | NixOS/Wayland blank window issues | None |
| GPUI + kit | Core likely absent (Narrator fail, A); kit claims roles | Japanese worked (A) | Theme system; tokens need codegen | Rust rebuilds only | Zed 142-222 MB vs 600 MB, conflicting (A) | Linux Vulkan failures; Win moved to DX11 | Zed, Longbridge Pro |
| Iced | **No AccessKit; issue #552 open since 2020** | IME added 0.14 | Theme structs; no token pipeline | Limited | 2023 comparison only (M, old) | Fine | Halloy, Icebreaker |
| egui | **Active: always on since 0.34**, native Win/mac | Desktop CJK quality unverified | `Visuals`; `StyleProvider` experimental | `egui_kittest`, MCP server | ~102-130 MB (A) | Fine; add-on crates lag core | Rerun Viewer |
| Floem | Narrator saw no text (A, 2025) | Japanese IME failed (A) | Not found | Unknown | No data | Unverified | Lapce |
| Xilem / Masonry | **AccessKit-native**, pre-1.0; Narrator partial (A) | Works, missing glyphs (A) | Not found | Limited | No data | Needs Rust 1.96+, git deps | None |
| Freya | AccessKit; Narrator partial (A) | Partial (A) | Not found | Devtools, headless tests (A) | Skia adds size | Unverified | None |
| Makepad | Not mentioned; Narrator failed (A) | Partial (A) | Live-edit DSL | Live editing | No data | Nightly toolchain | Robrix, Moly (A) |

## 4. Score matrix

Scale 0-3: 3 ready and shipped; 2 workable or unaudited claim; 1 build yourself or weak evidence; 0 blocker. Weights in headers; total /66; gaps under ~4 are ties.

| Framework | Editor x4 | Term x2 | Canvas x1 | A11y x3 | Dock+Win x2 | Ext UI x2 | Theme x1 | Licence x2 | Maturity x2 | OS parity x2 | RAM x1 | **Total** |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| GPUI + kit | 2 | 1 | 1 | 1 | 3 | 1 | 2 | 3 | 1 | 2 | 3 | **39** |
| egui | 1 | 1 | 2 | 2 | 3 | 1 | 1 | 3 | 2 | 2 | 2 | **39** |
| Slint | 1 | 0 | 1 | 2 | 1 | 2 | 3 | 1 | 3 | 2 | 3 | **35** |
| Freya | 2 | 2 | 1 | 1 | 1 | 0 | 1 | 3 | 1 | 1 | 2 | **31** |
| Floem | 2 | 1 | 1 | 0 | 1 | 1 | 1 | 3 | 1 | 1 | 2 | **28** |
| Iced | 1 | 1 | 2 | 0 | 1 | 1 | 1 | 3 | 1 | 2 | 2 | **27** |
| Makepad | 1 | 0 | 2 | 0 | 1 | 1 | 2 | 3 | 1 | 1 | 2 | **24** |
| Xilem / Masonry | 0 | 0 | 2 | 2 | 1 | 0 | 1 | 3 | 1 | 1 | 2 | **23** |
| Dioxus Native | 0 | 0 | 1 | 1 | 0 | 0 | 2 | 3 | 1 | 1 | 1 | **17** |

## 5. Excluded by user decision (webview / web)

| Name | Reason |
|---|---|
| Tauri 2 (wry + tao) | User decision; WebKitGTK/Wayland/NVIDIA issues, per-window RAM, uneven a11y |
| Dioxus webview mode | Same engine and Linux burden as Tauri, 0.x churn |
| Leptos | Web only; no desktop renderer |
| Yew, Sycamore | Web (WASM) only, slower cadence |
| wry + tao directly | Webview stack without the shell layer |

## 6. Also considered

| Name | Note |
|---|---|
| Ribir | 0.4 alpha; docs disagree with code; Narrator failed (A) |
| Vizia | 0.4.0; Narrator partial, IME works; "not ready" (A) |
| Kas | 0.17; outdated tutorials, IME would not activate (A) |
| Cushy | Stale since 2024-08; IME composer hidden (A) |
| Rinch | ~2 stars, not on crates.io |
| libcosmic | iced fork, Linux-first; cosmic-edit GPL-3.0 pre-alpha |
| GTK4 / Qt bindings | Not researched; mature a11y/IME (?), heavy deployment, Qt licence question |
| ratatui | TUI only; possible second daemon client, not the GUI |

## 7. Biggest risks

| Framework | Risk |
|---|---|
| GPUI + kit | Pinned snapshot, weekly breaks; a11y unproven; single-vendor kit; no terminal/canvas/ext UI |
| egui | Editor and terminal build-yourself; per-frame layout vs large files; breaking releases |
| Slint | Licence clause vs extension SDK; no editor, terminal, docking |
| Iced | No AccessKit; no docking; basic editor |
| Xilem / Freya | Pre-1.0 / rc; no docks; Freya single maintainer, claims unaudited |
| Dioxus Native | CSS gaps, no editor, unproven a11y/IME |
| All | Editor is the largest build item; ext UI has no precedent; a11y evidence is one 2025 test |

## 8. Still to verify

- gpui-component editor latency and large-file behaviour; AccessKit claim vs real screen readers.
- Screen-reader results on VoiceOver and Orca for any candidate (none found).
- Zed crate licences (GPL/AGPL/Apache split) before any reuse.
- Desktop CJK IME quality in egui, Slint, Iced.
- Slint "exposes APIs" clause vs interpreter-loaded extension UI (ask in writing).
- Measured idle RAM and startup per candidate (claims conflict up to ~5x).

## 9. Sources

- Slint: [repo](https://github.com/slint-ui/slint), [T&Cs](https://slint.dev/terms-and-conditions), [1.18](https://slint.dev/blog/slint-1.18-released), [interpreter](https://docs.rs/slint-interpreter/latest/slint_interpreter/)
- GPUI: [README](https://github.com/zed-industries/zed/blob/main/crates/gpui/README.md), [gpui-component](https://github.com/longbridge/gpui-component), [crates.io](https://crates.io/api/v1/crates/gpui), [gpui-ce #280](https://github.com/gpui-ce/gpui-ce/issues/280), [Zed extensions](https://zed.dev/blog/zed-decoded-extensions), [Zed Linux](https://zed.dev/docs/linux.html)
- Iced: [repo](https://github.com/iced-rs/iced), [0.14.0](https://github.com/iced-rs/iced/releases/tag/0.14.0), [#552](https://git.tdem.in/iced-rs/iced/issues/552)
- egui: [repo](https://github.com/emilk/egui), [releases](https://github.com/emilk/egui/releases), [egui_dock](https://docs.rs/crate/egui_dock/0.21.0), [egui_term](https://docs.rs/crate/egui_term/latest), [egui-snarl](https://docs.rs/egui-snarl/)
- Others: [floem](https://github.com/lapce/floem), [xilem](https://github.com/linebender/xilem), [freya](https://github.com/marc2332/freya), [makepad](https://github.com/makepad/makepad), [Blitz status](https://blitz.is/status), [Dioxus 0.7](https://dioxuslabs.com/blog/release-070/)
- Surveys/measurements: [boringcactus 2025](https://www.boringcactus.com/2025/04/13/2025-survey-of-rust-gui-libraries.html), [Kalbertodt 2023](https://lukaskalbertodt.github.io/2023/02/03/tauri-iced-egui-performance-comparison.html), [Better Stack](https://betterstack.com/community/guides/scaling-nodejs/tauri-vs-electron-vs-deno-vs-electrobun/)
- Local: `.agents/research_notes/GUI framework comparison/`
