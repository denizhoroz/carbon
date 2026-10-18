//! Shell integration sequences (OSC 133 and VS Code's OSC 633), taken out of
//! the PTY byte stream before the emulator sees it: the emulator drops OSCs it
//! does not know, and these mark where prompts and commands start and end.
//!
//! Everything else passes through unchanged, also when a sequence is split
//! across reads.

/// One piece of a PTY chunk, in order.
#[derive(Debug, PartialEq, Eq)]
pub enum Piece {
    /// Bytes for the emulator.
    Bytes(Vec<u8>),
    /// A shell integration sequence.
    Mark(Mark),
}

/// What a shell integration sequence says.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Mark {
    /// A: the prompt starts.
    PromptStart,
    /// B: the prompt ends; the user types the command after this.
    CommandStart,
    /// C: the command runs; its output follows.
    CommandExecuted,
    /// D: the command finished, with its exit code when the shell knows it.
    CommandFinished(Option<i32>),
    /// 633 E: the command line as typed.
    CommandLine(String),
    /// 633 P;Cwd=: the shell's working directory.
    Cwd(String),
}

const ESC: u8 = 0x1b;
const BEL: u8 = 0x07;
/// Longest payload kept; longer ones are dropped (a broken stream, not a shell).
const MAX_PAYLOAD: usize = 64 * 1024;

#[derive(Debug, Default)]
enum State {
    #[default]
    Normal,
    /// Saw ESC.
    Esc,
    /// Saw ESC ] and these bytes of the OSC number (deciding whether it is ours).
    Prefix(Vec<u8>),
    /// Inside a 133/633 sequence: its number and payload so far.
    Payload { number: u16, payload: Vec<u8> },
    /// Inside a payload, saw ESC (ESC \ ends it).
    PayloadEsc { number: u16, payload: Vec<u8> },
}

/// Splits PTY output into emulator bytes and shell integration marks.
#[derive(Debug, Default)]
pub struct Sniffer {
    state: State,
}

impl Sniffer {
    pub fn feed(&mut self, chunk: &[u8]) -> Vec<Piece> {
        let mut pieces = Vec::new();
        let mut bytes = Vec::with_capacity(chunk.len());
        for &byte in chunk {
            self.state = match std::mem::take(&mut self.state) {
                State::Normal if byte == ESC => State::Esc,
                State::Normal => {
                    bytes.push(byte);
                    State::Normal
                }
                State::Esc if byte == b']' => State::Prefix(Vec::new()),
                State::Esc => {
                    bytes.extend([ESC, byte]);
                    // ESC ESC: the second one may start a sequence.
                    if byte == ESC {
                        bytes.pop();
                        State::Esc
                    } else {
                        State::Normal
                    }
                }
                State::Prefix(mut prefix) => {
                    prefix.push(byte);
                    match prefix.as_slice() {
                        b"133;" => State::Payload {
                            number: 133,
                            payload: Vec::new(),
                        },
                        b"633;" => State::Payload {
                            number: 633,
                            payload: Vec::new(),
                        },
                        // Still a possible prefix of ours.
                        p if b"133;".starts_with(p) || b"633;".starts_with(p) => {
                            State::Prefix(prefix)
                        }
                        // Another OSC: hand it to the emulator as it came.
                        _ => {
                            bytes.extend([ESC, b']']);
                            bytes.extend(prefix);
                            State::Normal
                        }
                    }
                }
                State::Payload {
                    number,
                    mut payload,
                } => match byte {
                    BEL => {
                        flush(&mut bytes, &mut pieces);
                        pieces.extend(parse(number, &payload).map(Piece::Mark));
                        State::Normal
                    }
                    ESC => State::PayloadEsc { number, payload },
                    _ if payload.len() >= MAX_PAYLOAD => State::Normal,
                    _ => {
                        payload.push(byte);
                        State::Payload { number, payload }
                    }
                },
                State::PayloadEsc { number, payload } => {
                    // ESC \ (string terminator) ends it; any other ESC also
                    // ends a broken sequence.
                    flush(&mut bytes, &mut pieces);
                    pieces.extend(parse(number, &payload).map(Piece::Mark));
                    if byte == b'\\' {
                        State::Normal
                    } else if byte == ESC {
                        State::Esc
                    } else {
                        bytes.extend([ESC, byte]);
                        State::Normal
                    }
                }
            };
        }
        flush(&mut bytes, &mut pieces);
        pieces
    }
}

