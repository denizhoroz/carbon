//! Keystrokes to the bytes a terminal program expects (xterm conventions).
//!
//! Printable text does not come through here: it arrives through the
//! platform's text input (IME, AltGr and dead keys included), see
//! [`passes_as_text`].

use carbon_core::terminal::alacritty_terminal::term::TermMode;
use gpui_kit::Keystroke;

/// Keys the app keeps while a terminal has focus (like VS Code's
/// "commands to skip shell"): dock toggles, quit, saved layouts, dock resize,
/// and the palette (Ctrl+Shift+P, F1, Ctrl+P). Ctrl+G stays with the shell.
pub fn app_keeps(keystroke: &Keystroke) -> bool {
    let m = &keystroke.modifiers;
    let key = keystroke.key.as_str();
    if m.platform {
        return true;
    }
    match (m.control, m.alt, m.shift) {
        (true, false, false) => matches!(key, "b" | "j" | "q" | "p"),
        (true, false, true) => key == "p",
        (true, true, false) => key == "b" || key.len() == 1 && key.as_bytes()[0].is_ascii_digit(),
        (true, true, true) => matches!(key, "left" | "right" | "up" | "down"),
        (false, false, false) => key == "f1",
        _ => false,
    }
}

/// Whether the platform's text input delivers this key: a printable
/// character typed without Ctrl/Alt, or with both (AltGr on Windows).
pub fn passes_as_text(keystroke: &Keystroke) -> bool {
    let m = &keystroke.modifiers;
    let printable = keystroke
        .key_char
        .as_deref()
        .is_some_and(|text| !text.is_empty() && !text.chars().any(char::is_control));
    printable && m.control == m.alt && !is_special(&keystroke.key)
}

fn is_special(key: &str) -> bool {
    matches!(
        key,
        "enter"
            | "tab"
            | "backspace"
            | "escape"
            | "up"
            | "down"
            | "left"
            | "right"
            | "home"
            | "end"
            | "pageup"
            | "pagedown"
            | "insert"
            | "delete"
    ) || function_key(key).is_some()
}

fn function_key(key: &str) -> Option<u8> {
    key.strip_prefix('f')?
        .parse()
        .ok()
        .filter(|n| (1..=12).contains(n))
}

/// The bytes for a key that is not plain text, or `None` to leave it alone.
pub fn to_bytes(keystroke: &Keystroke, mode: TermMode) -> Option<Vec<u8>> {
    let m = &keystroke.modifiers;
    let key = keystroke.key.as_str();
    // xterm modifier parameter: 1 + shift + 2*alt + 4*ctrl.
    let param = 1 + m.shift as u8 + 2 * m.alt as u8 + 4 * m.control as u8;
    let app_cursor = mode.contains(TermMode::APP_CURSOR);
    let esc = |rest: &str| format!("\x1b{rest}").into_bytes();
    let cursor = |letter: char| {
        if param > 1 {
            esc(&format!("[1;{param}{letter}"))
        } else if app_cursor {
            esc(&format!("O{letter}"))
        } else {
            esc(&format!("[{letter}"))
        }
    };
    let tilde = |code: u8| {
        if param > 1 {
            esc(&format!("[{code};{param}~"))
        } else {
            esc(&format!("[{code}~"))
        }
    };
    let alt_prefix = |bytes: &[u8]| {
        let mut out = Vec::with_capacity(bytes.len() + 1);
        if m.alt {
            out.push(0x1b);
        }
        out.extend_from_slice(bytes);
        out
    };
    Some(match key {
        "enter" => alt_prefix(b"\r"),
        "tab" if m.shift => esc("[Z"),
        "tab" => alt_prefix(b"\t"),
        "backspace" if m.control => alt_prefix(b"\x08"),
        "backspace" => alt_prefix(b"\x7f"),
        "escape" => b"\x1b".to_vec(),
        "space" if m.control => alt_prefix(b"\x00"),
        "up" => cursor('A'),
        "down" => cursor('B'),
        "right" => cursor('C'),
        "left" => cursor('D'),
        "home" => cursor('H'),
        "end" => cursor('F'),
        "insert" => tilde(2),
        "delete" => tilde(3),
        "pageup" => tilde(5),
        "pagedown" => tilde(6),
        _ => {
            if let Some(n) = function_key(key) {
                return Some(match n {
                    1..=4 => {
                        let letter = (b'P' + n - 1) as char;
                        if param > 1 {
                            esc(&format!("[1;{param}{letter}"))
                        } else {
                            esc(&format!("O{letter}"))
                        }
                    }
                    5 => tilde(15),
                    6 => tilde(17),
                    7 => tilde(18),
                    8 => tilde(19),
                    9 => tilde(20),
                    10 => tilde(21),
                    11 => tilde(23),
                    _ => tilde(24),
                });
            }
            if m.control {
                return control_byte(key).map(|byte| alt_prefix(&[byte]));
            }
            if m.alt {
                // Alt+key: ESC then the character (meta).
                let text = keystroke.key_char.as_deref().unwrap_or(key);
                if text.chars().count() == 1 {
                    return Some(alt_prefix(text.as_bytes()));
                }
            }
            return None;
        }
    })
}

