//! JSON with comments (`settings.json`): reading it, and changing one
//! top-level key in place so the user's comments, order and layout survive
//! an edit from the Settings dialog.

use std::ops::Range;

use serde_json::{Map, Value};

/// Parses a JSONC object: `//` and `/* */` comments and trailing commas are
/// allowed. Empty text is an empty object.
pub fn parse(text: &str) -> Result<Map<String, Value>, String> {
    let plain = strip(text);
    if plain.trim().is_empty() {
        return Ok(Map::new());
    }
    match serde_json::from_str::<Value>(&plain) {
        Ok(Value::Object(map)) => Ok(map),
        Ok(_) => Err("the file must hold one JSON object ({ ... })".into()),
        Err(err) => Err(err.to_string()),
    }
}

/// `text` with comments blanked out (newlines kept, so error positions still
/// point at the right line) and trailing commas removed.
fn strip(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = String::with_capacity(text.len());
    let mut i = 0;
    // Byte offset in `out` of a comma that may turn out to be trailing.
    let mut pending_comma: Option<usize> = None;
    while i < bytes.len() {
        match bytes[i] {
            b'"' => {
                let end = string_end(bytes, i);
                out.push_str(&text[i..end]);
                pending_comma = None;
                i = end;
            }
            b'/' if bytes.get(i + 1) == Some(&b'/') => {
                while i < bytes.len() && bytes[i] != b'\n' {
                    i += 1;
                }
            }
            b'/' if bytes.get(i + 1) == Some(&b'*') => {
                let end = text[i + 2..]
                    .find("*/")
                    .map_or(bytes.len(), |at| i + 2 + at + 2);
                out.extend(text[i..end].chars().filter(|&c| c == '\n'));
                i = end;
            }
            b',' => {
                pending_comma = Some(out.len());
                out.push(',');
                i += 1;
            }
            b'}' | b']' => {
                if let Some(at) = pending_comma.take() {
                    out.replace_range(at..at + 1, " ");
                }
                out.push(bytes[i] as char);
                i += 1;
            }
            byte if byte.is_ascii_whitespace() => {
                out.push(byte as char);
                i += 1;
            }
            _ => {
                let ch = text[i..].chars().next().unwrap_or(' ');
                out.push(ch);
                pending_comma = None;
                i += ch.len_utf8();
            }
        }
    }
    out
}

/// One `"key": value` pair of the top-level object.
struct Member {
    key: String,
    /// From the key's opening quote to the end of the value.
    span: Range<usize>,
    value: Range<usize>,
}

/// The top-level object: its braces and members, in file order.
struct Object {
    open: usize,
    close: usize,
    members: Vec<Member>,
}

/// Sets top-level `key` to `value` in `text`, keeping everything else as it
/// is. A missing key is added after the last one. Fails on text that is not
/// a JSONC object (the caller should not overwrite a file it cannot read).
pub fn set(text: &str, key: &str, value: &Value) -> Result<String, String> {
    if text.trim().is_empty() {
        return Ok(format!("{{\n  {}: {value}\n}}\n", quote(key)));
    }
    parse(text)?;
    let object = scan(text).ok_or("cannot read the settings object")?;
    let value = value.to_string();
    let mut out = text.to_string();
    // The last one wins when a key is written twice, as in the parser.
    if let Some(member) = object.members.iter().rev().find(|m| m.key == key) {
        out.replace_range(member.value.clone(), &value);
        return Ok(out);
    }
    let entry = format!("{}: {value}", quote(key));
    match object.members.last() {
        Some(last) => {
            let indent = indent_at(text, last.span.start);
            out.insert_str(last.value.end, &format!(",\n{indent}{entry}"));
        }
        None => {
            let inner = object.open + 1..object.close;
            if text[inner.clone()].trim().is_empty() {
                out.replace_range(inner, &format!("\n  {entry}\n"));
            } else {
                // Only comments inside: keep them below the new key.
                out.insert_str(object.open + 1, &format!("\n  {entry},"));
            }
        }
    }
    Ok(out)
}

