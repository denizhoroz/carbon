//! The header (title, demo note, status pill), the messages, and the empty
//! state with starter prompts.

use carbon_core::chat::{ChatMessage, ChatStatus, ContextItem, ContextKind, MessageState, Role};
use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::clipboard::Clipboard;
use gpui_kit::component::message_scroller::MessageScroller;
use gpui_kit::component::spinner::Spinner;
use gpui_kit::component::tag::Tag;
use gpui_kit::component::text::TextView;
use gpui_kit::component::tooltip::Tooltip;
use gpui_kit::component::{ActiveTheme, Disableable as _, Icon, Sizable, Size, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use super::ChatPanel;
use crate::style::fixed;

/// Prompts offered on an empty chat (clicking one sends it).
pub const STARTERS: [&str; 4] = [
    "Explain the current file",
    "Write a test for the selected code",
    "Summarize this project",
    "Show a Rust example",
];

fn element_id(what: &str, id: u64) -> ElementId {
    SharedString::from(format!("chat-{what}-{id}")).into()
}

/// The status in words, never colour alone.
pub fn status_tag(status: &ChatStatus) -> Tag {
    let tag = match status {
        ChatStatus::Idle => Tag::secondary(),
        ChatStatus::Responding => Tag::info(),
        ChatStatus::Stopped => Tag::warning(),
        ChatStatus::Failed(_) => Tag::danger(),
    };
    tag.small()
}

impl ChatPanel {
    pub(super) fn render_header(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let status = self.status(cx);
        let backend = self.core.read(cx).chats.backend().clone();
        let subtitle: SharedString = if backend.is_demo() {
            format!("{} · canned replies, no model connected", backend.name()).into()
        } else {
            backend.name().to_string().into()
        };
        let has_messages = self.count(cx) > 0;
        h_flex()
            .flex_none()
            .w_full()
            .gap(fixed::PANEL_GAP)
            .px(fixed::CHAT_PADDING)
            .py_1()
            .border_b_1()
            .border_color(theme.border)
            .child(
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .child(div().truncate().child(self.title(cx)))
                    .child(
                        div()
                            .truncate()
                            .text_xs()
                            .text_color(theme.muted_foreground)
                            .child(subtitle),
                    ),
            )
            .child(
                status_tag(&status)
                    .flex_none()
                    .when(status == ChatStatus::Responding, |tag| {
                        tag.child(Spinner::new().xsmall())
                    })
                    .child(status.label()),
            )
            .child(
                Button::new("chat-clear")
                    .ghost()
                    .small()
                    .icon(IconName::Eraser)
                    .tooltip("Clear chat (Ctrl+L)")
                    .disabled(!has_messages)
                    .on_click(cx.listener(|this, _, window, cx| this.confirm_clear(window, cx))),
            )
    }

    pub(super) fn render_transcript(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let background = cx.theme().background;
        MessageScroller::new(
            "chat-transcript",
            self.scroller.clone(),
            cx.processor(|this, ix: usize, window, cx| this.render_row(ix, window, cx)),
        )
        .with_bottom_fade(background)
        .size_full()
    }

    fn render_row(&mut self, ix: usize, _: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let Some(message) = self.message_at(ix, cx) else {
            return div().into_any_element();
        };
        let is_last = ix + 1 == self.count(cx);
        let row = match message.role {
            Role::User => self.render_user(&message, cx).into_any_element(),
            _ => self.render_agent(&message, is_last, cx).into_any_element(),
        };
        // A centred column, so long replies read well in the editor area.
        // The scroller spaces rows itself.
        div()
            .w_full()
            .child(
                div()
                    .w_full()
                    .max_w(fixed::CHAT_COLUMN_MAX_WIDTH)
                    .mx_auto()
                    .child(row),
            )
            .into_any_element()
    }

    fn render_user(&self, message: &ChatMessage, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let id = message.id;
        let editing = self.editing == Some(id);
        let responding = self.running.is_some();
        let group = element_id("user", id.0);
        // The actions sit in the box's corner on hover, taking no row of their own.
        v_flex()
            .id(group.clone())
            .group(SharedString::from(format!("chat-user-{}", id.0)))
            .relative()
            .w_full()
            .gap_1()
            .p(fixed::CHAT_BUBBLE_PADDING)
            .rounded(theme.radius)
            .bg(theme.secondary)
            .border_1()
            .border_color(if editing { theme.ring } else { theme.border })
            .when(!message.context.is_empty(), |this| {
                this.child(h_flex().w_full().flex_wrap().gap_1().children(
                    message.context.iter().enumerate().map(|(ix, item)| {
                        chip(item, element_id(&format!("chip-{}", id.0), ix as u64), cx)
                    }),
                ))
            })
            .child(div().w_full().child(message.text.clone()))
            .child(
                h_flex()
                    .absolute()
                    .top_1()
                    .right_1()
                    .gap_1()
                    .rounded(theme.radius)
                    .bg(theme.secondary)
                    .opacity(0.)
                    .group_hover(format!("chat-user-{}", id.0), |style| style.opacity(1.))
                    .child(
                        Clipboard::new(element_id("copy", id.0))
                            .value(message.text.clone())
                            .tooltip("Copy message"),
                    )
                    .when(!responding, |this| {
                        this.child(
                            Button::new(element_id("edit", id.0))
                                .ghost()
                                .xsmall()
                                .icon(IconName::Pencil)
                                .tooltip("Edit and resend")
                                .on_click(cx.listener(move |this, _, window, cx| {
                                    this.edit(id, window, cx)
                                })),
                        )
                    }),
            )
    }

    fn render_agent(
        &mut self,
        message: &ChatMessage,
        is_last: bool,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let id = message.id;
        let streaming = message.state == MessageState::Streaming;
        let body = (!message.text.is_empty()).then(|| self.body(message, cx));
        let name = self.core.read(cx).chats.backend().name().to_string();
        let can_retry = is_last && self.running.is_none();
        let theme = cx.theme();
        let group = format!("chat-agent-{}", id.0);
        v_flex()
            .id(element_id("agent", id.0))
            .group(SharedString::from(group.clone()))
            .w_full()
            .gap_1()
            .child(
                h_flex()
                    .gap_1()
                    .text_xs()
                    .text_color(theme.muted_foreground)
                    .child(
                        Icon::new(IconName::Bot)
                            .with_size(Size::Size(fixed::CHAT_ROLE_ICON_SIZE))
                            .text_color(theme.muted_foreground),
                    )
                    .child(name),
            )
            .map(|this| match body {
                Some(body) => this.child(
                    TextView::new(&body)
                        .selectable(true)
                        .stream_fade(streaming)
                        .code_block_actions(|block, _, _| {
                            Clipboard::new(SharedString::from(format!(
                                "chat-code-{}",
                                block.span.map_or(0, |span| span.start)
                            )))
                            .value(block.code())
                            .tooltip("Copy code")
                        }),
                ),
                None if streaming => this.child(
                    h_flex()
                        .gap_1()
                        .text_sm()
                        .text_color(theme.muted_foreground)
                        .child(Spinner::new().xsmall())
                        .child("Thinking…"),
                ),
                None => this,
            })
            .map(|this| match &message.state {
                MessageState::Stopped => this.child(
                    h_flex()
                        .gap_2()
                        .text_xs()
                        .text_color(theme.muted_foreground)
                        .child(Tag::warning().small().child("Stopped"))
                        .child("The reply is incomplete."),
                ),
                MessageState::Failed(reason) => this.child(
                    h_flex()
                        .gap_2()
                        .text_xs()
                        .text_color(theme.muted_foreground)
                        .child(Tag::danger().small().child("Failed"))
                        .child(reason.clone()),
                ),
                _ => this,
            })
            .when(!streaming, |this| {
                this.child(
                    h_flex()
                        .gap_1()
                        // The last reply's actions always show, so keyboard and
                        // touch users reach Retry; older ones on hover.
                        .when(!is_last, |this| {
                            this.opacity(0.)
                                .group_hover(group.clone(), |style| style.opacity(1.))
                        })
                        .child(
                            Clipboard::new(element_id("copy", id.0))
                                .value(message.text.clone())
                                .tooltip("Copy reply"),
                        )
                        .when(can_retry, |this| {
                            this.child(
                                Button::new(element_id("retry", id.0))
                                    .ghost()
                                    .xsmall()
                                    .icon(IconName::RefreshCw)
                                    .tooltip("Retry")
                                    .on_click(cx.listener(|this, _, _, cx| this.retry(cx))),
                            )
                        }),
                )
            })
    }

    pub(super) fn render_empty(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let demo = self.core.read(cx).chats.backend().is_demo();
        v_flex()
            .id("chat-empty")
            .size_full()
            .overflow_y_scroll()
            .items_center()
            .justify_center()
            .gap(fixed::PANEL_GAP)
            .p(fixed::PANEL_PADDING)
            .child(
                Icon::new(IconName::MessageSquare)
                    .with_size(Size::Size(fixed::CHAT_EMPTY_ICON_SIZE))
                    .text_color(theme.muted_foreground),
            )
            .child(if demo {
                "Ask the demo agent"
            } else {
                "Start a chat"
            })
            .when(demo, |this| {
                this.child(
                    div()
                        .text_sm()
                        .text_center()
                        .text_color(theme.muted_foreground)
                        .child("Replies are canned; no model is connected yet."),
                )
            })
            .child(
                v_flex()
                    .w_full()
                    .max_w(fixed::CHAT_STARTERS_MAX_WIDTH)
                    .gap_1()
                    .children(STARTERS.iter().enumerate().map(|(ix, text)| {
                        Button::new(("chat-starter", ix))
                            .outline()
                            .small()
                            .w_full()
                            .label(*text)
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.submit(text, window, cx);
                            }))
                    })),
            )
    }
}

