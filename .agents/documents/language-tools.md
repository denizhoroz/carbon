# Language tools: how other editors ship them, and Carbon's options

Written 2026-10-19. User decision: Carbon ships **no preinstalled language tools** (language servers, linters, formatters, compilers); users install them from **Manage Extensions**. User asked how alternatives handle bundling. How Carbon extensions get tool binaries is **open** (s3). Sources in s4.

## 1. What other editors do

| Editor | Built-in language support | Where server/tool binaries come from | Compiler / toolchain | Notes | Ev |
|---|---|---|---|---|---|
| VS Code | JS, TS, CSS, HTML ship in the box; rest via Marketplace extensions | Extension carries the binary in a **platform-specific VSIX** (`vsce --target win32-x64`, `darwin-arm64`, ...; VS Code 1.61+ picks the match, a package without target is the fallback) or downloads it. rust-analyzer: server in the extension dir, asks before downloading a matching one; `rust-analyzer-no-server.vsix` + `rust-analyzer.server.path` for other platforms. Ruff: ships a ruff binary, prefers the project's Python env, then PATH, then bundled | Never bundled. Go extension installs gopls etc. (pinned versions, `Go: Install/Update Tools`, "Analysis Tools Missing" prompt) but the user installs Go | Marketplace signs every extension, VS Code verifies on install; auto-update, 12 h delay, opt-out; workspace recommendations (`.vscode/extensions.json`) prompted on first open; "recommended based on recently opened files"; extension packs | doc |
| Zed | Some languages native (Rust, ...); others via WASM extensions | **Download on demand, PATH first**: Rust: if `rust-analyzer --help` works from PATH it is used, else Zed installs its own stable build. Servers stored in Zed's data dir (`.../Zed/languages`), kept updated. Extension API: `latest_github_release`, `github_release_by_tag_name`, `download_file`, `make_file_executable`, `npm_install_package`, `node_binary_path`, `current_platform`, `set_language_server_installation_status`; `Worktree` finds binaries on PATH | Not bundled | Override per server: `lsp.<server>.binary.path / arguments / env / ignore_system_version`. User-restrictable capabilities: `process:exec`, `download_file` (per host), `npm:install` (`granted_extension_capabilities`). "May prompt you to install an extension for known file types." Grammars: extension names a git repo + rev. Some extensions auto-installed (HTML), `auto_install_extensions` opt-out | doc; S (NixOS wiki, commit) |
| Neovim + nvim-lspconfig | LSP client in core (0.11 `vim.lsp.config` / `vim.lsp.enable`); nvim-lspconfig = server configs only | **Installs nothing**: "nvim-lspconfig does not install language servers"; user installs, `cmd` set if not on PATH; `:checkhealth lsp` | Not bundled | | S |
| Neovim + Mason | Same client | **Mason registry**: one YAML spec per package; `source.id` purl with version (`pkg:github/rust-lang/rust-analyzer@...`, also `npm`, `pypi`, `cargo`, `golang`, `gem`, `composer`, `nuget`, `luarocks`, `openvsx`); per-`target` assets (e.g. `linux_x64_gnu`); `bin` map (symlinks on Unix, `.cmd` wrappers on Windows); categories `LSP`, `DAP`, `Linter`, `Formatter`, `Compiler`, `Runtime`; installs under Neovim's data dir, one `bin/` put on PATH; versions bumped by Renovate; JSON schema | Can shell out to cargo, npm, ..., so those must exist | Closest to "one catalogue for servers, linters, formatters, compilers". Third-party registries possible (trust risk). No checksum rule found in the contributing guide | doc, S |
| Helix | ~300 languages with default server names in `languages.toml` | **Installs nothing**: "you must first configure the appropriate Language Server"; `hx --health <lang>` shows what is found | Not bundled | Grammars fetched and compiled (`hx --grammar fetch/build`, needs a C++14 compiler) | doc |
| Sublime Text (LSP package) | LSP client package | Per-server packages (LSP-*); rust-analyzer guide assumes the binary is installed | Not bundled | | S |
| Emacs | lsp-mode / Eglot (built in since 29) | lsp-mode can prompt to install servers (`lsp-install-server`); Eglot installs nothing | Not bundled | | S |
| JetBrains (RustRover, CLion) | Own language engines bundled in the IDE; LSP API for third-party plugins (paid IDEs) | The IDE itself | RustRover: detects the toolchain; if missing, an **"Install Rustup"** button (new project wizard, banner, settings). CLion on Windows: **bundles MinGW-w64, GDB, CMake**; detects Visual Studio and Cygwin | Only family found that bundles a compiler | doc |
| Lapce | LSP client | WASI plugins ("volts") from plugins.lapce.dev; whether they download servers not found | | | S (?) |

## 2. Patterns

