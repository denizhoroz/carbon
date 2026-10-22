//! One setting in the Settings dialog's list: label and description (search
//! matches picked out), notes (invalid value, overridden in the other scope,
//! user only, restart), and on the right its control, a reset button when
//! the scope sets it, and a menu (copy ID, copy as JSON, reset).

use std::ops::Range;

use carbon_core::settings::search::Hit;
use carbon_core::settings::{Kind, Scope, Setting};
use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::input::{Input, InputEvent, InputState, NumberInput};
use gpui_kit::component::menu::{DropdownMenu, PopupMenuItem};
use gpui_kit::component::switch::Switch;
use gpui_kit::component::{ActiveTheme, Disableable, Sizable, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use serde_json::{Value, json};

use super::view::SettingsView;
use crate::style::fixed;

/// `text` with `ranges` (byte ranges) in the highlight colour, semibold, as
/// in the palette.
fn highlighted(text: String, ranges: &[Range<usize>], cx: &App) -> StyledText {
    let style = HighlightStyle {
        color: Some(cx.theme().list_active_border),
        font_weight: Some(FontWeight::SEMIBOLD),
        ..Default::default()
    };
    StyledText::new(text).with_highlights(ranges.iter().map(|range| (range.clone(), style)))
}

/// A number as typed: no `.0` on whole numbers.
fn number_text(value: &Value) -> String {
    match value.as_f64() {
        Some(n) if n.fract() == 0. => format!("{}", n as i64),
        Some(n) => n.to_string(),
        None => String::new(),
    }
}

/// What the user picked, as the JSON value it is stored as.
fn number_value(n: f64) -> Value {
    if n.fract() == 0. {
        json!(n as i64)
    } else {
        json!(n)
    }
}

/// The value a box's text stands for, if it is one. `clamp`: bring an
/// out-of-range number inside (on Enter or leaving the box); while typing,
/// out-of-range text is left alone.
fn parse_box(kind: &Kind, text: &str, clamp: bool) -> Option<Value> {
    let text = text.trim();
    match kind {
        Kind::Text => Some(json!(text)),
        Kind::Integer { min, max } => {
            let n = text.parse::<f64>().ok()?.round() as i64;
            let fitted = n.clamp(*min, *max);
            (clamp || fitted == n).then_some(json!(fitted))
        }
        Kind::Number { min, max, .. } => {
            let n = text.parse::<f64>().ok()?;
            let fitted = n.clamp(*min, *max);
            (clamp || fitted == n).then(|| number_value(fitted))
        }
        Kind::Bool | Kind::Choice(_) => None,
    }
}

/// Writes `value` unless it is what the scope already shows by inheritance
/// (a box re-synced from elsewhere must not copy the inherited value in).
fn commit(scope: Scope, key: &str, value: Value, window: &mut Window, cx: &mut App) {
    let store = super::get(cx);
    if store.value_in(scope, key).is_none() && store.get_in(scope, key) == value {
        return;
    }
    super::set(scope, key, value, window, cx);
}

impl SettingsView {
    /// The text box (or number box) of `key` in the current scope, made once
    /// and kept; while it is not being typed in it follows `text`.
    fn input_for(
        &mut self,
        key: &str,
        kind: &Kind,
        text: String,
        placeholder: &'static str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Entity<InputState> {
        let id = (self.scope, key.to_string());
        if let Some(input) = self.inputs.get(&id) {
            let input = input.clone();
            let editing = input.focus_handle(cx).is_focused(window);
            if !editing && input.read(cx).value() != text.as_str() {
                input.update(cx, |input, cx| input.set_value(text, window, cx));
            }
            return input;
        }
        let input = cx.new(|cx| {
            let state = InputState::new(window, cx)
                .default_value(text)
                .placeholder(placeholder);
            match kind {
                Kind::Integer { min, max } => state.step(1.).min(*min as f64).max(*max as f64),
                Kind::Number { min, max, step } => state.step(*step).min(*min).max(*max),
                _ => state,
            }
        });
        let (scope, key, kind) = (self.scope, key.to_string(), kind.clone());
        let sub = cx.subscribe_in(
            &input,
            window,
            move |_, input, event: &InputEvent, window, cx| {
                // Numbers apply as they are typed (when in range); text when
                // Enter is pressed or the box is left (a font name half typed
                // is not a font).
                let clamp = match event {
                    InputEvent::Change if !matches!(kind, Kind::Text) => false,
                    InputEvent::PressEnter { .. } | InputEvent::Blur => true,
                    _ => return,
                };
                let text = input.read(cx).value();
                if let Some(value) = parse_box(&kind, &text, clamp) {
                    commit(scope, &key, value, window, cx);
                }
            },
        );
        self.inputs.insert(id, input.clone());
        self.input_subs.push(sub);
        input
    }

    fn render_control(
        &mut self,
        ix: usize,
        setting: &Setting,
        value: &Value,
        disabled: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let scope = self.scope;
        let (key, kind) = (setting.key.clone(), &setting.kind);
        match kind {
            Kind::Bool => Switch::new(("setting-switch", ix))
                .checked(value.as_bool().unwrap_or_default())
                .disabled(disabled)
                .on_click(move |checked, window, cx| {
                    commit(scope, &key, json!(*checked), window, cx)
                })
                .into_any_element(),
            Kind::Choice(choices) => {
                let current = value.as_str().unwrap_or_default().to_string();
                let label = choices
                    .iter()
                    .find(|choice| choice.value == current)
                    .map_or(current.clone(), |choice| choice.label.clone());
                let choices = choices.clone();
                Button::new(("setting-choice", ix))
                    .label(label)
                    .dropdown_caret(true)
                    .outline()
                    .small()
                    .min_w(fixed::SETTINGS_CHOICE_MIN_WIDTH)
                    .disabled(disabled)
                    .dropdown_menu_with_anchor(Anchor::TopRight, move |menu, _, _| {
                        let menu = choices.iter().fold(menu, |menu, choice| {
                            let (key, value) = (key.clone(), choice.value.clone());
                            menu.item(
                                PopupMenuItem::new(choice.label.clone())
                                    .checked(choice.value == current)
                                    .on_click(move |_, window, cx| {
                                        commit(scope, &key, json!(value), window, cx)
                                    }),
                            )
                        });
                        menu.scrollable(true)
                    })
                    .into_any_element()
            }
            Kind::Integer { .. } | Kind::Number { .. } => {
                let input = self.input_for(&key, kind, number_text(value), "", window, cx);
                NumberInput::new(&input)
                    .small()
                    .w(fixed::SETTINGS_NUMBER_WIDTH)
                    .disabled(disabled)
                    .into_any_element()
            }
            Kind::Text => {
                let text = value.as_str().unwrap_or_default().to_string();
                // An empty default means "Carbon picks" (the default font).
                let placeholder = if setting.default == json!("") {
                    "Default"
                } else {
                    ""
                };
                let input = self.input_for(&key, kind, text, placeholder, window, cx);
                Input::new(&input)
                    .small()
                    .w(fixed::SETTINGS_TEXT_WIDTH)
                    .disabled(disabled)
                    .into_any_element()
            }
        }
    }

    pub(super) fn render_row(
        &mut self,
        ix: usize,
        hit: &Hit,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let scope = self.scope;
        let store = super::get(cx);
        let setting = store.definitions()[ix].clone();
        let key = setting.key.clone();
        let value = store.get_in(scope, &key);
        let modified = store.value_in(scope, &key).is_some();
        let invalid = store.is_invalid_in(scope, &key);
        let in_other = store.also_set_in_other(scope, &key);
        let user_only = scope == Scope::Workspace && setting.user_only;
        let theme = cx.theme().clone();

        let mut notes: Vec<(SharedString, Hsla)> = Vec::new();
        if user_only {
            notes.push((
                "Can only be set in User settings.".into(),
                theme.muted_foreground,
            ));
        }
        if invalid && !user_only {
            notes.push((
                format!(
                    "The value in {} settings is not valid here; the one shown is used instead.",
                    scope.label()
                )
                .into(),
                theme.danger,
            ));
        }
        if in_other && !setting.user_only {
            let note = match scope {
                Scope::User => "Overridden by Workspace settings.",
                Scope::Workspace => "Also set in User settings.",
            };
            notes.push((note.into(), theme.muted_foreground));
        }
        if setting.restart {
            notes.push(("Applies after a restart.".into(), theme.muted_foreground));
        }

        let control = self.render_control(ix, &setting, &value, user_only, window, cx);
        let reset = modified.then(|| {
            let key = key.clone();
            Button::new(("setting-reset", ix))
                .icon(IconName::Undo2)
                .ghost()
                .xsmall()
                .tooltip(format!("Reset (remove from {} settings)", scope.label()))
                .on_click(move |_, window, cx| super::reset(scope, &key, window, cx))
        });
        let more = {
            let key = key.clone();
            let value = value.clone();
            Button::new(("setting-more", ix))
                .icon(IconName::Ellipsis)
                .ghost()
                .xsmall()
                .tooltip("More")
                .dropdown_menu_with_anchor(Anchor::TopRight, move |menu, _, _| {
                    let (id, json_text) = (key.clone(), format!("\"{key}\": {value}"));
                    let reset_key = key.clone();
                    menu.item(
                        PopupMenuItem::new("Copy Setting ID").on_click(move |_, _, cx| {
                            cx.write_to_clipboard(ClipboardItem::new_string(id.clone()))
                        }),
                    )
                    .item(
                        PopupMenuItem::new("Copy Setting as JSON").on_click(move |_, _, cx| {
                            cx.write_to_clipboard(ClipboardItem::new_string(json_text.clone()))
                        }),
                    )
                    .when(modified, |menu| {
                        menu.separator()
                            .item(PopupMenuItem::new("Reset Setting").on_click(
                                move |_, window, cx| super::reset(scope, &reset_key, window, cx),
                            ))
                    })
                })
        };

        h_flex()
            .id(("setting-row", ix))
            .w_full()
            .flex_wrap()
            .items_center()
            .gap(fixed::SETTINGS_ROW_GAP)
            .px(fixed::SETTINGS_ROW_PADDING_X)
            .py(fixed::SETTINGS_ROW_PADDING_Y)
            // A bar on the left marks a value this scope sets (VS Code's).
            .border_l_2()
            .border_color(if modified {
                theme.primary
            } else {
                theme.transparent
            })
            .child(
                v_flex()
                    .flex_1()
                    .min_w(fixed::SETTINGS_TEXT_WIDTH)
                    .gap_1()
                    .child(div().font_weight(FontWeight::MEDIUM).child(highlighted(
                        setting.label.clone(),
                        &hit.label,
                        cx,
                    )))
                    .when(!setting.description.is_empty(), |this| {
                        this.child(div().text_sm().text_color(theme.muted_foreground).child(
                            highlighted(setting.description.clone(), &hit.description, cx),
                        ))
                    })
                    .children(
                        notes
                            .into_iter()
                            .map(|(note, color)| div().text_xs().text_color(color).child(note)),
                    ),
            )
            .child(
                h_flex()
                    .flex_none()
                    .gap_1()
                    .child(control)
                    .child(
                        // Same width with and without the reset button, so
                        // controls line up.
                        div().flex_none().w(px(24.)).children(reset),
                    )
                    .child(more),
            )
            .into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use carbon_core::settings::Kind;
    use serde_json::json;

    use super::{number_text, parse_box};

    #[test]
    fn boxes_parse_and_clamp() {
        let size = Kind::number(8., 40., 1.);
        assert_eq!(parse_box(&size, "14", false), Some(json!(14)));
        assert_eq!(parse_box(&size, " 13.5 ", false), Some(json!(13.5)));
        // Typing "1" on the way to "14" changes nothing; leaving the box at
        // "1" brings it to the minimum.
        assert_eq!(parse_box(&size, "1", false), None);
        assert_eq!(parse_box(&size, "1", true), Some(json!(8)));
        assert_eq!(parse_box(&size, "abc", true), None);
        let tabs = Kind::integer(1, 16);
        assert_eq!(parse_box(&tabs, "4.4", false), Some(json!(4)));
        assert_eq!(
            parse_box(&Kind::Text, " Fira Code ", true),
            Some(json!("Fira Code"))
        );
        assert_eq!(number_text(&json!(16)), "16");
        assert_eq!(number_text(&json!(13.5)), "13.5");
    }
}
