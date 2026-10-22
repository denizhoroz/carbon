//! Webview panel (experimental spike, `webview` feature; workflow-system.md
//! s4.4 "Webview spike"). A WebView2 / WKWebView page in a dock tab through
//! the kit's `gpui-wry`.
//!
//! The page is a native child window drawn above GPUI, so it is hidden
//! whenever it must not show: its tab is in the background, its dock is
//! closed, or a GPUI overlay (dialog, palette, menu) is open
//! (`Workbench::sync_webviews` decides, every frame). Carbon's own
//! shortcuts reach the app while the page has focus: an injected script
//! sends them over IPC and the panel dispatches them as keystrokes.
//!
//! The page is a file in the workflow's package (`page` in the manifest's
//! panel rule), served by `protocol`; a panel without one shows the spike's
//! test page (`spike.html`).

mod protocol;

pub use protocol::PageSource;

use carbon_core::workflow::WorkflowId;
use gpui_kit::component::dock::{
    BasePanel, Panel, PanelEvent, PanelInfo, PanelState, panel_handle, register_panel,
};
use gpui_kit::component::{ActiveTheme, Theme, v_flex};
use gpui_kit::*;
use gpui_wry::WebView;
use serde::Deserialize;
use wry::dpi::{LogicalPosition, LogicalSize};
use wry::raw_window_handle::{HandleError, HasWindowHandle, RawWindowHandle, WindowHandle};

use crate::style::fixed;
use crate::terminal_panel::input::app_keeps;

/// Persistence key in saved layouts. Never change it.
pub const PANEL_NAME: &str = "Webview";

/// The spike's test page.
const SPIKE_PAGE: &str = include_str!("webview_panel/spike.html");

/// Messages larger than this are dropped unread (IPC is privileged).
const MAX_MESSAGE_LEN: usize = 64 * 1024;

/// Sets up the process before GPUI starts. On Windows GPUI must draw into a
/// classic window swap chain, or a child webview window does not show (the
/// kit's webview example does the same; `gpui-pre-windows` window.rs).
pub fn prepare() {
    #[cfg(windows)]
    // SAFETY: called first thing in `main`, before any other thread exists.
    unsafe {
        std::env::set_var("GPUI_DISABLE_DIRECT_COMPOSITION", "true");
    }
}

/// Registers the builder that recreates webview panels when a saved layout loads.
pub fn register(cx: &mut App) {
    register_panel(cx, PANEL_NAME, |build, _, cx| {
        let (title, source) = match build.info() {
            PanelInfo::Panel(value) => {
                let text = |key: &str| value[key].as_str().map(str::to_string);
                let source =
                    text("workflow")
                        .zip(text("page"))
                        .map(|(workflow, page)| PageSource {
                            workflow: WorkflowId::new(workflow),
                            page,
                        });
                (text("title").unwrap_or_else(|| "Web".into()), source)
            }
            _ => ("Web".to_string(), None),
        };
        panel_handle(WebviewPanel::new(title, source, cx))
    });
}

/// The window a page is built in, held across the build's awaits.
struct ParentWindow(RawWindowHandle);

impl HasWindowHandle for ParentWindow {
    fn window_handle(&self) -> Result<WindowHandle<'_>, HandleError> {
        // SAFETY: taken from the panel's own GPUI window, which outlives the
        // panel and so the panel's task that uses it.
        Ok(unsafe { WindowHandle::borrow_raw(self.0) })
    }
}

/// What the page sends (`window.ipc.postMessage`, JSON).
#[derive(Debug, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "lowercase")]
enum PageMessage {
    /// One of Carbon's shortcuts, as a GPUI keystroke (`ctrl-shift-p`).
    Key { keystroke: String },
    /// The page was clicked: the panel takes GPUI focus.
    Focus,
    /// Spike round trip: answered with `pong`.
    Ping { text: String },
}

/// What the panel's task receives from the webview's callbacks.
enum Incoming {
    Message(String),
    Loaded,
}

enum Page {
    /// Not built yet: the native view needs the window, so it is made after
    /// the first render.
    Pending,
    Creating,
    Ready(Entity<WebView>),
    Failed(SharedString),
}

pub struct WebviewPanel {
    title: SharedString,
    /// `None`: the spike's test page.
    source: Option<PageSource>,
    focus_handle: FocusHandle,
    page: Page,
    /// The displayed tab of its group (kit `set_active`).
    active: bool,
    /// The page finished loading; shown only after that (no white flash).
    loaded: bool,
    /// Messages answered so far (spike counter).
    pongs: usize,
    _task: Task<()>,
    _theme_sub: Subscription,
}

