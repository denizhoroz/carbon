# Extension system: what other editors use, and what Carbon picked

Written 2026-10-23. User request: build the extension system so language servers, grammars, themes, workflow tools and other features leave the base app; Carbon stays light, users download what they need. Research first, then decisions (s3). Sources in s5. Earlier related work: `language-tools.md` (binary source), `../project/archived/workflow-system.md` (shelved branch with install, catalog, views, webviews).

## 1. What other products use

| Product | Runtime / isolation | Manifest + contributions | UI from extensions | Distribution / trust | Ev |
|---|---|---|---|---|---|
| VS Code | Node extension host, separate process, full permissions | `package.json` `contributes` (static) + `activationEvents` (lazy load) | Webviews, tree views, custom editors | Signed Marketplace; forks use Open VSX. GlassWorm 2026: hijacked publisher token poisoned 4 extensions (30 Jan), 72+ malicious extensions via `extensionPack` / `extensionDependencies` (31 Jan - 13 Mar); removal does not uninstall | doc, press |
| Zed | Wasmtime in-process (background executor), WIT per API version (`since_v0.x`), Rust -> `wasm32-wasip2` | `extension.toml`: languages, grammars (git repo + rev), LSP, DAP, themes, icon themes, snippets, MCP | None. RFC #53403 (Apr 2026): declarative element tree over WIT for panels and status items; maintainer: "not something we're likely to get to in the near future" | One git repo `zed-industries/extensions`, CI builds; capabilities `process:exec`, `download_file`, `npm:install` user-restrictable (`granted_extension_capabilities`) | doc, blog |
| JetBrains | JVM, same process, classloader per plugin | `plugin.xml` extension points | Full Swing | Marketplace; dynamic unload often fails (classloader leaks) -> restart | doc, A |
| Sublime Text | Separate `plugin_host` process (Python 3.3 / 3.8); a bad plugin cannot crash the editor | Package folder | Limited | Package Control | doc |
| Obsidian | JS in the app process, no sandbox ("cannot reliably restrict plugins") | `manifest.json` | Full DOM | Restricted Mode on by default; automated scans + scorecard per version | doc |
| Figma | Logic in QuickJS compiled to WASM (no browser APIs); UI in an iframe (no document access); message passing | manifest | iframe | Reviewed; network blocked by CSP | doc, S |
| Neovim | Lua in process | code | Full | `vim.pack` built in since 0.12 (29 Mar 2026), lockfile; Mason for tools | S |
| Helix | Steel (Scheme) plugin PR #8675 still a draft; fork `steelix` | - | - | - | S |
| Zellij | WASM / WASI plugins draw their own UI, get events, manage panes | KDL | Text UI | Permission prompts (?) | doc |
| Lapce | WASI "volts" (?) | `volt.toml` (?) | (?) | plugins.lapce.dev, `volts publish` | doc (thin) |

## 2. Runtime options for a Rust host

| Option | Sandbox | Size cost | Notes |
|---|---|---|---|
| WASM (wasmtime) | Yes | Carbon's release exe 42.7 → 62.1 MB with wasmtime + WASI + HTTPS + zip (M, 2026-10-23) | Component model + WIT; WASI 0.3 stable 11 Jun 2026 (wasmtime 46+), component model 1.0 not yet; WASI 0.2 still the common target. Extism (BSD-3) wraps it with HTTP and limits |
| JS (rquickjs) | Yes | ~210 KiB | No Node or web APIs; Boa ~6 MB pure Rust; deno_core 35-64 MB |
| Separate process (JSON-RPC like LSP/MCP) | Only with an OS sandbox | 0 | Any language; one process per extension |
| Native dylib (abi_stable / stabby / C ABI) | No | 0 | No stable Rust ABI; GPUI breaks every minor |
| Lua / Rhai / Rune | Partial | small | Scripts only, no compiled ecosystem |

**Constraint found (M, local source):** tree-sitter 0.26.13 feature `wasm` depends on `wasmtime-c-api` 36.0.x. A host on the same wasmtime major ships one wasmtime instead of two.

## 3. Decisions (user, 2026-10-23)

