//! The kit editor's language hooks, answered by the buffer's language server
//! through the core: completion, hover and go to definition. Diagnostics are
//! pushed into the editor by the panel (see `EditorPanel::sync_from_core`).

use std::rc::Rc;
use std::sync::Arc;

use carbon_core::lsp::{self, LanguageServer, lsp_types};
use carbon_core::{BufferId, Core, Rope};
use gpui_kit::component::input::{CompletionProvider, DefinitionProvider, HoverProvider};
use gpui_kit::*;
use lsp_types::{
    CompletionContext, CompletionItem, CompletionParams, CompletionResponse, GotoDefinitionParams,
    GotoDefinitionResponse, Hover, HoverParams, LocationLink, TextDocumentIdentifier,
    TextDocumentPositionParams,
};

/// Answers the editor's questions for one buffer.
pub struct LspHooks {
    core: Entity<Core>,
    buffer: BufferId,
}

impl LspHooks {
    pub fn new(core: Entity<Core>, buffer: BufferId) -> Rc<Self> {
        Rc::new(Self { core, buffer })
    }

    /// The server and position for a request at `offset`. Sends pending
    /// edits first: the server must see the text the question is about.
    fn at(
        &self,
        text: &Rope,
        offset: usize,
        cx: &mut App,
    ) -> Option<(Arc<LanguageServer>, TextDocumentPositionParams)> {
        let buffer = self.buffer;
        self.core.update(cx, |core, _| core.lsp_change(buffer));
        let (server, uri) = self.core.read(cx).lsp.document(buffer)?;
        Some((
            server,
            TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri },
                position: lsp::offset_to_position(text, offset),
            },
        ))
    }
}

impl CompletionProvider for LspHooks {
    fn completions(
        &self,
        text: &Rope,
        offset: usize,
        trigger: CompletionContext,
        _: &mut Window,
        cx: &mut App,
    ) -> Task<anyhow::Result<CompletionResponse>> {
        let Some((server, position)) = self.at(text, offset, cx) else {
            return Task::ready(Ok(CompletionResponse::Array(Vec::new())));
        };
        let request = server.request::<lsp_types::request::Completion>(CompletionParams {
            text_document_position: position,
            work_done_progress_params: Default::default(),
            partial_result_params: Default::default(),
            context: Some(trigger),
        });
        let typed = word_before(text, offset);
        cx.background_spawn(async move {
            let items = match request.await? {
                None => Vec::new(),
                Some(CompletionResponse::Array(items)) => items,
                Some(CompletionResponse::List(list)) => list.items,
            };
            Ok(CompletionResponse::Array(narrow(items, &typed)))
        })
    }

    /// Identifier characters and member access (`.`, `::`) ask again.
    fn is_completion_trigger(&self, _: usize, new_text: &str, _: &mut App) -> bool {
        new_text
            .chars()
            .last()
            .is_some_and(|c| c.is_alphanumeric() || matches!(c, '_' | '.' | ':'))
    }
}

impl HoverProvider for LspHooks {
    fn hover(
        &self,
        text: &Rope,
        offset: usize,
        _: &mut Window,
        cx: &mut App,
    ) -> Task<anyhow::Result<Option<Hover>>> {
        let Some((server, position)) = self.at(text, offset, cx) else {
            return Task::ready(Ok(None));
        };
        let request = server.request::<lsp_types::request::HoverRequest>(HoverParams {
            text_document_position_params: position,
            work_done_progress_params: Default::default(),
        });
        cx.background_spawn(async move { Ok(request.await?) })
    }
}

impl DefinitionProvider for LspHooks {
    fn definitions(
        &self,
        text: &Rope,
        offset: usize,
        _: &mut Window,
        cx: &mut App,
    ) -> Task<anyhow::Result<Vec<LocationLink>>> {
        let Some((server, position)) = self.at(text, offset, cx) else {
            return Task::ready(Ok(Vec::new()));
        };
        let request = server.request::<lsp_types::request::GotoDefinition>(GotoDefinitionParams {
            text_document_position_params: position,
            work_done_progress_params: Default::default(),
            partial_result_params: Default::default(),
        });
        cx.background_spawn(async move {
            Ok(match request.await? {
                None => Vec::new(),
                Some(GotoDefinitionResponse::Link(links)) => links,
                Some(GotoDefinitionResponse::Scalar(location)) => vec![link(location)],
                Some(GotoDefinitionResponse::Array(locations)) => {
                    locations.into_iter().map(link).collect()
                }
            })
        })
    }
}

/// Most items shown in the completion menu.
const COMPLETION_LIMIT: usize = 100;

/// The identifier being typed: the word characters right before `offset`.
fn word_before(text: &Rope, offset: usize) -> String {
    let start = offset.saturating_sub(256);
    let start = (start..offset)
        .find(|&ix| text.is_char_boundary(ix))
        .unwrap_or(offset);
    let before = text.slice(start..offset).to_string();
    let word: Vec<char> = before
        .chars()
        .rev()
        .take_while(|c| c.is_alphanumeric() || *c == '_')
        .collect();
    word.into_iter().rev().collect()
}

/// The kit shows items as given, so narrow them to the typed word: items
/// containing its letters in order, those starting with it first, then the
/// server's own order.
fn narrow(items: Vec<CompletionItem>, typed: &str) -> Vec<CompletionItem> {
    let typed = typed.to_lowercase();
    let mut items: Vec<(bool, CompletionItem)> = items
        .into_iter()
        .filter_map(|item| {
            let key = item
                .filter_text
                .as_deref()
                .unwrap_or(&item.label)
                .to_lowercase();
            let mut chars = key.chars();
            let matches = typed.chars().all(|c| chars.any(|k| k == c));
            matches.then(|| (key.starts_with(&typed), item))
        })
        .collect();
    items.sort_by(|(a_prefix, a), (b_prefix, b)| {
        b_prefix.cmp(a_prefix).then_with(|| {
            let key = |item: &CompletionItem| item.sort_text.clone().unwrap_or(item.label.clone());
            key(a).cmp(&key(b))
        })
    });
    items
        .into_iter()
        .take(COMPLETION_LIMIT)
        .map(|(_, item)| item)
        .collect()
}

fn link(location: lsp_types::Location) -> LocationLink {
    LocationLink {
        origin_selection_range: None,
        target_uri: location.uri,
        target_range: location.range,
        target_selection_range: location.range,
    }
}