impl WebviewPanel {
    pub fn new(
        title: impl Into<SharedString>,
        source: Option<PageSource>,
        cx: &mut App,
    ) -> Entity<Self> {
        let title = title.into();
        cx.new(|cx| Self {
            title,
            source,
            focus_handle: cx.focus_handle(),
            page: Page::Pending,
            active: true,
            loaded: false,
            pongs: 0,
            _task: Task::ready(()),
            // Themes load after start and can change at any time.
            _theme_sub: cx.observe_global::<Theme>(|this: &mut Self, cx| this.push_theme(cx)),
        })
    }

    /// Gives the page the current theme colours and background.
    fn push_theme(&self, cx: &App) {
        let Page::Ready(view) = &self.page else {
            return;
        };
        let view = view.read(cx);
        let _ = view.set_background_color(rgba(cx.theme().background));
        let script = format!("window.carbon.setTheme({})", theme_vars(cx));
        if let Err(err) = view.evaluate_script(&script) {
            eprintln!("webview: theme update failed: {err}");
        }
    }

    /// Builds the native webview as a child of the window, hidden until the
    /// page has loaded. WebView2 runs the window's message loop while it
    /// starts, which runs other GPUI tasks; so it is built inside this
    /// panel's task, never inside an update (that panics: "RefCell already
    /// borrowed").
    fn create(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let parent = match window.window_handle() {
            Ok(handle) => ParentWindow(handle.as_raw()),
            Err(err) => {
                self.page = Page::Failed(format!("The page cannot be shown: {err}").into());
                return;
            }
        };
        let (tx, rx) = async_channel::unbounded();
        let ipc = tx.clone();
        let background = rgba(cx.theme().background);
        let script = init_script(&theme_vars(cx));
        let builder = match &self.source {
            Some(source) => match page_builder(source) {
                Ok(builder) => builder,
                Err(problem) => {
                    self.page = Page::Failed(format!("The page cannot be shown: {problem}").into());
                    cx.notify();
                    return;
                }
            },
            None => wry::WebViewBuilder::new().with_html(SPIKE_PAGE),
        };
        let builder = builder
            .with_initialization_script(script)
            .with_background_color(background)
            .with_visible(false)
            .with_ipc_handler(move |request: wry::http::Request<String>| {
                let _ = ipc.try_send(Incoming::Message(request.into_body()));
            })
            .with_on_page_load_handler(move |event, _| {
                if matches!(event, wry::PageLoadEvent::Finished) {
                    let _ = tx.try_send(Incoming::Loaded);
                }
            });
        self._task = cx.spawn_in(window, async move |this, cx| {
            let built = builder.build_as_child_async(&parent).await;
            let attached = this.update_in(cx, |this, window, cx| {
                this.page = match built {
                    Ok(native) => Page::Ready(cx.new(|cx| {
                        let mut view = WebView::new(native, window, cx);
                        // Matches the native state (built hidden).
                        view.hide();
                        view
                    })),
                    Err(err) => Page::Failed(format!("The page cannot be shown: {err}").into()),
                };
                cx.notify();
            });
            if attached.is_err() {
                return;
            }
            while let Ok(incoming) = rx.recv().await {
                let handled = this.update_in(cx, |this, window, cx| match incoming {
                    Incoming::Loaded => {
                        // The theme may have changed while the page loaded.
                        this.push_theme(cx);
                        this.loaded = true;
                        cx.notify();
                    }
                    Incoming::Message(text) => this.on_message(&text, window, cx),
                });
                if handled.is_err() {
                    break;
                }
            }
        });
    }

    fn on_message(&mut self, text: &str, window: &mut Window, cx: &mut Context<Self>) {
        if text.len() > MAX_MESSAGE_LEN {
            return;
        }
        let Ok(message) = serde_json::from_str::<PageMessage>(text) else {
            eprintln!("webview: unknown message ignored");
            return;
        };
        match message {
            PageMessage::Key { keystroke } => {
                // Only Carbon's reserved keys; a page cannot type for the user.
                let Ok(keystroke) = Keystroke::parse(&keystroke) else {
                    return;
                };
                if !app_keeps(&keystroke) {
                    return;
                }
                self.focus_handle.focus(window, cx);
                // After this update: the action's handlers may read this panel.
                window.defer(cx, move |window, cx| {
                    window.dispatch_keystroke(keystroke, cx);
                });
            }
            PageMessage::Focus => self.focus_handle.focus(window, cx),
            PageMessage::Ping { text } => {
                self.pongs += 1;
                let reply =
                    serde_json::json!({ "type": "pong", "text": text, "count": self.pongs });
                if let Page::Ready(view) = &self.page {
                    let script = format!("window.carbon.receive({reply})");
                    if let Err(err) = view.read(cx).evaluate_script(&script) {
                        eprintln!("webview: reply failed: {err}");
                    }
                }
            }
        }
    }