| Question | Options | Picked |
|---|---|---|
| Code runtime | WASM / separate process / no-code only / QuickJS | **WASM**: wasmtime 36 (shared with tree-sitter), component model, versioned WIT `carbon:extension@x.y`, user-granted capabilities |
| Extension UI | drawn by Carbon / webview / both / none | **Both**: declarative views drawn by Carbon, webview pages for heavy UIs |
| Shelved `feature/workflow` code | reuse install parts / fresh / per part | **Reuse the install parts** (manifest rules, dirs, package checks, folder/zip/git install, catalog + minisign, hash pins, capability grants, requires), renamed; Section Bar, per-workflow layouts, overlays, Manage Workflows dropped |
| Language-tool binaries | PATH first else download / config only / bundled / later | **PATH first, else download** (per-platform asset, SHA-256, consent prompt). Closes `language-tools.md` s3 row 1 |
| Grammars | WASM / native dylib / keep a few | **WASM grammars** in extensions; the 12 compiled-in grammars go |
| Official extensions | download on demand / bundled on / bundled off | **Download on demand**: suggested when a matching file opens, first-run picker; nothing preinstalled |
| Stays in base Carbon | - | **Terminal, Chat panel, Git markers, LSP client** |
| Sources | own catalog + folder/zip/git / + Open VSX / catalog only | Own signed catalog + folder/zip/git; **Open VSX deferred** (its grammars are TextMate, Carbon highlights with tree-sitter) |
| Official extensions live in | `extensions/` here / separate repo | **`extensions/` in this repo**, built by a script; the catalog pins this repo's commits |

## 4. Design

Built 2026-10-23: `../project/built/extensions.md`.

Package = folder with `extension.toml` (schema 1): `id` (`publisher.name`), `name`, `version`, `carbon_api` range, `[capabilities]`, `[requires]`, contributions; optional `extension.wasm`, `grammars/`, `themes/`, `views/`, `pages/`.

| Contribution | Form |
|---|---|
| `[[languages]]` | id, name, suffixes / file names, grammar, comment tokens |
| `[[grammars]]` | `.wasm` + `highlights/injections/locals.scm` -> kit `LanguageRegistry` |
| `[[language_servers]]` | command, args, env, install hint; `[download]` per target (url, sha256, archive, bin) or a WASM hook |
| `[[themes]]` | kit `ThemeSet` JSON |
| `[[settings]]` | typed keys under `<id>.*` |
| `[[commands]]` / `[[menus]]` | id, title, menu path; runs a WASM export or opens a panel |
| `[[panels]]` | `view` / `webview`, title, placement, `explorer_section` |

Trust (GlassWorm lessons): capabilities asked at install, updates ask only for new ones; `requires` never installs on its own, each dependency shown; hash and commit pins; minisign-signed catalog; no silent auto-update; a faulting extension is turned off.

## 5. Sources

- VS Code: code.visualstudio.com/api/references/extension-manifest, /api/references/activation-events, /api/get-started/extension-anatomy (doc). GlassWorm: thehackernews.com/2026/02/open-vsx-supply-chain-attack-used.html; labs.cloudsecurityalliance.org GlassWorm Open VSX transitive dependency note (press, S).
- Zed: zed.dev/docs/extensions/developing-extensions, /docs/extensions/capabilities (doc); zed.dev/blog/zed-decoded-extensions (2024-10-21); github.com/zed-industries/zed/discussions/53403 (RFC, Apr 2026).
- JetBrains: plugins.jetbrains.com/docs/intellij/dynamic-plugins.html; platform.jetbrains.com unload-failure thread (A).
- Sublime: sublimetext.com/docs/api_environments.html.
- Obsidian: obsidian.md/help/plugin-security.
- Figma: developers.figma.com/docs/plugins/how-plugins-run; macwright.com/2024/03/29/figma-plugins (S).
- Neovim: heise.de Neovim 0.12 article; alternativeto.net 2026/3 news (S).
- Helix: github.com/helix-editor/helix/pull/8675; Arch wiki mirror 2026-04 (S).
- Zellij: zellij.dev/documentation/plugins, /news/new-plugin-system.
- Lapce: docs.lapce.dev/development/plugin-development (thin).
- WASI: wasi.dev/roadmap, wasi.dev/releases/wasi-p3, bytecodealliance.org/articles/WASI-0.3. Extism: extism.org, github.com/extism/extism.
- JS engines: docs.rs/crate/rquickjs/0.12.1; dev.to ahaoboy yt-dlp-ejs (Boa size, S). Stable ABI: docs.rs stabby, xabi (doc).
- Local: `~/.cargo/registry/src/.../tree-sitter-0.26.13/Cargo.toml` (`wasm` -> `wasmtime-c-api` 36.0.14).
