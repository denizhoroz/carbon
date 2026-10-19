//! A stand-in backend: streams a canned Markdown answer, so the chat panel
//! works (streaming, Stop, code blocks, context) before any model exists.

use std::time::Duration;

use super::{CancelFlag, ChatBackend, ContextKind, Reply, ReplyEvent, ReplyRequest, Role};

/// Words per streamed chunk.
const WORDS_PER_CHUNK: usize = 3;
/// A prompt containing this makes the demo fail halfway (to see the UI).
pub const FAIL_TRIGGER: &str = "/fail";

#[derive(Clone, Debug)]
pub struct DemoBackend {
    /// Pause before the first chunk ("thinking").
    pub first_delay: Duration,
    pub chunk_delay: Duration,
}

impl Default for DemoBackend {
    fn default() -> Self {
        Self {
            first_delay: Duration::from_millis(400),
            chunk_delay: Duration::from_millis(35),
        }
    }
}

impl DemoBackend {
    pub fn with_timing(first_delay: Duration, chunk_delay: Duration) -> Self {
        Self {
            first_delay,
            chunk_delay,
        }
    }
}

impl ChatBackend for DemoBackend {
    fn name(&self) -> &str {
        "Demo agent"
    }

    fn is_demo(&self) -> bool {
        true
    }

    fn reply(&self, request: ReplyRequest) -> Reply {
        let (sender, events) = async_channel::unbounded();
        let cancel = CancelFlag::default();
        let flag = cancel.clone();
        let (first_delay, chunk_delay) = (self.first_delay, self.chunk_delay);
        let fail = last_prompt(&request).is_some_and(|text| text.contains(FAIL_TRIGGER));
        let parts = chunks(&demo_reply(&request), WORDS_PER_CHUNK);
        std::thread::Builder::new()
            .name("carbon-chat-demo".into())
            .spawn(move || {
                std::thread::sleep(first_delay);
                for (ix, part) in parts.iter().enumerate() {
                    if flag.is_cancelled() {
                        return;
                    }
                    if fail && ix == parts.len() / 2 {
                        let _ = sender.send_blocking(ReplyEvent::Failed(
                            "Demo failure (asked for with /fail).".into(),
                        ));
                        return;
                    }
                    if sender
                        .send_blocking(ReplyEvent::Delta(part.clone()))
                        .is_err()
                    {
                        return;
                    }
                    std::thread::sleep(chunk_delay);
                }
                if !flag.is_cancelled() {
                    let _ = sender.send_blocking(ReplyEvent::Done);
                }
            })
            .expect("failed to start the demo chat thread");
        Reply::new(events, cancel)
    }
}

fn last_prompt(request: &ReplyRequest) -> Option<&str> {
    request
        .messages
        .iter()
        .rev()
        .find(|message| message.role == Role::User)
        .map(|message| message.text.as_str())
}

/// The canned answer for `request`.
pub fn demo_reply(request: &ReplyRequest) -> String {
    let prompt = last_prompt(request).unwrap_or("");
    let first_line = prompt.lines().next().unwrap_or("").trim();
    let context: Vec<_> = request
        .messages
        .iter()
        .rev()
        .find(|message| message.role == Role::User)
        .map(|message| message.context.iter().collect())
        .unwrap_or_default();

    let mut out = String::from("*Demo agent: canned reply, no model is connected.*\n\n");
    out.push_str(&format!("You asked: **{first_line}**\n\n"));
    if context.is_empty() {
        out.push_str(
            "No context was attached. Use **Add context** to send a file or a selection.\n\n",
        );
    } else {
        let names: Vec<String> = context
            .iter()
            .map(|item| {
                let what = match item.kind {
                    ContextKind::File => "file",
                    ContextKind::Selection => "selection",
                };
                format!("`{}` ({what})", item.chip_label())
            })
            .collect();
        out.push_str(&format!(
            "I received {}: {}.\n\n",
            count(context.len(), "context item"),
            names.join(", ")
        ));
    }
    out.push_str(
        "A real agent would answer here. Until one is connected, this reply shows how \
answers look:\n\n\
- **Markdown** with lists, `inline code` and links\n\
- code blocks with highlighting and a copy button\n\
- streaming text you can stop at any time\n\n\
```rust\n\
fn greet(name: &str) -> String {\n    format!(\"Hello, {name}!\")\n}\n\
```\n\n\
Try **Retry**, edit your message, or type `/fail` to see a failed reply.",
    );
    out
}

fn count(n: usize, what: &str) -> String {
    if n == 1 {
        format!("1 {what}")
    } else {
        format!("{n} {what}s")
    }
}

/// Splits `text` into chunks of `words` words, keeping every character
/// (whitespace included), so the chunks join back to `text`.
pub fn chunks(text: &str, words: usize) -> Vec<String> {
    let words = words.max(1);
    let mut out = Vec::new();
    let mut current = String::new();
    let mut seen = 0;
    let mut in_word = false;
    for ch in text.chars() {
        let space = ch.is_whitespace();
        if !space && !in_word {
            if seen == words {
                out.push(std::mem::take(&mut current));
                seen = 0;
            }
            seen += 1;
        }
        in_word = !space;
        current.push(ch);
    }
    if !current.is_empty() {
        out.push(current);
    }
    out
}
