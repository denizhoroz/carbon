# Plugin / extension architectures for a modular, failure-isolated, versioned workflow system in a Rust GPUI desktop app

Scope note: PROJECT.md, read first, already commits Carbon to "declarative VSIX and LSP/DAP launchers first, WASM (WIT, capability grants) + MCP native, Open VSX + own gallery" and defines a workflow as a `Section` (one Section Bar icon, own layout). I did not re-read RESEARCH.md s4, language-tools.md or ui-framework.md in this pass; the notes below stand on PROJECT.md plus the web sources. Items labelled [BG] are unsourced background knowledge and need verification before use. Pages I tried but could not extract are listed under Gaps. Searches ran roughly 8 over the 15-call guideline; several targets (Theia, Raycast, Helix/Neovim, Godot, Backstage, Fig/Warp, Kubernetes CRDs, Rhai/mlua/Starlark, gRPC plugins, Lapce volt.toml, Blender manifest, Home Assistant manifest) were not verified from primary sources and appear only as [BG] or Gaps.

## 1. Which approach gives the best isolation and compatibility guarantees for a Rust GUI host, and what does each cost to build?

### Takeaway
WASM components (wasmtime + WIT) give the strongest in-process isolation and the best-defined versioned contract, and are what Zed, Zellij, SWC (via wasmer) and Extism chose. Out-of-process stdio protocols (Nushell, LSP/MCP style) give the strongest crash isolation and the simplest versioning, at the cost of IPC latency and no direct UI access. Native dylibs (abi_stable) are fastest but give no isolation. The realistic Carbon design is a hybrid: declarative manifest, WASM for in-process logic, subprocess for heavy tools (LSP/DAP/MCP).

