//! Drawing palette rows: icon, label with the matched letters picked out,
//! and the full shortcut (every key of a chord) on the right.

use std::ops::Range;

use gpui_kit::assets::IconName;
use gpui_kit::component::kbd::Kbd;
use gpui_kit::component::{ActiveTheme, Icon, Sizable, Size, h_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use super::query::Mode;
use crate::commands::CommandSpec;
use crate::style::fixed;

fn icon(name: IconName, cx: &App) -> Icon {
    Icon::new(name)
        .with_size(Size::Size(fixed::PALETTE_ICON_SIZE))
        .flex_none()
        .text_color(cx.theme().muted_foreground)
}

/// `text` with `ranges` (byte ranges) in the highlight colour, semibold.
fn highlighted(text: impl Into<SharedString>, ranges: &[Range<usize>], cx: &App) -> StyledText {
    let style = HighlightStyle {
        color: Some(cx.theme().list_active_border),
        font_weight: Some(FontWeight::SEMIBOLD),
        ..Default::default()
    };
    StyledText::new(text).with_highlights(ranges.iter().map(|range| (range.clone(), style)))
}

/// Splits byte ranges at `at`: those before it, and those after it shifted to
/// start at 0.
fn split_ranges(ranges: &[Range<usize>], at: usize) -> (Vec<Range<usize>>, Vec<Range<usize>>) {
    let mut before = Vec::new();
    let mut after = Vec::new();
    for range in ranges {
        if range.start < at {
            before.push(range.start..range.end.min(at));
        }
        if range.end > at {
            after.push(range.start.max(at) - at..range.end - at);
        }
    }
    (before, after)
}

/// The key chips of the action's binding where focus was when the palette
/// opened (that is where it will run), else anywhere.
fn shortcut(action: &dyn Action, focus: Option<&FocusHandle>, window: &Window) -> Option<Div> {
    let binding = focus
        .and_then(|focus| window.highest_precedence_binding_for_action_in(action, focus))
        .or_else(|| window.highest_precedence_binding_for_action(action))?;
    Some(
        h_flex().flex_none().gap_1().children(
            binding
                .keystrokes()
                .iter()
                .map(|key| Kbd::new(key.as_keystroke().clone())),
        ),
    )
}

/// `Category: Name`, the category muted.
pub fn command(
    spec: &CommandSpec,
    ranges: &[Range<usize>],
    focus: Option<&FocusHandle>,
    window: &mut Window,
    cx: &mut App,
) -> AnyElement {
    let label = spec.label.as_ref();
    let split = label.find(": ").map_or(0, |colon| colon + 2);
    let (category_ranges, name_ranges) = split_ranges(ranges, split);
    let keys = shortcut(spec.action.as_ref(), focus, window);
    let checked = spec.checked && keys.is_none();
    h_flex()
        .flex_1()
        .min_w_0()
        .gap_2()
        .child(match spec.icon {
            Some(name) => icon(name, cx).into_any_element(),
            None => div()
                .flex_none()
                .size(fixed::PALETTE_ICON_SIZE)
                .into_any_element(),
        })
        .child(
            h_flex()
                .flex_1()
                .min_w_0()
                .overflow_hidden()
                .whitespace_nowrap()
                .when(split > 0, |this| {
                    this.child(
                        div()
                            .flex_none()
                            .text_color(cx.theme().muted_foreground)
                            .child(highlighted(
                                label[..split].to_string(),
                                &category_ranges,
                                cx,
                            )),
                    )
                })
                .child(div().min_w_0().truncate().child(highlighted(
                    label[split..].to_string(),
                    &name_ranges,
                    cx,
                ))),
        )
        .when_some(keys, |this, keys| this.child(keys))
        .when(checked, |this| {
            this.child(Icon::new(IconName::Check).xsmall().flex_none())
        })
        .into_any_element()
}

/// File name, then its folder (muted), both with matched letters.
pub fn file(
    name: (&str, Vec<Range<usize>>),
    dir: (&str, Vec<Range<usize>>),
    cx: &mut App,
) -> AnyElement {
    h_flex()
        .flex_1()
        .min_w_0()
        .gap_2()
        .whitespace_nowrap()
        .child(icon(IconName::File, cx))
        .child(
            div()
                .flex_none()
                .child(highlighted(name.0.to_string(), &name.1, cx)),
        )
        .when(!dir.0.is_empty(), |this| {
            this.child(
                // Long folders lose their start, keeping the part near the file.
                div()
                    .min_w_0()
                    .overflow_hidden()
                    .whitespace_nowrap()
                    .text_ellipsis_start()
                    .text_sm()
                    .text_color(cx.theme().muted_foreground)
                    .child(highlighted(dir.0.to_string(), &dir.1, cx)),
            )
        })
        .into_any_element()
}

pub fn line(line: u32, column: Option<u32>, cx: &mut App) -> AnyElement {
    let text = match column {
        Some(column) => format!("Go to line {line}, column {column}"),
        None => format!("Go to line {line}"),
    };
    h_flex()
        .flex_1()
        .gap_2()
        .child(icon(IconName::ListOrdered, cx))
        .child(text)
        .into_any_element()
}

/// A `?` row: the prefix in a key chip, then what it does.
pub fn mode(mode: Mode, cx: &mut App) -> AnyElement {
    let prefix: SharedString = match mode.prefix() {
        "" => "…".into(),
        prefix => prefix.into(),
    };
    h_flex()
        .flex_1()
        .gap_2()
        .child(
            div()
                .flex_none()
                .w(fixed::PALETTE_ICON_SIZE * 1.5)
                .text_center()
                .font_family(cx.theme().mono_font_family.clone())
                .text_color(cx.theme().list_active_border)
                .child(prefix),
        )
        .child(mode.title())
        .when(!mode.available(), |this| {
            this.child(
                div()
                    .text_sm()
                    .text_color(cx.theme().muted_foreground)
                    .child("coming soon"),
            )
        })
        .into_any_element()
}

/// A message row (nothing to run).
pub fn info(text: SharedString, cx: &mut App) -> AnyElement {
    h_flex()
        .flex_1()
        .min_w_0()
        .gap_2()
        .text_color(cx.theme().muted_foreground)
        .child(icon(IconName::Info, cx))
        .child(div().min_w_0().truncate().child(text))
        .into_any_element()
}

/// Bottom hint row: the keys, and on the right a note about the mode.
pub fn footer(note: Option<SharedString>, cx: &App) -> AnyElement {
    let hint = |key: &'static str, what: &'static str| {
        h_flex()
            .gap_1()
            .child(
                div()
                    .font_family(cx.theme().mono_font_family.clone())
                    .text_color(cx.theme().foreground)
                    .child(key),
            )
            .child(what)
    };
    h_flex()
        .flex_none()
        .px_3()
        .py_1()
        .gap_3()
        .border_t_1()
        .border_color(cx.theme().border)
        .text_xs()
        .text_color(cx.theme().muted_foreground)
        .child(hint("↑↓", "navigate"))
        .child(hint("↵", "run"))
        .child(hint("esc", "close"))
        .child(div().flex_1())
        .when_some(note, |this, note| this.child(note))
        .into_any_element()
}

/// What the list shows when nothing matches.
pub fn empty(message: SharedString, cx: &App) -> AnyElement {
    div()
        .py_6()
        .w_full()
        .text_center()
        .text_sm()
        .text_color(cx.theme().muted_foreground)
        .child(message)
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::split_ranges;

    #[test]
    fn ranges_split_at_the_category() {
        // "View: Toggle" split at 6; one range straddles it.
        let (before, after) = split_ranges(&[0..1, 4..8, 10..11], 6);
        assert_eq!(before, vec![0..1, 4..6]);
        assert_eq!(after, vec![0..2, 4..5]);
        let (before, after) = split_ranges(&[0..2, 3..4], 0);
        assert!(before.is_empty());
        assert_eq!(after, vec![0..2, 3..4]);
    }
}
