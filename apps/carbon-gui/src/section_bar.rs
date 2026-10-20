//! Section Bar: vertical icon strip on the left (like VS Code's activity bar).
//! Each icon at the top switches the whole workbench to that workflow's layout
//! (one per available workflow in `workflows::registry`); the icons at the
//! bottom open Manage dialogs (`manage.rs`). When the workflows do not fit,
//! the list scrolls (mouse wheel) and up/down arrows appear at its ends
//! (workflow-system.md s4.10); the Manage buttons stay put.

use carbon_core::workflow::WorkflowId;
use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::{ActiveTheme, Disableable, Icon, Selectable, Sizable, Size, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::manage::{self, Manage};
use crate::menus::SwitchSection;
use crate::style::fixed;
use crate::workflows;

pub struct SectionBar {
    active: WorkflowId,
    scroll: ScrollHandle,
}

impl SectionBar {
    pub fn new(active: WorkflowId) -> Self {
        Self {
            active,
            scroll: ScrollHandle::new(),
        }
    }

    pub fn set_active(&mut self, active: WorkflowId, cx: &mut Context<Self>) {
        self.active = active;
        cx.notify();
    }

    /// Scrolls the workflow list by one button (`down` = towards the end).
    fn step(&mut self, down: bool, cx: &mut Context<Self>) {
        let step = fixed::SECTION_BUTTON_SIZE + fixed::SECTION_BAR_STEP_GAP;
        let max = self.scroll.max_offset().y;
        let mut offset = self.scroll.offset();
        offset.y = if down {
            (offset.y - step).max(-max)
        } else {
            (offset.y + step).min(px(0.))
        };
        self.scroll.set_offset(offset);
        cx.notify();
    }
}

impl Render for SectionBar {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // From the last layout: does the list overflow, and where is it?
        let max = self.scroll.max_offset().y;
        let offset = self.scroll.offset().y;
        let overflow = max > px(0.);
        let (at_top, at_bottom) = (offset >= px(0.), offset <= -max);
        let registry = workflows::registry();
        let arrow = |id: &'static str, icon: IconName, label: &'static str, down: bool| {
            Button::new(id)
                .icon(Icon::new(icon).with_size(Size::Size(fixed::SECTION_ARROW_ICON_SIZE)))
                .ghost()
                .with_size(Size::Size(fixed::SECTION_ARROW_HEIGHT))
                .w(fixed::SECTION_BUTTON_SIZE)
                .tooltip(label)
                .accessibility_label(label)
                .disabled(if down { at_bottom } else { at_top })
                .on_click(cx.listener(move |this, _, _, cx| this.step(down, cx)))
        };
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
            .when(overflow, |bar| {
                bar.child(arrow(
                    "workflows-up",
                    IconName::ChevronUp,
                    "More workflows above",
                    false,
                ))
            })
            .child(
                v_flex()
                    .id("section-bar-workflows")
                    .flex_1()
                    .min_h_0()
                    .w_full()
                    .items_center()
                    .gap(fixed::SECTION_BAR_GAP)
                    .overflow_y_scroll()
                    .track_scroll(&self.scroll)
                    // Wheel scrolling moves the list; redraw so the arrows follow.
                    .on_scroll_wheel(cx.listener(|_, _, _, cx| cx.notify()))
                    .children(registry.available().map(|def| {
                        let id = def.id.clone();
                        bar_button(
                            SharedString::from(format!("workflow-{id}")),
                            workflows::icon(def),
                            SharedString::from(def.name.clone()),
                        )
                        .flex_none()
                        .selected(def.id == self.active)
                        .on_click(move |_, window, cx| {
                            window.dispatch_action(Box::new(SwitchSection { id: id.clone() }), cx)
                        })
                    })),
            )
            .when(overflow, |bar| {
                bar.child(arrow(
                    "workflows-down",
                    IconName::ChevronDown,
                    "More workflows below",
                    true,
                ))
            })
            // Bottom group: Manage dialogs, below the (growing) workflow list.
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
