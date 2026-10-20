# Chat panel v1 (demo backend)

Built 2026-10-19. User request: basic chat functionality as a GUI only; no local model loader, API or agent backend is built now. User picked: a pluggable backend interface in the core with a built-in streaming **demo agent** (canned replies, labelled as such); IDE-style layout (agent replies full width, user messages in a box); scope = core loop + session header with status + edit & retry + context chips; chats saved to disk and restored with their tab.

## What others do (2026-10-19)

| Product | Idea Carbon took |
|---|---|
| VS Code Copilot Chat | Full-width turns in a side bar; Add Context; edit a past prompt |
| Zed agent panel | Click a sent message to edit and resend; context as mentions/chips |
| Cursor | Chats as tabs (Carbon: one tab per chat, "Chat 2") |
| JetBrains AI Assistant | Attach files/snippets as removable context; copy button on code blocks |
| Common patterns | Enter sends, Shift+Enter new line; Send becomes Stop; Stop keeps the partial answer; Retry; copy on hover; starter prompts on an empty chat |

Sources: [VS Code chat](https://code.visualstudio.com/docs/copilot/chat/copilot-chat), [VS Code context](https://code.visualstudio.com/docs/copilot/chat/copilot-chat-context), [Zed agent panel](https://git.secluded.site/zed/blob/main/docs/src/ai/agent-panel.md?source=1), [JetBrains chat](https://jetbrains.com/guide/ai/links/7-things-about-chat), [stop/retry states](https://dev.to/babycat/streaming-chat-needs-cancel-and-retry-states-not-just-a-spinner-3d87).

## Behaviour

| Input | Result |
|---|---|
| View > Add Panel > Chat, a chat tab's "..." > New Chat, palette "Add Panel: Chat" | A new chat tab ("Chat", "Chat 2", ...) beside the other chats, else in the right dock. Old "Chat" placeholders and the Agents preset's "Agent Chat 1/2" open as chats |
| Header | Chat title (first line of the first message, or "New chat"), backend name ("Demo agent · canned replies, no model connected"), status label in words: Idle / Responding (with spinner) / Stopped / Failed; Clear button |
| Empty chat | "Ask the demo agent" and four starter prompts; clicking one sends it (with any chips) |
| Typing | Auto-growing box (2 to 10 rows). Enter sends, Shift+Enter makes a new line. Send is disabled while the box is empty |
| While a reply streams | Text appears word by word (Markdown, fade-in), the transcript follows the end; Send becomes **Stop**; Esc or Stop ends the reply, keeping what arrived, marked "Stopped · The reply is incomplete." |
| Agent replies | Full width, Markdown (lists, inline code, links), code blocks highlighted with a copy button; Copy reply and **Retry** under the last reply (always visible), under older ones on hover |
| User messages | A subtle box with its context chips; Copy and **Edit** in the box's corner on hover. Edit puts the text and chips back in the composer ("Editing a sent message…", Cancel / Esc); sending replaces that message and every reply after it |
| Add context (paperclip menu, palette "Chat: Attach Current File / Selection") | A chip for the last focused editor's file, or its selection (lines a-b, up to 20 KB of text); x removes it. Chips are sent with the message and shown on it; the demo agent lists what it received |
| Clear (header button, Ctrl+L, palette) | Asks "Remove all N messages of this chat? This cannot be undone." |
| `/fail` in a message | The demo agent fails halfway (to see the Failed state) |
| Scrolled up | "Jump to latest" button, fade at the bottom |
| Restart | Each chat tab comes back with its conversation (a reply cut off by quitting shows as Stopped) |
| Palette | Chat: Stop Response / Retry Last Reply / Clear Chat / Attach Current File / Attach Selection / Cancel Editing, each listed only when it applies to the focused chat |

## Code

| Part | File | What |
|---|---|---|
| Model | core `chat.rs` | `ChatId` (stable, file-name safe: `[0-9a-z-]{1,64}`), `MessageId`, `ContextItem` (file / selection), `MessageState` (Done, Streaming, Stopped, Failed), `ChatMessage`, `Chat` (`send`, `push_delta`, `finish`, `stop`, `fail`, `retry`, `edit_and_resend`, `clear`, `request_for`, derived `status`, `normalize_after_load`), `title_from`; all serde |
| Backend interface | core `chat/backend.rs` | `ChatBackend` (`name`, `is_demo`, `reply`), `ReplyRequest`, `ReplyEvent` (Delta, Done, Failed), `Reply` (async_channel receiver + `CancelFlag`; dropping it cancels) |
| Demo agent | core `chat/demo.rs` | `DemoBackend`: a thread streams `demo_reply` in 3-word chunks (400 ms, then 35 ms each), checks cancel per chunk, fails on `/fail`; `chunks` |
| Store | core `chat/store.rs` | `ChatStore` on `Core.chats`: chats in memory for the run, `open` (memory, else file, else new), `save` (temp file + rename), `start_reply`; `save_chat` / `load_chat` |
| Panel | gui `chat_panel.rs` | `ChatPanel` (bound to a `ChatId` like a terminal to its id): send/submit, reply task, Stop, Retry, Edit, Clear (dialog), attach, Markdown body cache with remeasure on parse, immediate + 1 s debounced saves, dump `{"label","chat"}`, actions + keys (`Chat` context: Esc, Ctrl+L) |
| Transcript | gui `chat_panel/transcript.rs` | Header + status tag, virtualized `MessageScroller` rows (user box / agent Markdown), footers, hover actions, empty state, chip builder |
| Composer | gui `chat_panel/composer.rs` | Lazy `TextareaState` (needs the window), Enter handling, draft chips, Add context menu, Send / Stop |
| Helpers | gui `chat_panel/context.rs` | `parse_info`, `is_chat_title`, `relative_label`, `should_send` |
| Wiring | `main.rs` (register, chat dir), `workbench.rs` (`add_panel` kinds/titles), `layouts.rs` (preset titles), `test_panel.rs` (placeholder migration), `commands.rs` (palette), `editor_panel.rs` (`selection`, `path`/`name` crate-visible), `style/fixed.rs` (`CHAT_*`), `app_assets.rs` (Pencil, Paperclip, TextCursorInput, MessageSquare, Eraser) | |

## Plugging in a real backend

Implement `carbon_core::chat::ChatBackend` and call `core.chats.set_backend(Arc::new(..))` at startup. `reply(request)` gets the conversation up to the user's message (with context items); start the work off the caller's thread, send `ReplyEvent::Delta(markdown)` chunks, then `Done` or `Failed(reason)`, and stop when `CancelFlag::is_cancelled()` or the channel closes. `is_demo()` false removes the demo wording.

## Design notes

- Chats are a new model next to `AgentSession` (`agent.rs`, kept for tool-running agents with approvals); `Role` is shared.
- No core `notify` per streamed chunk: the app root rebuilds menus on every core notify.
- Markdown is parsed in the background, so a reply's row height is measured again when the parsed text arrives (observer per body).
- The Enter event also fires for Shift+Enter (after it inserted the newline), so only `shift: false` sends; `enter` is never bound in the Chat context.
- Chat files are not deleted when a tab closes: a saved named layout may still show that chat.
- `LAYOUT_VERSION` unchanged; old placeholders migrate through the TestPanel builder.

## Verified

- `cargo test --workspace`: 116 pass, 4 ignored (was 96). New: core `tests/chat.rs` (15: send/refusals, deltas/finish/title, stop/fail, retry, edit truncation, clear, serde + Stopped after load, store save/reload, id uniqueness and path safety, context caps/labels/dedupe, chunks, demo stream = canned text with context count, cancel, `/fail`); gui `context.rs` (4), `transcript.rs` (1). Clippy and fmt clean.
- Click-tested 2026-10-19 (separate test build, window messages; your layout restored and test chat files removed afterwards): add Chat (right dock, narrow) → empty state; moved to the editor area → centred column; send → streaming, Responding, Stop button, Markdown + highlighted code with copy, Retry/Copy; Esc mid-reply → Stopped footer and label; `/fail` → Failed with reason; restart → chat restored (title, messages, Failed); Retry (button and palette); palette lists Chat commands only when they apply; Clear → confirmation → empty; open a file, "Chat: Attach Current File" → chip in draft, on the sent message, demo says "1 context item: `context.rs` (file)"; scroll up → Jump to latest; Edit → highlighted box, banner, text + chip back, resend replaced the turn (caret now at the end, fixed during the test); the hidden user-message actions took a row of space (fixed: overlay in the box corner).
- Not tested: attach Selection (selecting by keyboard needs Shift, which window messages cannot send), Ctrl+L, hover visuals, light themes, macOS/Linux.

## Limits

- Replies are canned; no model, API or ACP agent (`../not_built/README.md`).
- No `@` mentions, no list or search of past chats, no queued follow-ups, no accessible linear view yet (DESIGN.md §5.3, §8).
- Chat files of closed tabs stay in `target/carbon-gui/chats/`.
- Context is the last focused editor's file or selection only (no folders, symbols, diagrams).
- Every drawn reply keeps its parsed Markdown in memory for the life of the tab.
