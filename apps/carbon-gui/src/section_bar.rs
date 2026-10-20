//! Section Bar: vertical icon strip on the left (like VS Code's activity bar).
//! Each icon at the top switches the whole workbench to that workflow's layout
//! (one per entry in `workflows::registry`); the icons at the bottom open
//! Manage dialogs (`manage.rs`).

use carbon_core::workflow::WorkflowId;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::{ActiveTheme, Icon, Selectable, Sizable, Size, v_flex};
use gpui_kit::*;

use crate::manage::{self, Manage};
use crate::menus::SwitchSection;
use crate::style::fixed;
use crate::workflows;

pub struct SectionBar {
    active: WorkflowId,
}

impl SectionBar {
    pub fn new(active: WorkflowId) -> Self {
        Self { active }
    }

    pub fn set_active(&mut self, active: WorkflowId, cx: &mut Context<Self>) {
        self.active = active;
        cx.notify();
    }
}

impl Render for SectionBar {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .id("section-bar")
            .flex_none()
            .w(fixed::SECTION_BAR_WIDTH)
            .h_full()
            .items_center()
            .py(fixed::SECTION_BAR_PADDING)
            .gap(fixed::SECTION_BAR_GAP)
            .bg(cx.theme().title_bar)
            .border_r_1()
            .border_color(cx.theme().title_bar_border)
            .children(workflows::registry().workflows().iter().map(|def| {
                let id = def.id.clone();
                bar_button(
                    SharedString::from(format!("workflow-{id}")),
                    workflows::icon(def),
                    SharedString::from(def.name.clone()),
                )
                .selected(def.id == self.active)
                .on_click(move |_, window, cx| {
                    window.dispatch_action(Box::new(SwitchSection { id: id.clone() }), cx)
                })
            }))
            // Bottom group: Manage dialogs, pushed down by the spacer.
            .child(div().flex_1())
            .children(Manage::ALL.map(|target| {
                bar_button(target.id(), Icon::new(target.icon()), target.label())
                    .on_click(move |_, window, cx| manage::open(target, window, cx))
            }))
    }
}

fn bar_button(id: impl Into<ElementId>, icon: Icon, label: impl Into<SharedString>) -> Button {
    let label = label.into();
    Button::new(id)
        .icon(icon.with_size(Size::Size(fixed::SECTION_ICON_SIZE)))
        .ghost()
        .with_size(Size::Size(fixed::SECTION_BUTTON_SIZE))
        .tooltip(label.clone())
        .accessibility_label(label)
}
