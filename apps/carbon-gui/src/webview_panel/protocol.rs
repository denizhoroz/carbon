//! Serving an extension's page to its webview: a custom protocol, one origin
//! per extension, files only from that extension's package
//! (`resolve_package_file`: Q22 path rules, exact case, no links).
//!
//! The page may load its own files and nothing from the network (CSP);
//! navigation stays inside the package and new windows are refused. Network
//! access comes with the `net` capability later (`.agents/documents/extensions.md`).

use std::borrow::Cow;
use std::path::{Path, PathBuf};

use carbon_core::extension::{ExtensionId, resolve_package_file};
use wry::http::{Request, Response, StatusCode, header};

/// Scheme of package pages (`carbon-page://<extension>/<path>`; WebView2 serves
/// it as `http://carbon-page.<extension>/<path>`).
pub const SCHEME: &str = "carbon-page";

/// What a page may load: its own files (and inline/data/blob content), no
/// network, WebAssembly allowed.
const CSP: &str = "default-src 'self' data: blob:; \
                   script-src 'self' 'unsafe-inline' 'wasm-unsafe-eval'; \
                   style-src 'self' 'unsafe-inline'";

/// The host naming an extension in page URLs: its id with `.` written `--`, so
/// every extension has its own origin (storage, cookies) and ids do not clash
/// (`a.b-c` and `a-b.c` stay apart).
pub fn host(extension: &ExtensionId) -> String {
    extension.as_str().replace('.', "--")
}

/// The URL origin of an extension's pages on this platform.
pub fn origin(extension: &ExtensionId) -> String {
    if cfg!(any(windows, target_os = "android")) {
        format!("http://{SCHEME}.{}", host(extension))
    } else {
        format!("{SCHEME}://{}", host(extension))
    }
}

/// Whether the page may navigate to `url`: only inside its own origin.
pub fn allows(origin: &str, url: &str) -> bool {
    url == origin
        || url.strip_prefix(origin).is_some_and(|rest| {
            rest.starts_with('/') || rest.starts_with('?') || rest.starts_with('#')
        })
}

/// Answers one request from the package at `root`.
pub fn serve(root: &Path, request: &Request<Vec<u8>>) -> Response<Cow<'static, [u8]>> {
    if request.method() != wry::http::Method::GET {
        return status(StatusCode::METHOD_NOT_ALLOWED, "Only GET");
    }
    let Some(path) = decode(request.uri().path().trim_start_matches('/')) else {
        return status(StatusCode::BAD_REQUEST, "Bad path");
    };
    let file = match resolve_package_file(root, &path) {
        Ok(file) => file,
        Err(problem) => return status(StatusCode::NOT_FOUND, &problem),
    };
    match std::fs::read(&file) {
        Ok(bytes) => Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, mime(&file))
            .header(header::CONTENT_SECURITY_POLICY, CSP)
            .header(header::CACHE_CONTROL, "no-cache")
            .body(Cow::Owned(bytes))
            .unwrap_or_else(|_| status(StatusCode::INTERNAL_SERVER_ERROR, "Response")),
        Err(err) => status(StatusCode::NOT_FOUND, &err.to_string()),
    }
}

fn status(code: StatusCode, text: &str) -> Response<Cow<'static, [u8]>> {
    Response::builder()
        .status(code)
        .header(header::CONTENT_TYPE, "text/plain; charset=utf-8")
        .body(Cow::Owned(text.as_bytes().to_vec()))
        .unwrap_or_else(|_| Response::new(Cow::Borrowed(&[][..])))
}

/// `%XX` escapes decoded; `None` for a broken escape or non-UTF-8 result.
fn decode(path: &str) -> Option<String> {
    let bytes = path.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' {
            let hex = std::str::from_utf8(bytes.get(i + 1..i + 3)?).ok()?;
            out.push(u8::from_str_radix(hex, 16).ok()?);
            i += 3;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8(out).ok()
}

/// Content type by extension; unknown files are plain bytes.
fn mime(file: &Path) -> &'static str {
    let extension = file
        .extension()
        .and_then(|extension| extension.to_str())
        .map(str::to_ascii_lowercase);
    match extension.as_deref() {
        Some("html" | "htm") => "text/html; charset=utf-8",
        Some("js" | "mjs") => "text/javascript; charset=utf-8",
        Some("css") => "text/css; charset=utf-8",
        Some("json") => "application/json",
        Some("svg") => "image/svg+xml",
        Some("png") => "image/png",
        Some("jpg" | "jpeg") => "image/jpeg",
        Some("gif") => "image/gif",
        Some("webp") => "image/webp",
        Some("ico") => "image/x-icon",
        Some("wasm") => "application/wasm",
        Some("woff") => "font/woff",
        Some("woff2") => "font/woff2",
        Some("ttf") => "font/ttf",
        Some("txt" | "md") => "text/plain; charset=utf-8",
        _ => "application/octet-stream",
    }
}

/// Where a webview panel's page comes from: a file in an extension's package.
#[derive(Clone, Debug, PartialEq)]
pub struct PageSource {
    pub extension: ExtensionId,
    /// Path inside the package (`check_package_path` rules).
    pub page: String,
}

impl PageSource {
    /// The page's URL.
    pub fn url(&self) -> String {
        format!("{}/{}", origin(&self.extension), self.page)
    }

    /// The package folder, from the loaded extensions (the protocol runs
    /// on the webview's thread, so it reads the shared snapshot).
    pub fn package_dir(&self) -> Result<PathBuf, String> {
        crate::extensions::registry_snapshot()
            .get(&self.extension)
            .map(|loaded| loaded.dir.clone())
            .ok_or_else(|| format!("extension `{}` is not installed", self.extension))
    }
}

#[cfg(test)]
mod tests {
    use carbon_core::extension::ExtensionId;

    use super::{allows, decode, host, origin};

    #[test]
    fn each_extension_has_its_own_host() {
        assert_eq!(host(&ExtensionId::new("test.web-spike")), "test--web-spike");
        assert_ne!(
            host(&ExtensionId::new("a.b-c")),
            host(&ExtensionId::new("a-b.c"))
        );
    }

    #[test]
    fn navigation_stays_in_the_origin() {
        let origin = origin(&ExtensionId::new("test.web"));
        assert!(allows(&origin, &format!("{origin}/index.html")));
        assert!(allows(&origin, &format!("{origin}/a/b.html?x=1#y")));
        assert!(!allows(&origin, &format!("{origin}.evil.com/index.html")));
        assert!(!allows(&origin, "https://example.com/"));
    }

    #[test]
    fn escapes_decode() {
        assert_eq!(decode("a%20b/c.html").as_deref(), Some("a b/c.html"));
        assert_eq!(decode("bad%2"), None);
    }
}