/// Removes top-level `key` (every copy of it) from `text`, with its comma
/// and, when it had the line to itself, its line.
pub fn remove(text: &str, key: &str) -> Result<String, String> {
    if text.trim().is_empty() {
        return Ok(String::new());
    }
    parse(text)?;
    let mut out = text.to_string();
    while let Some(object) = scan(&out) {
        let Some(ix) = object.members.iter().position(|m| m.key == key) else {
            break;
        };
        let member = &object.members[ix];
        let bytes = out.as_bytes();
        let after = skip_space(bytes, member.span.end);
        let range = if bytes.get(after) == Some(&b',') {
            // `"key": v,` : the member and its comma; the line too if nothing
            // else is on it.
            let mut start = member.span.start;
            let line_start = out[..start].rfind('\n').map_or(0, |at| at + 1);
            let mut end = after + 1;
            if out[line_start..start].trim().is_empty() {
                let line_end = out[end..].find('\n').map_or(out.len(), |at| end + at + 1);
                if out[end..line_end].trim().is_empty() {
                    start = line_start;
                    end = line_end;
                }
            }
            start..end
        } else if ix > 0 {
            // The last member: the comma before it goes instead.
            object.members[ix - 1].span.end..member.span.end
        } else {
            // The only member: empty the braces.
            object.open + 1..object.close
        };
        let only = ix == 0 && bytes.get(after) != Some(&b',');
        out.replace_range(range, if only { "\n" } else { "" });
    }
    Ok(out)
}

fn quote(key: &str) -> String {
    Value::String(key.to_string()).to_string()
}

/// The whitespace a line starts with, for the line holding `at`.
fn indent_at(text: &str, at: usize) -> String {
    let line_start = text[..at].rfind('\n').map_or(0, |ix| ix + 1);
    let indent = &text[line_start..at];
    if indent.trim().is_empty() {
        indent.to_string()
    } else {
        "  ".into()
    }
}

fn scan(text: &str) -> Option<Object> {
    let bytes = text.as_bytes();
    let open = skip_space(bytes, 0);
    if bytes.get(open) != Some(&b'{') {
        return None;
    }
    let mut members = Vec::new();
    let mut i = open + 1;
    loop {
        i = skip_space(bytes, i);
        match bytes.get(i)? {
            b'}' => {
                return Some(Object {
                    open,
                    close: i,
                    members,
                });
            }
            b',' => i += 1,
            b'"' => {
                let key_end = string_end(bytes, i);
                let key: String = serde_json::from_str(&text[i..key_end]).ok()?;
                let colon = skip_space(bytes, key_end);
                if bytes.get(colon) != Some(&b':') {
                    return None;
                }
                let value_start = skip_space(bytes, colon + 1);
                let value_end = value_end(bytes, value_start)?;
                members.push(Member {
                    key,
                    span: i..value_end,
                    value: value_start..value_end,
                });
                i = value_end;
            }
            _ => return None,
        }
    }
}

/// Past whitespace and comments.
fn skip_space(bytes: &[u8], mut i: usize) -> usize {
    loop {
        match bytes.get(i) {
            Some(byte) if byte.is_ascii_whitespace() => i += 1,
            Some(b'/') if bytes.get(i + 1) == Some(&b'/') => {
                while i < bytes.len() && bytes[i] != b'\n' {
                    i += 1;
                }
            }
            Some(b'/') if bytes.get(i + 1) == Some(&b'*') => {
                i += 2;
                while i < bytes.len() && !(bytes[i] == b'*' && bytes.get(i + 1) == Some(&b'/')) {
                    i += 1;
                }
                i = (i + 2).min(bytes.len());
            }
            _ => return i,
        }
    }
}

/// Just past the string that starts at `start` (its opening quote).
fn string_end(bytes: &[u8], start: usize) -> usize {
    let mut i = start + 1;
    while i < bytes.len() {
        match bytes[i] {
            b'\\' => i += 2,
            b'"' => return i + 1,
            _ => i += 1,
        }
    }
    bytes.len()
}

