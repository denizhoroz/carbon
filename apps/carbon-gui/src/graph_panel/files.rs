//! A graph panel without a file: the kind's files in the project (its `dir`,
//! by `suffix`), each opening here, and New, which makes one from the
//! extension's template (or with the one block a graph must start with).

use std::path::{Path, PathBuf};

use carbon_core::extension::GraphKind;
use carbon_core::graph::{GraphDoc, Pos, from_yaml, write_file};
use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::{ActiveTheme, Icon, Sizable, WindowExt as _, h_flex, v_flex};
use gpui_kit::*;

use super::GraphPanel;
use crate::style::fixed;

fn workspace_root(cx: &App) -> Option<PathBuf> {
    crate::workspace_ops::current_core(cx)
        .read(cx)
        .workspace()
        .map(|w| w.root().to_path_buf())
}

/// The kind's files in `dir`, sorted by name.
fn list(dir: &Path, suffix: &str) -> Vec<PathBuf> {
    let ending = format!(".{}", suffix.to_ascii_lowercase());
    let mut files: Vec<PathBuf> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| {
            path.is_file()
                && path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .is_some_and(|n| n.to_ascii_lowercase().ends_with(&ending))
        })
        .collect();
    files.sort();
    files
}

pub(super) fn render_files(
    panel: &GraphPanel,
    kind: &GraphKind,
    cx: &mut Context<GraphPanel>,
) -> AnyElement {
    let theme = cx.theme();
    let muted = theme.muted_foreground;
    let Some(root) = workspace_root(cx) else {
        return super::message(
            format!("Open a folder to keep {} in it.", kind.spec.name),
            cx,
        );
    };
    let dir = root.join(&kind.spec.dir);
    let files = list(&dir, &kind.spec.suffix);
    let rel = kind.spec.dir.clone();
    let _ = panel;
    v_flex()
        .size_full()
        .p(fixed::PANEL_PADDING)
        .gap(fixed::PANEL_GAP)
        .child(
            h_flex()
                .justify_between()
                .child(
                    div()
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(kind.spec.name.clone()),
                )
                .child(
                    Button::new("graph-new")
                        .label("New")
                        .icon(IconName::Plus)
                        .small()
                        .primary()
                        .on_click(cx.listener(|this, _, window, cx| this.new_file(window, cx))),
                ),
        )
        .child(
            div()
                .text_xs()
                .text_color(muted)
                .child(format!("Files in {rel}/ of the open folder")),
        )
        .children(if files.is_empty() {
            vec![
                div()
                    .text_sm()
                    .text_color(muted)
                    .child("None yet. New makes one.")
                    .into_any_element(),
            ]
        } else {
            files
                .into_iter()
                .enumerate()
                .map(|(ix, path)| {
                    let name = path
                        .file_name()
                        .map(|n| n.to_string_lossy().into_owned())
                        .unwrap_or_default();
                    h_flex()
                        .id(("graph-file", ix))
                        .gap_2()
                        .px_2()
                        .py_1()
                        .rounded(px(4.))
                        .cursor_pointer()
                        .hover(|row| row.bg(cx.theme().list_hover))
                        .child(Icon::new(IconName::File).small().text_color(muted))
                        .child(div().text_sm().child(name))
                        .on_click(cx.listener(move |this, _, _, cx| this.open(path.clone(), cx)))
                        .into_any_element()
                })
                .collect()
        })
        .into_any_element()
}

impl GraphPanel {
    /// Makes a new graph file (`<dir>/<graph id>.<suffix>`, numbered when
    /// taken) and opens it.
    fn new_file(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let registry = crate::extensions::registry(cx);
        let Some(kind) = self.kind(&registry) else {
            return;
        };
        let Some(root) = workspace_root(cx) else {
            return;
        };
        let dir = root.join(&kind.spec.dir);
        let path = (1..)
            .map(|n| {
                let stem = if n == 1 {
                    kind.spec.id.clone()
                } else {
                    format!("{}-{n}", kind.spec.id)
                };
                dir.join(format!("{stem}.{}", kind.spec.suffix))
            })
            .find(|p| !p.exists())
            .expect("some number is free");
        let doc = match new_doc(&kind) {
            Ok(doc) => doc,
            Err(err) => {
                window.push_notification(format!("Cannot make a new file: {err}"), cx);
                return;
            }
        };
        if let Err(err) = write_file(&path, &doc) {
            window.push_notification(format!("Cannot make a new file: {err}"), cx);
            return;
        }
        self.open(path, cx);
        self.fit_pending = true;
    }
}

/// A new graph: the template, else the starting block (a type with no
/// inputs that a graph has at most one of).
fn new_doc(kind: &GraphKind) -> Result<GraphDoc, String> {
    let name = format!("New {}", kind.spec.name.to_lowercase());
    if let Some(template) = &kind.spec.template {
        let file = kind.loaded.file(template)?;
        let text = std::fs::read_to_string(&file).map_err(|err| err.to_string())?;
        let mut doc = from_yaml(&text)?;
        doc.kind = kind.kind();
        if doc.name.trim().is_empty() {
            doc.name = name;
        }
        return Ok(doc);
    }
    let mut doc = GraphDoc::new(kind.kind(), name);
    if let Some(start) = kind
        .node_types
        .iter()
        .find(|t| t.inputs.is_empty() && t.max_count == Some(1))
    {
        let id = doc.next_node_id(&start.id);
        doc.nodes.insert(id.clone(), start.new_node());
        doc.layout.nodes.insert(id, Pos::new(80., 160.));
    }
    Ok(doc)
}
