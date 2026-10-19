//! Chat panel: a conversation with the chat backend in `carbon_core::chat`
//! (for now the demo agent, which streams canned replies). Transcript with
//! Markdown, a composer with context chips, Stop / Retry / Edit, and the
//! conversation saved to `target/carbon-gui/chats/<id>.json`.

mod composer;
mod context;
mod transcript;

use std::collections::HashMap;

use carbon_core::Core;
use carbon_core::chat::{
    ChatId, ChatMessage, ChatStatus, ContextItem, MessageId, Reply, ReplyEvent, add_context,
};
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::dock::{
    BasePanel, Panel, PanelEvent, PanelInfo, PanelState, panel_handle, register_panel,
};
use gpui_kit::component::input::{RopeExt as _, TextareaState};
use gpui_kit::component::menu::PopupMenu;
use gpui_kit::component::message_scroller::MessageScrollerState;
use gpui_kit::component::text::TextViewState;
use gpui_kit::component::{ActiveTheme, WindowExt, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

pub use context::is_chat_title;

use crate::menus::AddPanel;
use crate::style::fixed;
use crate::workspace_ops::current_core;

/// Persistence key in saved layouts. Never change it.
const PANEL_NAME: &str = "Chat";
const KEY_CONTEXT: &str = "Chat";
/// Where chats are saved (beside `layout.json`).
pub const CHAT_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../target/carbon-gui/chats");

actions!(
    chat,
    [
        StopReply,
        RetryReply,
        ClearChat,
        AttachFile,
        AttachSelection,
        CancelEdit
    ]
);

/// Binds the chat keys and the builder that restores chats from a layout.
pub fn register(cx: &mut App) {
    let context = Some(KEY_CONTEXT);
    cx.bind_keys([
        // Esc stops a reply, else leaves editing (only the one that applies
        // has a handler, so the other key binding is tried next).
        KeyBinding::new("escape", StopReply, context),
        KeyBinding::new("escape", CancelEdit, context),
        KeyBinding::new("secondary-l", ClearChat, context),
    ]);
    register_panel(cx, PANEL_NAME, |build, _, cx| {
        let (label, chat) = match build.info() {
            PanelInfo::Panel(value) => context::parse_info(value),
            _ => ("Chat".into(), None),
        };
        panel_handle(ChatPanel::restore(label, chat, cx))
    });
}

pub struct ChatPanel {
    core: Entity<Core>,
    focus_handle: FocusHandle,
    /// Tab name ("Chat", "Chat 2").
    label: SharedString,
    chat: ChatId,
    scroller: Entity<MessageScrollerState>,
    /// Rendered Markdown of agent replies, made when first drawn.
    bodies: HashMap<MessageId, (Entity<TextViewState>, Subscription)>,
    /// Made on first render (a text area needs the window).
    composer: Option<Composer>,
    /// Context chips for the next message.
    draft: Vec<ContextItem>,
    /// The sent message being edited in the composer.
    editing: Option<MessageId>,
    running: Option<Running>,
    _save_task: Task<()>,
}

struct Composer {
    input: Entity<TextareaState>,
    _events: Subscription,
    _focus: Subscription,
}

/// A reply streaming in. Dropping it cancels the backend.
struct Running {
    id: MessageId,
    _reply: Reply,
    _task: Task<()>,
}

impl ChatPanel {
    /// A new, empty chat.
    pub fn new(label: impl Into<SharedString>, cx: &mut App) -> Entity<Self> {
        Self::restore(label.into(), None, cx)
    }

    /// The chat `chat` (from memory or its file), or a new one.
    pub fn restore(label: SharedString, chat: Option<ChatId>, cx: &mut App) -> Entity<Self> {
        let core = current_core(cx);
        let (chat, count) = core.update(cx, |core, _| {
            let id = chat.unwrap_or_else(|| core.chats.create());
            let count = core.chats.open(&id).messages().len();
            (id, count)
        });
        cx.new(|cx| Self {
            core,
            focus_handle: cx.focus_handle(),
            label,
            chat,
            scroller: cx.new(|cx| MessageScrollerState::new(count, cx)),
            bodies: HashMap::new(),
            composer: None,
            draft: Vec::new(),
            editing: None,
            running: None,
            _save_task: Task::ready(()),
        })
    }

    pub fn label(&self) -> SharedString {
        self.label.clone()
    }

    // ---- reading the chat ------------------------------------------------

    fn messages(&self, cx: &App) -> Vec<ChatMessage> {
        self.core
            .read(cx)
            .chats
            .get(&self.chat)
            .map(|chat| chat.messages().to_vec())
            .unwrap_or_default()
    }

    fn message_at(&self, ix: usize, cx: &App) -> Option<ChatMessage> {
        self.core
            .read(cx)
            .chats
            .get(&self.chat)?
            .messages()
            .get(ix)
            .cloned()
    }

    fn count(&self, cx: &App) -> usize {
        self.core
            .read(cx)
            .chats
            .get(&self.chat)
            .map_or(0, |chat| chat.messages().len())
    }

    fn status(&self, cx: &App) -> ChatStatus {
        self.core
            .read(cx)
            .chats
            .get(&self.chat)
            .map_or(ChatStatus::Idle, |chat| chat.status())
    }

    fn can_retry(&self, cx: &App) -> bool {
        self.core
            .read(cx)
            .chats
            .get(&self.chat)
            .is_some_and(|chat| chat.can_retry())
    }

    fn title(&self, cx: &App) -> SharedString {
        self.core
            .read(cx)
            .chats
            .get(&self.chat)
            .and_then(|chat| chat.title.clone())
            .map_or_else(|| "New chat".into(), Into::into)
    }

    fn update_chat<R>(&self, cx: &mut App, f: impl FnOnce(&mut carbon_core::chat::Chat) -> R) -> R {
        // No core notify: the app root redraws menus on every one.
        self.core
            .update(cx, |core, _| f(core.chats.open(&self.chat)))
    }

    // ---- sending and replies ---------------------------------------------

    /// Sends the composer's text (or resends the message being edited).
    fn send(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(input) = self.composer.as_ref().map(|c| c.input.clone()) else {
            return;
        };
        let text = input.read(cx).value().to_string();
        if self.submit(&text, window, cx) {
            input.update(cx, |input, cx| input.clean(window, cx));
        }
    }

    /// Sends `text` with the draft chips; returns whether it went out.
    fn submit(&mut self, text: &str, window: &mut Window, cx: &mut Context<Self>) -> bool {
        let responding = self.running.is_some();
        if !context::should_send(false, text, responding) {
            return false;
        }
        let draft = self.draft.clone();
        let editing = self.editing;
        let reply = self.update_chat(cx, |chat| match editing {
            Some(user) => chat.edit_and_resend(user, text, draft),
            None => chat.send(text, draft),
        });
        let Some(reply) = reply else {
            return false;
        };
        self.draft.clear();
        self.editing = None;
        let count = self.count(cx);
        if editing.is_some() {
            self.bodies.clear();
            self.scroller.update(cx, |s, cx| s.reset(count, cx));
        } else {
            self.scroller.update(cx, |s, cx| {
                s.append(2, cx);
            });
        }
        self.scroller.update(cx, |s, cx| s.scroll_to_end(cx));
        self.start(reply, cx);
        self.save_now(cx);
        self.focus_composer(window, cx);
        cx.notify();
        true
    }

    /// Asks the backend for reply `id` and follows its events.
    fn start(&mut self, id: MessageId, cx: &mut Context<Self>) {
        let Some(reply) = self.core.read(cx).chats.start_reply(&self.chat, id) else {
            return;
        };
        let events = reply.events();
        let task = cx.spawn(async move |this, cx| {
            while let Ok(event) = events.recv().await {
                let last = !matches!(event, ReplyEvent::Delta(_));
                let gone = this
                    .update(cx, |this, cx| this.on_reply_event(id, event, cx))
                    .is_err();
                if last || gone {
                    break;
                }
            }
        });
        self.running = Some(Running {
            id,
            _reply: reply,
            _task: task,
        });
    }

    fn on_reply_event(&mut self, id: MessageId, event: ReplyEvent, cx: &mut Context<Self>) {
        if self.running.as_ref().is_none_or(|running| running.id != id) {
            return;
        }
        match event {
            ReplyEvent::Delta(text) => {
                self.update_chat(cx, |chat| chat.push_delta(id, &text));
                if let Some((body, _)) = self.bodies.get(&id) {
                    body.update(cx, |body, cx| body.push_str(&text, cx));
                }
                self.remeasure(id, cx);
                self.schedule_save(cx);
            }
            ReplyEvent::Done => {
                self.update_chat(cx, |chat| chat.finish(id));
                self.end_reply(id, cx);
            }
            ReplyEvent::Failed(reason) => {
                self.update_chat(cx, |chat| chat.fail(id, reason));
                self.end_reply(id, cx);
            }
        }
        cx.notify();
    }

    fn end_reply(&mut self, id: MessageId, cx: &mut Context<Self>) {
        self.running = None;
        self.remeasure(id, cx);
        self.save_now(cx);
        cx.notify();
    }

    /// Stop: the reply keeps what arrived and says it is incomplete.
    fn stop(&mut self, cx: &mut Context<Self>) {
        if let Some(running) = self.running.take() {
            self.update_chat(cx, |chat| chat.stop(running.id));
            self.remeasure(running.id, cx);
            drop(running);
            self.save_now(cx);
            cx.notify();
        }
    }

    fn retry(&mut self, cx: &mut Context<Self>) {
        let old = self.messages(cx).last().map(|message| message.id);
        let Some(reply) = self.update_chat(cx, |chat| chat.retry()) else {
            return;
        };
        if let Some(old) = old {
            self.bodies.remove(&old);
        }
        let last = self.count(cx) - 1;
        self.scroller
            .update(cx, |s, cx| s.splice(last..last + 1, 1, cx));
        self.start(reply, cx);
        self.save_now(cx);
        cx.notify();
    }

    /// Puts sent message `id` into the composer; sending replaces it and
    /// everything after it.
    fn edit(&mut self, id: MessageId, window: &mut Window, cx: &mut Context<Self>) {
        let Some(message) = self.messages(cx).into_iter().find(|m| m.id == id) else {
            return;
        };
        self.editing = Some(id);
        self.draft = message.context.clone();
        if let Some(composer) = &self.composer {
            composer.input.update(cx, |input, cx| {
                input.set_value(message.text.clone(), window, cx);
                // Caret at the end, ready to add to the message.
                let end = input.text().offset_to_position(input.text().len());
                input.set_cursor_position(end, window, cx);
            });
        }
        self.focus_composer(window, cx);
        cx.notify();
    }

    fn cancel_edit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.editing.take().is_some() {
            self.draft.clear();
            if let Some(composer) = &self.composer {
                composer
                    .input
                    .update(cx, |input, cx| input.clean(window, cx));
            }
            cx.notify();
        }
    }

    /// Asks first: clearing cannot be undone.
    fn confirm_clear(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let count = self.count(cx);
        if count == 0 {
            return;
        }
        let this = cx.entity().downgrade();
        let clear: crate::workbench::WindowCallback = std::rc::Rc::new(move |window, cx| {
            _ = this.update(cx, |this, cx| this.clear(window, cx));
        });
        window.open_dialog(cx, move |dialog, _, cx| {
            let on_ok = clear.clone();
            let on_click = clear.clone();
            dialog
                .title("Clear Chat")
                .w(fixed::DIALOG_WIDTH)
                .on_ok(move |_, window, cx| {
                    on_ok(window, cx);
                    true
                })
                .child(
                    v_flex()
                        .gap(fixed::PANEL_GAP)
                        .child(format!("Remove all {count} messages of this chat?"))
                        .child(
                            div()
                                .text_sm()
                                .text_color(cx.theme().muted_foreground)
                                .child("This cannot be undone."),
                        ),
                )
                .footer(
                    h_flex()
                        .gap_2()
                        .justify_end()
                        .child(
                            Button::new("chat-clear-cancel")
                                .label("Cancel")
                                .on_click(|_, window, cx| window.close_dialog(cx)),
                        )
                        .child(
                            Button::new("chat-clear-ok")
                                .danger()
                                .label("Clear")
                                .on_click(move |_, window, cx| {
                                    on_click(window, cx);
                                    window.close_dialog(cx);
                                }),
                        ),
                )
        });
    }

    fn clear(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.stop(cx);
        self.update_chat(cx, |chat| chat.clear());
        self.bodies.clear();
        self.editing = None;
        self.scroller.update(cx, |s, cx| s.reset(0, cx));
        self.save_now(cx);
        self.focus_composer(window, cx);
        cx.notify();
    }

    /// Adds the active editor's file or selection as a chip.
    fn attach(&mut self, selection: bool, window: &mut Window, cx: &mut Context<Self>) {
        let item = self.context_from_editor(selection, cx);
        match item {
            Ok(item) => {
                add_context(&mut self.draft, item);
                self.focus_composer(window, cx);
                cx.notify();
            }
            Err(reason) => window.push_notification(reason, cx),
        }
    }

    fn context_from_editor(&self, selection: bool, cx: &App) -> Result<ContextItem, &'static str> {
        let editor =
            crate::workbench::active_editor(cx).ok_or("Open a file in an editor first.")?;
        let editor = editor.read(cx);
        let path = editor.path(cx);
        let root = self
            .core
            .read(cx)
            .workspace()
            .map(|workspace| workspace.root().to_path_buf());
        let label = match &path {
            Some(path) => context::relative_label(path, root.as_deref()),
            None => editor.name(cx).to_string(),
        };
        if !selection {
            return Ok(ContextItem::file(path, label));
        }
        let (text, first, last) = editor
            .selection(cx)
            .ok_or("Select text in an editor first.")?;
        Ok(ContextItem::selection(path, label, (first, last), &text))
    }

    // ---- view helpers ----------------------------------------------------

    /// The Markdown of agent message `message`, made on first use and kept.
    fn body(&mut self, message: &ChatMessage, cx: &mut Context<Self>) -> Entity<TextViewState> {
        if let Some((body, _)) = self.bodies.get(&message.id) {
            return body.clone();
        }
        let body = cx.new(|cx| TextViewState::markdown(&message.text, cx));
        // Markdown is parsed in the background: measure the row again when
        // the parsed text (and so its height) arrives.
        let id = message.id;
        let sub = cx.observe(&body, move |this, _, cx| this.remeasure(id, cx));
        self.bodies.insert(message.id, (body.clone(), sub));
        body
    }

    fn remeasure(&mut self, id: MessageId, cx: &mut Context<Self>) {
        let ix = self
            .core
            .read(cx)
            .chats
            .get(&self.chat)
            .and_then(|chat| chat.index_of(id));
        if let Some(ix) = ix {
            self.scroller.update(cx, |s, cx| {
                s.remeasure_items(ix..ix + 1, cx);
            });
        }
    }

    fn focus_composer(&self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(composer) = &self.composer {
            composer
                .input
                .update(cx, |input, cx| input.focus(window, cx));
        }
    }

    // ---- saving ------------------------------------------------------------

    fn save_now(&mut self, cx: &mut Context<Self>) {
        self._save_task = Task::ready(());
        if let Err(err) = self.core.read(cx).chats.save(&self.chat) {
            eprintln!("cannot save chat {}: {err}", self.chat);
        }
    }

    /// While a reply streams: save at most once per `CHAT_SAVE_DELAY`.
    fn schedule_save(&mut self, cx: &mut Context<Self>) {
        self._save_task = cx.spawn(async move |this, cx| {
            cx.background_executor().timer(fixed::CHAT_SAVE_DELAY).await;
            _ = this.update(cx, |this, cx| this.save_now(cx));
        });
    }
}