/// Just past the value that starts at `start`.
fn value_end(bytes: &[u8], start: usize) -> Option<usize> {
    match bytes.get(start)? {
        b'"' => Some(string_end(bytes, start)),
        b'{' | b'[' => {
            let mut depth = 0usize;
            let mut i = start;
            while i < bytes.len() {
                match bytes[i] {
                    b'"' => {
                        i = string_end(bytes, i);
                        continue;
                    }
                    b'/' if matches!(bytes.get(i + 1), Some(b'/' | b'*')) => {
                        i = skip_space(bytes, i);
                        continue;
                    }
                    b'{' | b'[' => depth += 1,
                    b'}' | b']' => {
                        depth -= 1;
                        if depth == 0 {
                            return Some(i + 1);
                        }
                    }
                    _ => {}
                }
                i += 1;
            }
            None
        }
        _ => {
            let mut i = start;
            while i < bytes.len()
                && !matches!(bytes[i], b',' | b'}' | b']' | b'/')
                && !bytes[i].is_ascii_whitespace()
            {
                i += 1;
            }
            (i > start).then_some(i)
        }
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    const COMMENTED: &str = r#"{
  // Theme
  "appearance.theme.mode": "light", /* inline */
  "editor.tab_size": 4,
}
"#;

    #[test]
    fn parses_comments_and_trailing_commas() {
        let map = parse(COMMENTED).unwrap();
        assert_eq!(map["appearance.theme.mode"], json!("light"));
        assert_eq!(map["editor.tab_size"], json!(4));
        assert_eq!(parse("").unwrap().len(), 0);
        assert_eq!(
            parse(r#"{"a": "// not a comment", "b": [1, 2,],}"#).unwrap()["a"],
            json!("// not a comment")
        );
        assert!(parse("[1]").is_err());
        assert!(parse("{\"a\": }").is_err());
    }

    #[test]
    fn set_keeps_comments() {
        let out = set(COMMENTED, "editor.tab_size", &json!(2)).unwrap();
        assert!(out.contains("// Theme") && out.contains("/* inline */"));
        assert_eq!(parse(&out).unwrap()["editor.tab_size"], json!(2));

        let out = set(COMMENTED, "editor.soft_wrap", &json!(true)).unwrap();
        let map = parse(&out).unwrap();
        assert_eq!(map["editor.soft_wrap"], json!(true));
        assert_eq!(map.len(), 3);
        assert!(out.contains("\n  \"editor.soft_wrap\": true"));
    }

    #[test]
    fn set_into_empty() {
        assert_eq!(set("", "a", &json!(1)).unwrap(), "{\n  \"a\": 1\n}\n");
        assert_eq!(set("{}", "a", &json!(1)).unwrap(), "{\n  \"a\": 1\n}");
        let out = set("{\n  // keep\n}", "a", &json!("x")).unwrap();
        assert!(out.contains("// keep"));
        assert_eq!(parse(&out).unwrap()["a"], json!("x"));
    }

    #[test]
    fn set_refuses_broken_text() {
        assert!(set("{ \"a\": ", "a", &json!(1)).is_err());
    }

    #[test]
    fn remove_member() {
        // Middle member with its line.
        let out = remove(COMMENTED, "appearance.theme.mode").unwrap();
        assert!(!out.contains("appearance.theme.mode"));
        assert_eq!(parse(&out).unwrap().len(), 1);
        // Last member: the comma before it goes.
        let text = "{\n  \"a\": 1,\n  \"b\": 2\n}";
        assert_eq!(remove(text, "b").unwrap(), "{\n  \"a\": 1\n}");
        // The only one.
        assert_eq!(remove("{\"a\": 1}", "a").unwrap(), "{\n}");
        // Missing: unchanged.
        assert_eq!(remove(text, "zzz").unwrap(), text);
        // Twice.
        let twice = "{\"a\": 1, \"b\": 2, \"a\": 3}";
        assert_eq!(parse(&remove(twice, "a").unwrap()).unwrap().len(), 1);
    }
}
