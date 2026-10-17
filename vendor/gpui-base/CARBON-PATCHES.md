# Carbon patches to gpui-base 0.7.3

Copied unchanged from crates.io `gpui-base` 0.7.3 (Apache-2.0, `LICENSE-APACHE`), then changed as listed. Used through `[patch.crates-io]` in the root `Cargo.toml`. Every change is marked `CARBON PATCH` in the source. When upgrading gpui-kit: copy the new version here and re-apply each item, or drop it if upstream gained an equivalent. Patches to `gpui-component` are in `../gpui-component/CARBON-PATCHES.md`.

| # | Date | File | Change | Why |
|---|---|---|---|---|
| 1 | 2026-10-17 | `src/input/base/element.rs` (`layout_line_numbers`) | The fold column's width (18 px) is reserved whenever line numbers show, not only with folding on | Carbon turns folding off (cost, crash); without the column, code sat 6 px from the line numbers. Now 24 px, as with the kit's default folding |
