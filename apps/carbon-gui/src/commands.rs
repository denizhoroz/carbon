//! Command registry for the command palette: one entry per thing the user can
//! run. The menus are the source for everything they show (so the two cannot
//! drift); `extras` adds commands that live only in the palette.

use std::sync::OnceLock;

use carbon_core::terminal::shells;
use gpui_kit::assets::IconName;
use gpui_kit::*;

use crate::chat_panel;
use crate::editor_panel::CloseEditor;
use crate::explorer_panel;
use crate::menus::{self, Direction, MenuState, ResizeDock};
use crate::terminal_panel::{Find, NewTerminal, Restart};

/// One command of the palette.
pub struct CommandSpec {
    /// Stable key for usage counts: action name plus label.
    pub id: SharedString,
    /// Shown and searched, `"<menu>: <item>"` (VS Code's "Category: Name").
    pub label: SharedString,
    /// Heading on an empty query: the top-level menu or the extras' group.
    pub group: SharedString,
    /// Extra search terms (synonyms), never shown.
    pub keywords: &'static [&'static str],
    pub icon: Option<IconName>,
    pub action: Box<dyn Action>,
    /// A menu check mark (current section, auto-hide on).
    pub checked: bool,
    pub disabled: bool,
}

impl Clone for CommandSpec {
    fn clone(&self) -> Self {
        Self {
            id: self.id.clone(),
            label: self.label.clone(),
            group: self.group.clone(),
            keywords: self.keywords,
            icon: self.icon,
            action: self.action.boxed_clone(),
            checked: self.checked,
            disabled: self.disabled,
        }
    }
}

impl CommandSpec {
    fn new(group: &str, label: impl Into<SharedString>, action: Box<dyn Action>) -> Self {
        let label = label.into();
        let mut spec = Self {
            id: format!("{}|{label}", action.name()).into(),
            label,
            group: SharedString::from(group.to_string()),
            keywords: &[],
            icon: None,
            action,
            checked: false,
            disabled: false,
        };
        decorate(&mut spec);
        spec
    }
}

/// Every command, menus first. The palette further hides those whose action
/// has no handler where focus was (`Window::available_actions`).
pub fn all(state: &MenuState) -> Vec<CommandSpec> {
    let mut commands = from_menus(menus::app_menus(state));
    commands.extend(extras());
    commands
}

/// Flattens the menus: each action item becomes `"<innermost menu>: <item>"`,
/// grouped under its top-level menu. The palette's own menu item is skipped.
fn from_menus(menus: Vec<Menu>) -> Vec<CommandSpec> {
    fn walk(menu: Menu, group: &str, out: &mut Vec<CommandSpec>) {
        for item in menu.items {
            match item {
                MenuItem::Submenu(sub) => walk(sub, group, out),
                MenuItem::Action {
                    name,
                    action,
                    checked,
                    disabled,
                    ..
                } => {
                    if action.partial_eq(&menus::CommandPalette) {
                        continue;
                    }
                    let item = name.trim_end_matches("...").trim_end_matches('…');
                    let mut spec =
                        CommandSpec::new(group, format!("{}: {item}", menu.name), action);
                    spec.checked = checked;
                    spec.disabled = disabled;
                    out.push(spec);
                }
                MenuItem::Separator | MenuItem::SystemMenu(_) => {}
            }
        }
    }
    let mut out = Vec::new();
    for menu in menus {
        let group = menu.name.to_string();
        walk(menu, &group, &mut out);
    }
    out
}

