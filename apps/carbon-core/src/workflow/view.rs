//! Declarative views, tier 1 (workflow-system.md Q17, Q18): a widget tree
//! written in the manifest, no code. Carbon draws it with its own widgets;
//! buttons and list items run the workflow's `[[commands]]`, so a view can
//! do only what a command can.
//!
//! ```toml
//! [panels]
//! home = { kind = "view", title = "Home", view = "home" }
//!
//! [[views.home]]
//! type = "heading"
//! text = "Paradigm Editor"
//!
//! [[views.home]]
//! type = "row"
//! children = [
//!   { type = "button", label = "New Diagram", command = "new-diagram", style = "primary" },
//! ]
//! ```

use std::collections::HashSet;

use serde::{Deserialize, Serialize};

/// Deepest nesting of `row` / `column` a view may use.
pub const MAX_VIEW_DEPTH: usize = 8;
/// Most widgets one view may hold (list items count).
pub const MAX_VIEW_WIDGETS: usize = 500;

/// One element of a view.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum Widget {
    Heading {
        text: String,
    },
    Text {
        text: String,
        /// Secondary text colour.
        #[serde(default, skip_serializing_if = "is_false")]
        muted: bool,
    },
    /// Markdown text (links open in the system browser).
    Markdown {
        text: String,
    },
    Button {
        label: String,
        /// A `[[commands]]` id of the workflow.
        command: String,
        /// Icon name from Carbon's icon set.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        icon: Option<String>,
        #[serde(default)]
        style: ButtonStyle,
    },
    /// Children side by side.
    Row {
        children: Vec<Widget>,
    },
    /// Children stacked; `align = "center"` centers them (empty states).
    Column {
        children: Vec<Widget>,
        #[serde(default)]
        align: Align,
    },
    Divider,
    /// Rows of label + description; a row with a `command` is clickable.
    List {
        items: Vec<ListItem>,
    },
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ButtonStyle {
    #[default]
    Secondary,
    Primary,
    Ghost,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Align {
    #[default]
    Start,
    Center,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ListItem {
    pub label: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    /// A `[[commands]]` id; makes the row clickable.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
}

fn is_false(value: &bool) -> bool {
    !value
}

/// Problems in view `name`: commands it names must be among `commands`,
/// labels must not be empty, nesting and size stay within the limits.
pub fn check_view(name: &str, widgets: &[Widget], commands: &HashSet<&str>) -> Vec<String> {
    let mut problems = Vec::new();
    let mut count = 0;
    check_widgets(name, widgets, commands, 1, &mut count, &mut problems);
    if count > MAX_VIEW_WIDGETS {
        problems.push(format!(
            "view `{name}` has {count} widgets (at most {MAX_VIEW_WIDGETS})"
        ));
    }
    problems
}

fn check_widgets(
    name: &str,
    widgets: &[Widget],
    commands: &HashSet<&str>,
    depth: usize,
    count: &mut usize,
    problems: &mut Vec<String>,
) {
    if depth > MAX_VIEW_DEPTH {
        problems.push(format!(
            "view `{name}` nests deeper than {MAX_VIEW_DEPTH} levels"
        ));
        return;
    }
    let command = |id: &str, problems: &mut Vec<String>| {
        if !commands.contains(id) {
            problems.push(format!("view `{name}`: no command `{id}` in [[commands]]"));
        }
    };
    for widget in widgets {
        *count += 1;
        match widget {
            Widget::Button {
                label, command: id, ..
            } => {
                if label.trim().is_empty() {
                    problems.push(format!("view `{name}`: a button needs a label"));
                }
                command(id, problems);
            }
            Widget::Row { children } | Widget::Column { children, .. } => {
                check_widgets(name, children, commands, depth + 1, count, problems);
            }
            Widget::List { items } => {
                for item in items {
                    *count += 1;
                    if item.label.trim().is_empty() {
                        problems.push(format!("view `{name}`: a list item needs a label"));
                    }
                    if let Some(id) = &item.command {
                        command(id, problems);
                    }
                }
            }
            Widget::Heading { .. }
            | Widget::Text { .. }
            | Widget::Markdown { .. }
            | Widget::Divider => {}
        }
    }
}
