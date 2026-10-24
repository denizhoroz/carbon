//! Finding an agent program: on PATH (with Windows' `.cmd`/`.bat` too), else
//! installed from npm into Carbon's tools folder and run with `node`, else
//! `npx -y <package>@<version>` when the extension allows it.

use std::path::{Path, PathBuf};
use std::process::Command;

use crate::extension::NpmDef;

/// An agent program as an extension declares it (`[[agents]]`).
#[derive(Clone, Debug, PartialEq)]
pub struct AgentProgram {
    pub name: String,
    pub command: String,
    pub args: Vec<String>,
    pub env: Vec<(String, String)>,
    pub install_hint: Option<String>,
    pub npm: Option<NpmDef>,
}

/// How to start it.
#[derive(Clone, Debug, PartialEq)]
pub struct Launch {
    pub program: PathBuf,
    pub args: Vec<String>,
    pub env: Vec<(String, String)>,
}

/// `name` on PATH. On Windows also `name.exe`, `.cmd`, `.bat` (npm's shims
/// are `.cmd`), in PATHEXT order.
pub fn find_on_path(name: &str) -> Option<PathBuf> {
    let candidates: Vec<String> = if cfg!(windows) && Path::new(name).extension().is_none() {
        let exts = std::env::var("PATHEXT").unwrap_or_else(|_| ".EXE;.CMD;.BAT".into());
        exts.split(';')
            .filter(|e| !e.is_empty())
            .map(|e| format!("{name}{}", e.to_ascii_lowercase()))
            .collect()
    } else {
        vec![name.to_string()]
    };
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path)
        .find_map(|dir| candidates.iter().map(|c| dir.join(c)).find(|p| p.is_file()))
}

/// Where an npm package is installed for Carbon: `<tools>/npm/<package>@<version>`.
pub fn npm_dir(tools: &Path, npm: &NpmDef) -> PathBuf {
    let safe: String = npm
        .package
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '.' {
                c
            } else {
                '_'
            }
        })
        .collect();
    tools.join("npm").join(format!("{safe}@{}", npm.version))
}

/// The JavaScript file the package's `bin` entry names, if installed.
fn npm_entry(dir: &Path, npm: &NpmDef) -> Option<PathBuf> {
    let package = dir.join("node_modules").join(&npm.package);
    let manifest: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(package.join("package.json")).ok()?).ok()?;
    let bin = match &manifest["bin"] {
        serde_json::Value::String(path) => path.clone(),
        serde_json::Value::Object(map) => map.get(&npm.bin)?.as_str()?.to_string(),
        _ => return None,
    };
    let entry = package.join(bin);
    entry.is_file().then_some(entry)
}

/// A command with no console window on Windows.
pub(super) fn quiet(program: &Path) -> Command {
    let mut command = Command::new(program);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt as _;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        command.creation_flags(CREATE_NO_WINDOW);
    }
    command
}

impl AgentProgram {
    /// How to start the program; may install it from npm first (blocking,
    /// a minute at worst). `status` hears what is going on.
    pub fn resolve(&self, tools: Option<&Path>, status: &dyn Fn(String)) -> Result<Launch, String> {
        let launch = |program: PathBuf, mut args: Vec<String>| {
            args.extend(self.args.iter().cloned());
            Launch {
                program,
                args,
                env: self.env.clone(),
            }
        };
        if let Some(program) = find_on_path(&self.command) {
            return Ok(launch(program, Vec::new()));
        }
        let Some(npm) = &self.npm else {
            return Err(self.missing(None));
        };
        let node = find_on_path("node");
        if let (Some(tools), Some(node)) = (tools, &node) {
            let dir = npm_dir(tools, npm);
            if let Some(entry) = npm_entry(&dir, npm) {
                return Ok(launch(node.clone(), vec![entry.display().to_string()]));
            }
            if let Some(npm_program) = find_on_path("npm") {
                status(format!("Installing {} {}…", self.name, npm.version));
                std::fs::create_dir_all(&dir).map_err(|err| format!("{}: {err}", dir.display()))?;
                let output = quiet(&npm_program)
                    .args([
                        "install",
                        "--no-audit",
                        "--no-fund",
                        "--loglevel=error",
                        "--prefix",
                    ])
                    .arg(&dir)
                    .arg(format!("{}@{}", npm.package, npm.version))
                    .output()
                    .map_err(|err| format!("cannot run npm: {err}"))?;
                if let Some(entry) = npm_entry(&dir, npm) {
                    return Ok(launch(node.clone(), vec![entry.display().to_string()]));
                }
                let stderr = String::from_utf8_lossy(&output.stderr);
                if !npm.npx_fallback {
                    return Err(self.missing(Some(stderr.trim().to_string())));
                }
            }
        }
        if npm.npx_fallback
            && let Some(npx) = find_on_path("npx")
        {
            return Ok(launch(
                npx,
                vec!["-y".into(), format!("{}@{}", npm.package, npm.version)],
            ));
        }
        Err(self.missing(
            node.is_none()
                .then(|| "Node.js is not installed".to_string()),
        ))
    }

    fn missing(&self, detail: Option<String>) -> String {
        let mut text = format!("{} is not installed", self.name);
        if let Some(detail) = detail.filter(|d| !d.is_empty()) {
            text.push_str(&format!(" ({detail})"));
        }
        if let Some(hint) = &self.install_hint {
            text.push_str(&format!(". {hint}"));
        }
        text
    }
}