/// Commands no menu shows: editor, terminal, explorer and dock-edge commands.
fn extras() -> Vec<CommandSpec> {
    let mut out = vec![CommandSpec::new(
        "Editor",
        "Editor: Close Editor",
        Box::new(CloseEditor),
    )];

    out.push(CommandSpec::new(
        "Terminal",
        "Terminal: New Terminal",
        Box::new(NewTerminal {
            profile: SharedString::default(),
        }),
    ));
    for name in profile_names() {
        out.push(CommandSpec::new(
            "Terminal",
            format!("Terminal: New {name}"),
            Box::new(NewTerminal {
                profile: name.clone().into(),
            }),
        ));
    }
    out.push(CommandSpec::new(
        "Terminal",
        "Terminal: Find",
        Box::new(Find),
    ));
    out.push(CommandSpec::new(
        "Terminal",
        "Terminal: Restart",
        Box::new(Restart),
    ));

    for (label, direction) in [
        ("Left", Direction::Left),
        ("Right", Direction::Right),
        ("Up", Direction::Up),
        ("Down", Direction::Down),
    ] {
        out.push(CommandSpec::new(
            "View",
            format!("Dock: Move Edge {label}"),
            Box::new(ResizeDock { direction }),
        ));
    }

    for (label, action) in [
        (
            "Stop Response",
            Box::new(chat_panel::StopReply) as Box<dyn Action>,
        ),
        ("Retry Last Reply", Box::new(chat_panel::RetryReply)),
        ("Clear Chat", Box::new(chat_panel::ClearChat)),
        ("Attach Current File", Box::new(chat_panel::AttachFile)),
        ("Attach Selection", Box::new(chat_panel::AttachSelection)),
        ("Cancel Editing", Box::new(chat_panel::CancelEdit)),
    ] {
        out.push(CommandSpec::new("Chat", format!("Chat: {label}"), action));
    }

    let explorer: [(&str, Box<dyn Action>); 10] = [
        ("New File", Box::new(explorer_panel::NewFile)),
        ("New Folder", Box::new(explorer_panel::NewFolder)),
        ("Rename", Box::new(explorer_panel::Rename)),
        ("Delete", Box::new(explorer_panel::Delete)),
        ("Copy Path", Box::new(explorer_panel::CopyPath)),
        (
            "Copy Relative Path",
            Box::new(explorer_panel::CopyRelativePath),
        ),
        (
            explorer_panel::REVEAL_LABEL,
            Box::new(explorer_panel::RevealInOs),
        ),
        ("Collapse All", Box::new(explorer_panel::CollapseAll)),
        ("Undo File Operation", Box::new(explorer_panel::UndoFileOp)),
        ("Select All", Box::new(explorer_panel::SelectAll)),
    ];
    for (label, action) in explorer {
        out.push(CommandSpec::new(
            "Explorer",
            format!("Explorer: {label}"),
            action,
        ));
    }
    out
}

/// Installed shells, probed once (it looks on disk).
fn profile_names() -> &'static [String] {
    static NAMES: OnceLock<Vec<String>> = OnceLock::new();
    NAMES.get_or_init(|| {
        shells::profiles()
            .into_iter()
            .map(|profile| profile.name)
            .collect()
    })
}

/// Icon and synonyms by action (the menus only give a name and an action).
fn decorate(spec: &mut CommandSpec) {
    let name = spec.action.name();
    let short = name.rsplit("::").next().unwrap_or(name);
    let (icon, keywords): (Option<IconName>, &'static [&'static str]) = match short {
        "OpenFolder" => (Some(IconName::FolderOpen), &["workspace", "project"]),
        "CreateFolder" => (
            Some(IconName::FolderPlus),
            &["workspace", "project", "mkdir"],
        ),
        "CloseFolder" => (Some(IconName::FolderX), &["workspace", "project"]),
        "NewFile" => (Some(IconName::FilePlus), &["create", "untitled"]),
        "SaveFile" => (Some(IconName::Save), &["write"]),
        "Quit" => (Some(IconName::LogOut), &["exit", "close app"]),
        "Undo" => (Some(IconName::Undo), &[]),
        "Redo" => (Some(IconName::Redo), &[]),
        "Cut" => (Some(IconName::Scissors), &["clipboard"]),
        "Copy" => (Some(IconName::Copy), &["clipboard"]),
        "Paste" => (Some(IconName::ClipboardPaste), &["clipboard"]),
        "QuickOpen" => (
            Some(IconName::FileSearch),
            &["open file", "find file", "quick open"],
        ),
        "GoToLine" => (Some(IconName::ListOrdered), &["jump", "line number"]),
        "SwitchSection" => (Some(IconName::Code), &["mode", "workspace"]),
        "ToggleDock" | "ToggleAutoHide" => (
            Some(side_icon(&spec.label)),
            &["sidebar", "panel", "show", "hide"],
        ),
        "ResizeDock" => (Some(IconName::Move), &["resize", "size", "width", "height"]),
        "MovePanel" => (Some(IconName::Move), &["dock", "tab"]),
        "MoveBesideEditor" => (
            Some(IconName::Move),
            &["split", "side", "editor area", "terminal", "chat"],
        ),
        "AddPanel" => (Some(IconName::Plus), &["open", "show", "tab"]),
        "SaveLayoutAs" | "ApplyLayout" | "DeleteLayout" | "ApplyPreset" | "ResetLayout" => (
            Some(IconName::LayoutDashboard),
            &["workspace", "arrangement"],
        ),
        "NewTerminal" | "Restart" => (Some(IconName::SquareTerminal), &["shell", "console"]),
        "Find" => (Some(IconName::Search), &["search"]),
        "StopReply" => (Some(IconName::Square), &["cancel", "chat", "agent"]),
        "RetryReply" => (Some(IconName::RefreshCw), &["regenerate", "again", "chat"]),
        "ClearChat" => (Some(IconName::Eraser), &["new chat", "reset", "start over"]),
        "AttachFile" | "AttachSelection" => (
            Some(IconName::Paperclip),
            &["context", "chip", "agent", "chat"],
        ),
        "CancelEdit" => (Some(IconName::Close), &["chat", "edit"]),
        "CloseEditor" => (Some(IconName::Close), &["tab"]),
        "About" => (Some(IconName::Info), &["version"]),
        _ if spec.group.as_ref() == "Explorer" => (Some(IconName::Folder), &["file", "tree"]),
        _ => (None, &[]),
    };
    spec.icon = icon;
    spec.keywords = keywords;
}