impl EventEmitter<PanelEvent> for ChatPanel {}

impl Focusable for ChatPanel {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl BasePanel for ChatPanel {
    fn panel_name(&self) -> &'static str {
        PANEL_NAME
    }

    /// Closing the tab stops a running reply; the chat file stays (a saved
    /// layout may still show it).
    fn on_removed(&mut self, _: &mut Window, cx: &mut Context<Self>) {
        self.stop(cx);
        self.save_now(cx);
    }

    fn dump(&self, _: &App) -> PanelState {
        let mut state = PanelState::new(PANEL_NAME);
        state.info = PanelInfo::panel(serde_json::json!({
            "label": self.label.as_ref(),
            "chat": self.chat.as_str(),
        }));
        state
    }
}

impl Panel for ChatPanel {
    fn tab_name(&self, _: &App) -> Option<SharedString> {
        Some(self.label.clone())
    }

    fn title(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        self.label.clone()
    }

    fn always_tab_bar(&self, _: &App) -> bool {
        true
    }

    fn inner_padding(&self, _: &App) -> bool {
        false
    }

    fn request_close(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        crate::workbench::close_panel(cx.entity(), window, cx);
        true
    }

    fn dropdown_menu(
        &mut self,
        menu: PopupMenu,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> PopupMenu {
        menu.menu(
            "New Chat",
            Box::new(AddPanel {
                name: "Chat".into(),
            }),
        )
        .menu("Clear Chat", Box::new(ClearChat))
        .action_context(self.focus_handle.clone())
    }
}

impl Render for ChatPanel {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.ensure_composer(window, cx);
        let responding = self.running.is_some();
        let editing = self.editing.is_some();
        let has_messages = self.count(cx) > 0;
        let can_retry = self.can_retry(cx);

