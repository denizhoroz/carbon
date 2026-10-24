# Making Carbon extensions

Carbon ships small: no languages, colour themes or language servers are built in beyond its own two themes. Everything else comes from **extensions**. This guide shows how to write one, from a simple theme to an extension with its own code.

**Contents**

1. [What an extension is](#1-what-an-extension-is)
2. [Your first extension in five minutes](#2-your-first-extension-in-five-minutes)
3. [The manifest: `extension.toml`](#3-the-manifest-extensiontoml)
4. [Recipes](#4-recipes): [theme](#41-a-colour-theme) · [language](#42-a-language-colours) · [language server](#43-a-language-server) · [settings](#44-settings) · [commands and menus](#45-commands-and-menus) · [panels and views](#46-panels-and-views) · [code](#47-code-webassembly) · [graphs and agents](#48-graphs-and-agents-api-02)
5. [Permissions (capabilities)](#5-permissions-capabilities)
6. [Rules and limits](#6-rules-and-limits)
7. [Testing and troubleshooting](#7-testing-and-troubleshooting)
8. [Sharing your extension](#8-sharing-your-extension)

---

## 1. What an extension is

An extension is **a folder** with an `extension.toml` file at its root, plus the files that manifest names:

```
my-theme/
  extension.toml        ← the manifest: what the extension is and adds
  icon.png              ← optional icon
  themes/
    my-theme.json
```

What an extension can add:

| You want to add | You write | Code needed? |
|---|---|---|
| Colour themes | a theme JSON file | No |
| Syntax colours for a language | a tree-sitter grammar (`.wasm`) and its queries | No |
| A language server (completions, errors, go to definition) | the program's name, and optionally download links | No |
| Settings (shown in Settings) | `[[settings]]` entries | No |
| Commands (palette, menus) | `[[commands]]` and `[[menus]]` entries | Only to run your own logic |
| Panels | a view described in the manifest | Only for views that change |
| Custom logic | a WebAssembly component written in Rust | Yes |

Most extensions need no code at all.

---

## 2. Your first extension in five minutes

A colour theme is the simplest extension.

**1. Make a folder** with this `extension.toml`:

```toml
id = "yourname.midnight"          # publisher.name, lowercase
name = "Midnight"
version = "0.1.0"
schema_version = 1                # always 1 for now
carbon_api = ">=0.2, <0.3"        # the Carbon extension API you support
description = "A dark blue theme."
categories = ["theme"]

[[themes]]
file = "themes/midnight.json"
```

**2. Add the theme file** at `themes/midnight.json`. The easiest start is to copy an existing theme, for example [Gruvbox](https://github.com/denizhoroz/carbon-extensions-basic/blob/main/themes/gruvbox/themes/gruvbox.json), rename it and change the colours.

**3. Load it in Carbon:** open **Extensions** (status bar menu, or the command palette), then **Install ▾ > Link Development Folder…**, and pick your folder.

Your theme now appears in **Settings > Appearance**. After you edit any file, click **Refresh** in the Extensions dialog to load your changes.

A linked folder is loaded straight from where it is, with no copying, so it's the way to develop. Installing (section 8) copies a package into Carbon.

---

## 3. The manifest: `extension.toml`

### Top-level fields

| Field | Required | What it is |
|---|---|---|
| `id` | yes | `publisher.name`: two or more parts of lowercase letters, digits and `-`, joined by `.` (`acme.sketch`). Never change it after you publish. **`carbon.*` is reserved** for official extensions. |
| `name` | yes | Shown to people (`"Sketch"`). |
| `version` | yes | [Semantic version](https://semver.org) (`"1.2.0"`). |
| `schema_version` | yes | Manifest format. Always `1` today. |
| `carbon_api` | yes | Version range of Carbon's extension API you support. Today Carbon offers `0.1` and `0.2`; use `">=0.2, <0.3"` (the SDK builds code for 0.2, and graphs, agents and events need it). |
| `description` | no | One sentence, shown in the Extensions list. |
| `authors` | no | `["Ada <ada@example.com>"]` |
| `repository` | no | Source URL. |
| `icon` | no | Image file in the package: `png`, `svg`, `jpg` or `webp`, square (128 px, or SVG), at most 256 KB. Without one, Carbon shows a placeholder. |
| `categories` | no | Any of `language`, `theme`, `tool`, `panel`, `agent`, `other`. |
| `capabilities` | no | Permissions the extension asks for (section 5). |
| `wasm` | no | The extension's code, a WebAssembly component (section 4.7). |
| `[requires]` | no | Other extensions it needs (below). |

Then the contributions, each covered in section 4: `[[languages]]`, `[[grammars]]`, `[[language_servers]]`, `[[themes]]`, `[[settings]]`, `[[commands]]`, `[[menus]]`, `[[panels]]`, `[views]`.

Unknown top-level keys are ignored with a warning. Unknown keys *inside* a contribution are an error, so typos are caught.

### Requiring other extensions

```toml
[requires]
extensions = { "acme.base" = ">=1.0" }
```

Carbon never installs a requirement on its own. If one is missing, your extension stays loaded but unused, and the Extensions dialog shows what's needed, with an Install button when the catalog has it.

---

## 4. Recipes

### 4.1 A colour theme

```toml
[[themes]]
file = "themes/midnight.json"
```

One file may hold several themes, for example a dark and a light one. Each theme has:
- **`name`** and **`mode`** (`"dark"` or `"light"`);
- **`colors`**: interface colours;
- **`highlight`**: editor syntax colours and terminal ANSI colours.

Missing colour keys fall back to defaults. If two themes share a name, the first one loaded wins, so pick a distinctive name.

### 4.2 A language (colours)

A language needs two things: which files it covers (`[[languages]]`), and a tree-sitter grammar compiled to WebAssembly that colours them (`[[grammars]]`).

```toml
[[languages]]
id = "zig"                    # used by language servers and the status bar
name = "Zig"
suffixes = ["zig", "zon"]     # file endings without the dot, any case
file_names = ["build.zig"]    # whole file names, exact
grammar = "zig"               # a [[grammars]] name (yours or another extension's)
line_comment = "//"

[[grammars]]
name = "zig"                          # the .wasm must export tree_sitter_zig
wasm = "grammars/zig.wasm"
highlights = "grammars/zig/highlights.scm"
injections = "grammars/zig/injections.scm"   # optional: code inside code
locals = "grammars/zig/locals.scm"           # optional
```

**Getting the `.wasm`:**
- Many grammars publish one with each release (`tree-sitter-<lang>.wasm`).
- Otherwise build it from the grammar's repository with the tree-sitter CLI: `tree-sitter build --wasm`.

**Getting the queries:** the queries (`highlights.scm` and the others) come from the grammar's `queries/` folder. Use the queries from the same version as the `.wasm`.

### 4.3 A language server

```toml
capabilities = ["process.exec:zls", "net:github.com"]

[[language_servers]]
id = "zls"                    # lowercase letters, digits, -
name = "ZLS"
languages = ["zig"]           # language ids it serves
command = "zls"               # looked up on PATH (.exe added on Windows)
args = []
install_hint = "Install ZLS from https://github.com/zigtools/zls/releases"
```

Carbon looks for the program in this order:
1. If your extension has code, its `language_server_command` (section 4.7) may give the command.
2. The program on `PATH`.
3. A copy Carbon downloaded earlier.
4. A download, only after the user agrees (section 4.3.1).

If none of these works, the status bar shows the server as missing, with your `install_hint`.

A language server needs **`process.exec:<command>`** in `capabilities`. Without it, the manifest is refused.

#### 4.3.1 Offering a download

Add one download per platform you support. Platform names: `windows-x64`, `windows-arm64`, `macos-x64`, `macos-arm64`, `linux-x64`, `linux-arm64`.

```toml
[language_servers.download.windows-x64]
url = "https://github.com/zigtools/zls/releases/download/0.14.0/zls-x86_64-windows.zip"
sha256 = "<64 lowercase hex digits of the file>"
version = "0.14.0"
archive = "zip"           # none | gz | zip | tar-gz
bin = "zls.exe"           # the program's path inside the archive
```

Rules for downloads:
- **`https://` only.**
- The URL's host needs `net:<host>` in `capabilities`.
- **The SHA-256 must match**, or the download is refused. Get it with `sha256sum file` (Linux/macOS) or `certutil -hashfile file SHA256` (Windows).
- Archives can't write outside their folder (no `..`, no links).
- Bump `version` together with the URL. Carbon keeps one folder per version.

The Rust extension is a complete real example: [languages/rust/extension.toml](https://github.com/denizhoroz/carbon-extensions-basic/blob/main/languages/rust/extension.toml).

### 4.4 Settings

Settings appear in **Settings** under your extension's name, saved as `<extension id>.<key>`.

```toml
[[settings]]
key = "check.command"         # words separated by . or _
label = "Check Command"
description = "Command run on save."
kind = "choice"               # bool | choice | integer | number | text
default = "check"
choices = [
  { value = "check", label = "cargo check" },
  { value = "clippy", label = "cargo clippy" },
]
category = ["Checks"]         # optional sub-category
# min / max / step            for integer and number
# user_only = true            not settable per folder
# restart = true              "takes effect after restart"
```

Your code reads a setting with `setting("check.command")` (section 4.7).

### 4.5 Commands and menus

A command appears in the command palette (F1). There are three kinds:

```toml
# Runs your code's run_command("count")
[[commands]]
id = "count"
title = "Count Files"
keywords = ["files", "statistics"]

# Opens one of your panels
[[commands]]
id = "open-home"
title = "Open Sketch Home"
panel = "home"

# Runs an existing Carbon action
[[commands]]
id = "new"
title = "New Sketch File"
action = "carbon::NewFile"

# Puts a command at the end of a top-level menu: File, Edit, View or Help
[[menus]]
menu = "View"
command = "open-home"
```

### 4.6 Panels and views

A panel is a tab the user can place anywhere: View > Add Panel, or a command with `panel = …`.

```toml
[[panels]]
id = "home"
title = "Sketch"
kind = "view"                 # drawn by Carbon from the manifest
view = "home"                 # a name under [views]
placement = "right"           # center | left | right | bottom (where it opens first)
# explorer_section = true     a tab beside the Explorer instead
# multi = true                more than one may be open
# dynamic = true              your code draws it (render_view), redrawn after each command

[[views.home]]
type = "heading"
text = "Sketch"

[[views.home]]
type = "text"
text = "Diagrams for your project."
muted = true

[[views.home]]
type = "row"
children = [
  { type = "button", label = "New Diagram", command = "new", style = "primary" },
  { type = "button", label = "Count", command = "count" },
]

[[views.home]]
type = "list"
items = [
  { label = "Recent", description = "Last opened diagrams", command = "open-home" },
]
```

| Widget | Fields |
|---|---|
| `heading` | `text` |
| `text` | `text`, `muted` |
| `markdown` | `text` (links open in the browser) |
| `button` | `label`, `command`, `icon`, `style` (`secondary`, `primary`, `ghost`) |
| `row` | `children` (side by side) |
| `column` | `children`, `align` (`start`, `center`) |
| `divider` | none |
| `list` | `items`: `label`, `description`, `icon`, `command` |

Buttons and list items run your `[[commands]]`, so a view can do only what a command can.

**Limits:** at most 8 levels of `row`/`column` nesting and 500 widgets per view.

> **Webview panels** (`kind = "webview"`, a web page from your package) exist but are experimental. They're off in normal Carbon builds, so don't rely on them yet.

### 4.7 Code (WebAssembly)

For logic, write a small Rust crate and compile it to a WebAssembly component. It runs inside a sandbox: it can only reach things through Carbon's functions, and only with the permissions the user granted.

**One-time setup:** `rustup target add wasm32-wasip2`

**`Cargo.toml`**

```toml
[package]
name = "sketch"
version = "0.1.0"
edition = "2024"

[lib]
crate-type = ["cdylib"]

[dependencies]
carbon-extension-api = { git = "https://github.com/denizhoroz/carbon", package = "carbon-extension-api" }
```

**`src/lib.rs`**

```rust
use carbon_extension_api::{
    Event, Extension, ServerCommand, graphs, notify, register_extension, workspace_root,
};

struct Sketch;

impl Extension for Sketch {
    // Called once after loading.
    fn init() -> Result<(), String> {
        Ok(())
    }

    // Runs your [[commands]] that have neither `action` nor `panel`.
    fn run_command(id: String) -> Result<(), String> {
        match id.as_str() {
            "count" => {
                let root = workspace_root().unwrap_or_else(|| "no folder open".into());
                notify(&format!("Counting in {root}"));
                Ok(())
            }
            other => Err(format!("unknown command {other}")),
        }
    }

    // Lets you compute a language server's command; None uses the manifest's.
    fn language_server_command(_server_id: String) -> Result<Option<ServerCommand>, String> {
        Ok(None)
    }

    // Draws `dynamic = true` views: a JSON list of widgets, same format as [views].
    fn render_view(_view: String) -> Result<String, String> {
        Ok(r#"[{"type":"text","text":"Hello"}]"#.into())
    }

    // Something you started finished (an agent answered, a timer fired, ...).
    fn on_event(_event: Event) -> Result<(), String> {
        Ok(())
    }

    // Your own checks of a graph file of your kinds (section 4.8): a JSON list.
    fn check_graph(_doc: graphs::GraphDoc) -> Result<String, String> {
        Ok("[]".into())
    }
}

register_extension!(Sketch);
```

**Build:** `cargo build --release --target wasm32-wasip2`. Then copy `target/wasm32-wasip2/release/sketch.wasm` into your package and point the manifest at it:

```toml
wasm = "sketch.wasm"
```

**What your code can call:**

| Function | Does | Needs |
|---|---|---|
| `log(msg)` | Writes to Carbon's log | none |
| `notify(msg)` | Shows a short message in the window | none |
| `setting(key)` | One of your settings, as JSON text (`None` if not yours) | none |
| `workspace_root()` | The open folder, if any | none |
| `read_file(path)` | Reads a UTF-8 file | `fs.read:workspace` (open folder) or `fs.read:package` (your package, relative paths) |
| `run_process(program, args)` | Runs a program in the open folder and waits | `process.exec:<program>` |
| `http_get(url)` | Fetches an `https://` URL | `net:<host>` |
| `write_file(path, text)` | Writes a UTF-8 file in the open folder (folders made, atomic) | `fs.write:workspace` |
| `storage_get/set/delete/keys` | Your key/value storage for the open folder; survives restarts (1 MiB a value, 16 MiB in all) | none |
| `set_timer(ms, tag)`, `now_ms()` | `on_event(TimerFired(tag))` after `ms`; the time | none |
| `jobs::start_job/set_job_progress/finish_job` | A job in the status bar, with Stop (`JobCancelRequested`) | none |
| `acp::*`, `graphs::*`, `approvals::*` | Agents, graph runs, questions for the user (section 4.8) | `agent.invoke` for agents |

A call without the needed permission returns an error, so handle `Err`.

**Every call returns at once.** Work that takes longer (an agent answering, a timer, the user deciding) comes back later through `on_event`. Carbon may reload your code between events, so keep what you need in storage, not in memory.

**Limits:**
- each call may run at most **5 seconds**, including waiting on `run_process` or `http_get`;
- each extension may use at most **256 MB** of memory.

An extension that crashes, panics or runs too long is **turned off** and the user is told. They can turn it back on in the Extensions dialog.

A complete example with commands, a dynamic view and permission checks: [`apps/carbon-extension-api/examples/hello`](../apps/carbon-extension-api/examples/hello/src/lib.rs).

### 4.8 Graphs and agents (API 0.2)

An extension can add a **graph panel**: a canvas of blocks joined by arrows that Carbon draws and edits (pan, zoom, connect, undo, autosave). The graph is a YAML file in the project. You declare the kind of graph and its block types; your code decides what running one means. The [Agents extension](https://github.com/denizhoroz/carbon-extensions-agents) is a complete example.

```toml
carbon_api = ">=0.2, <0.3"
wasm = "flows.wasm"

[[graphs]]
id = "flow"
name = "Agent flow"
dir = ".carbon/flows"              # where new files go
suffix = "flow.yaml"               # files ending like this open in the panel
check = true                       # also ask your code's check_graph
actions = [
  { id = "run", title = "Run", icon = "play", primary = true, when = "idle" },
  { id = "stop", title = "Stop", icon = "square", when = "running" },
]
fields = [ { key = "task", label = "Task", kind = "text", multiline = true } ]

[[node_types]]
graph = "flow"
id = "agent"
title = "Agent"
icon = "bot"
category = "Agents"
inputs = [ { id = "in", label = "In", multi = true } ]
outputs = [ { id = "out", label = "Out", multi = true } ]
fields = [
  { key = "prompt", label = "Prompt", kind = "text", multiline = true, required = true },
  { key = "agent", label = "Agent", kind = "agent", default = "claude-code" },
]

[[panels]]
id = "flows"
title = "Flows"
kind = "graph"
graph = "flow"
placement = "center"
```

- **Ports:** `multi = true` lets several arrows use one port. An input with `loop_back = true` is the only place arrows may go round in a circle. `outputs_from = "<list field>"` makes one output per list item (by its `id`), plus Else: that's how a router gets its routes.
- **Field kinds:** `bool`, `text` (`multiline`), `integer`, `number` (`min`, `max`), `choice` (`choices`), `list` (`item_fields`), `agent` (picks from every extension's `[[agents]]`), `file`. There is no secret kind: keys never go in a graph file.
- **Carbon checks** types, ports, arrows, `max_count` and `required` fields itself. `check_graph` adds yours: a JSON list of `{ "node", "edge", "severity": "error"|"warning"|"info", "message" }`.
- **Actions** send `on_event(GraphAction { path, action, selection })`; the file is saved first.

**Agents** are programs that speak ACP (the Agent Client Protocol). Declare them like language servers:

```toml
capabilities = ["agent.invoke", "process.exec:claude-agent-acp", "process.exec:node",
                "process.exec:npm", "process.exec:npx", "net:registry.npmjs.org"]

[[agents]]
id = "claude-code"
name = "Claude Code"
command = "claude-agent-acp"       # on PATH first
install_hint = "Needs Node.js 22 or newer."
[agents.npm]                       # else installed into Carbon's tools folder
package = "@agentclientprotocol/claude-agent-acp"
version = "0.95.0"                 # exact
bin = "claude-agent-acp"
npx_fallback = true                # else `npx -y package@version`
```

From code: `acp::start_session(&SessionOptions { agent, permission_mode, job, run, node, .. })` returns a session id at once; `SessionReady` (or `SessionFailed`) follows, then `acp::prompt(session, text)` and later `PromptFinished { text, usage }`. `permission_mode` is `ask`, `allow-edits` or `allow-all`, and the user's setting caps it. When the agent asks permission for something, **Carbon asks the user**; your code is never asked and cannot answer for them.

**Runs:** `graphs::begin_run(path, job)` gives a run id; `set_node_state(run, node, "running"|"waiting"|"done"|"failed"|..., badge)`, `set_node_output`, `run_log` and `end_run` are what the panel shows (borders, state words, the Run tab). `approvals::request(...)` puts a question card in front of the user; the answer arrives as `ApprovalAnswered { id, choice }`.

---

## 5. Permissions (capabilities)

An extension asks for permissions in `capabilities`. The user sees them in plain words when installing and decides. An update asks only for permissions that are new.

| Capability | Lets the extension |
|---|---|
| `fs.read:workspace` / `fs.read:package` / `fs.read:home` | Read files in the open folder / its own package / the home folder |
| `net:<host>` | Reach a host: `api.github.com`, `*.example.com`, or `*` for any |
| `process.exec:<program>` | Run a program (`git`), or `*` for any |
| `fs.write:workspace` | Write files in the open folder |
| `agent.invoke` | Start agent sessions (API 0.2) |

Also accepted, but reserved: no function uses them yet. They are `fs.write:package` / `fs.write:home`, `gpu`, `audio.output`, `audio.input` and `midi`.

Ask for as little as possible: a specific program and host, not `*`. Extensions with broad permissions get less trust from users.

Development folders (Link Development Folder) get every permission they ask for, because you're the one running them.

---

## 6. Rules and limits

- **File paths** in the manifest are relative to the package, use `/`, have no `.` or `..` parts, and are at most 160 bytes long. Names Windows rejects (`con`, `nul`, `a:b`, ...) aren't allowed. **Case matters:** `Icon.png` is not `icon.png`.
- **Every file the manifest names must exist.** A missing file stops the extension from loading or installing.
- **No symbolic links** in a package.
- **Size:** at most 10,000 files and 256 MB per package.
- **Ids** of language servers, commands, panels and settings must be unique within the extension.
- **Installed packages are read-only.** Carbon checks their files against what was installed and turns an extension off if they change ("Files changed").

---

## 7. Testing and troubleshooting

1. Link your folder: Extensions > Install ▾ > Link Development Folder…
2. Edit files, then click **Refresh** in the Extensions dialog.
3. Check the **Installed** tab:

| You see | Meaning |
|---|---|
| **On** | Loaded and in use. |
| A **Did not load** section at the bottom | The manifest has problems. Each one is listed with its reason (unknown key, missing file, bad id, …). |
| **Needs more** | A `[requires]` entry is missing. |
| **Turned off after a failure** | Your code crashed or ran too long. Fix it, Refresh, then Turn On. |
| **Files changed** | An installed package's files were edited. Reinstall, or keep the changes. |

Tips:
- **Language not coloured?** Check that `grammar` in `[[languages]]` matches a `[[grammars]]` `name`, and that the `.wasm` exports `tree_sitter_<name>`.
- **Server never starts?** Check that the language ids match, that `process.exec:<command>` is in `capabilities`, and that the program runs from a terminal.
- **Logs:** start Carbon from a terminal to see `log(...)` lines and load errors.

---

## 8. Sharing your extension

**As a folder or zip:** others install it with Extensions > Install ▾ > **From Folder…** or **From Zip…**. A zip may hold the package at its root or in one top folder.

**From git:** Extensions > Install ▾ > **From Git…** fetches a repository whose root holds `extension.toml`, at a tag, branch or commit. A full commit id pins it exactly.

**In a catalog:** a catalog is a git repository with a `catalog.json` at its root that lists packages. Each version is pinned by commit and content hash, so users get exactly the files you listed:

```json
{ "schema": 1, "packages": [
  { "id": "yourname.midnight", "name": "Midnight",
    "description": "A dark blue theme.", "categories": ["theme"],
    "icon": "themes/midnight/icon.png", "icon_sha256": "<64 hex>",
    "versions": [
      { "version": "0.1.0",
        "git": "https://github.com/yourname/carbon-extensions",
        "commit": "<full 40-character commit id>",
        "path": "themes/midnight",
        "hash": "sha256:<content hash>" } ] } ] }
```

- `path` is the package's folder in the repository. Leave it out when the package is the repository root.
- **`hash` is Carbon's content hash** of the package's files. Don't compute it by hand: use [`catalog.py`](https://github.com/denizhoroz/carbon-extensions-basic/blob/main/catalog.py) from the official extensions repository as a starting point. It hashes the committed files the way Carbon does.
- Optional `suffixes` / `file_names` let Carbon suggest your extension when a user opens a matching file.
- A catalog can be signed with [minisign](https://jedisct1.github.io/minisign/): `minisign -Sm catalog.json`, then commit `catalog.json.minisig`. Users who add your public key in Extensions > Catalog Source… then accept only catalogs that you signed.

Users point Carbon at a catalog in **Extensions > Catalog Source…**. Carbon's own catalog is the default: [carbon-extensions-basic](https://github.com/denizhoroz/carbon-extensions-basic).

**Publishing an update:** bump `version` in `extension.toml`, commit and push, add the new version to `catalog.json`, then push again. Never change the files of a version you already published. Users of a catalog see an **Update** button; nothing updates without their click.
