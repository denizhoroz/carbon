mod app_assets;
mod chat_panel;
mod commands;
mod editor_panel;
mod explorer_panel;
mod layouts;
mod lsp_ops;
mod manage;
mod manage_workflows;
mod menus;
mod palette;
mod save_ops;
mod section_bar;
mod status_bar;
mod style;
mod terminal_panel;
mod test_panel;
mod title_bar;
mod view_panel;
#[cfg(feature = "webview")]
mod webview_panel;
mod workbench;
mod workflow_install;
mod workflows;
mod workspace_ops;

use std::path::PathBuf;

use carbon_core::{Core, FsChanges};
use editor_panel::{CloseEditor, SaveFile};
use gpui_kit::component::{ActiveTheme, TitleBar, WindowExt, h_flex, v_flex};
use gpui_kit::*;
use menus::{
    AddPanel, ApplyLayout, ApplyPreset, CloseFolder, CommandPalette, CreateFolder, DeleteLayout,
    GoToLine, MenuState, MoveBesideEditor, MovePanel, NewFile, OpenFolder, QuickOpen, ResetLayout,
    ResizeDock, SaveLayoutAs, SwitchSection, ToggleAutoHide, ToggleDock,
};
use palette::{FileIndex, Palette};
use section_bar::SectionBar;
use status_bar::CarbonStatusBar;
use style::fixed;
use title_bar::AppTitleBar;
use workbench::{MenusChanged, Workbench};

/// Root view: title bar on top; Section Bar and dock workbench in the middle;
/// status bar at the bottom. Holds the window's base focus so menu actions
/// reach these handlers even before a panel is focused.
struct CarbonApp {
    title_bar: Entity<AppTitleBar>,
    section_bar: Entity<SectionBar>,
    status_bar: Entity<CarbonStatusBar>,
    core: Entity<Core>,
    workbench: Entity<Workbench>,
    focus_handle: FocusHandle,
    _menus_sub: Subscription,
    _core_sub: Subscription,
    _focus_lost_sub: Subscription,
    /// The watcher channel being applied, and the task applying it.
    watch: Option<(async_channel::Receiver<FsChanges>, Task<()>)>,
    /// The open command palette, drawn over everything under the title bar.
    palette: Option<(Entity<Palette>, Subscription)>,
    /// Quick Open's file list; kept between palette openings.
    files: Entity<FileIndex>,
}

impl CarbonApp {
    /// `folder`: workspace to open (from the command line), if any.
    fn new(folder: Option<PathBuf>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let core = cx.new(|_| {
            let mut core = Core::new();
            core.chats.set_dir(chat_panel::CHAT_DIR);
            if let Some(folder) = folder
                && let Err(err) = core.open_workspace(&folder)
            {
                eprintln!("cannot open {}: {err}", folder.display());
            }
            #[cfg(debug_assertions)]
            workspace_ops::seed_test_marks(&mut core);
            core
        });
        // Panels built from saved layouts find the core here (before the workbench loads).
        cx.set_global(workspace_ops::CurrentCore(core.clone()));
        let status_bar = cx.new(|cx| CarbonStatusBar::new(core.clone(), cx));
        let focus_handle = cx.focus_handle();
        focus_handle.focus(window, cx);
        let title_bar = cx.new(|cx| AppTitleBar::new(cx));
        let workbench = cx.new(|cx| Workbench::new(window, cx));
        cx.set_global(workbench::CurrentWorkbench(workbench.downgrade()));
        let section = workbench.read(cx).menu_state().section;
        let section_bar = cx.new(|_| SectionBar::new(section));
        let _menus_sub = cx.subscribe(&workbench, |this, _, _: &MenusChanged, cx| {
            this.refresh_menus(cx)
        });
        // Close Folder is enabled only with a folder open.
        let _core_sub = cx.observe(&core, |this, _, cx| {
            this.refresh_menus(cx);
            this.sync_watch(cx);
        });
        // With nothing focused, actions start at the window root and miss the
        // handlers below (closing a menu or dialog can leave focus empty).
        let _focus_lost_sub = cx.on_focus_lost(window, |this, window, cx| {
            this.focus_handle.focus(window, cx)
        });
        // Alt+F4, the taskbar: unsaved changes are asked about first.
        window.on_window_should_close(cx, save_ops::can_close);
        let mut this = Self {
            title_bar,
            section_bar,
            status_bar,
            core,
            _core_sub,
            workbench,
            focus_handle,
            _menus_sub,
            _focus_lost_sub,
            watch: None,
            palette: None,
            files: cx.new(|_| FileIndex::new()),
        };
        this.refresh_menus(cx);
        this.sync_watch(cx);
        // Start-up messages (safe mode, files that did not load, reset layouts)
        // once the window can show notifications.
        cx.defer_in(window, |this, window, cx| {
            let mut notices = workflows::notices().to_vec();
            notices.extend(this.workbench.update(cx, |w, _| w.take_notices()));
            for notice in notices {
                window.push_notification(notice, cx);
            }
        });
        this
    }