        v_flex()
            .track_focus(&self.focus_handle)
            .key_context(KEY_CONTEXT)
            .size_full()
            .bg(cx.theme().background)
            .on_action(
                cx.listener(|this, _: &AttachFile, window, cx| this.attach(false, window, cx)),
            )
            .on_action(
                cx.listener(|this, _: &AttachSelection, window, cx| this.attach(true, window, cx)),
            )
            // Only what applies has a handler, so the palette lists only that
            // and Esc falls through to the next binding.
            .when(responding, |this| {
                this.on_action(cx.listener(|this, _: &StopReply, _, cx| this.stop(cx)))
            })
            .when(editing && !responding, |this| {
                this.on_action(
                    cx.listener(|this, _: &CancelEdit, window, cx| this.cancel_edit(window, cx)),
                )
            })
            .when(can_retry, |this| {
                this.on_action(cx.listener(|this, _: &RetryReply, _, cx| this.retry(cx)))
            })
            .when(has_messages, |this| {
                this.on_action(
                    cx.listener(|this, _: &ClearChat, window, cx| this.confirm_clear(window, cx)),
                )
            })
            .child(self.render_header(cx))
            .child(div().flex_1().min_h_0().map(|this| match has_messages {
                true => this.child(self.render_transcript(cx)),
                false => this.child(self.render_empty(cx)),
            }))
            .child(self.render_composer(window, cx))
    }
}