fn flush(bytes: &mut Vec<u8>, pieces: &mut Vec<Piece>) {
    if !bytes.is_empty() {
        pieces.push(Piece::Bytes(std::mem::take(bytes)));
    }
}

fn parse(number: u16, payload: &[u8]) -> Option<Mark> {
    let text = String::from_utf8_lossy(payload);
    let mut parts = text.splitn(2, ';');
    let kind = parts.next()?;
    let rest = parts.next();
    Some(match (kind, number) {
        ("A", _) => Mark::PromptStart,
        ("B", _) => Mark::CommandStart,
        ("C", _) => Mark::CommandExecuted,
        ("D", _) => {
            Mark::CommandFinished(rest.and_then(|code| code.split(';').next()?.trim().parse().ok()))
        }
        ("E", 633) => Mark::CommandLine(unescape(rest?.split(';').next()?)),
        ("P", 633) => {
            let property = rest?;
            Mark::Cwd(unescape(property.strip_prefix("Cwd=")?))
        }
        _ => return None,
    })
}

/// 633 values escape `\` as `\\` and other bytes as `\xAB`.
fn unescape(value: &str) -> String {
    let mut out = Vec::with_capacity(value.len());
    let bytes = value.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'\\' {
            if bytes.get(i + 1) == Some(&b'\\') {
                out.push(b'\\');
                i += 2;
                continue;
            }
            if bytes.get(i + 1) == Some(&b'x')
                && let Some(hex) = value.get(i + 2..i + 4)
                && let Ok(byte) = u8::from_str_radix(hex, 16)
            {
                out.push(byte);
                i += 4;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

#[cfg(test)]
mod tests {
    use super::{Mark, Piece, Sniffer};

    fn feed_all(chunks: &[&[u8]]) -> Vec<Piece> {
        let mut sniffer = Sniffer::default();
        let mut out: Vec<Piece> = Vec::new();
        for chunk in chunks {
            for piece in sniffer.feed(chunk) {
                // Merge byte runs split by chunk borders, for easy comparison.
                match (out.last_mut(), piece) {
                    (Some(Piece::Bytes(prev)), Piece::Bytes(next)) => prev.extend(next),
                    (_, piece) => out.push(piece),
                }
            }
        }
        out
    }

    #[test]
    fn marks_come_out_and_other_bytes_pass() {
        let pieces =
            feed_all(&[b"a\x1b]633;A\x07$ \x1b]633;B\x07ls\x1b]633;C\x07out\x1b]633;D;2\x07"]);
        assert_eq!(
            pieces,
            [
                Piece::Bytes(b"a".to_vec()),
                Piece::Mark(Mark::PromptStart),
                Piece::Bytes(b"$ ".to_vec()),
                Piece::Mark(Mark::CommandStart),
                Piece::Bytes(b"ls".to_vec()),
                Piece::Mark(Mark::CommandExecuted),
                Piece::Bytes(b"out".to_vec()),
                Piece::Mark(Mark::CommandFinished(Some(2))),
            ]
        );
    }

    #[test]
    fn sequences_split_across_reads() {
        let pieces = feed_all(&[
            b"x\x1b",
            b"]6",
            b"33;P;Cwd=C:\\\\Users\\x3bme",
            b"\x1b",
            b"\\y",
        ]);
        assert_eq!(
            pieces,
            [
                Piece::Bytes(b"x".to_vec()),
                Piece::Mark(Mark::Cwd("C:\\Users;me".into())),
                Piece::Bytes(b"y".to_vec()),
            ]
        );
    }

    #[test]
    fn other_escapes_are_untouched() {
        let input: &[u8] = b"\x1b]0;title\x07\x1b[31mred\x1b]1337;x\x07\x1b\x1b]133;D\x07";
        let pieces = feed_all(&[input]);
        assert_eq!(
            pieces,
            [
                Piece::Bytes(b"\x1b]0;title\x07\x1b[31mred\x1b]1337;x\x07\x1b".to_vec()),
                Piece::Mark(Mark::CommandFinished(None)),
            ]
        );
    }

    #[test]
    fn command_line_is_unescaped() {
        let pieces = feed_all(&[b"\x1b]633;E;echo a\\x3bb;nonce\x07"]);
        assert_eq!(pieces, [Piece::Mark(Mark::CommandLine("echo a;b".into()))]);
    }
}