    fn menu_state(&self, cx: &App) -> MenuState {
        let mut state = self.workbench.read(cx).menu_state();
        state.has_workspace = self.core.read(cx).workspace().is_some();
        state.workflow_commands = workflows::commands(&state.section, cx);
        state
    }

    fn refresh_menus(&self, cx: &mut App) {
        let state = self.menu_state(cx);
        self.section_bar
            .update(cx, |bar, cx| bar.set_active(state.section.clone(), cx));
        menus::refresh(&state, cx);
        self.title_bar
            .update(cx, |title_bar, cx| title_bar.reload_menus(cx));
    }

    /// Follows the core's watcher: a new workspace has a new channel.
    fn sync_watch(&mut self, cx: &mut App) {
        let changes = self.core.read(cx).fs_changes();
        let current = self.watch.as_ref().map(|(changes, _)| changes);
        let same = match (&changes, current) {
            (Some(new), Some(old)) => new.same_channel(old),
            (None, None) => true,
            _ => false,
        };
        if !same {
            self.watch = changes.map(|changes| {
                let task = workspace_ops::apply_changes(&self.core, changes.clone(), cx);
                (changes, task)
            });
        }
    }

    /// Opens the palette with `prefix` typed (`>` commands, `` files, `:`
    /// line), or switches an open one to it.
    fn open_palette(&mut self, prefix: &'static str, window: &mut Window, cx: &mut Context<Self>) {
        if let Some((palette, _)) = &self.palette {
            palette.update(cx, |palette, cx| palette.set_query(prefix, window, cx));
            return;
        }
        // Deferred: a menu gives focus back after its action runs, and the
        // palette must see where focus really is.
        cx.defer_in(window, move |this, window, cx| {
            if this.palette.is_some() {
                return;
            }
            let menu = this.menu_state(cx);
            let (core, files) = (this.core.clone(), this.files.clone());
            let palette = cx.new(|cx| Palette::new(prefix, &menu, core, files, window, cx));
            let sub = cx.subscribe(&palette, |this, _, _: &DismissEvent, cx| {
                this.palette = None;
                cx.notify();
            });
            this.palette = Some((palette, sub));
            cx.notify();
        });
    }

    /// Wires an action to a `Workbench` method.
    fn on<A: Action>(
        &self,
        f: impl Fn(&mut Workbench, &A, &mut Window, &mut Context<Workbench>) + 'static,
        cx: &mut Context<Self>,
    ) -> impl Fn(&A, &mut Window, &mut App) + 'static {
        cx.listener(move |this, action: &A, window, cx| {
            this.workbench
                .update(cx, |workbench, cx| f(workbench, action, window, cx))
        })
    }
}

