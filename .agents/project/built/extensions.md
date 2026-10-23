# Extension system

Built 2026-10-23. User request: separate workflow tools, language servers and other features from base Carbon; users download what they need. Research and decisions: `../../documents/extensions.md` (WASM code, views drawn by Carbon + webview pages, install code from the shelved workflow branch, tools PATH first else download, WASM grammars, official extensions downloaded on demand from `extensions/`, Open VSX deferred).

## What changed for the user

| Before | Now |
|---|---|
| 12 tree-sitter grammars compiled in; Rust colours and rust-analyzer always there | No languages built in (only the kit's JSON). `.rs` is plain text until the Rust extension is installed or linked |
| Gruvbox in the app, default dark theme Gruvbox Dark Hard | Gruvbox is the `carbon.gruvbox` extension; default dark theme Carbon Dark; a chosen theme that is not installed falls back to Carbon Dark / Light |
| Themes from the source tree (`style/themes/`, broke in a build run elsewhere) | Carbon Dark/Light compiled in; extension themes; the user's `<config>/themes/*.json` (watched) |
| Extensions dialog "Nothing here yet" | Manage Extensions (below) |

## Behaviour

| Input | Result |
|---|---|
| App menu (three lines) > Extensions, File > Preferences > Extensions, palette | Manage Extensions (as big as Settings; opens on Catalog): Catalog / Installed (n) tabs, search, footer Install â–¾ (From Folderâ€¦ / From Zipâ€¦ / From Gitâ€¦ / Link Development Folderâ€¦), Catalog Sourceâ€¦, Open Folder, Refresh, Close |
| Installed row | Icon, name, origin (Installed / Development), state (On / Off / Needs more / Turned off after a failure / Files changed), Signed / Pinned, id Â· version Â· permissions allowed, description, what it adds, source + commit + hash or the linked folder; missing requirements with Install when the catalog has them; buttons Turn Off/On, Permissions (installed ones), Remove (installed) / Unlink (linked), Reinstall + Keep Changes (files changed) |
| Install from folder / zip / git (tag, branch or commit, optional folder in the repo) / catalog | Fetching dialog with Cancel; install dialog: new / update / reinstall / downgrade, file count and size, source, pin level and short hash, permissions (new ones tagged), requirements, problems; "Allow and Install" grants what it lists; a notification when done |
| Catalog tab | The catalog repository (`catalog.toml`, Catalog Sourceâ€¦; `CARBON_EXTENSION_CATALOG` overrides), Signed / Not checked, its extensions with Install / Update / Installed n; package icons (catalog `icon` = image file in the catalog repo + `icon_sha256`, read from the same commit as `catalog.json`, cached as `catalog-icons/<sha256>.<type>`, unlisted ones deleted), puzzle placeholder without one or when the image does not load |
| Open a file whose type no extension covers but the (cached) catalog lists | Notification "Install <name>?" with Install (once per extension per session) |
| First start with no extensions | Notification "Carbon starts light" with Open Extensions (once) |
| A language server whose program is not on PATH and whose extension has a download for this computer | Status bar "<server>: not installed" (tooltip: version, host); click: "Download <server> <version>?" dialog (what, from where, SHA-256 check, folder); Download: a job "Downloading <server>: x / y MB", then the server starts |
| Extension commands | In their menu (after a separator) and the palette (`<Extension>: <Title>`, with keywords); run a Carbon action, open the extension's panel, or its code |
| Extension panels | View > Add Panel lists them; `explorer_section` panels sit beside the Explorer (added when the extension is on, removed when off; the Explorer stays selected; not closable); others open where the manifest says; a panel of an off or removed extension says so |
| Extension code (WASM) | Loads on first use off the UI thread; `notify` shows a notification, `log` goes to stderr; a trap, a call over 5 s or over 256 MB turns the extension off ("Turned off after a failure", Turn On lifts it) |
| Turn off / on, remove, unlink, install | Applies at once: grammars, themes (and the theme settings' choices), settings, language servers, menus, palette, panels |
| Installed files changed on disk | Checked at start (content hash); turned off until Reinstall or Keep Changes |

## Code

| Part | File | What |
|---|---|---|
| Manifest | core `extension/manifest.rs` | `extension.toml` schema 1: languages, grammars, language_servers (+ per-target downloads), themes, settings, commands, menus, panels, views, `wasm`, `icon` (package image file: png / svg / jpg / webp, at most 256 KB, else the placeholder; 2026-10-23), capabilities, requires; validation (language servers need `process.exec:<command>`, downloads `net:<host>`, `https://` only, SHA-256) |
| Registry | core `extension/mod.rs` | `Registry::load` (installed newest version + linked dev folders, dev wins), blocked reasons (`Blocked`), requires check, contributions (`languages`, `language_for_path`, `grammars`, `language_servers`, `themes`, `settings`, `commands`, `panels`, keys `<id>/<item>`), `server_table` for the LSP client; `carbon.*` installs only from a signed catalog |
| Install, catalog, git, package rules, capabilities, state | core `extension/{install,catalog,git,package,capability,requires,state,dirs}.rs` | Ported from branch `feature/workflow` (renamed); catalog and git gained `path` (a folder in the repo) |
| Tools | core `extension/tools.rs` | `download_tool` (reqwest blocking, progress, cancel, 512 MB cap), `install_tool_file` (SHA-256 check, unpack none/gz/zip/tar.gz with package path rules, executable bit), `downloaded_program` |
| WASM host | core `extension/wasm.rs`, `wit/since_v0.1.0/extension.wit` | wasmtime 36 component model, `carbon:extension@0.1.0`: host `log`, `notify`, `setting`, `workspace-root`, `read-file`, `run-process`, `http-get` (each checks grants); exports `init`, `run-command`, `language-server-command`, `render-view`; WASI shares nothing; epoch deadline 5 s; 256 MB; `CallError::{Returned, Fault}` |
| LSP | core `lsp.rs` | `ServerConfig` owned (args, env, user path, downloaded copy, download info), `ServerTable` (language rules + servers), `LspStore::{set_table, reset, forget}` |
| Settings | core `settings/mod.rs` | `Settings::retain` (an off extension's settings go) |
| SDK | `apps/carbon-extension-api/` | wit-bindgen 0.57 bindings, `Extension` trait, `register_extension!`; `examples/hello` (commands, dynamic view, server command, grant checks) |
| GUI glue | gui `extensions.rs` (+ `extensions/{grammars,tools,code,install,manage}.rs`) | Global registry + state, reload/apply, grammars into the kit registry (compiled on a 16 MB-stack thread, parser factory per language), download prompt + job, code calls + quarantine, install/catalog dialogs, Manage Extensions, suggestions, welcome, start-up hash check |
| Panels | gui `extension_panel.rs`, `webview_panel.rs` (`webview` feature) | Declarative views (static or dynamic); web pages (ported, off by default) |
| Wiring | gui `editor_panel.rs` (language from registry), `lsp_ops.rs` (server command from code), `status_bar.rs`, `menus.rs` (`RunExtensionCommand`, extension items), `commands.rs`, `palette.rs` (owned keywords), `workbench.rs` (extension panels, Explorer-section tabs), `style/theme.rs`, `main.rs`, `build.rs` (8 MB main stack) | |
| Kit patches | gpui-component 8 (`ThemeRegistry::set_custom_themes`), 9 (`LanguageRegistry::unregister`) | `vendor/gpui-component/CARBON-PATCHES.md` |
| Official extensions | `github.com/denizhoroz/carbon-extensions-basic` (moved out 2026-10-23; local checkout `carbon-extensions-basic/`, ignored by git; grouped by type: `languages/<name>`, `themes/<name>`) | `rust` (grammar + rust-analyzer 2026-10-19 downloads for 6 targets), `toml`, `markdown` (+ markdown_inline), `yaml`, `python`, `javascript`, `typescript` (+ tsx), `html`, `css`, `bash`, `gruvbox`; `grammars.toml` pins every `.wasm` (upstream release builds of the versions Carbon compiled in) by SHA-256; `fetch_grammars.py` downloads / `--check`s them; queries copied from the kit / grammar crates |

Locations: preferences in `<config>/extensions/` (`state.json`, `catalog.toml`, cached `catalog.json`, `catalog-icons/`); packages in `%LOCALAPPDATA%\Carbon\extensions\<id>\<version>\`; tools in `%LOCALAPPDATA%\Carbon\tools\<id>\<server>\<version>\`. `CARBON_EXTENSIONS_DIR` puts all in one folder; with only `CARBON_CONFIG_DIR` set, data goes under it too.

## Developing an extension

1. A folder with `extension.toml` (schema: `../../documents/extensions.md` s4, `manifest.rs` doc comment) and the files it names.
2. Manage Extensions > Install > Link Development Folderâ€¦: loaded in place with every permission it asks for; Refresh reloads it.
3. Code: a `cdylib` crate on `carbon-extension-api`, `cargo build --release --target wasm32-wasip2` (`rustup target add wasm32-wasip2`), `wasm = "<file>.wasm"`.
4. Grammars: a tree-sitter `.wasm` (`tree-sitter build --wasm`, or the grammar's release asset) exporting `tree_sitter_<name>`, plus `highlights.scm`.

## Publishing official extensions

Since 2026-10-23 (later): the default catalog is `OFFICIAL_CATALOG` = `github.com/denizhoroz/carbon-extensions-basic`; `catalog.py` there builds `catalog.json`; until a key is set, `carbon.*` from that URL install unsigned (security: trust rests on HTTPS to GitHub and the repo's push access). Signing (below) is still to do.

The catalog is a git repository with `catalog.json` at its root, each version pinned by commit (+ `path`, e.g. `languages/rust`) and content hash; `carbon.*` ids install only when `catalog.json.minisig` is signed by a key in the user's trusted keys. Steps: create a minisign key pair (keep the secret key private), make the catalog repo (or a branch), list each package folder (`languages/<name>`, `themes/<name>`) at a commit of `carbon-extensions-basic` with its hash, sign `catalog.json`, and put the public key as Carbon's default trusted key and the repo as the default catalog URL (both still empty in code).

## Measurements (Windows)

| What | Result |
|---|---|
| Full parse of a 3.5 MB Rust file | native grammar 388 ms, WASM grammar 600 ms (release, spike 2026-10-23) |
| Reparse after a one-character edit | native 4.8 ms, WASM 5.4 ms |
| Compile the Rust grammar `.wasm` | 23 ms (release), once per start, on a background thread |
| New parser (fresh `WasmStore`) | ~8 ms |
| Load the example extension component | 139 ms (debug build) |
| Release `carbon-gui.exe` size | 42.7 MB before (commit `cde51f1`, 12 grammars compiled in) â†’ 62.1 MB after (+19.4 MB): wasmtime 36 + Cranelift (shared by grammars and extension code), wasmtime-wasi, reqwest/rustls/aws-lc for HTTPS (git installs, tool downloads), zip/tar; minus the 12 grammars. Breakdown per crate not measured. Ways down: rustls on `ring` instead of `aws-lc`, wasmtime's Pulley interpreter or Winch instead of Cranelift (slower code) |

## Verified

- `cargo test --workspace`: 167 pass, 8 ignored (`-j 4`: with more parallel jobs the linker ran out of memory, LNK1102). New: `tests/extension.rs` (9: manifest rules, capabilities for servers, languages by file, global keys, flags and requires, state file), `tests/extension_install.rs` (18 + 1 network: package paths, folder/zip/git installs, zip slip, official ids need a signed catalog, updates/downgrades/capabilities, dev link wins, remove, catalog parse/pins/signatures, changed files, repo folder), `tests/extension_tools.rs` (3 + 1 network: zip/gz unpack, hash refusal, table resolution; the real rust-analyzer download and `--version`), `tests/extension_wasm.rs` (ignored: builds the example guest; grants refused/allowed, reading outside the package refused, settings, workspace, views, server command, 5 s limit, panic), `tests/lsp.rs` (server table). Ignored ones run by hand: all pass. Clippy and fmt clean (also with `--features webview` builds).
- Click-tested (test build, scratch config, PATH without rust-analyzer): Rust, Markdown (with an injected Rust block) and TOML colours from WASM grammars; rust-analyzer starts from the Rust extension; "not installed" â†’ Download dialog â†’ job with MB progress â†’ rust-analyzer running; a development extension's Explorer-section tab (Explorer stays selected), its buttons running its WASM code, a dynamic view in the right dock redrawing after each run ("Ran 1/2 times"), its View menu command with a notification showing its setting and the open folder; Manage Extensions list, Turn Off (tab removed, panel says it is off, live) / Turn On; Catalog tab from a local git catalog (Not checked), the `.foo` suggestion â†’ Fetching â†’ install dialog (pinned, hash) â†’ installed with its record in `state.json`.
- Not tested: zip / folder / git installs through the GUI dialogs (core tests cover them), Permissions dialog, Reinstall / Keep Changes, the welcome note, webview pages (feature builds only), macOS/Linux, a signed official catalog (none published).

## Limits

- No official catalog yet (needs the user's signing key, above); until then official extensions are used by linking `carbon-extensions-basic/<type>/<name>` folders.
- Grammars cost ~1.5x on a full parse; every parse makes a new `WasmStore` (~8 ms, background).
- Host calls that wait (`run-process`, `http-get`) count toward the 5 s limit.
- Extension code reaches the app only through `notify`, `log` and dynamic views; no editor or file API yet; views tier 2 (inputs, tables) not built.
- Webview pages: experimental, `webview` cargo feature, off by default (page is a native window above GPUI; GPUI drawn without DirectComposition on Windows); no page API.
- No Open VSX, no TextMate grammars, no auto-update (the catalog shows Update), no extension bisect, no per-workspace trust for extensions in a project.
- The Rust extension's rust-analyzer pin (2026-10-19) needs bumping by hand.