fn side_icon(label: &str) -> IconName {
    if label.contains("Left") {
        IconName::PanelLeft
    } else if label.contains("Right") {
        IconName::PanelRight
    } else {
        IconName::PanelBottom
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    // Not `super::*`: the gpui glob brings a `test` macro that shadows the built-in one.
    use gpui_kit::{Action, Menu, MenuItem};

    use super::{all, profile_names};
    use crate::layouts::Side;
    use crate::menus::{self, MenuState};

    fn full_state() -> MenuState {
        MenuState {
            has_workspace: true,
            saved: vec!["Mine".into(), "Review".into()],
            auto_hide: vec![Side::Left],
            ..MenuState::default()
        }
    }

    fn menu_actions(menus: Vec<Menu>) -> Vec<Box<dyn Action>> {
        fn walk(items: Vec<MenuItem>, out: &mut Vec<Box<dyn Action>>) {
            for item in items {
                match item {
                    MenuItem::Submenu(menu) => walk(menu.items, out),
                    MenuItem::Action { action, .. } => out.push(action),
                    _ => {}
                }
            }
        }
        let mut out = Vec::new();
        for menu in menus {
            walk(menu.items, &mut out);
        }
        out
    }

    #[test]
    fn every_menu_action_is_a_command_once() {
        let state = full_state();
        let commands = all(&state);
        for action in menu_actions(menus::app_menus(&state)) {
            if action.partial_eq(&menus::CommandPalette) {
                continue;
            }
            let count = commands
                .iter()
                .filter(|spec| spec.action.partial_eq(action.as_ref()))
                .count();
            assert_eq!(count, 1, "{} should be one command", action.name());
        }
    }

    /// By action type: Ctrl+Alt+N is bound for all nine layout slots, used or not.
    #[test]
    fn every_bound_action_has_a_command() {
        let commands = all(&full_state());
        for binding in menus::app_bindings() {
            let action = binding.action();
            if action.partial_eq(&menus::CommandPalette) {
                continue;
            }
            assert!(
                commands
                    .iter()
                    .any(|spec| spec.action.name() == action.name()),
                "{} has a key but no command",
                action.name()
            );
        }
    }

    #[test]
    fn ids_and_labels_are_unique() {
        let commands = all(&full_state());
        let ids: HashSet<_> = commands.iter().map(|spec| &spec.id).collect();
        let labels: HashSet<_> = commands.iter().map(|spec| &spec.label).collect();
        assert_eq!(ids.len(), commands.len());
        assert_eq!(labels.len(), commands.len());
    }

    #[test]
    fn labels_name_their_menu_and_drop_ellipsis() {
        let commands = all(&full_state());
        let labels: Vec<&str> = commands.iter().map(|spec| spec.label.as_ref()).collect();
        assert!(labels.contains(&"File: Open Folder"));
        assert!(labels.contains(&"View: Toggle Left Dock"));
        assert!(labels.contains(&"Apply Layout: Mine"));
        assert!(labels.contains(&"Terminal: New Terminal"));
        assert!(!labels.iter().any(|label| label.ends_with("...")));
    }

    #[test]
    fn checks_carry_over() {
        let commands = all(&full_state());
        let auto_hide_left = commands
            .iter()
            .find(|spec| spec.label.as_ref() == "View: Auto-Hide Left Dock")
            .unwrap();
        assert!(auto_hide_left.checked);
    }

    #[test]
    fn terminal_profiles_are_commands() {
        let commands = all(&full_state());
        for name in profile_names() {
            let label = format!("Terminal: New {name}");
            assert!(commands.iter().any(|spec| spec.label.as_ref() == label));
        }
    }
}
