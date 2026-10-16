# Rust

Rust knowledge from previous projects. General rules are in `../DESIGN.md`, `../ARCHITECTURE.md`, `../SECURITY.md`, `../EXPERIENCE.md`; frameworks in this folder have their own files (`DIOXUS.md`).


# Architecture

- **Cargo workspace of crates**: core (logic and I/O), types (serde only, also builds for wasm), GUI. Heavy crates (HTTP client, async runtime, archive, image) stay in the core so they never reach the browser build. The core re-exports the types crate.
- **Core error type** `Box<dyn Error + Send + Sync>`, so futures holding it stay `Send`. Blocking jobs run with `spawn_blocking`.
- Manifests and settings stored as TOML (serde).
- `default-members` set to the GUI crate.


# Release size

- Profiles in the root `Cargo.toml`: `release` (fat LTO, 1 codegen unit, strip), `server-release` (`opt-level = "s"`, hot crates at 3), `wasm-release` (`opt-level = "z"`).
- No `panic = "abort"` in a server: one panicking request would close the whole app.
- One HTTP stack: the core's reqwest version matches the UI framework's (rustls + ring). Check duplicates with `cargo tree -d`, sizes with `cargo bloat --crates`.


# Security

- Credential store through the `keyring` crate.


# Experience

## Futures weren't `Send`
**Cause:** `Box<dyn Error>` held across `.await`. **Solution:** Error type `Box<dyn Error + Send + Sync>`; blocking jobs via `spawn_blocking`.

## Browser code can't use the core
**Cause:** The core depends on crates that don't build for wasm (reqwest, tokio, zip, image). **Solution:** Shared types and rules in a serde-only crate; the core re-exports it.

## Two HTTP stacks in the release
**Cause:** The core used reqwest 0.13 (rustls + aws-lc) next to the framework's reqwest 0.12 (rustls + ring). **Solution:** Match the framework's reqwest version (`rustls-tls`). With release profiles added: server 24.5 → 17.5 MB, wasm 6.1 → 3.0 MB.

## Linked tools must move together
**Problem:** A newer Rust stable's LLVM stripped wasm-bindgen's custom section; its macOS `rust-objcopy` lacked `libLLVM.dylib`. **Solution:** Pin the toolchain in CI to the local working version (with its `llvm-tools`), update both together.