    /// Shows or hides the native page. `allowed`: its dock is open and no
    /// GPUI overlay is open (from the workbench).
    pub fn sync(&mut self, allowed: bool, cx: &mut Context<Self>) {
        let show = allowed && self.active && self.loaded;
        let Page::Ready(view) = &self.page else {
            return;
        };
        view.update(cx, |view, cx| {
            if show && !view.visible() {
                // The last laid-out place first, so it never flashes at an old one.
                let bounds = view.bounds();
                let _ = view.raw().set_bounds(wry::Rect {
                    position: LogicalPosition::new(
                        f32::from(bounds.origin.x),
                        f32::from(bounds.origin.y),
                    )
                    .into(),
                    size: LogicalSize::new(
                        f32::from(bounds.size.width),
                        f32::from(bounds.size.height),
                    )
                    .into(),
                });
                view.show();
                cx.notify();
            } else if !show && view.visible() {
                view.hide();
            }
        });
    }
}

/// A builder loading `source` through the package protocol: files from the
/// workflow's package only, navigation kept inside it, no new windows.
fn page_builder(source: &PageSource) -> Result<wry::WebViewBuilder<'static>, String> {
    let root = source.package_dir()?;
    // Checked here too, for a clear message instead of a 404 page.
    carbon_core::workflow::resolve_package_file(&root, &source.page)?;
    let origin = protocol::origin(&source.workflow);
    Ok(wry::WebViewBuilder::new()
        .with_custom_protocol(protocol::SCHEME.into(), move |_, request| {
            protocol::serve(&root, &request)
        })
        .with_navigation_handler(move |url| protocol::allows(&origin, &url))
        .with_new_window_req_handler(|_, _| wry::NewWindowResponse::Deny)
        .with_url(source.url()))
}

/// The keystrokes Carbon keeps while a page has focus: the terminal's list
/// (`app_keeps`), spelled `ctrl-alt-shift-key` like the injected script does.
fn reserved_keys() -> Vec<String> {
    let keys = ('a'..='z')
        .chain('0'..='9')
        .map(String::from)
        .chain(["left", "right", "up", "down"].map(String::from))
        .chain((1..=12).map(|n| format!("f{n}")));
    let keys: Vec<String> = keys.collect();
    let mut reserved = Vec::new();
    for bits in 0..8 {
        let prefix = format!(
            "{}{}{}",
            if bits & 1 != 0 { "ctrl-" } else { "" },
            if bits & 2 != 0 { "alt-" } else { "" },
            if bits & 4 != 0 { "shift-" } else { "" },
        );
        for key in &keys {
            let text = format!("{prefix}{key}");
            if Keystroke::parse(&text).is_ok_and(|keystroke| app_keeps(&keystroke)) {
                reserved.push(text);
            }
        }
    }
    reserved
}

/// The theme colours a page gets, as CSS variables (`--carbon-background`, ...).
fn theme_vars(cx: &App) -> serde_json::Value {
    let theme = cx.theme();
    let colors = [
        ("background", theme.background),
        ("foreground", theme.foreground),
        ("muted-foreground", theme.muted_foreground),
        ("border", theme.border),
        ("primary", theme.primary),
        ("primary-foreground", theme.primary_foreground),
    ];
    serde_json::Value::Object(
        colors
            .into_iter()
            .map(|(name, color)| (format!("--carbon-{name}"), hex(color).into()))
            .collect(),
    )
}

/// Runs before the page's own scripts: the `carbon` bridge, the reserved
/// shortcuts, focus reporting and the theme colours (`carbon.setTheme`).
fn init_script(colors: &serde_json::Value) -> String {
    let keys = serde_json::to_string(&reserved_keys()).unwrap_or_else(|_| "[]".into());
    format!(
        r#"(() => {{
  const keys = new Set({keys});
  const send = (message) => window.ipc.postMessage(JSON.stringify(message));
  window.carbon = {{
    send,
    onMessage: null,
    receive(message) {{ if (this.onMessage) this.onMessage(message); }},
    setTheme(colors) {{
      const paint = () => {{
        for (const [name, value] of Object.entries(colors)) {{
          document.documentElement.style.setProperty(name, value);
        }}
      }};
      if (document.documentElement) paint();
      else addEventListener("DOMContentLoaded", paint);
    }},
  }};
  window.carbon.setTheme({colors});
  const keyName = (event) => {{
    const code = event.code;
    if (code.startsWith("Key")) return code.slice(3).toLowerCase();
    if (code.startsWith("Digit")) return code.slice(5);
    if (code.startsWith("Arrow")) return code.slice(5).toLowerCase();
    if (/^F\d+$/.test(code)) return code.toLowerCase();
    return event.key.toLowerCase();
  }};
  addEventListener("keydown", (event) => {{
    const keystroke = (event.ctrlKey ? "ctrl-" : "") + (event.altKey ? "alt-" : "")
      + (event.shiftKey ? "shift-" : "") + keyName(event);
    if (keys.has(keystroke)) {{
      event.preventDefault();
      event.stopPropagation();
      send({{ type: "key", keystroke }});
    }}
  }}, true);
  addEventListener("mousedown", () => send({{ type: "focus" }}), true);
}})();"#
    )
}

