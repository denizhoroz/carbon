//! The message box: draft chips, an auto-growing text area (Enter sends,
//! Shift+Enter makes a new line), Add context, and Send / Stop.

use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::input::{InputEvent, Textarea, TextareaState};
use gpui_kit::component::menu::DropdownMenu as _;
use gpui_kit::component::{ActiveTheme, Disableable as _, Sizable, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use super::transcript::chip;
use super::{AttachFile, AttachSelection, ChatPanel, Composer};
use crate::style::fixed;

impl ChatPanel {
    /// Makes the text area on first render (it needs the window).
    pub(super) fn ensure_composer(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.composer.is_some() {
            return;
        }
        let demo = self.core.read(cx).chats.backend().is_demo();
        let placeholder = if demo {
            "Message the demo agent"
        } else {
            "Message the agent"
        };
        let input = cx.new(|cx| {
            TextareaState::new(window, cx)
                .auto_grow(fixed::CHAT_COMPOSER_MIN_ROWS, fixed::CHAT_COMPOSER_MAX_ROWS)
                .placeholder(placeholder)
                .submit_on_enter(true)
        });
        let events = cx.subscribe_in(&input, window, |this, _, event: &InputEvent, window, cx| {
            match event {
                // Also sent for Shift+Enter, which has already made a new line.
                InputEvent::PressEnter { shift: false, .. } => this.send(window, cx),
                InputEvent::Change => cx.notify(),
                _ => {}
            }
        });
        // The tab and the palette focus the panel: hand it to the text area.
        let focus = cx.on_focus(&self.focus_handle, window, |this, window, cx| {
            this.focus_composer(window, cx)
        });
        self.composer = Some(Composer {
            input,
            _events: events,
            _focus: focus,
        });
    }

    pub(super) fn render_composer(
        &self,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let theme = cx.theme();
        let responding = self.running.is_some();
        let empty = self
            .composer
            .as_ref()
            .is_none_or(|composer| composer.input.read(cx).value().trim().is_empty());
        let focus = self.focus_handle.clone();

        let send_or_stop = if responding {
            Button::new("chat-stop")
                .small()
                .icon(IconName::Square)
                .label("Stop")
                .tooltip("Stop the reply (Esc)")
                .on_click(cx.listener(|this, _, _, cx| this.stop(cx)))
        } else {
            Button::new("chat-send")
                .primary()
                .small()
                .icon(IconName::ArrowUp)
                .tooltip("Send (Enter) · new line: Shift+Enter")
                .disabled(empty)
                .on_click(cx.listener(|this, _, window, cx| this.send(window, cx)))
        };

        v_flex()
            .flex_none()
            .w_full()
            .px(fixed::CHAT_PADDING)
            .pb(fixed::CHAT_PADDING)
            .pt_1()
            .child(
                v_flex()
                    .w_full()
                    .max_w(fixed::CHAT_COLUMN_MAX_WIDTH)
                    .mx_auto()
                    .gap_1()
                    .when(self.editing.is_some(), |this| {
                        this.child(
                            h_flex()
                                .gap_2()
                                .text_xs()
                                .text_color(theme.muted_foreground)
                                .child("Editing a sent message: sending replaces it and the replies after it.")
                                .child(
                                    Button::new("chat-cancel-edit")
                                        .ghost()
                                        .xsmall()
                                        .label("Cancel")
                                        .tooltip("Cancel editing (Esc)")
                                        .on_click(cx.listener(|this, _, window, cx| {
                                            this.cancel_edit(window, cx)
                                        })),
                                ),
                        )
                    })
                    .child(
                        v_flex()
                            .w_full()
                            .gap_1()
                            .p_1()
                            .rounded(theme.radius_lg)
                            .border_1()
                            .border_color(theme.input)
                            .bg(theme.background)
                            .when(!self.draft.is_empty(), |this| {
                                this.child(h_flex().w_full().flex_wrap().gap_1().px_1().pt_1().children(
                                    self.draft.iter().enumerate().map(|(ix, item)| {
                                        chip(item, ("chat-draft-chip", ix).into(), cx).child(
                                            Button::new(("chat-draft-remove", ix))
                                                .ghost()
                                                .xsmall()
                                                .icon(IconName::Close)
                                                .tooltip("Remove")
                                                .on_click(cx.listener(move |this, _, _, cx| {
                                                    if ix < this.draft.len() {
                                                        this.draft.remove(ix);
                                                        cx.notify();
                                                    }
                                                })),
                                        )
                                    }),
                                ))
                            })
                            .when_some(self.composer.as_ref(), |this, composer| {
                                this.child(
                                    Textarea::new(&composer.input)
                                        .appearance(false)
                                        .aria_label("Message"),
                                )
                            })
                            .child(
                                h_flex()
                                    .w_full()
                                    .gap_1()
                                    .child(
                                        Button::new("chat-attach")
                                            .ghost()
                                            .small()
                                            .icon(IconName::Paperclip)
                                            .label("Add context")
                                            .tooltip("Attach the current file or selection")
                                            .dropdown_menu(move |menu, _, _| {
                                                menu.menu("Current File", Box::new(AttachFile))
                                                    .menu("Selection", Box::new(AttachSelection))
                                                    .action_context(focus.clone())
                                            }),
                                    )
                                    .child(div().flex_1())
                                    .child(send_or_stop),
                            ),
                    ),
            )
    }
}
