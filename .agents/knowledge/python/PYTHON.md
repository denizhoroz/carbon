# Python

Python knowledge from previous projects. General rules are in `../DESIGN.md`, `../ARCHITECTURE.md`, `../SECURITY.md`, `../EXPERIENCE.md`; tools in this folder have their own files (`UV.md`).


# Architecture

- Python as a helper runtime the app owns (see `../ARCHITECTURE.md` → Helper runtime): runs from a work folder in the app's write root, so libraries that download files put them there; libraries' settings folders point into the write root too.


# Experience

## Edits broke UTF-8 in files
**Cause:** `open()` defaults to cp1252 on Windows. **Solution:** Always pass `encoding="utf-8"`.