impl Render for CarbonApp {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = self.palette.as_ref().map(|(palette, _)| {
            let viewport = window.viewport_size();
            let width = fixed::PALETTE_WIDTH
                .min(viewport.width - fixed::PALETTE_WINDOW_MARGIN * 2.)
                .max(px(0.));
            let origin = point(
                (viewport.width - width) / 2.,
                fixed::BAR_HEIGHT + fixed::PALETTE_TOP_GAP,
            );
            deferred(
                anchored()
                    .position(origin)
                    .snap_to_window_with_margin(fixed::PALETTE_WINDOW_MARGIN)
                    .child(div().w(width).child(palette.clone())),
            )
            .with_priority(1)
        });
        v_flex()
            .track_focus(&self.focus_handle)
            .on_action(self.on(
                |w, a: &ToggleDock, window, cx| w.toggle_dock(a.side, window, cx),
                cx,
            ))
            .on_action(self.on(
                |w, a: &ToggleAutoHide, _, cx| w.toggle_auto_hide(a.side, cx),
                cx,
            ))
            .on_action(self.on(
                |w, a: &MovePanel, window, cx| w.move_panel(a.side, window, cx),
                cx,
            ))
            .on_action(self.on(
                |w, a: &ResizeDock, window, cx| w.resize_dock(a.direction, window, cx),
                cx,
            ))
            .on_action(self.on(
                |w, a: &MoveBesideEditor, window, cx| w.move_beside_editor(a.direction, window, cx),
                cx,
            ))
            .on_action(self.on(
                |w, a: &ApplyPreset, window, cx| w.apply_preset(a.preset, window, cx),
                cx,
            ))
            .on_action(self.on(
                |w, _: &ResetLayout, window, cx| w.reset_layout(window, cx),
                cx,
            ))
            .on_action(self.on(
                |w, a: &SwitchSection, window, cx| w.switch_section(&a.id, window, cx),
                cx,
            ))
            .on_action(self.on(
                |w, _: &SaveLayoutAs, window, cx| w.open_save_dialog(window, cx),
                cx,
            ))
            .on_action(self.on(
                |w, a: &AddPanel, window, cx| w.add_panel(&a.name, None, window, cx),
                cx,
            ))
            .on_action(self.on(
                |w, a: &terminal_panel::NewTerminal, window, cx| {
                    w.add_panel("Terminal", Some(&a.profile), window, cx)
                },
                cx,
            ))
            .on_action(self.on(
                |w, a: &ApplyLayout, window, cx| w.apply_layout(a.index, window, cx),
                cx,
            ))
            .on_action(self.on(
                |w, a: &DeleteLayout, _, cx| w.delete_layout(a.index, cx),
                cx,
            ))
            .on_action(cx.listener(|this, _: &OpenFolder, window, cx| {
                workspace_ops::open_folder(this.core.clone(), window, cx)
            }))
            .on_action(cx.listener(|this, _: &CreateFolder, window, cx| {
                workspace_ops::open_create_dialog(this.core.clone(), window, cx)
            }))
            .on_action(cx.listener(|this, _: &CloseFolder, _, cx| {
                workspace_ops::close_folder(&this.core, cx)
            }))
            .on_action(|_: &NewFile, window, cx| editor_panel::new_file(window, cx))
            .on_action(cx.listener(|this, _: &CommandPalette, window, cx| {
                this.open_palette(">", window, cx)
            }))
            .on_action(
                cx.listener(|this, _: &QuickOpen, window, cx| this.open_palette("", window, cx)),
            )
            .on_action(
                cx.listener(|this, _: &GoToLine, window, cx| this.open_palette(":", window, cx)),
            )
            // Save / close with focus outside the editors act on the active one.
            .on_action(|action: &SaveFile, window, cx| {
                if let Some(editor) = workbench::active_editor(cx) {
                    editor.update(cx, |editor, cx| editor.save(action, window, cx));
                }
            })
            .on_action(|action: &terminal_panel::Find, window, cx| {
                terminal_panel::on_root_action(action, window, cx)
            })
            .on_action(|action: &terminal_panel::Restart, window, cx| {
                terminal_panel::on_root_action(action, window, cx)
            })
            .on_action(|action: &CloseEditor, window, cx| {
                if let Some(editor) = workbench::active_editor(cx) {
                    editor.update(cx, |editor, cx| editor.close(action, window, cx));
                }
            })
            // Auto-hide: after any click is handled, close auto-hide docks it landed outside of.
            .capture_any_mouse_down(cx.listener(|this, event: &MouseDownEvent, window, cx| {
                let position = event.position;
                let workbench = this.workbench.clone();
                window.defer(cx, move |window, cx| {
                    workbench.update(cx, |workbench, cx| {
                        workbench.hide_auto_docks_outside(position, window, cx)
                    });
                });
            }))
            .size_full()
            .bg(cx.theme().background)
            .child(self.title_bar.clone())
            .child(
                h_flex()
                    .flex_1()
                    .min_h_0()
                    .child(self.section_bar.clone())
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .h_full()
                            .child(self.workbench.clone()),
                    ),
            )
            .child(self.status_bar.clone())
            .children(palette)
    }
}

/// Usage: `carbon-gui [--safe-mode] [folder]` opens `folder` as the workspace.
/// `--safe-mode`: every workflow opens with its default layout and saved
/// layouts are left untouched.
fn main() {
    #[cfg(feature = "webview")]
    webview_panel::prepare();
    let mut safe_mode = false;
    let mut folder = None;
    for arg in std::env::args_os().skip(1) {
        if arg == "--safe-mode" {
            safe_mode = true;
        } else if folder.is_none() {
            folder = Some(PathBuf::from(arg));
        }
    }
    application()
        .with_assets(app_assets::AppAssets)
        .run(move |cx| {
            init(cx);
            workflows::init(safe_mode);
            style::theme::init(cx);
            menus::init(cx);
            test_panel::register(cx);
            explorer_panel::register(cx);
            editor_panel::register(cx);
            terminal_panel::register(cx);
            chat_panel::register(cx);
            view_panel::register(cx);
            #[cfg(feature = "webview")]
            webview_panel::register(cx);
            palette::register(cx);
            let options = WindowOptions {
                window_bounds: Some(WindowBounds::centered(style::fixed::WINDOW_SIZE, cx)),
                ..TitleBar::window_options()
            };
            open_window(options, cx, move |window, cx| {
                cx.new(|cx| CarbonApp::new(folder.clone(), window, cx))
            })
            .expect("failed to open window");
        });
}
