# Architecture Knowledge

System design choices from previous projects, kept to reuse in future ones. No language or framework details here; those live in the language folders (see `README.md`). They fit a local, project-folder-based app with a core library, a UI, and long-running jobs.

Related: `DESIGN.md` (UI), `SECURITY.md`, `EXPERIENCE.md`.


# 1. Layers

- **Core / UI split.** All logic lives in a UI-free core library. The UI's server layer is thin: validate input, call the core, map results for display. A second frontend (CLI, other UI) reuses the core as-is.
- **Shared types package**: plain data types and input rules only (no I/O, no heavy dependencies), used by the core, the server and the browser side, so validation and shared lists (formats, colours) are the same on both ends. The core re-exports it.
- **Progress through an interface.** Long tasks report through a reporter interface; each frontend shows progress its own way.
- **One in-memory model for many file formats.** Every supported file format is read into one internal model; tools work on it and write back. New formats plug in at the reader / writer; tools never change.
- **Errors carry just the message** to the UI; details stay in logs.


# 2. Data on disk

- **The project folder is the database.** A workspace is a folder with a manifest (name, path, seed, description, defaults) and a fixed layout:

  ```
  <workspace>/
  ├─ <manifest>
  ├─ <items>/         # one folder per item, each with its own manifest
  ├─ <runs>/          # one folder per job run: settings, config, console log, results
  └─ .cache/tmp/      # downloads and extraction happen here first
  ```

- Each item has its own small manifest (id, name, type, formats, counts, size, timestamps, optional description, link, license, tags).
- **Everything lands inside the workspace**: downloads, imports and tool results go to their fixed folder, temporary work to `.cache/tmp/`, then moves in by rename. No custom target folders.
- **A stored path is not trusted**: the loader sets the workspace's path to the folder it was loaded from.
- **Watch the folder.** Items added, deleted, renamed or edited outside the app show up on their own.
- **Upgrade old layouts on load** (e.g. rename an old folder) instead of refusing them; old values are read as their nearest new meaning.
- **Lineage**: every tool records its operation, parents and settings in the output's manifest.
- **One workspace seed** used by every random step that has no seed of its own, so results repeat.
- **Links without copies**: a "linked" item is built from hard links, so it is a normal item that takes no extra space and keeps its files when the source goes. Same drive only; on a drive without hard links stop with a clear message, never a silent copy. Size counting leaves the linked bytes out.
- **Caches are disposable**: previews, statistics and history live in the cache folder; "Clear cache" removes them safely. Invalidate a cache when its source changes; after a small edit update it in place instead of a full re-read. Keep only a few large items in memory (e.g. the last 3).


# 3. Jobs

- Long work runs as jobs on the server, never in a request.
- One job per tool at a time; a job is refused when another running job uses the same data or the "jobs at once" limit (sized to the machine's cores and memory) is reached.
- Every job is cancellable and reports per-item outcomes (done / failed / warning / skipped / cancelled); failed items can be re-run.
- Every job and one-step action goes to a workspace history log in the cache.
- **Live updates by long-poll, and only one.** The server waits until a version number passes the client's, then answers; the client calls again. One app-wide long-poll for all job activity (plus the workspace watcher): browsers allow only 6 open requests per server over HTTP, so a long-poll per component starves everything else.


# 4. Install and runtime

- **Read-only install, separate write root.** Installed apps can't write to their install folder. Read-only resources ship next to the executable (the install folder may be Program Files); everything written goes to the user's local data folder: `%LOCALAPPDATA%\<App>` (Windows), `~/Library/Application Support/<App>` (macOS), `~/.local/share/<app>` (Linux). One environment variable puts both in one folder for development or portable use.
- **Resources are found next to the executable**, never from the working folder, so the app runs from anywhere.
- **Local web app**: a local server plus the browser as UI. Native dialogs (folder pickers) open on the server side, since the browser can't give real paths.
- **Ship small.** Measure the release bundle, not the dev build folder. Download big optional tools on demand (checksum-verified) instead of bundling them. Keep one HTTP/TLS stack in the dependency tree. Clean old build output before bundling and measuring.


# 5. Helper runtime (a second language)

When part of the work runs in another language:

- **The app owns the runtime**: its own interpreter version, never the user's; installed into the write root:

  ```
  <write root>/
  ├─ <env>/          # the environment + a JSON record of what was installed
  ├─ <interpreter>/  # managed interpreter
  ├─ <cache>/        # package download cache
  ├─ bin/            # package manager, downloaded if missing
  └─ <work>/         # helper runs here; files it downloads land here
  ```

- **Set up on launch**, when needed: the default libraries are installed and can't be removed (separate install / remove steps kept breaking the environment). A half-finished setup is repaired and an outdated one updated, the same way.
- **One switch for hardware**: choosing a hardware-specific build (e.g. CPU or GPU) only swaps the packages that differ; "Remove" goes back to the default. Ask which build when matching hardware is found.
- **Locked dependencies**, committed lock file; dependency conflicts settled in the project file, not at run time.
- **Child process protocol**: the helper takes a command and a config file per run (e.g. `check`, then one command per job type). `check` prints one JSON object. Long commands pass the library's normal output through and add tagged JSON event lines (`@<app> {json}`: start, progress, done, error) the app parses. The final event carries a summary, so results don't have to be scraped from logs later; old runs fall back to reading the console log.
- **Clean up after the helper**: delete files it leaves behind that aren't needed, clean the package cache after installing.


# 6. Releases

- One script builds a release folder per platform: server binary + web assets + helper sources (code, project file, lock file).
- Windows: zip + installer. Linux: tarball with a desktop entry and a small install script. macOS: an app bundle that opens the binary in a terminal (closing it stops the app).
- CI builds on a tag into a draft release; a manual trigger builds artifacts only. Pin the toolchain version CI uses to the one known to work locally; update linked tools together.
- Line endings for shell scripts and config files pinned to LF in the repo.