### Cited Findings
- Zed loads each extension's `extension.wasm` into Wasmtime as a component, creates a `Store`, instantiates it and calls `init-extension`; WIT describes the API; two generators (`wit_bindgen::generate!` extension side, `wasmtime::component::bindgen!` host side) read the same WIT. Zed first considered an embedded JS runtime, but preferred Rust extensions; WASM was "not mature enough" until the Component Model. — [Zed Decoded: Extensions](https://zed.dev/blog/zed-decoded-extensions)
- Zed's host bindings are generated with `async: true`, so a blocking extension (e.g. downloading a language server) does not stall the host. The API is deliberately narrow: no panels, no arbitrary HTTP, no free file-system access. — [Zed Decoded: Extensions](https://zed.dev/blog/zed-decoded-extensions)
- Zed extensions compile to `wasm32-wasip2`, crate type `cdylib`, depend on `zed_extension_api`. Most extensions need no Rust at all; only language-server, context-server (MCP) and debugger extensions need custom Rust. Themes, icon themes, snippets, languages are declarative. — [Zed developing extensions](https://zed.dev/docs/extensions/developing-extensions)
- Zellij: "a Webassembly / WASI plugin system"; Zellij's own UI is built from these plugins; plugins render UI, react to state and control Zellij; Rust is the only officially supported language. — [Zellij plugins](https://zellij.dev/documentation/plugins.html)
- Nushell plugins are separate executables exchanging serialized data (JSON or MessagePack) over stdio or a local socket / named pipe; each plugin is its own process. — [Nushell contributor book: plugins](https://www.nushell.sh/contributor-book/plugins.html)
- VS Code runs extensions in a separate Extension Host (local Node.js, web worker, or remote) so they cannot affect startup performance, slow UI operations or modify the UI; extensions load lazily on activation events. — [VS Code Extension Host](https://code.visualstudio.com/api/advanced-topics/extension-host)
- abi_stable: stable ABI is workable but adding custom types is tricky, generics unsupported, incompatible API/library versions panic so you manage versions yourself; native-Rust-ABI dylibs only make sense for crates compiled together. Panics cannot unwind across FFI and must be caught plugin-side (`catch_unwind`); some panics (e.g. in `Future::drop`) abort the whole process. — [ffi_rpc docs](https://docs.rs/crate/ffi_rpc/0.5.0), [async-ffi](https://docs.rs/async-ffi), [Rust forum thread](https://users.rust-lang.org/t/rust-commercial-apps-and-public-sdk-plugins/141984)
- SWC evaluated native abi_stable plugins vs Wasm and chose Wasm (wasmer): native was fast, but publishing CI was very hard and dynamic-library paths are platform-dependent; Wasm lets authors ignore native targets. — [SWC ADR 00001](https://github.com/swc-project/swc/blob/main/docs/adr/00001-support-transformation-plugin-native.md)
- Extism describes itself as a plug-in system where the app decides where user logic runs, what data it receives and returns; official host SDKs exist for several languages (Rust not confirmed on the overview page). — [Extism overview](https://extism.org/docs/overview)
- IntelliJ dynamic plugins (load/unload without restart) are possible but constrained: no components, all extension points used must be marked dynamic, no service overrides, no native libraries (those need `require-restart="true"`), and a failed unload (class-loader leaks) forces an IDE restart. — [IntelliJ dynamic plugins](https://plugins.jetbrains.com/docs/intellij/dynamic-plugins.html)

### Inferences
- Build cost ranking (low to high): declarative-only < out-of-process stdio < Rhai/mlua scripting [BG] < Extism-style WASM wrapper < raw wasmtime component model with own WIT < abi_stable (low build cost but high ongoing per-platform CI cost, per SWC).
- Native dylibs share the host address space and allocator, so a panic abort, memory corruption or deadlock takes down Carbon; this contradicts the stated goal "without breaking other systems", so only first-party/bundled code should ever use them.
- Because JetBrains' unload-in-process model is fragile (leaks force restart) while WASM stores can simply be dropped, a WASM `Store` per workflow gives cleaner remove/disable than JVM-style dynamic loading.
- Direct GPUI view code cannot safely live in a WASM guest or a subprocess (GPUI types are not ABI-stable; gpui-kit breaks every minor per PROJECT.md), so UI must be declarative data or a host-rendered tree [see Q5].
- [BG] Subprocess isolation also protects against host-memory exfiltration only if combined with OS sandboxing; PROJECT.md already lists "OS sandbox on".

### Gaps
- No measured numbers found for WASM component call overhead vs abi_stable vs IPC for UI-rate calls.
- Extism host SDK list, memory/timeout limits, runtime and licence not confirmed (overview page too thin). [BG] Extism is built on wasmtime/wasmer and BSD-3 licensed; unverified.
- mlua/Rhai/Boa/QuickJS/Starlark isolation properties not sourced.

## 2. What contribution-point model (declarative manifest + WASM logic) do the most successful systems use?

### Takeaway
The dominant pattern is: a static manifest declares contributions and activation conditions (so the host can show UI, menus and commands without running extension code), with code loaded lazily only when a contribution is used. Eclipse pioneered it, VS Code popularised it, Zed applies it with a WASM back end.

### Cited Findings
- Eclipse: the declarative part lives in `plugin.xml`, loaded into a registry at startup; the platform can show a plug-in's contributions without starting its code, enabling lazy activation. Extension points define an API; extensions are contributed by one or more plug-ins. Registry objects can become invalid if the declaring plug-in is updated/uninstalled, so clients should listen to registry change events. — [Eclipse FAQ: when does a plug-in get started](https://wiki.eclipse.org/FAQ_When_does_a_plug-in_get_started%3F), [vogella tutorial](https://vogella.com/tutorials/EclipseExtensionPoint/article.html), [Equinox IExtension Javadoc mirror](https://code.yawk.at/org.eclipse.platform/org.eclipse.equinox.registry/3.8.600/org/eclipse/core/runtime/IExtension.java)
- IntelliJ: `plugin.xml` `<extensions defaultExtensionNs="...">` with an element named after the target extension point; extension points are interface-based (`implementation` class) or bean-based (XML properties only, declarative); an extension namespace can be another plugin's ID (requires a dependency); implementations must be stateless with no constructor initialisation. — [IntelliJ extensions](https://plugins.jetbrains.com/docs/intellij/plugin-extensions.html)
- VS Code: activation events (`onCommand`, `onLanguage`, `onView`, `workspaceContains`, `onStartupFinished`, `*`, etc.); since 1.74 contributed languages, commands, views, custom editors, authentication providers no longer need explicit activation events (implicit activation from `contributes`). — [VS Code activation events](https://code.visualstudio.com/api/references/activation-events)
- Zed `extension.toml` carries id, name, version, schema_version, authors, description, repository; capabilities are languages, debuggers, themes, icon themes, snippets, MCP servers; each category is largely declarative with Rust/WASM only for dynamic parts (language server command, context server, debugger). — [Zed developing extensions](https://zed.dev/docs/extensions/developing-extensions)
- Obsidian `manifest.json`: required `id`, `name`, `version` (semver), `minAppVersion`, `description`, `author`, `isDesktopOnly`; community directory enforces naming and uniqueness rules. — [Obsidian manifest](https://docs.obsidian.md/Reference/Manifest)
- Grafana signs plugins; `MANIFEST.txt` signature manifest holds `manifestVersion`, `signatureType`, `signedByOrg`, `plugin`, `version`, `keyId`, `files` (per-file hashes). — [Grafana: sign a plugin](https://grafana.com/developers/plugin-tools/publish-a-plugin/sign-a-plugin)
- Nushell: `plugin add` records plugin command signatures in a registry file so commands are known in later sessions; `plugin use` activates. — [Nushell plugins](https://www.nushell.sh/contributor-book/plugins.html)

### Inferences
- For Carbon a workflow manifest (e.g. `workflow.toml`) should declare: id, name, version, `carbon_api` range, section icon, default layout (panel ids, dock positions), commands, menus, keybindings, panel declarations, required capabilities, activation (`onSectionOpen`, `onCommand`), and an optional `wasm` entry. Layouts and commands being pure data means a workflow with no code still works (Zed: "most extensions need no Rust").
- Registry-with-change-events (Eclipse) is the right shape for Manage Workflows: Section Bar, command palette and menus subscribe to a registry that is mutated on install/remove/enable.
- Persisting contribution signatures (Nushell) lets the Section Bar draw icons at startup without spawning anything.
- [BG] Theia reuses VS Code's `contributes`; Raycast uses package.json with `commands` and a React-like API rendered natively by the host (the closest precedent for host-rendered declarative UI from extension code); Home Assistant uses `manifest.json` with domain/dependencies/requirements; Kubernetes CRDs are declarative schemas reconciled by operators. None were verified here.

### Gaps
- Theia, Raycast, Backstage, Godot EditorPlugin, Helix/Neovim, Blender manifest, Home Assistant manifest, Lapce volt.toml, Fig/Warp, and Kubernetes pages not retrieved (fetch failed, redirected, or out of call budget).

## 3. How do they version the host API and keep old extensions running?

### Takeaway
Best practice combines (a) the extension declaring the API version it was built for, (b) the host keeping multiple binding sets and selecting by version at load, and (c) a hard "minimum host version" gate. Zed does exactly this with WIT; Nushell uses a handshake with a strict pre-1.0 minor match; Obsidian uses `minAppVersion`.

### Cited Findings
- Zed: the compiled-against API version is recorded in the expanded `extension.toml`; at load Zed reads the version from the Wasm binary and chooses matching bindings; Dev/Nightly may use the latest API version while Stable/Preview do not. — [Zed Decoded: Extensions](https://zed.dev/blog/zed-decoded-extensions)
- Zed compatibility table: e.g. Zed 0.192.x supports `zed_extension_api` 0.0.1 to 0.6.0; Zed 0.149.x supports 0.0.1 to 0.1.0; every Zed version still supports the earliest 0.0.1 API. Reverse is not supported: "Extensions created using newer versions of the Zed extension API won't be compatible with older versions of Zed." — [zed_extension_api README](https://github.com/zed-industries/zed/blob/main/crates/extension_api/README.md)
- WIT packages carry optional semver (`package documentation:example@1.0.1`); resolution is tool-defined. — [Component Model: WIT](https://component-model.bytecodealliance.org/design/wit.html)
- Nushell: plugin and engine exchange `Hello`; pre-1.0 the minor version must match (a plugin built against 0.116.0 works with any 0.116.x, not 0.115 or 0.117); if incompatible the plugin exits without answering. — [Nushell plugins](https://www.nushell.sh/contributor-book/plugins.html)
- Obsidian: `minAppVersion` required in every manifest. — [Obsidian manifest](https://docs.obsidian.md/Reference/Manifest)
- abi_stable: incompatible versions panic and the host must manage versioning itself. — [ffi_rpc docs](https://docs.rs/crate/ffi_rpc/0.5.0)

### Inferences
- Adopt Zed's model: versioned WIT packages (`carbon:workflow@0.1.0`, `@0.2.0`, ...), host keeps `bindgen!` modules for the last N versions and adapts old to new; manifest field `api = "0.2"`; semver-range `carbon = ">=0.9"`.
- Zed's table shows the cost: every old API version stays in the host forever (or is deprecated with notice). Budget for a supported-versions window and a deprecation policy; show "built for old API, may be removed" in Manage Workflows.
- Declarative data (layout, commands, menus) should have its own `schema_version` independent of the WASM API (Zed's `schema_version`).
- Since gpui-kit breaks every minor (PROJECT.md), the plugin API must never expose gpui types; this is the single most important compatibility decision.

### Gaps
- Zed's exact policy on dropping old API versions not found.
- VS Code `engines.vscode` semantics, IntelliJ `since-build`/`until-build`, Grafana `grafanaDependency` not verified from a primary page (Grafana page fetched did not mention it).

## 4. Capability / permission model, isolation controls, and rust-specific mechanisms

### Takeaway
Wasmtime gives three independent limiters (fuel, epoch deadlines, ResourceLimiter) plus capability-by-import via WIT worlds and WASI; Zed layers a user-grantable capability policy on top. Without WASM, capabilities need OS sandboxing.

### Cited Findings
- Wasmtime `consume_fuel`: deterministic; traps when fuel is out; a new `Store` starts with zero fuel. `epoch_interruption`: lighter (often faster than fuel, sometimes 2-3x), non-deterministic; deadline set per `Store` with `set_epoch_deadline` else traps immediately; behaviour at deadline: trap, callback, or async yield; does not wake guests blocked in host calls; bulk-memory ops such as `memory.copy` check the epoch once, so bound heap with a `ResourceLimiter` for strict limits; `max_wasm_stack` default 512 KiB (stack overflow traps); exhausting the host thread stack can abort the process. — [wasmtime Config docs](https://docs.rs/wasmtime/latest/wasmtime/struct.Config.html)
- A WIT world is the contract: imports are what a component needs from the host, exports what it provides; a world importing an interface must be given the full interface; imports/exports are per interface. — [Component Model: WIT](https://component-model.bytecodealliance.org/design/wit.html)
- Zed grants: `granted_extension_capabilities` user setting with `process:exec` (command+args patterns), `download_file` (host, path), `npm:install` (package); calls without capability return an error; removing all will make many extensions non-functional. — [Zed capabilities docs (search result)](https://zed.dev/docs/extensions/capabilities), [Zed commit #39472](https://git.secluded.site/zed/commit/3bf71c690f53554d4bb44291b4875db40ee5e967)
- Reported wasmtime advisories: CVE-2026-27195, epoch-related panic in `call_async` (Wasmtime 39+, fixed in 40.0.4 and 41.0.4); a component-model trampoline crash fixed in 38.0.3. Secondary source (ALT Linux errata); verify on wasmtime's own advisory page. — [ALT errata 5689](https://errata.altlinux.org/ALT-PU-2026-5689), [ALT errata 7613](https://errata.altlinux.org/ALT-PU-2026-7613)
- One third-party host (forge-host) links no WASI at all so plugins get no FS/network/clock; all access goes through WIT host functions. — [forge-host docs](https://docs.rs/forge-host) (small project; indicative only)
- Nushell plugins run as separate processes, idle ones are garbage collected via `Goodbye`; the page does not describe crash/hang behaviour. — [Nushell plugins](https://www.nushell.sh/contributor-book/plugins.html)
- Obsidian `isDesktopOnly` flags NodeJS/Electron API use (a coarse capability flag). — [Obsidian manifest](https://docs.obsidian.md/Reference/Manifest)

### Inferences
- Recommended per-call envelope: fresh or pooled `Store` per workflow, epoch deadline (wall-clock, via a ticker thread) as the hang guard, `ResourceLimiter` for memory, fuel only where determinism is wanted; a trap returns `Err` to the host, which marks the workflow "faulted", shows a banner, and optionally auto-disables after N faults in M minutes. Hung host calls (blocking I/O the guest requested) need their own timeouts because epochs do not interrupt them.
- Capability model for Carbon: workflow manifest lists requested capabilities (fs:read workspace, process:exec patterns, net hosts, agent:invoke, settings namespace); the install dialog shows them; grants are stored per user; denied calls return typed errors (Zed pattern).
- Pin wasmtime and track security releases; the CVE items show epoch/async paths have had bugs, so do not treat limits as unconditional.
- [BG] Rust scripting options: Rhai (pure Rust, easy sandbox, op/depth limits, slow), mlua/Luau (Luau designed for sandboxing, C dependency), QuickJS/Boa (JS ecosystem familiarity), Starlark (deterministic, hermetic, used by Bazel/Buck2). All run in-process with no memory isolation from host bugs beyond what the interpreter enforces; good for lightweight commands, weaker than WASM for third-party code. Unverified here.
- [BG] gRPC/JSON-RPC subprocess plugins (LSP, MCP, Terraform go-plugin) allow any language and full crash isolation; version negotiation is via handshake (as Nushell Hello). Carbon already commits to MCP and LSP/DAP this way.

### Gaps
- Official wasmtime security/LTS policy text not retrieved (page truncated).
- No source on component-model resource-handle lifetime semantics or WASI preview 2/3 async status as of 2026.
- Extism's limits (memory, timeouts, allowed hosts) not retrieved.

## 5. UI contribution: declarative vs imperative, and what a GPUI host needs

### Takeaway
Successful systems keep extension code out of the host's render path: VS Code contributes views as data and lets extensions fill them via providers or webviews; Zed's WASM API has no panel API; Zellij's plugins render into host-managed panes. For GPUI, workflows should contribute a declarative layout plus a host-owned widget vocabulary.

### Cited Findings
- VS Code's Extension Host exists so extensions cannot slow UI operations or modify the UI directly. — [VS Code Extension Host](https://code.visualstudio.com/api/advanced-topics/extension-host)
- Zed's extension API lacks panels, arbitrary HTTP and free filesystem access, by design. — [Zed Decoded: Extensions](https://zed.dev/blog/zed-decoded-extensions)
- Zellij: plugins "can render a UI" and are first-class workspace components like terminal panes; details on a separate rendering page (not read). — [Zellij plugins](https://zellij.dev/documentation/plugins.html)
- IntelliJ bean-based extension points configure host features via XML attributes only (declarative); interface-based ones run plugin code. — [IntelliJ extensions](https://plugins.jetbrains.com/docs/intellij/plugin-extensions.html)

### Inferences
- GPUI host requirements: (1) a `WorkflowRegistry` entity that GPUI views observe (Section Bar, menus, command palette); (2) panel kinds registered by id in the host, workflows reference them in a layout manifest (kit docking already persists layout); (3) a small serialisable UI-tree vocabulary (list, tree, text, form, table, button, markdown, diagram/canvas) rendered by host GPUI code, with events sent back to the guest as messages; (4) built-in panel kinds first (editor, terminal, chat, explorer) so most workflows are pure layout; (5) never expose gpui types in WIT.
- Imperative GPUI views remain possible only for first-party workflows compiled into the host (Code Editor, Paradigm Editor, Agent Automation), registered through the same registry trait.
- [BG] Webview-based contribution is excluded by PROJECT.md; the host-rendered UI tree substitutes for it.

### Gaps
- Zellij UI-rendering page, Raycast's host-rendered React model, and Lapce/Zed panel proposals not read.
- No source found for GPUI-specific plugin precedents.

## 6. Install / uninstall / enable / disable / update, dependencies, registry and signing, hot reload

### Takeaway
Dev install overrides the published version (Zed); IntelliJ supports dynamic load/unload with hard constraints; Grafana enforces signed plugins by default; marketplaces are the main supply-chain exposure.

### Cited Findings
- Zed: "Install Dev Extension" overrides an installed published version, which shows "Overridden by dev extension"; publishing has prerequisites. — [Zed developing extensions](https://zed.dev/docs/extensions/developing-extensions)
- Grafana requires plugins to be signed before loading by default; unsigned allowed in development mode; public plugins are signed Community or Commercial after Grafana team review; private plugins require signing with `rootUrls`. — [Grafana: sign a plugin](https://grafana.com/developers/plugin-tools/publish-a-plugin/sign-a-plugin)
- IntelliJ: uninstall/update waits for unload; if unload fails the IDE asks for restart; leaks via class loader are the common cause. — [IntelliJ dynamic plugins](https://plugins.jetbrains.com/docs/intellij/dynamic-plugins.html)
- IntelliJ plugins can extend another plugin's extension points only with a declared plugin dependency. — [IntelliJ extensions](https://plugins.jetbrains.com/docs/intellij/plugin-extensions.html)
- Nushell: plugins GC-stopped when idle; `plugin add/use/list/stop`. — [Nushell plugins](https://www.nushell.sh/contributor-book/plugins.html)
- Eclipse registry supports listeners for extension add/remove at runtime. — [Equinox IExtension Javadoc mirror](https://code.yawk.at/org.eclipse.platform/org.eclipse.equinox.registry/3.8.600/org/eclipse/core/runtime/IExtension.java)

### Inferences
- Lifecycle states for Carbon: installed -> enabled -> active (lazy) -> faulted -> disabled; hot enable/disable is cheap for declarative-only and WASM workflows (drop Store, unregister contributions, close panels). Updates: install new version side by side, swap atomically, keep previous for rollback.
- Dependencies between workflows should be minimal: let a workflow depend on host capabilities/APIs and on other extensions only via declared ids and version ranges; resolve at install time with a refusal on conflict (IntelliJ pattern). Avoid transitive plugin-to-plugin runtime calls initially.
- Signing: start with a content hash manifest (Grafana-style file list + hashes) and signature by registry key; unsigned = explicit user warning/dev mode.

### Gaps
- Open VSX publisher verification/namespace policy not retrieved; Zed's registry (a git repo of submodules) workflow not verified.
- Hot-reload behaviour of Zed/Lapce/Zellij not sourced.

## 7. Safe mode / disable all / bisect

### Takeaway
VS Code's Extension Bisect (binary search over enabled extensions) is the documented model; combine it with a start-up safe mode flag and auto-disable on repeated faults.

### Cited Findings
- VS Code "Help: Start Extension Bisect": first disables all enabled extensions to test whether VS Code itself is at fault, then halves the remaining set each round; ~4 to 5 rounds for 24 extensions; user answers "Good now" / "This is bad"; a known-good extension can be excluded by disabling it first. — [VS Code Extension Bisect](https://code.visualstudio.com/blogs/2021/02/16/extension-bisect)
- Zed users can strip capabilities via settings to restrict extensions (partial "least-privilege mode"). — [Zed capabilities docs (search result)](https://zed.dev/docs/extensions/capabilities)

### Inferences
- Carbon: `carbon --safe-mode` (and Shift-at-launch) loads only built-in Code Editor workflow, skips all third-party workflows and extensions; persisted `enabled` set untouched so the user can return. Manage Workflows offers "Bisect" using the same algorithm, plus automatic quarantine of a workflow that traps/crashes N times, with a crash log. Startup journaling ("last launch did not finish: offer safe mode") fits the write-ahead journal already planned.
- [BG] VS Code also has `--disable-extensions` and a safe-mode banner; not verified here.

### Gaps
- Safe-mode docs for Zed, JetBrains (`-Didea.load.plugins`), Eclipse (`-clean`) not retrieved.

## 8. Known failures: supply chain, host crashes, breaking API changes

### Takeaway
Marketplaces with broad-permission extensions are repeatedly abused; in-process loading amplifies plugin bugs; fast-moving APIs strand extensions.

### Cited Findings
- GlassWorm (first detected 17 Oct 2025): self-spreading malware via VS Code extensions on Open VSX and one on Microsoft's Marketplace; used invisible Unicode characters to hide code, Solana blockchain for command-and-control and Google Calendar as fallback; spread by stealing victims' credentials to infect other extensions; roughly 35,000+ downloads reported (figures vary). Reported by Koi Security; coverage is mostly vendor and news sources. — [The Hacker News](https://thehackernews.com/2025/10/self-spreading-glassworm-infects-vs.html), [Hackmag](https://hackmag.com/news/glassworm), [Teiss](https://www.teiss.co.uk/news/self-spreading-glassworm-targets-vs-code-extensions-in-a-major-supply-chain-attack-16602)
- IntelliJ: unload leaks force IDE restart; third-party paid plugins cannot be installed/updated/removed without restart. — [IntelliJ dynamic plugins](https://plugins.jetbrains.com/docs/intellij/dynamic-plugins.html)
- Wasmtime CVE-2026-27195 and a component-model trampoline crash (see section 4). — [ALT errata](https://errata.altlinux.org/ALT-PU-2026-5689)
- Nushell: pre-1.0 plugins break every minor release (strict minor match). — [Nushell plugins](https://www.nushell.sh/contributor-book/plugins.html)
- Zed: extensions built on newer API fail on older Zed; API went 0.0.1 to 0.6.0 in under two years of Zed versions. — [zed_extension_api README](https://github.com/zed-industries/zed/blob/main/crates/extension_api/README.md)

### Inferences
- Open VSX is Carbon's planned registry and was a direct GlassWorm victim: plan for publisher verification, hash-pinned installs, malware scanning, install-time capability display, update review/delay, and kill-list (revocation) support.
- Declarative-only workflows are inherently immune to most of this class; WASM with minimal grants limits blast radius even if a publisher account is compromised (unlike VS Code extensions, which run with full user privileges [BG]).
- [BG] Well-known earlier incidents (VS Code extension typosquats, Chrome extension takeovers, npm event-stream) were not verified here; do not cite without sources.

### Gaps
- No sourced incident of a plugin crashing a host process (e.g. native Neovim/IDE plugin) found; no breaking-API-change post-mortem found for Obsidian, Grafana or VS Code.
- No sourced detail of Grafana's backend/frontend plugin split (page fetched did not cover it).

## Suggested minimum design (inference, for the report writer)
1. Declarative `workflow.toml` (layout, commands, menus, keybindings, capabilities, activation) with its own `schema_version`.
2. Optional WASM component implementing a versioned `carbon:workflow` WIT world; host keeps last N API bindings; no gpui types across the boundary.
3. Heavy/foreign tooling as subprocesses (LSP/DAP/MCP) with handshake versioning.
4. Per-workflow Store with epoch deadline + ResourceLimiter, fault counter and quarantine.
5. User-granted capabilities (Zed model), signed packages with hashes (Grafana model), safe mode and bisect (VS Code model).
6. First-party workflows (Code Editor, Paradigm Editor, Agent Automation) register through the same registry trait, optionally compiled-in.
