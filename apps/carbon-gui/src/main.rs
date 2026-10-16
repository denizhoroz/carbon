mod app_assets;
mod explorer_panel;
mod layouts;
mod menus;
mod section_bar;
mod status_bar;
mod style;
mod test_panel;
mod title_bar;
mod workbench;
mod workspace_ops;

use std::path::PathBuf;

use carbon_core::Core;
use gpui_kit::component::{ActiveTheme, TitleBar, h_flex, v_flex};
use gpui_kit::*;
use menus::{
    ApplyLayout, ApplyPreset, CloseFolder, CreateFolder, DeleteLayout, MovePanel, OpenFolder,
    ResetLayout, ResizeDock, SaveLayoutAs, SwitchSection, ToggleAutoHide, ToggleDock,
};
use section_bar::SectionBar;
use status_bar::CarbonStatusBar;
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
}

impl CarbonApp {
    /// `folder`: workspace to open (from the command line), if any.
    fn new(folder: Option<PathBuf>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let core = cx.new(|_| {
            let mut core = Core::new();
            if let Some(folder) = folder
                && let Err(err) = core.open_workspace(&folder)
            {
                eprintln!("cannot open {}: {err}", folder.display());
            }
            core
        });
        // Panels built from saved layouts find the core here (before the workbench loads).
        cx.set_global(workspace_ops::CurrentCore(core.clone()));
        let status_bar = cx.new(|cx| CarbonStatusBar::new(core.clone(), cx));
        let focus_handle = cx.focus_handle();
        focus_handle.focus(window, cx);
        let title_bar = cx.new(|cx| AppTitleBar::new(cx));
        let workbench = cx.new(|cx| Workbench::new(window, cx));
        let section = workbench.read(cx).menu_state().section;
        let section_bar = cx.new(|_| SectionBar::new(section));
        let _menus_sub = cx.subscribe(&workbench, |this, _, _: &MenusChanged, cx| {
            this.refresh_menus(cx)
        });
        // Close Folder is enabled only with a folder open.
        let _core_sub = cx.observe(&core, |this, _, cx| this.refresh_menus(cx));
        let this = Self {
            title_bar,
            section_bar,
            status_bar,
            core,
            _core_sub,
            workbench,
            focus_handle,
            _menus_sub,
        };
        this.refresh_menus(cx);
        this
    }

    fn refresh_menus(&self, cx: &mut App) {
        let mut state = self.workbench.read(cx).menu_state();
        state.has_workspace = self.core.read(cx).workspace().is_some();
        self.section_bar
            .update(cx, |bar, cx| bar.set_active(state.section, cx));
        menus::refresh(&state, cx);
        self.title_bar
            .update(cx, |title_bar, cx| title_bar.reload_menus(cx));
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
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .track_focus(&self.focus_handle)
            .on_action(self.on(|w, a: &ToggleDock, window, cx| w.toggle_dock(a.side, window, cx), cx))
            .on_action(self.on(|w, a: &ToggleAutoHide, _, cx| w.toggle_auto_hide(a.side, cx), cx))
            .on_action(self.on(|w, a: &MovePanel, window, cx| w.move_panel(a.side, window, cx), cx))
            .on_action(self.on(|w, a: &ResizeDock, window, cx| w.resize_dock(a.direction, window, cx), cx))
            .on_action(self.on(|w, a: &ApplyPreset, window, cx| w.apply_preset(a.preset, window, cx), cx))
            .on_action(self.on(|w, _: &ResetLayout, window, cx| w.reset_layout(window, cx), cx))
            .on_action(self.on(|w, a: &SwitchSection, window, cx| w.switch_section(a.section, window, cx), cx))
            .on_action(self.on(|w, _: &SaveLayoutAs, window, cx| w.open_save_dialog(window, cx), cx))
            .on_action(self.on(|w, a: &ApplyLayout, window, cx| w.apply_layout(a.index, window, cx), cx))
            .on_action(self.on(|w, a: &DeleteLayout, _, cx| w.delete_layout(a.index, cx), cx))
            .on_action(cx.listener(|this, _: &OpenFolder, window, cx| {
                workspace_ops::open_folder(this.core.clone(), window, cx)
            }))
            .on_action(cx.listener(|this, _: &CreateFolder, window, cx| {
                workspace_ops::open_create_dialog(this.core.clone(), window, cx)
            }))
            .on_action(cx.listener(|this, _: &CloseFolder, _, cx| {
                workspace_ops::close_folder(&this.core, cx)
            }))
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
                    .child(div().flex_1().min_w_0().h_full().child(self.workbench.clone())),
            )
            .child(self.status_bar.clone())
    }
}

/// Usage: `carbon-gui [folder]` opens `folder` as the workspace.
fn main() {
    let folder = std::env::args_os().nth(1).map(PathBuf::from);
    application().with_assets(app_assets::AppAssets).run(|cx| {
        init(cx);
        style::theme::init(cx);
        menus::init(cx);
        test_panel::register(cx);
        explorer_panel::register(cx);
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