/// Ctrl+key as a C0 control character.
fn control_byte(key: &str) -> Option<u8> {
    let [byte] = key.as_bytes() else {
        return None;
    };
    Some(match byte {
        b'a'..=b'z' => byte - b'a' + 1,
        b'@' | b'2' => 0x00,
        b'[' | b'3' => 0x1b,
        b'\\' | b'4' => 0x1c,
        b']' | b'5' => 0x1d,
        b'^' | b'6' => 0x1e,
        b'_' | b'-' | b'7' | b'/' => 0x1f,
        b'8' => 0x7f,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use carbon_core::terminal::alacritty_terminal::term::TermMode;
    use gpui_kit::Keystroke;

    use super::{app_keeps, passes_as_text, to_bytes};

    fn key(text: &str) -> Keystroke {
        Keystroke::parse(text).unwrap()
    }

    #[test]
    fn special_keys_and_control_codes() {
        let none = TermMode::empty();
        assert_eq!(to_bytes(&key("enter"), none).unwrap(), b"\r");
        assert_eq!(to_bytes(&key("backspace"), none).unwrap(), b"\x7f");
        assert_eq!(to_bytes(&key("up"), none).unwrap(), b"\x1b[A");
        assert_eq!(
            to_bytes(&key("up"), TermMode::APP_CURSOR).unwrap(),
            b"\x1bOA"
        );
        assert_eq!(to_bytes(&key("ctrl-right"), none).unwrap(), b"\x1b[1;5C");
        assert_eq!(to_bytes(&key("ctrl-c"), none).unwrap(), [3]);
        assert_eq!(to_bytes(&key("ctrl-w"), none).unwrap(), [23]);
        assert_eq!(to_bytes(&key("alt-b"), none).unwrap(), b"\x1bb");
        assert_eq!(to_bytes(&key("f5"), none).unwrap(), b"\x1b[15~");
        assert_eq!(to_bytes(&key("shift-tab"), none).unwrap(), b"\x1b[Z");
        assert_eq!(to_bytes(&key("delete"), none).unwrap(), b"\x1b[3~");
    }

    #[test]
    fn app_shortcuts_and_text() {
        assert!(app_keeps(&key("ctrl-b")));
        assert!(app_keeps(&key("ctrl-alt-3")));
        assert!(!app_keeps(&key("ctrl-w")));
        assert!(!app_keeps(&key("ctrl-k")));
        // The palette keys work from a terminal; Ctrl+G (readline abort) does not.
        assert!(app_keeps(&key("ctrl-shift-p")));
        assert!(app_keeps(&key("ctrl-p")));
        assert!(app_keeps(&key("f1")));
        assert!(!app_keeps(&key("ctrl-g")));
        assert!(!app_keeps(&key("ctrl-shift-c")));
        assert!(!app_keeps(&key("f2")));
        let mut a = key("a");
        a.key_char = Some("a".into());
        assert!(passes_as_text(&a));
        let mut ctrl_a = key("ctrl-a");
        ctrl_a.key_char = Some("a".into());
        assert!(!passes_as_text(&ctrl_a));
        let mut altgr = key("ctrl-alt-q");
        altgr.key_char = Some("@".into());
        assert!(passes_as_text(&altgr));
    }
}
