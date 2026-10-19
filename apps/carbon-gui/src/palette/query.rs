//! What the palette's query asks for: VS Code Quick Open prefixes.

/// The palette's mode, from the query's first character.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    /// No prefix: go to a file of the workspace.
    Files,
    /// `>`: run a command.
    Commands,
    /// `:`: go to a line of the active editor.
    Line,
    /// `?`: list the prefixes.
    Help,
    /// `@`: symbols of the active file (needs language-server symbols).
    Symbols,
    /// `#`: symbols of the workspace (needs language-server symbols).
    WorkspaceSymbols,
}

impl Mode {
    /// Every mode in the order `?` lists them.
    pub const ALL: [Mode; 6] = [
        Mode::Files,
        Mode::Commands,
        Mode::Line,
        Mode::Symbols,
        Mode::WorkspaceSymbols,
        Mode::Help,
    ];

    pub fn prefix(self) -> &'static str {
        match self {
            Mode::Files => "",
            Mode::Commands => ">",
            Mode::Line => ":",
            Mode::Help => "?",
            Mode::Symbols => "@",
            Mode::WorkspaceSymbols => "#",
        }
    }

    /// What the `?` list calls the mode.
    pub fn title(self) -> &'static str {
        match self {
            Mode::Files => "Go to File",
            Mode::Commands => "Show and Run Commands",
            Mode::Line => "Go to Line",
            Mode::Help => "Show Help",
            Mode::Symbols => "Go to Symbol in File",
            Mode::WorkspaceSymbols => "Go to Symbol in Workspace",
        }
    }

    /// Whether the mode works yet (symbols wait for the language servers).
    pub fn available(self) -> bool {
        !matches!(self, Mode::Symbols | Mode::WorkspaceSymbols)
    }

    pub fn placeholder(self) -> &'static str {
        match self {
            Mode::Files => "Search files by name (> commands, : line, ? help)",
            Mode::Commands => "Type a command",
            Mode::Line => "Type a line number, then optionally :column",
            Mode::Help => "Pick a mode",
            Mode::Symbols | Mode::WorkspaceSymbols => "Symbols are coming soon",
        }
    }
}

/// Splits a query into its mode and the trimmed rest.
pub fn parse(query: &str) -> (Mode, &str) {
    let mut chars = query.chars();
    let mode = match chars.next() {
        Some('>') => Mode::Commands,
        Some(':') => Mode::Line,
        Some('?') => Mode::Help,
        Some('@') => Mode::Symbols,
        Some('#') => Mode::WorkspaceSymbols,
        _ => return (Mode::Files, query.trim()),
    };
    (mode, chars.as_str().trim())
}

/// A go-to-line target: 1-based line and optional column, as `42`, `42:7`
/// or `42,7`. `None` when the text is not that.
pub fn parse_line(rest: &str) -> Option<(u32, Option<u32>)> {
    let (line, column) = match rest.split_once([':', ',']) {
        Some((line, column)) => (line, Some(column)),
        None => (rest, None),
    };
    let line = line.trim().parse().ok().filter(|line| *line > 0)?;
    let column = match column.map(str::trim) {
        None | Some("") => None,
        Some(column) => Some(column.parse().ok().filter(|column| *column > 0)?),
    };
    Some((line, column))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prefixes_pick_the_mode() {
        assert_eq!(parse(""), (Mode::Files, ""));
        assert_eq!(parse(" main "), (Mode::Files, "main"));
        assert_eq!(parse(">tog"), (Mode::Commands, "tog"));
        assert_eq!(parse("> tog"), (Mode::Commands, "tog"));
        assert_eq!(parse(":42"), (Mode::Line, "42"));
        assert_eq!(parse("?"), (Mode::Help, ""));
        assert_eq!(parse("@a"), (Mode::Symbols, "a"));
        assert_eq!(parse("#a"), (Mode::WorkspaceSymbols, "a"));
    }

    #[test]
    fn every_mode_round_trips_its_prefix() {
        for mode in Mode::ALL {
            assert_eq!(parse(mode.prefix()).0, mode);
        }
    }

    #[test]
    fn line_targets() {
        assert_eq!(parse_line("42"), Some((42, None)));
        assert_eq!(parse_line("42:7"), Some((42, Some(7))));
        assert_eq!(parse_line("42,7"), Some((42, Some(7))));
        assert_eq!(parse_line("42:"), Some((42, None)));
        assert_eq!(parse_line(""), None);
        assert_eq!(parse_line("x"), None);
        assert_eq!(parse_line("0"), None);
        assert_eq!(parse_line("4:x"), None);
    }
}