| # | Pattern | Who | Good | Bad |
|---|---|---|---|---|
| 1 | **Config only**: editor knows the command; user installs the tool | Helix, Neovim without Mason, Eglot | Smallest app and extensions; nothing to download, update or trust | Worst first run; every tool installed by hand |
| 2 | **Download on demand, PATH first**: use the user's install, else fetch a per-platform build into the app's data dir, keep it updated | Zed, Mason, VS Code rust-analyzer (fallback) | Small app; one-click setup; user's own install wins | Needs downloader, platform matrix, checksums, updates, consent prompt |
| 3 | **Bundled in a per-platform extension package** | VS Code platform VSIX, Ruff | Offline; simple at runtime | Big packages; ~6+ builds per extension; tool update = extension release |
| 4 | **Toolchain helper, not bundle**: detect compiler/SDK, offer to install it | RustRover (Install Rustup), VS Code Go (tools), CLion detection | Compilers stay user-owned and shared with the terminal | Installer flow per toolchain |

Common to all: per-tool binary path override; a health/status view (Zed LSP logs, `hx --health`, `:checkhealth`); trust controls (VS Code signing, Zed capabilities); auto-update with opt-out; suggest an extension when a known file type opens. Only CLion bundles a compiler.

## 3. Open choices for Carbon

Decided (user, 2026-10-19): no preinstalled language tools; users install them via Manage Extensions (`../PROJECT.md`, `../DESIGN.md` s4).

| Question | Options | State |
|---|---|---|
| How an extension gets its tool binary | A config only (pattern 1) / B download, PATH first (pattern 2) / C bundled per platform (pattern 3) | **Open** (user: decide later). Research leans B: dominant in modern editors, matches the "lightweight" pillar and the Mason-style "one catalogue incl. linters/compilers" idea |
| Compilers and toolchains | Detect + offer installer (pattern 4) / bundle (CLion) / config only | Open; every editor except CLion leaves them user-installed |
| Extension format for tools | Declarative manifest (Mason-like: source, per-target assets, `bin`, categories) / WASM code (Zed-like) / both | Open; `../PROJECT.md` already commits WASM + capability grants for extensions |
| Updates | Auto with opt-out / ask / manual | Open; `../PROJECT.md`: no silent extension auto-update |
| Trust | Signed gallery / checksums in manifest / capability prompt (download host, exec) | Open |
| Discovery | Suggest extension when a known file type opens (Zed, VS Code) / none | Open |
| Health view | Status bar line (exists: "not installed" / "stopped") / per-tool health list in Manage Extensions | Open |

## 4. Sources

- VS Code: [Languages overview](https://code.visualstudio.com/docs/languages/overview), [Extension Marketplace](https://code.visualstudio.com/docs/configure/extensions/extension-marketplace) (signing, auto-update, recommendations), [Publishing: platform-specific extensions](https://code.visualstudio.com/api/working-with-extensions/publishing-extension), [vscode-go tools](https://github.com/golang/vscode-go/wiki/tools), [Ruff extension](https://marketplace.visualstudio.com/items?itemName=charliermarsh.ruff).
- rust-analyzer: [VS Code page of the manual](https://rust-analyzer.github.io/book/vs_code.html), [Marketplace page](https://marketplace.visualstudio.com/items?itemName=rust-lang.rust-analyzer).
- Zed: [Rust](https://zed.dev/docs/languages/rust), [Configuring languages](https://zed.dev/docs/configuring-languages), [Language extensions](https://zed.dev/docs/extensions/languages), [Developing extensions](https://zed.dev/docs/extensions/developing-extensions), [Capabilities](https://zed.dev/docs/extensions/capabilities), [zed_extension_api](https://docs.rs/zed_extension_api/latest/zed_extension_api/); S: [NixOS wiki](https://wiki.nixos.org/wiki/Zed_editor), HTML auto-install commit (git.secluded.site mirror).
- Mason: [mason-registry](https://github.com/mason-org/mason-registry), [CONTRIBUTING.md](https://raw.githubusercontent.com/mason-org/mason-registry/main/CONTRIBUTING.md); S: [mason.nvim README](https://github.com/williamboman/mason.nvim).
- Neovim: S: [nvim-lspconfig README](https://github.com/neovim/nvim-lspconfig/), [xnacly: 0.11 LSP changes](https://xnacly.me/posts/2025/neovim-lsp-changes/).
- Helix: [Language support](https://docs.helix-editor.com/lang-support.html), [Building from source](https://github.com/helix-editor/helix/blob/master/book/src/building-from-source.md); S: [LSP config wiki](https://github.com/helix-editor/helix/wiki/Language-Server-Configurations).
- JetBrains: [RustRover toolchain](https://jetbrains.com/help/rust/rust-toolchain.html), [CLion on Windows](https://www.jetbrains.com/help/clion/quick-tutorial-on-configuring-clion-on-windows.html), [LSP API for plugins](https://blog.jetbrains.com/platform/2025/09/the-lsp-api-is-now-available-to-all-intellij-idea-users-and-plugin-developers/).
- Sublime, Emacs, Lapce (S): rust-analyzer docs mirror (Sublime setup), Doom Emacs LSP module README, [Lapce plugin development](https://docs.lapce.dev/development/plugin-development).
- Not verified: Zed default granted capabilities; Mason checksum handling; Lapce server download behaviour; full list of VS Code built-in language servers (JSON not stated on the overview page).
