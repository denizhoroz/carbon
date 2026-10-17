//! File paths as `file://` URIs and back, the way language servers name files.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::str::FromStr as _;

use lsp_types::Uri;

/// `C:\a b\x.rs` → `file:///C:/a%20b/x.rs`.
pub fn path_to_uri(path: &Path) -> Uri {
    let text = path.to_string_lossy().replace('\\', "/");
    let mut uri = String::from("file://");
    if !text.starts_with('/') {
        uri.push('/');
    }
    for byte in text.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' | b'/' | b':' => {
                uri.push(byte as char)
            }
            _ => _ = write!(uri, "%{byte:02X}"),
        }
    }
    Uri::from_str(&uri).expect("an encoded file path is a valid URI")
}

/// The file a `file://` URI names; `None` for other schemes.
pub fn uri_to_path(uri: &Uri) -> Option<PathBuf> {
    let rest = uri.as_str().strip_prefix("file://")?;
    let mut bytes = Vec::with_capacity(rest.len());
    let mut iter = rest.bytes();
    while let Some(byte) = iter.next() {
        if byte == b'%' {
            let hex = [iter.next()?, iter.next()?];
            let hex = std::str::from_utf8(&hex).ok()?;
            bytes.push(u8::from_str_radix(hex, 16).ok()?);
        } else {
            bytes.push(byte);
        }
    }
    let text = String::from_utf8(bytes).ok()?;
    // `/C:/x` → `C:/x` on Windows.
    let text = match text.as_bytes() {
        [b'/', drive, b':', ..] if cfg!(windows) && drive.is_ascii_alphabetic() => &text[1..],
        _ => text.as_str(),
    };
    if cfg!(windows) {
        Some(PathBuf::from(text.replace('/', "\\")))
    } else {
        Some(PathBuf::from(text))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_paths_with_spaces_and_unicode() {
        let path = if cfg!(windows) {
            PathBuf::from(r"C:\Users\a b\çode\main.rs")
        } else {
            PathBuf::from("/home/a b/çode/main.rs")
        };
        let uri = path_to_uri(&path);
        assert!(uri.as_str().starts_with("file:///"));
        assert!(uri.as_str().contains("a%20b"));
        assert_eq!(uri_to_path(&uri).unwrap(), path);
    }
}
