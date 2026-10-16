//! Section Bar: vertical icon strip on the left (like VS Code's activity bar).
//! Each icon switches the whole workbench to that section's layout.

use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::{ActiveTheme, Icon, Selectable, Sizable, Size, v_flex};
use gpui_kit::*;
use serde::{Deserialize, Serialize};

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
                Button::new(section.id())
                    .icon(Icon::new(section.icon()).with_size(Size::Size(fixed::SECTION_ICON_SIZE)))
                    .ghost()
                    .with_size(Size::Size(fixed::SECTION_BUTTON_SIZE))
                    .selected(section == self.active)
                    .tooltip(section.label())
                    .accessibility_label(section.label())
                    .on_click(move |_, window, cx| {
                        window.dispatch_action(Box::new(SwitchSection { section }), cx)
                    })
            }))
    }
}
