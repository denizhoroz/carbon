//! Application menus: native menu bar on macOS, in-window `AppMenuBar` elsewhere.
//! The View > Layout part changes with saved layouts, so menus are rebuilt by `refresh`.

use gpui_kit::component::GlobalState;
use gpui_kit::component::input::{Copy, Cut, Paste, Redo, Undo};
use gpui_kit::*;

use crate::editor_panel::SaveFile;

use crate::layouts::{MAX_SAVED, Preset, Side};

// About is a placeholder: it has no handler yet.
actions!(
    carbon,
    [
        NewFile,
        OpenFolder,
        CreateFolder,
        CloseFolder,
        CommandPalette,
        QuickOpen,
        GoToLine,
        About,
        Quit,
        ResetLayout,
        SaveLayoutAs,
        ToggleSectionSide,
        ManageExtensions,
        ManageSettings
    ]
);

/// Panels the user can add (View > Add Panel).
pub const ADDABLE_PANELS: [&str; 3] = ["Explorer", "Chat", "Terminal"];

/// Add a panel by name: the Explorer is shown (there is one); other kinds get
/// a new numbered tab beside their kind.
#[derive(Clone, PartialEq, Action)]
#[action(namespace = carbon, no_json)]
pub struct AddPanel {
    pub name: SharedString,
}

/// Run an extension's command (`key`: `<extension id>/<command id>`).
#[derive(Clone, PartialEq, Action)]
#[action(namespace = carbon, no_json)]
pub struct RunExtensionCommand {
    pub key: SharedString,
}

/// A menu item or panel an extension adds.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ExtensionItem {
    /// Top-level menu (`File`, `Edit`, `View`, `Help`).
    pub menu: SharedString,
    pub title: SharedString,
    /// Command key, or the `AddPanel` name of a panel.
    pub key: SharedString,
}

/// Show or hide a dock.
#[derive(Clone, PartialEq, Action)]
#[action(namespace = carbon, no_json)]
pub struct ToggleDock {
    pub side: Side,
}

/// Auto-hide: the dock closes on a click outside it and reopens with its toggle.
#[derive(Clone, PartialEq, Action)]
#[action(namespace = carbon, no_json)]
pub struct ToggleAutoHide {
    pub side: Side,
}

/// Move the focused panel to a dock, or to the center when `side` is `None`.
#[derive(Clone, PartialEq, Action)]
#[action(namespace = carbon, no_json)]
pub struct MovePanel {
    pub side: Option<Side>,
}

/// Move the focused panel into the editor area, beside the editors (VS Code's
/// "to the side"): a terminal or chat next to, below, left of or above the code.
#[derive(Clone, PartialEq, Action)]
#[action(namespace = carbon, no_json)]
pub struct MoveBesideEditor {
    pub direction: Direction,
}

