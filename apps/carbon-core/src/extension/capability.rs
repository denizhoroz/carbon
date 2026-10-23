//! Capabilities an extension asks for: plain
//! strings in the manifest, `name` or `name:scope`, so they are easy to show,
//! compare between versions and store as grants.
//!
//! ```toml
//! capabilities = ["fs.read:workspace", "net:api.github.com", "process.exec:git", "gpu"]
//! ```
//!
//! The user grants them at install; an update asks only for new ones. Nothing
//! Language servers need `process.exec:<command>` and downloads `net:<host>`
//! (checked by the manifest); the webview page API and WASM host functions
//! check the grants.

/// Capabilities that need a scope after `:`.
const SCOPED: [&str; 4] = ["fs.read", "fs.write", "net", "process.exec"];
/// Capabilities without a scope.
const PLAIN: [&str; 5] = ["agent.invoke", "gpu", "audio.output", "audio.input", "midi"];
/// Scopes `fs.*` may name.
const FS_SCOPES: [&str; 3] = ["workspace", "package", "home"];

/// Checks one capability string.
pub fn check_capability(capability: &str) -> Result<(), String> {
    let (name, scope) = match capability.split_once(':') {
        Some((name, scope)) => (name, Some(scope)),
        None => (capability, None),
    };
    let bad = |why: String| Err(format!("capability `{capability}`: {why}"));
    match scope {
        None if PLAIN.contains(&name) => Ok(()),
        None if SCOPED.contains(&name) => bad(format!("needs a scope, like `{name}:...`")),
        Some(_) if PLAIN.contains(&name) => bad("takes no scope".into()),
        Some(scope) if name.starts_with("fs.") && SCOPED.contains(&name) => {
            if FS_SCOPES.contains(&scope) {
                Ok(())
            } else {
                bad(format!("scope must be one of {}", FS_SCOPES.join(", ")))
            }
        }
        Some(scope) if name == "net" => {
            let host = scope.strip_prefix("*.").unwrap_or(scope);
            if scope == "*"
                || (!host.is_empty()
                    && host.chars().all(|c| {
                        c.is_ascii_lowercase() || c.is_ascii_digit() || c == '.' || c == '-'
                    }))
            {
                Ok(())
            } else {
                bad("scope must be a lowercase host name, `*.domain` or `*`".into())
            }
        }
        Some(scope) if name == "process.exec" => {
            if scope == "*"
                || (!scope.is_empty()
                    && scope
                        .chars()
                        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_')))
            {
                Ok(())
            } else {
                bad("scope must be a program name or `*`".into())
            }
        }
        _ => bad(format!(
            "unknown; known: {}, {}",
            SCOPED.map(|name| format!("{name}:...")).join(", "),
            PLAIN.join(", ")
        )),
    }
}

/// What a capability lets the extension do, in plain words.
pub fn describe_capability(capability: &str) -> String {
    let (name, scope) = capability.split_once(':').unwrap_or((capability, ""));
    let place = |scope: &str| match scope {
        "workspace" => "in the open folder".to_string(),
        "package" => "in its own package".to_string(),
        "home" => "in your home folder".to_string(),
        other => format!("in `{other}`"),
    };
    match (name, scope) {
        ("fs.read", scope) => format!("Read files {}", place(scope)),
        ("fs.write", scope) => format!("Change files {}", place(scope)),
        ("net", "*") => "Connect to any internet address".into(),
        ("net", host) => format!("Connect to {host}"),
        ("process.exec", "*") => "Run any program".into(),
        ("process.exec", program) => format!("Run the program `{program}`"),
        ("agent.invoke", _) => "Ask the AI agent to do work".into(),
        ("gpu", _) => "Use the graphics card".into(),
        ("audio.output", _) => "Play sound".into(),
        ("audio.input", _) => "Record from the microphone".into(),
        ("midi", _) => "Use MIDI devices".into(),
        _ => capability.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capabilities_are_checked() {
        for good in [
            "fs.read:workspace",
            "fs.write:package",
            "net:api.github.com",
            "net:*.example.com",
            "net:*",
            "process.exec:git",
            "gpu",
            "audio.input",
        ] {
            assert!(check_capability(good).is_ok(), "{good}");
        }
        for bad in [
            "fs.read",
            "fs.read:C:/",
            "net:Bad Host",
            "gpu:fast",
            "process.exec:rm -rf",
            "teleport",
        ] {
            assert!(check_capability(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn capabilities_read_plainly() {
        assert_eq!(
            describe_capability("net:api.github.com"),
            "Connect to api.github.com"
        );
        assert_eq!(
            describe_capability("fs.write:workspace"),
            "Change files in the open folder"
        );
    }
}
