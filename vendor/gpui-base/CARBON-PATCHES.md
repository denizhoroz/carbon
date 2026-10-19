# Carbon patches to gpui-base 0.7.3

Copied unchanged from crates.io `gpui-base` 0.7.3 (Apache-2.0, `LICENSE-APACHE`), then changed as listed. Used through `[patch.crates-io]` in the root `Cargo.toml`. Every change is marked `CARBON PATCH` in the source. When upgrading gpui-kit: copy the new version here and re-apply each item, or drop it if upstream gained an equivalent. Patches to `gpui-component` are in `../gpui-component/CARBON-PATCHES.md`.

| # | Date | File | Change | Why |
|---|---|---|---|---|
| 3 | 2026-10-19 | `src/dock/drag.rs` (`split_placement_at`, its tests) | Drop split zones are edge strips a quarter of the side, at most 80 px (`SPLIT_EDGE_RATIO`, `SPLIT_EDGE_MAX`); the rest of a group is the "drop as a tab" zone (was 35% per edge, centre only 30% x 30%) | The centre zone is the usual target and was too small to hit (user) |
| 2 | 2026-10-18 | `src/dock/dock_area.rs` (`resize_dock`, `render_dock`, `dock_extent`, `DockAreaRenderer::render_closed_dock`) | Every collapsible dock snaps when dragged: under half `PANEL_MIN_SIZE` it closes, back over half it opens at the minimum, in the same drag (was: bottom dock only, with a live strip); a closed bottom dock is hidden (was a 29 px strip); a closed dock with panels asks the renderer for a grab strip at its edge (new hook, default none) | VS Code: drag the terminal/sidebar edge to the window edge to hide it, drag the edge out to bring it back (user) |
| 1 | 2026-10-17 | `src/input/base/element.rs` (`layout_line_numbers`) | The fold column's width (18 px) is reserved whenever line numbers show, not only with folding on | Carbon turns folding off (cost, crash); without the column, code sat 6 px from the line numbers. Now 24 px, as with the kit's default folding |
