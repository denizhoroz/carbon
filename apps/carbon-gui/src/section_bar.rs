//! Section Bar: vertical icon strip on the left (like VS Code's activity bar).
//! Each icon at the top switches the whole workbench to that section's layout;
//! the icons at the bottom open Manage dialogs (`manage.rs`).

use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::{ActiveTheme, Icon, Selectable, Sizable, Size, v_flex};
use gpui_kit::*;
use serde::{Deserialize, Serialize};

use crate::manage::{self, Manage};
use crate::menus::SwitchSection;
use crate::style::fixed;

/// A top-level section of the app. Each has its own layout, kept while switching.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Section {
    Code,
}

impl Section {
    pub const ALL: [Section; 1] = [Section::Code];

    pub fn label(self) -> &'static str {
        match self {
            Section::Code => "Code Editor",
        }
    }

    fn icon(self) -> IconName {
        match self {
            Section::Code => IconName::Code,
        }
    }

    fn id(self) -> &'static str {
        match self {
            Section::Code => "section-code",
        }
    }
}

pub struct SectionBar {
    active: Section,
}

impl SectionBar {
    pub fn new(active: Section) -> Self {
        Self { active }
    }

    pub fn set_active(&mut self, active: Section, cx: &mut Context<Self>) {
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
            .children(Section::ALL.map(|section| {
                bar_button(section.id(), section.icon(), section.label())
                    .selected(section == self.active)
                    .on_click(move |_, window, cx| {
                        window.dispatch_action(Box::new(SwitchSection { section }), cx)
                    })
            }))
            // Bottom group: Manage dialogs, pushed down by the spacer.
            .child(div().flex_1())
            .children(Manage::ALL.map(|target| {
                bar_button(target.id(), target.icon(), target.label())
                    .on_click(move |_, window, cx| manage::open(target, window, cx))
            }))
    }
}

fn bar_button(id: &'static str, icon: IconName, label: &'static str) -> Button {
    Button::new(id)
        .icon(Icon::new(icon).with_size(Size::Size(fixed::SECTION_ICON_SIZE)))
        .ghost()
        .with_size(Size::Size(fixed::SECTION_BUTTON_SIZE))
        .tooltip(label)
        .accessibility_label(label)
}
