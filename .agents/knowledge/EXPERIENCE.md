# Experience

Problems met in previous projects and how they were solved, kept to reuse in future ones. Only solved problems are listed. **General** lessons hold anywhere; the rest are grouped by platform. Language and framework problems live in the language folders (see `README.md`).

Each entry: **Problem** (what was seen), **Cause** (when known), **Solution**.

Related: `DESIGN.md`, `ARCHITECTURE.md`, `SECURITY.md`.


# General

## Stale UI after the data changed
**Problem:** After renaming an entry and overwriting the source, a page still showed the old state (the new name with zero uses, marked as unmapped). **Cause:** The page's derived state was rebuilt only when the selection changed, not when the data under it changed. **Solution:** Rebuild derived state when the data's version changes too, keeping the user's edits that still apply.

## Form lost its state after saving
**Cause:** Reloading data after a save made it "empty while loading", so everything rendered from it unmounted and reset. **Solution:** Keep the last loaded value and render from that while the new one loads.

## Live updates starved other requests
**Problem:** The status bar missed updates and pages waited on requests. **Cause:** Browsers allow 6 open requests per server over HTTP; one long-poll per component filled them. **Solution:** One app-wide long-poll carrying all job activity. Never add a long-poll per component.

## Live progress without client timers
**Solution:** Long-poll: the server holds the request until the state's version passes the client's (or a timeout), the client calls again in a loop. Animations end on their own end event.

## Stored paths point to the wrong place
**Cause:** A manifest stored a relative path (`.`). **Solution:** On load, set the path to the folder actually loaded from.

## Error messages look noisy
**Solution:** Show only the message to the user; keep wrappers and stack details for logs.

## Optional install / remove steps broke the environment
**Cause:** Separate install and remove steps for helper libraries left half-installed environments with no record of what was there. **Solution:** Make the needed libraries base dependencies installed on launch; the only user choice left is one switch (e.g. CPU / GPU build) applied by a single sync command.

## Helper runtime used global folders
**Solution:** Point every tool the helper uses (package manager, interpreter install, cache, library settings) at the app's own write root, explicitly, on every call.

## A library left big files behind
**Problem:** Each run of a repeated experiment left several MB of output although saving was off. **Cause:** The library still saved on its last step. **Solution:** The helper deletes those files after reading what it needs.

## Results only in old logs
**Problem:** Runs made before the app stored a summary had no details. **Solution:** New runs send a summary in the final event and store it; old runs fall back to parsing the console log.

## External API: resource can't be exported as it is
**Problem:** Some public resources on a third-party service can't be exported until the owner prepares them. **Solution:** On request only (a button on the item): copy into the user's own account, prepare it there, download, delete the copy.

## External API: option not allowed for this resource type
**Cause:** Each resource type allows its own export options; the error answer lists them. **Solution:** Parse that list and switch to an option the app reads, with a note to the user. Option names not in the API's docs may be refused: fall back to an allowed one of the same family.

## Package index stops at an old version
**Cause:** A library publishes new versions only for its newest hardware-specific builds; the older build line stopped getting updates. **Solution:** Move to the newest build line. Keep the line in one place (project file + the app's build list) so the next move is one change plus a re-lock.

## Release bundle grew with every build
**Cause:** The bundler never removes old hashed files from its output folder. **Solution:** Delete the release output folder before bundling and before measuring size.

## Testing an install-changing feature without touching the real environment
**Solution:** A scratch home folder pointing at the real environment through links (read only: never run installs there, they write through), plus the new library installed into a separate scratch target. Remove the links as links, never recursively.

## Clicking through the GUI without browser tools
**Solution:** Headless Edge with remote debugging, driven over the DevTools protocol from a small Node script: navigate, evaluate (click, set values, measure positions), set the window size through device-metrics emulation (the window-size flag is ignored). Set up data first through the server's own endpoints. Stop only that browser process, not every one.


# Windows

## "Access is denied" now and then when a tool finishes
**Cause:** Windows can't rename / delete a folder while a file in it is open (virus scanner, indexer, Explorer, viewers). **Solution:** Retry rename / delete for up to 10 s, then name the busy path in the error.

## Native dialogs open behind the browser
**Cause:** The server has no window, and Windows' foreground lock. **Solution:** A 1×1 topmost owner window that attaches to the foreground thread's input.

## Environment size looked doubled
**Cause:** Explorer counts the package cache's hard links twice. **Solution:** Clean the package cache after installing; measure with an empty cache.


# macOS

## Dialogs panic "RFD in NonWindowed environment"
**Cause:** With no app event loop, dialogs on macOS open only on the main thread; they ran on a worker, and the server held the main thread. **Solution:** On macOS run the server on a side thread; the main thread runs dialog jobs from a queue and answers over a channel; the process exits when the server thread ends.

## Busy cursor where a closed dialog was
**Cause:** The main thread waited on the job queue without handling AppKit events, so the closed panel never left the screen. **Solution:** Pump `NSApplication` events (up to 100 ms per wait) between jobs.


# Linux

## Prebuilt CLI tool needs a newer glibc
**Problem:** The prebuilt `dx` needs glibc 2.39. **Solution:** Install it from source (`cargo install`) on older distributions and cache it in CI.


# Browser

## Browser can't give a folder path
**Solution:** Open the native dialog on the server; the server runs on the same machine.

## Uploads and drop zones
**Solution:** Send the file as the request body (streamed). Drop zones need `preventDefault()` on `dragenter`, `dragover` and `drop`.

## Wheel events have no pointer position
**Solution:** Keep the last pointer position from `pointermove`.
