# Third-party files in Carbon's official extensions

| Files | From | Licence |
|---|---|---|
| `*/grammars/*.wasm` | Release builds of the tree-sitter grammars listed in `grammars.toml` (tree-sitter-rust, -javascript, -typescript, -python, -bash, -css, -html from github.com/tree-sitter; -markdown, -toml, -yaml from github.com/tree-sitter-grammars) | MIT, each repository's `LICENSE` |
| `*/grammars/*/*.scm` | The highlight queries of those grammars (crates.io packages of the same versions), or of gpui-component 0.7.3 where Carbon used the kit's | MIT (grammars); Apache-2.0 (gpui-component) |
| `gruvbox/themes/gruvbox.json` | Port of the Gruvbox palette by Pavel Pertsev (github.com/morhetz/gruvbox) | MIT |