/// One context chip: icon and short label; the full path in its tooltip.
pub(super) fn chip(item: &ContextItem, id: ElementId, cx: &App) -> Stateful<Div> {
    let theme = cx.theme();
    let icon = match item.kind {
        ContextKind::File => IconName::File,
        ContextKind::Selection => IconName::TextCursorInput,
    };
    let tooltip: SharedString = match item.lines {
        Some((first, last)) => format!("{} (lines {first}-{last})", item.label).into(),
        None => item.label.clone().into(),
    };
    h_flex()
        .id(id)
        .max_w(fixed::CHAT_CHIP_MAX_WIDTH)
        .gap_1()
        .px_1()
        .rounded(theme.radius)
        .border_1()
        .border_color(theme.border)
        .bg(theme.muted)
        .text_xs()
        .tooltip(move |window, cx| Tooltip::new(tooltip.clone()).build(window, cx))
        .child(
            Icon::new(icon)
                .xsmall()
                .flex_none()
                .text_color(theme.muted_foreground),
        )
        .child(div().min_w_0().truncate().child(item.chip_label()))
}

#[cfg(test)]
mod tests {
    // Not `super::*`: the gpui glob brings a `test` macro that shadows the built-in one.
    use std::collections::HashSet;

    use super::STARTERS;

    #[test]
    fn starters_are_unique_and_non_empty() {
        let set: HashSet<_> = STARTERS.iter().collect();
        assert_eq!(set.len(), STARTERS.len());
        assert!(STARTERS.iter().all(|text| !text.trim().is_empty()));
    }
}
