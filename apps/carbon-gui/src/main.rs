mod app_assets;
mod layouts;
mod menus;
mod section_bar;
mod style;
mod test_panel;
mod title_bar;
mod workbench;

use gpui_kit::component::{ActiveTheme, TitleBar, h_flex, v_flex};
use gpui_kit::*;
use menus::{
    ApplyLayout, ApplyPreset, DeleteLayout, MovePanel, ResetLayout, ResizeDock, SaveLayoutAs,
    SwitchSection, ToggleAutoHide, ToggleDock,
};
use section_bar::SectionBar;
use title_bar::AppTitleBar;
use workbench::{MenusChanged, Workbench};

/// Root view: title bar on top; Section Bar and dock workbench below. Holds the
/// window's base focus so menu actions reach these handlers even before a panel is focused.
struct CarbonApp {
    title_bar: Entity<AppTitleBar>,
    section_bar: Entity<SectionBar>,
    workbench: Entity<Workbench>,
    focus_handle: FocusHandle,
    _menus_sub: Subscription,
}

impl CarbonApp {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let focus_handle = cx.focus_handle();
        focus_handle.focus(window, cx);
        let title_bar = cx.new(|cx| AppTitleBar::new(cx));
        let workbench = cx.new(|cx| Workbench::new(window, cx));
        let section = workbench.read(cx).menu_state().section;
        let section_bar = cx.new(|_| SectionBar::new(section));
        let _menus_sub = cx.subscribe(&workbench, |this, _, _: &MenusChanged, cx| {
            this.refresh_menus(cx)
        });
        let this = Self {
            title_bar,
            section_bar,
            workbench,
            focus_handle,
            _menus_sub,
        };
        this.refresh_menus(cx);
        this
    }

    fn refresh_menus(&self, cx: &mut App) {
        let state = self.workbench.read(cx).menu_state();
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
    }
}

fn main() {
    application().with_assets(app_assets::AppAssets).run(|cx| {
        init(cx);
        style::theme::init(cx);
        menus::init(cx);
        test_panel::register(cx);
        let options = WindowOptions {
            window_bounds: Some(WindowBounds::centered(style::fixed::WINDOW_SIZE, cx)),
            ..TitleBar::window_options()
        };
        open_window(options, cx, |window, cx| cx.new(|cx| CarbonApp::new(window, cx)))
            .expect("failed to open window");
    });
}