/// Move the focused dock's inner edge one step (JetBrains-style keyboard resize).
#[derive(Clone, PartialEq, Action)]
#[action(namespace = carbon, no_json)]
pub struct ResizeDock {
    pub direction: Direction,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Direction {
    Left,
    Right,
    Up,
    Down,
}

#[derive(Clone, PartialEq, Action)]
#[action(namespace = carbon, no_json)]
pub struct ApplyPreset {
    pub preset: Preset,
}

/// Apply or delete the saved layout at `index` (0-based; hotkey Ctrl+Alt+index+1).
#[derive(Clone, PartialEq, Action)]
#[action(namespace = carbon, no_json)]
pub struct ApplyLayout {
    pub index: usize,
}

#[derive(Clone, PartialEq, Action)]
#[action(namespace = carbon, no_json)]
pub struct DeleteLayout {
    pub index: usize,
}

/// What the dynamic part of the menus shows.
#[derive(Default)]
pub struct MenuState {
    pub has_workspace: bool,
    pub saved: Vec<String>,
    pub auto_hide: Vec<Side>,
    /// Commands extensions place in the menus.
    pub extension_items: Vec<ExtensionItem>,
    /// Panels extensions add (View > Add Panel).
    pub extension_panels: Vec<ExtensionItem>,
    /// Every extension command for the palette: group (the extension's
    /// name), title, keywords, key.
    pub extension_commands: Vec<(SharedString, SharedString, Vec<SharedString>, SharedString)>,
}

pub(crate) fn app_menus(state: &MenuState) -> Vec<Menu> {
    let toggles = Side::ALL
        .map(|side| MenuItem::action(format!("Toggle {} Dock", side.label()), ToggleDock { side }));
    let auto_hide = Side::ALL.map(|side| {
        MenuItem::action(
            format!("Auto-Hide {} Dock", side.label()),
            ToggleAutoHide { side },
        )
        .checked(state.auto_hide.contains(&side))
    });

    let mut layout = vec![MenuItem::action("Save Layout As...", SaveLayoutAs)];
    if !state.saved.is_empty() {
        let items = |delete: bool| {
            state.saved.iter().enumerate().map(move |(index, name)| {
                if delete {
                    MenuItem::action(name.clone(), DeleteLayout { index })
                } else {
                    MenuItem::action(name.clone(), ApplyLayout { index })
                }
            })
        };
        layout.push(MenuItem::submenu(
            Menu::new("Apply Layout").items(items(false)),
        ));
        layout.push(MenuItem::submenu(
            Menu::new("Delete Layout").items(items(true)),
        ));
    }
    layout.push(MenuItem::separator());
    layout.extend(Preset::ALL.map(|preset| {
        MenuItem::action(
            format!("Preset: {}", preset.label()),
            ApplyPreset { preset },
        )
    }));
    layout.push(MenuItem::separator());
    layout.push(MenuItem::action(
        "Move Explorer Section to Other Side",
        ToggleSectionSide,
    ));
    layout.push(MenuItem::action("Reset Layout", ResetLayout));

    let mut add: Vec<MenuItem> = ADDABLE_PANELS
        .map(|name| MenuItem::action(name, AddPanel { name: name.into() }))
        .into();
    if !state.extension_panels.is_empty() {
        add.push(MenuItem::separator());
        add.extend(state.extension_panels.iter().map(|panel| {
            MenuItem::action(
                panel.title.clone(),
                AddPanel {
                    name: panel.key.clone(),
                },
            )
        }));
    }
    // Extension commands go at the end of their menu, after a separator.
    let extension_items = |menu: &str| -> Vec<MenuItem> {
        let items: Vec<MenuItem> = state
            .extension_items
            .iter()
            .filter(|item| item.menu == menu)
            .map(|item| {
                MenuItem::action(
                    item.title.clone(),
                    RunExtensionCommand {
                        key: item.key.clone(),
                    },
                )
            })
            .collect();
        if items.is_empty() {
            items
        } else {
            std::iter::once(MenuItem::separator())
                .chain(items)
                .collect()
        }
    };

    let panel = [
        MenuItem::action(
            "Move to Left Dock",
            MovePanel {
                side: Some(Side::Left),
            },
        ),
        MenuItem::action(
            "Move to Right Dock",
            MovePanel {
                side: Some(Side::Right),
            },
        ),
        MenuItem::action(
            "Move to Bottom Dock",
            MovePanel {
                side: Some(Side::Bottom),
            },
        ),
        MenuItem::action("Move to Center", MovePanel { side: None }),
        MenuItem::separator(),
        MenuItem::action(
            "Move Next to Editor (Right)",
            MoveBesideEditor {
                direction: Direction::Right,
            },
        ),
        MenuItem::action(
            "Move Next to Editor (Below)",
            MoveBesideEditor {
                direction: Direction::Down,
            },
        ),
        MenuItem::action(
            "Move Next to Editor (Left)",
            MoveBesideEditor {
                direction: Direction::Left,
            },
        ),
        MenuItem::action(
            "Move Next to Editor (Above)",
            MoveBesideEditor {
                direction: Direction::Up,
            },
        ),
    ];

    vec![
        Menu::new("File").items(
            [
                MenuItem::action("Open Folder...", OpenFolder),
                MenuItem::action("Create Folder...", CreateFolder),
                MenuItem::action("Close Folder", CloseFolder).disabled(!state.has_workspace),
                MenuItem::separator(),
                MenuItem::action("New File", NewFile),
                MenuItem::action("Save", SaveFile),
                MenuItem::separator(),
                // Also under the title bar's three-line button (VS Code: File > Preferences).
                MenuItem::submenu(Menu::new("Preferences").items([
                    MenuItem::action("Extensions", ManageExtensions),
                    MenuItem::action("Settings", ManageSettings),
                ])),
            ]
            .into_iter()
            .chain(extension_items("File"))
            .chain([MenuItem::separator(), MenuItem::action("Exit", Quit)]),
        ),
        Menu::new("Edit").items(
            [
                MenuItem::action("Undo", Undo),
                MenuItem::action("Redo", Redo),
                MenuItem::separator(),
                MenuItem::action("Cut", Cut),
                MenuItem::action("Copy", Copy),
                MenuItem::action("Paste", Paste),
            ]
            .into_iter()
            .chain(extension_items("Edit")),
        ),
        Menu::new("View").items(
            [
                MenuItem::action("Command Palette", CommandPalette),
                MenuItem::action("Go to File...", QuickOpen),
                MenuItem::action("Go to Line...", GoToLine),
                MenuItem::separator(),
            ]
            .into_iter()
            .chain(toggles)
            .chain([MenuItem::separator()])
            .chain(auto_hide)
            .chain([
                MenuItem::separator(),
                MenuItem::submenu(Menu::new("Layout").items(layout)),
                MenuItem::submenu(Menu::new("Add Panel").items(add)),
                MenuItem::submenu(Menu::new("Panel").items(panel)),
            ])
            .chain(extension_items("View")),
        ),
        Menu::new("Help").items(
            [MenuItem::action("About", About)]
                .into_iter()
                .chain(extension_items("Help")),
        ),
    ]
}

/// Registers the quit action and all key bindings, and sets the initial menus.
/// Call after `gpui_kit::init`.
pub fn init(cx: &mut App) {
    cx.on_action(|_: &Quit, cx| crate::save_ops::quit(cx));
    cx.bind_keys(app_bindings());
    refresh(&MenuState::default(), cx);
}

/// The app-wide key bindings (panels bind their own in their key context).
pub fn app_bindings() -> Vec<KeyBinding> {
    // Palette keys follow VS Code: Ctrl+Shift+P / F1 commands, Ctrl+P files, Ctrl+G line.
    // Dock toggles follow VS Code: Ctrl+B side bar, Ctrl+Alt+B secondary side bar, Ctrl+J panel.
    // Resize follows JetBrains: Ctrl+Alt+Shift+arrows. Move: Ctrl+K then an arrow.
    let mut bindings = vec![
        KeyBinding::new("secondary-q", Quit, None),
        KeyBinding::new("secondary-n", NewFile, None),
        KeyBinding::new("secondary-shift-p", CommandPalette, None),
        KeyBinding::new("f1", CommandPalette, None),
        KeyBinding::new("secondary-p", QuickOpen, None),
        KeyBinding::new("secondary-g", GoToLine, None),
        // VS Code: Ctrl+, opens Settings.
        KeyBinding::new("secondary-,", ManageSettings, None),
        // VS Code: Ctrl+K Ctrl+O opens a folder.
        KeyBinding::new("secondary-k secondary-o", OpenFolder, None),
        KeyBinding::new("secondary-b", ToggleDock { side: Side::Left }, None),
        KeyBinding::new("secondary-alt-b", ToggleDock { side: Side::Right }, None),
        KeyBinding::new("secondary-j", ToggleDock { side: Side::Bottom }, None),
        KeyBinding::new(
            "secondary-k left",
            MovePanel {
                side: Some(Side::Left),
            },
            None,
        ),
        KeyBinding::new(
            "secondary-k right",
            MovePanel {
                side: Some(Side::Right),
            },
            None,
        ),
        KeyBinding::new(
            "secondary-k down",
            MovePanel {
                side: Some(Side::Bottom),
            },
            None,
        ),
        KeyBinding::new("secondary-k up", MovePanel { side: None }, None),
    ];
    for (key, direction) in [
        ("left", Direction::Left),
        ("right", Direction::Right),
        ("up", Direction::Up),
        ("down", Direction::Down),
    ] {
        bindings.push(KeyBinding::new(
            &format!("ctrl-alt-shift-{key}"),
            ResizeDock { direction },
            None,
        ));
    }
    for index in 0..MAX_SAVED {
        bindings.push(KeyBinding::new(
            &format!("secondary-alt-{}", index + 1),
            ApplyLayout { index },
            None,
        ));
    }
    bindings
}

/// Rebuilds both the native and the in-window menus. The in-window `AppMenuBar`
/// must also be told to reload (`AppTitleBar::reload_menus`).
pub fn refresh(state: &MenuState, cx: &mut App) {
    cx.set_menus(app_menus(state));
    GlobalState::global_mut(cx)
        .set_app_menus(app_menus(state).into_iter().map(Menu::owned).collect());
}
