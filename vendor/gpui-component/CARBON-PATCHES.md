# Carbon patches to gpui-component 0.7.3

Copied unchanged from crates.io `gpui-component` 0.7.3 (Apache-2.0, `LICENSE-APACHE`), then changed as listed. Used through `[patch.crates-io]` in the root `Cargo.toml`. Every change is marked `CARBON PATCH` in the source. When upgrading gpui-kit: copy the new version here and re-apply each item, or drop it if upstream gained an equivalent.

| # | Date | File | Change | Why |
|---|---|---|---|---|
| 1 | 2026-10-17 | `src/dock/panel.rs`, `src/dock/tab_panel.rs` | New `Panel::always_tab_bar` (default `false`), forwarded through `PanelView`; a single-panel group whose panel returns `true` draws the tab bar instead of the plain title | Editor: one open file looks like several (tab + close button); side panels keep their title. The kit only had a dock-wide `PanelStyle` |
| 3 | 2026-10-17 | `src/dock/panel.rs`, `src/dock/tab_panel.rs` | New `Panel::request_close` (default `false`), forwarded through `PanelView`; a tab's close button asks the panel first and does nothing more when it returns `true`; panels with `always_tab_bar` show the button even as a group's last panel | Editor: x on a single tab; x on an unsaved tab asks Save / Don't Save; closing the last editor clears it instead of leaving the area blank. Also (same day) the "..." menu's Close: goes through `request_close` too (it closed unsaved editors without asking) and shows for always-tab panels that are a group's last panel. A panel whose `closable` is false gets no tab close button (Explorer) |
| 4 | 2026-10-17 | `src/dialog/dialog.rs` | Without `margin_top`, a dialog is anchored by its left-middle at the window's vertical middle (was top-left at a tenth of the height) | Dialogs appeared too high (user) |
| 2 | 2026-10-17 | `src/tab/tab.rs` | Default-size `Tab` variant: height 32 -> 30 px, inner height 30 -> 28 px | Tabs as tall as the dock's single-panel title bar (30 px), so the bar keeps its height when tabs come and go |
