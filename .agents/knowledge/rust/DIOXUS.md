# Dioxus 0.7 (fullstack, web platform)

Dioxus knowledge from previous projects. Rust-wide notes are in `RUST.md`.


# Design

- One module per shared UI family: tables (column, sort, sort header, selection bar, row checkbox, check header, plus small selection helpers), icons (one `Icon` component with an icon-name enum), charts.
- Shared lists and colours live in a serde-only types crate, so the browser and the server use the same ones.
- Check both builds (`cargo check`, `cargo check --features server`); view with `dx serve --platform web -p <gui crate>` from the repo root.


# Architecture

- Core calls through server functions; the API module validates, calls the core, maps results.
- File routes: an axum router merged into `dioxus::server::router(App)`, served with `dioxus::serve`.
- `dx bundle --release --platform web -p <gui crate>` picks the `server-release` / `wasm-release` profiles by itself; `Dioxus.toml` `[web.wasm_opt] level = "z"`.
- Assets are found exe-relative (`public_path`), so the release folder can move.


# Security

- One request guard in front of every server function and file route checks `Host` and cross-site headers.


# Experience

## Fonts / files referenced from CSS don't load
**Cause:** Dioxus hashes asset names and bundles only `asset!()` files. **Solution:** Declare each with `asset!()` in Rust and inject the CSS with `document::Style`.

## `rsx!` text can't hold nested `format!` calls
**Solution:** Build the string in a `let` or helper before `rsx!`, then use `"{note}"`.

## Serving files
**Solution:** `dioxus::serve` with an axum router merged into `dioxus::server::router(App)`.

## Uploading files
**Solution:** A server fn taking `dioxus::fullstack::FileStream` (`#[post("/path?token")]`, file as the body); client passes `FileStream::from(file_data)`, server reads with `futures_util::StreamExt::next`. `DragEvent::files()` needs `dioxus::html::HasFileData` in scope.

## Client-side delay without a timer crate
**Solution:** `document::eval("await new Promise(r => setTimeout(r, 1500));").await` (eval code runs inside an async function).

## `aria-hidden` on `svg` doesn't compile
**Solution:** Use string attribute names: `"aria-hidden": "true"`, `"focusable": "false"`.

## `dx` crashes when run inside the GUI crate's folder
**Cause:** dx 0.7.10 resolves `default-members` relative to the current folder. **Solution:** Run from the repo root: `dx serve --platform web -p <crate>`.

## Release profiles
**Solution:** `dx bundle --release` picks `server-release` / `wasm-release` by itself; `dioxus-devtools` can't be dropped (dioxus-server / web depend on it).

## Prebuilt `dx` needs a newer glibc on Linux
**Problem:** The prebuilt `dx` needs glibc 2.39. **Solution:** Install it from source (`cargo install`) on older distributions and cache it in CI.
