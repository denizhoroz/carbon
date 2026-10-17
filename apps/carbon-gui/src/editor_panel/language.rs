//! Which highlighting language a file gets, by name and extension. The names
//! are the kit's tree-sitter language names; one without a grammar compiled
//! in shows as plain text.

use std::path::Path;

/// Plain text: no highlighting.
pub const PLAIN: &str = "text";

pub fn for_path(path: &Path) -> &'static str {
    let name = path
        .file_name()
        .map(|name| name.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    match name.as_str() {
        "cargo.lock" | "uv.lock" | "poetry.lock" => return "toml",
        "makefile" | "gnumakefile" => return "make",
        "cmakelists.txt" => return "cmake",
        ".bashrc" | ".bash_profile" | ".profile" | ".zshrc" => return "bash",
        _ => {}
    }
    let extension = path
        .extension()
        .map(|ext| ext.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    match extension.as_str() {
        "rs" => "rust",
        "toml" => "toml",
        "json" | "jsonc" | "json5" => "json",
        "md" | "markdown" | "mdx" => "markdown",
        "yaml" | "yml" => "yaml",
        "py" | "pyi" => "python",
        "js" | "mjs" | "cjs" | "jsx" => "javascript",
        "ts" | "mts" | "cts" => "typescript",
        "tsx" => "tsx",
        "html" | "htm" => "html",
        "css" | "scss" => "css",
        "sh" | "bash" | "zsh" => "bash",
        "c" | "h" => "c",
        "cpp" | "cc" | "cxx" | "hpp" | "hh" => "cpp",
        "go" => "go",
        "sql" => "sql",
        "diff" | "patch" => "diff",
        _ => PLAIN,
    }
}
