# Security Knowledge

Safety and privacy choices from previous projects, kept to reuse in future ones. No language or framework details here; those live in the language folders (see `README.md`).

Related: `DESIGN.md`, `ARCHITECTURE.md`, `EXPERIENCE.md`.


# 1. Local server

- **Local only.** The server refuses requests whose `Host` isn't `localhost` / `127.0.0.1` / `[::1]`, and requests from cross-site pages (another site open in the same browser can't drive the app).
- Serving to other machines is off by design; native dialogs open on the server machine anyway.
- Nothing loads from the internet: fonts, icons and styles are bundled.


# 2. Files

- **File routes are fenced**: routes that serve files only serve them from inside the project's own folders.
- **Names from requests are never paths**: names and "replacing" targets are checked so they can't climb out of their folder (`..`, separators, absolute paths).


# 3. Secrets

- API keys live in the OS credential store.
- The browser only ever sees a hint ("Saved"), never the key or part of it.
- Nothing reads a `.env` file; keys come from the credential store or real environment variables.


# 4. Downloads and outputs

- Downloaded executables are checked by hash before use.
- Generated artefacts must not leak the build environment (e.g. output files carrying git details of the repo they were made in).


# 5. Process

- Run a security audit as its own pass: list findings with an id and severity, fix, keep the still-open ones in the known-issues list.
- Run a personal-info audit before publishing (names, paths, emails in code, logs and docs).