fn hex(color: Hsla) -> String {
    let (r, g, b, _) = rgba(color);
    format!("#{r:02x}{g:02x}{b:02x}")
}

fn rgba(color: Hsla) -> (u8, u8, u8, u8) {
    let color = color.to_rgb();
    let byte = |value: f32| (value.clamp(0., 1.) * 255.).round() as u8;
    (byte(color.r), byte(color.g), byte(color.b), byte(color.a))
}

impl EventEmitter<PanelEvent> for WebviewPanel {}

impl Focusable for WebviewPanel {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl BasePanel for WebviewPanel {
    fn panel_name(&self) -> &'static str {
        PANEL_NAME
    }

    fn set_active(&mut self, active: bool, _: &mut Window, cx: &mut Context<Self>) {
        self.active = active;
        // Hide at once; showing waits for the workbench's next frame.
        if !active {
            self.sync(false, cx);
        }
        cx.notify();
    }

    fn on_removed(&mut self, _: &mut Window, cx: &mut Context<Self>) {
        // Dropping the view hides the native page; it is destroyed with the panel.
        self.page = Page::Pending;
        self.loaded = false;
        self._task = Task::ready(());
        cx.notify();
    }

    fn dump(&self, _: &App) -> PanelState {
        let mut state = PanelState::new(PANEL_NAME);
        let mut info = serde_json::json!({ "title": self.title.as_ref() });
        if let Some(source) = &self.source {
            info["workflow"] = source.workflow.as_str().into();
            info["page"] = source.page.as_str().into();
        }
        state.info = PanelInfo::Panel(info);
        state
    }
}

impl Panel for WebviewPanel {
    fn tab_name(&self, _: &App) -> Option<SharedString> {
        Some(self.title.clone())
    }

    fn title(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        self.title.clone()
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
}

impl Render for WebviewPanel {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // The native page needs the window; built after this frame (`create`).
        if matches!(self.page, Page::Pending) {
            self.page = Page::Creating;
            cx.defer_in(window, |this, window, cx| {
                if matches!(this.page, Page::Creating) {
                    this.create(window, cx)
                }
            });
        }
        let theme = cx.theme();
        let body = match &self.page {
            Page::Ready(view) => view.clone().into_any_element(),
            Page::Failed(text) => div()
                .p(fixed::PANEL_PADDING)
                .text_sm()
                .text_color(theme.muted_foreground)
                .child(text.clone())
                .into_any_element(),
            Page::Pending | Page::Creating => div().into_any_element(),
        };
        v_flex()
            .track_focus(&self.focus_handle)
            .size_full()
            .bg(theme.background)
            .child(body)
    }
}

#[cfg(test)]
mod tests {
    // Named imports only: the `gpui_kit::*` glob would shadow the test macro.
    use super::{PageMessage, reserved_keys};

    #[test]
    fn reserved_keys_are_the_terminals_app_keys() {
        let keys = reserved_keys();
        for expected in ["ctrl-p", "ctrl-shift-p", "f1", "ctrl-b", "ctrl-alt-1"] {
            assert!(keys.iter().any(|key| key == expected), "{expected} missing");
        }
        for typed in ["a", "ctrl-c", "ctrl-v", "shift-a", "ctrl-g"] {
            assert!(
                !keys.iter().any(|key| key == typed),
                "{typed} must reach the page"
            );
        }
    }

    #[test]
    fn page_messages_parse() {
        let key: PageMessage =
            serde_json::from_str(r#"{"type":"key","keystroke":"ctrl-p"}"#).unwrap();
        assert_eq!(
            key,
            PageMessage::Key {
                keystroke: "ctrl-p".into()
            }
        );
        assert!(serde_json::from_str::<PageMessage>(r#"{"type":"run","cmd":"rm"}"#).is_err());
    }
}
