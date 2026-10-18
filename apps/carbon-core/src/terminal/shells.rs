//! Shell profiles: which shells are installed, which one a new terminal gets,
//! and how each is started with Carbon's shell integration.

use std::path::{Path, PathBuf};

use super::TerminalSpec;
use crate::lsp::find_program;

const PWSH_SCRIPT: &str = include_str!("scripts/carbon.ps1");
const BASH_SCRIPT: &str = include_str!("scripts/carbon.bash");
const ZSH_SCRIPT: &str = include_str!("scripts/carbon.zsh");

/// The shell integration a profile starts with.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Integration {
    Pwsh,
    Bash,
    Zsh,
}

/// One shell the user can open a terminal with.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Profile {
    /// Shown in menus and on the tab ("PowerShell", "Git Bash", ...).
    pub name: String,
    pub program: PathBuf,
    pub args: Vec<String>,
    pub env: Vec<(String, String)>,
    pub integration: Option<Integration>,
}

impl Profile {
    fn new(name: &str, program: PathBuf, integration: Option<Integration>) -> Self {
        Self {
            name: name.to_string(),
            program,
            args: Vec::new(),
            env: Vec::new(),
            integration,
        }
    }

    /// What to start, in `cwd`, with the integration script when the shell
    /// has one (written to a temp folder; a failure just leaves it out).
    pub fn spec(&self, cwd: Option<PathBuf>, cols: u16, rows: u16) -> TerminalSpec {
        let mut args = self.args.clone();
        let mut env = self.env.clone();
        env.push(("TERM_PROGRAM".into(), "carbon".into()));
        if let Some(integration) = self.integration
            && let Err(err) = add_integration(integration, &mut args, &mut env)
        {
            eprintln!("shell integration left out: {err}");
        }
        TerminalSpec {
            program: Some(self.program.to_string_lossy().into_owned()),
            args,
            cwd,
            env,
            cols,
            rows,
        }
    }
}

fn add_integration(
    integration: Integration,
    args: &mut Vec<String>,
    env: &mut Vec<(String, String)>,
) -> std::io::Result<()> {
    let dir = std::env::temp_dir().join("carbon-shell-integration");
    std::fs::create_dir_all(&dir)?;
    match integration {
        Integration::Pwsh => {
            let script = write(&dir, "carbon.ps1", PWSH_SCRIPT)?;
            args.extend([
                "-NoLogo".into(),
                "-NoExit".into(),
                "-Command".into(),
                format!("try {{ . '{}' }} catch {{}}", script.display()),
            ]);
        }
        Integration::Bash => {
            let script = write(&dir, "carbon.bash", BASH_SCRIPT)?;
            args.extend(["--init-file".into(), bash_path(&script), "-i".into()]);
            env.push(("CHERE_INVOKING".into(), "1".into()));
        }
        Integration::Zsh => {
            let zdotdir = dir.join("zsh");
            std::fs::create_dir_all(&zdotdir)?;
            write(&zdotdir, ".zshrc", ZSH_SCRIPT)?;
            let user = std::env::var("ZDOTDIR")
                .or_else(|_| std::env::var("HOME"))
                .unwrap_or_default();
            // Loads the user's .zshenv, then points zsh back here for .zshrc.
            write(
                &zdotdir,
                ".zshenv",
                &format!(
                    "ZDOTDIR=\"${{CARBON_USER_ZDOTDIR:-$HOME}}\"\n\
                     [ -f \"$ZDOTDIR/.zshenv\" ] && . \"$ZDOTDIR/.zshenv\"\n\
                     ZDOTDIR=\"{}\"\n",
                    zdotdir.display()
                ),
            )?;
            env.push(("CARBON_USER_ZDOTDIR".into(), user));
            env.push(("ZDOTDIR".into(), zdotdir.to_string_lossy().into_owned()));
        }
    }
    Ok(())
}

fn write(dir: &Path, name: &str, text: &str) -> std::io::Result<PathBuf> {
    let path = dir.join(name);
    std::fs::write(&path, text)?;
    Ok(path)
}

/// Git Bash takes Windows paths with forward slashes.
fn bash_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

/// The installed shells, the default first.
pub fn profiles() -> Vec<Profile> {
    let mut found = if cfg!(windows) {
        windows_profiles()
    } else {
        unix_profiles()
    };
    found.dedup_by(|a, b| a.program == b.program);
    found
}

/// The shell a new terminal starts: PowerShell 7, else Windows PowerShell,
/// else cmd on Windows; the user's `$SHELL` elsewhere.
pub fn default_profile() -> Option<Profile> {
    profiles().into_iter().next()
}

/// A profile by its name, for restoring a saved terminal tab.
pub fn profile_named(name: &str) -> Option<Profile> {
    profiles().into_iter().find(|profile| profile.name == name)
}

fn windows_profiles() -> Vec<Profile> {
    let env_path = |var: &str, rest: &str| {
        std::env::var_os(var)
            .map(|base| PathBuf::from(base).join(rest))
            .filter(|path| path.is_file())
    };
    let mut out = Vec::new();
    if let Some(pwsh) =
        find_program("pwsh").or_else(|| env_path("ProgramFiles", r"PowerShell\7\pwsh.exe"))
    {
        out.push(Profile::new("PowerShell", pwsh, Some(Integration::Pwsh)));
    }
    if let Some(powershell) = find_program("powershell").or_else(|| {
        env_path(
            "SystemRoot",
            r"System32\WindowsPowerShell\v1.0\powershell.exe",
        )
    }) {
        out.push(Profile::new(
            "Windows PowerShell",
            powershell,
            Some(Integration::Pwsh),
        ));
    }
    if let Some(cmd) = std::env::var_os("COMSPEC")
        .map(PathBuf::from)
        .or_else(|| find_program("cmd"))
    {
        out.push(Profile::new("Command Prompt", cmd, None));
    }
    if let Some(bash) = env_path("ProgramFiles", r"Git\bin\bash.exe") {
        out.push(Profile::new("Git Bash", bash, Some(Integration::Bash)));
    }
    if let Some(wsl) = env_path("SystemRoot", r"System32\wsl.exe") {
        out.push(Profile::new("WSL", wsl, None));
    }
    out
}

fn unix_profiles() -> Vec<Profile> {
    let mut paths: Vec<PathBuf> = std::env::var_os("SHELL")
        .map(PathBuf::from)
        .into_iter()
        .collect();
    if let Ok(text) = std::fs::read_to_string("/etc/shells") {
        paths.extend(
            text.lines()
                .map(str::trim)
                .filter(|line| !line.is_empty() && !line.starts_with('#'))
                .map(PathBuf::from),
        );
    }
    let mut out: Vec<Profile> = Vec::new();
    for path in paths.into_iter().filter(|path| path.is_file()) {
        let name = path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default();
        let integration = match name.as_str() {
            "bash" => Some(Integration::Bash),
            "zsh" => Some(Integration::Zsh),
            "fish" | "sh" | "dash" | "ksh" | "tcsh" | "csh" => None,
            _ => continue,
        };
        // One entry per shell name ($SHELL wins, being first).
        if !out.iter().any(|profile| profile.name == name) {
            out.push(Profile::new(&name, path, integration));
        }
    }
    out
}
