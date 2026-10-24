//! Language servers: which server a file gets, the running servers, the
//! documents they have open, and the diagnostics they publish.
//!
//! The core holds the state and sends document notifications; the UI drives
//! the async parts (initializing a server, reading its notifications) and
//! calls back in here. Which servers exist comes from extensions
//! ([`ServerTable`], built from the extension registry); none are built in.

mod transport;
mod uri;

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::sync::Arc;

pub use lsp_types;
use lsp_types::notification::{
    DidChangeTextDocument, DidCloseTextDocument, DidOpenTextDocument, DidSaveTextDocument,
};
use lsp_types::{
    DidChangeTextDocumentParams, DidCloseTextDocumentParams, DidOpenTextDocumentParams,
    DidSaveTextDocumentParams, TextDocumentContentChangeEvent, TextDocumentIdentifier,
    TextDocumentItem, VersionedTextDocumentIdentifier,
};
pub use transport::{LanguageServer, ServerNotification};
pub use uri::{path_to_uri, uri_to_path};

use ropey::{LineType, Rope};

use crate::{Buffer, BufferId, Result};

/// A language server Carbon knows how to start (from an extension).
#[derive(Clone, Debug, PartialEq)]
pub struct ServerConfig {
    /// LSP language id, also the key of the running server.
    pub language: String,
    /// Shown in the status bar.
    pub name: String,
    /// Program name, looked up on `PATH`.
    pub command: String,
    pub args: Vec<String>,
    pub env: BTreeMap<String, String>,
    /// How to get the server, shown when it is missing.
    pub install_hint: String,
    /// The extension that declared it, and the server's id there.
    pub extension: String,
    pub server_id: String,
    /// A program path the user set; wins over `PATH`.
    pub program: Option<PathBuf>,
    /// A copy downloaded earlier; used when the others are not found.
    pub downloaded: Option<PathBuf>,
    /// The build the extension offers for this computer, if any.
    pub download: Option<ServerDownload>,
}

/// What a download of a server would fetch (for the prompt).
#[derive(Clone, Debug, PartialEq)]
pub struct ServerDownload {
    pub version: String,
    pub host: String,
}

impl ServerConfig {
    /// The program to start: the user's path, else `command` on `PATH`,
    /// else the downloaded copy.
    pub fn resolve(&self) -> Option<PathBuf> {
        let file = |path: &Option<PathBuf>| path.clone().filter(|path| path.is_file());
        file(&self.program)
            .or_else(|| find_program(&self.command))
            .or_else(|| file(&self.downloaded))
    }
}

/// Which language a file is, by exact name or suffix.
#[derive(Clone, Debug, PartialEq)]
pub struct LanguageRule {
    pub language: String,
    /// Lowercase endings without the dot (`rs`, `d.ts`).
    pub suffixes: Vec<String>,
    pub file_names: Vec<String>,
}

/// The languages and servers extensions declare. One server per language:
/// the first declared wins.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ServerTable {
    pub languages: Vec<LanguageRule>,
    pub servers: Vec<Arc<ServerConfig>>,
}

impl ServerTable {
    /// The language of a file: an exact file name first, then the longest
    /// matching suffix (case-insensitive).
    pub fn language_for_path(&self, path: &Path) -> Option<&str> {
        let name = path.file_name()?.to_str()?;
        if let Some(rule) = self
            .languages
            .iter()
            .find(|rule| rule.file_names.iter().any(|file| file == name))
        {
            return Some(&rule.language);
        }
        let lower = name.to_ascii_lowercase();
        self.languages
            .iter()
            .flat_map(|rule| rule.suffixes.iter().map(move |suffix| (rule, suffix)))
            .filter(|(_, suffix)| {
                lower
                    .strip_suffix(suffix.to_ascii_lowercase().as_str())
                    .is_some_and(|rest| rest.ends_with('.'))
            })
            .max_by_key(|(_, suffix)| suffix.len())
            .map(|(rule, _)| rule.language.as_str())
    }

    /// The server for a language.
    pub fn server(&self, language: &str) -> Option<&Arc<ServerConfig>> {
        self.servers
            .iter()
            .find(|config| config.language == language)
    }

    /// The server for a file, by its language.
    pub fn server_for_path(&self, path: &Path) -> Option<&Arc<ServerConfig>> {
        self.server(self.language_for_path(path)?)
    }
}

/// Byte offset → LSP position (columns in UTF-16 units, the LSP default).
pub fn offset_to_position(rope: &Rope, offset: usize) -> lsp_types::Position {
    let offset = offset.min(rope.len());
    let line = rope.byte_to_line_idx(offset, LineType::LF_CR);
    let start = rope.line_to_byte_idx(line, LineType::LF_CR);
    let column = rope.slice(start..offset).len_utf16();
    lsp_types::Position::new(line as u32, column as u32)
}

/// LSP position → byte offset, clamped to the text.
pub fn position_to_offset(rope: &Rope, position: lsp_types::Position) -> usize {
    let lines = rope.len_lines(LineType::LF_CR);
    let line = (position.line as usize).min(lines.saturating_sub(1));
    let start = rope.line_to_byte_idx(line, LineType::LF_CR);
    let text = rope.line(line, LineType::LF_CR);
    let column = (position.character as usize).min(text.len_utf16());
    start + text.utf16_to_byte_idx(column)
}

/// Sends `initialize` (what Carbon can handle) and then `initialized`.
pub fn initialize(
    server: Arc<LanguageServer>,
    root: &Path,
) -> impl Future<Output = Result<lsp_types::InitializeResult>> + Send + 'static {
    use lsp_types::*;
    let markup = Some(vec![MarkupKind::Markdown, MarkupKind::PlainText]);
    let params = InitializeParams {
        process_id: Some(std::process::id()),
        workspace_folders: Some(vec![WorkspaceFolder {
            uri: path_to_uri(root),
            name: root
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_default(),
        }]),
        capabilities: ClientCapabilities {
            general: Some(GeneralClientCapabilities {
                position_encodings: Some(vec![PositionEncodingKind::UTF16]),
                ..Default::default()
            }),
            window: Some(WindowClientCapabilities {
                work_done_progress: Some(true),
                ..Default::default()
            }),
            text_document: Some(TextDocumentClientCapabilities {
                synchronization: Some(TextDocumentSyncClientCapabilities {
                    did_save: Some(true),
                    ..Default::default()
                }),
                completion: Some(CompletionClientCapabilities {
                    completion_item: Some(CompletionItemCapability {
                        snippet_support: Some(false),
                        documentation_format: markup.clone(),
                        ..Default::default()
                    }),
                    ..Default::default()
                }),
                hover: Some(HoverClientCapabilities {
                    content_format: markup,
                    ..Default::default()
                }),
                definition: Some(GotoCapability {
                    link_support: Some(true),
                    ..Default::default()
                }),
                publish_diagnostics: Some(PublishDiagnosticsClientCapabilities::default()),
                ..Default::default()
            }),
            ..Default::default()
        },
        ..Default::default()
    };
    async move {
        let result = server.request::<request::Initialize>(params).await?;
        server.notify::<notification::Initialized>(InitializedParams {})?;
        Ok(result)
    }
}

/// Finds `command` on `PATH` (with `.exe` on Windows).
pub fn find_program(command: &str) -> Option<PathBuf> {
    let name = if cfg!(windows) {
        format!("{command}.exe")
    } else {
        command.to_string()
    };
    std::env::split_paths(&std::env::var_os("PATH")?)
        .map(|dir| dir.join(&name))
        .find(|path| path.is_file())
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LspStatus {
    Starting,
    /// Working on something it reported (`$/progress`), e.g. "Indexing".
    Busy(String),
    Ready,
    /// Not installed; the text says how to get it.
    Missing(String),
    Failed(String),
}

pub struct ServerEntry {
    pub config: Arc<ServerConfig>,
    pub status: LspStatus,
    server: Option<Arc<LanguageServer>>,
    /// Progress tokens the server reported as begun and not yet ended.
    pub progress: HashMap<String, String>,
}

impl ServerEntry {
    pub fn server(&self) -> Option<&Arc<LanguageServer>> {
        self.server.as_ref()
    }
}

struct Document {
    language: String,
    uri: lsp_types::Uri,
    version: i32,
    /// Buffer revision the server last got.
    sent_revision: u64,
}

/// Diagnostics of one file, as the server published them.
#[derive(Clone, Debug, Default)]
pub struct FileDiagnostics {
    pub items: Vec<lsp_types::Diagnostic>,
    /// Bumped on every publish, so views can tell they are stale.
    pub generation: u64,
}

#[derive(Default)]
pub struct LspStore {
    table: ServerTable,
    servers: HashMap<String, ServerEntry>,
    documents: HashMap<BufferId, Document>,
    diagnostics: HashMap<PathBuf, FileDiagnostics>,
    generation: u64,
}

impl LspStore {
    /// Forgets running servers, documents and diagnostics (another folder
    /// opened); the table stays.
    pub fn reset(&mut self) {
        *self = Self {
            table: std::mem::take(&mut self.table),
            ..Self::default()
        };
    }

    pub fn table(&self) -> &ServerTable {
        &self.table
    }

    /// Replaces the table (extensions changed). Servers whose config is gone
    /// or changed stop (dropping the last handle ends the process), with their
    /// documents; the UI starts new ones as files need them.
    pub fn set_table(&mut self, table: ServerTable) {
        let stale: Vec<String> = self
            .servers
            .iter()
            .filter(|(language, entry)| table.server(language) != Some(&entry.config))
            .map(|(language, _)| language.clone())
            .collect();
        for language in &stale {
            self.servers.remove(language);
        }
        self.documents
            .retain(|_, document| !stale.contains(&document.language));
        self.table = table;
    }

    /// The server for a file, if an extension declares one.
    pub fn server_for_path(&self, path: &Path) -> Option<Arc<ServerConfig>> {
        self.table.server_for_path(path).cloned()
    }

    pub fn entry(&self, language: &str) -> Option<&ServerEntry> {
        self.servers.get(language)
    }

    pub fn entries(&self) -> impl Iterator<Item = &ServerEntry> {
        self.servers.values()
    }

    /// Forgets a language's server (stopping it), so the next file of that
    /// language starts it again (after a download).
    pub fn forget(&mut self, language: &str) {
        self.servers.remove(language);
        self.documents
            .retain(|_, document| document.language != language);
    }

    /// Starts the server for `config` in `root` unless one was tried
    /// already. Returns the new server, which the caller must initialize.
    pub fn start(&mut self, config: Arc<ServerConfig>, root: &Path) -> Option<Arc<LanguageServer>> {
        if self.servers.contains_key(&config.language) {
            return None;
        }
        let (status, server) = match config.resolve() {
            None => (
                LspStatus::Missing(if config.install_hint.is_empty() {
                    format!("{} is not installed", config.name)
                } else {
                    format!(
                        "{} is not installed (install: {})",
                        config.name, config.install_hint
                    )
                }),
                None,
            ),
            Some(program) => {
                let args: Vec<&str> = config.args.iter().map(String::as_str).collect();
                match LanguageServer::spawn(&config.name, &program, &args, &config.env, root) {
                    Ok(server) => (LspStatus::Starting, Some(server)),
                    Err(err) => (LspStatus::Failed(err.to_string()), None),
                }
            }
        };
        self.servers.insert(
            config.language.clone(),
            ServerEntry {
                config,
                status,
                server: server.clone(),
                progress: HashMap::new(),
            },
        );
        server
    }

    pub fn set_status(&mut self, language: &str, status: LspStatus) {
        if let Some(entry) = self.servers.get_mut(language) {
            if matches!(status, LspStatus::Failed(_)) {
                entry.server = None;
            }
            entry.status = status;
        }
    }

    pub fn entry_mut(&mut self, language: &str) -> Option<&mut ServerEntry> {
        self.servers.get_mut(language)
    }

    /// The running, initialized server for a language.
    pub fn ready_server(&self, language: &str) -> Option<Arc<LanguageServer>> {
        let entry = self.servers.get(language)?;
        match entry.status {
            LspStatus::Ready | LspStatus::Busy(_) => entry.server.clone(),
            _ => None,
        }
    }

    /// The server and document uri for a buffer the server has open.
    pub fn document(&self, id: BufferId) -> Option<(Arc<LanguageServer>, lsp_types::Uri)> {
        let document = self.documents.get(&id)?;
        Some((self.ready_server(&document.language)?, document.uri.clone()))
    }

    /// Opens `buffer` in its ready server, if it has one and is not open yet.
    pub fn open_document(&mut self, buffer: &Buffer) -> Result<()> {
        if self.documents.contains_key(&buffer.id()) {
            return Ok(());
        }
        let Some(path) = buffer.path() else {
            return Ok(());
        };
        let Some(config) = self.server_for_path(path) else {
            return Ok(());
        };
        let Some(server) = self.ready_server(&config.language) else {
            return Ok(());
        };
        let uri = path_to_uri(path);
        server.notify::<DidOpenTextDocument>(DidOpenTextDocumentParams {
            text_document: TextDocumentItem {
                uri: uri.clone(),
                language_id: config.language.clone(),
                version: 1,
                text: buffer.text(),
            },
        })?;
        self.documents.insert(
            buffer.id(),
            Document {
                language: config.language.clone(),
                uri,
                version: 1,
                sent_revision: buffer.revision(),
            },
        );
        Ok(())
    }

    /// Sends the whole text if it changed since the server last got it.
    pub fn change_document(&mut self, buffer: &Buffer) -> Result<()> {
        let Some(document) = self.documents.get_mut(&buffer.id()) else {
            return Ok(());
        };
        if document.sent_revision == buffer.revision() {
            return Ok(());
        }
        let Some(entry) = self.servers.get(&document.language) else {
            return Ok(());
        };
        let Some(server) = entry.server.as_ref() else {
            return Ok(());
        };
        document.version += 1;
        document.sent_revision = buffer.revision();
        server.notify::<DidChangeTextDocument>(DidChangeTextDocumentParams {
            text_document: VersionedTextDocumentIdentifier {
                uri: document.uri.clone(),
                version: document.version,
            },
            content_changes: vec![TextDocumentContentChangeEvent {
                range: None,
                range_length: None,
                text: buffer.text(),
            }],
        })
    }

    pub fn save_document(&mut self, buffer: &Buffer) -> Result<()> {
        self.change_document(buffer)?;
        let Some((server, uri)) = self.document(buffer.id()) else {
            return Ok(());
        };
        server.notify::<DidSaveTextDocument>(DidSaveTextDocumentParams {
            text_document: TextDocumentIdentifier { uri },
            text: None,
        })
    }

    pub fn close_document(&mut self, id: BufferId) {
        let Some(document) = self.documents.remove(&id) else {
            return;
        };
        if let Some(server) = self
            .servers
            .get(&document.language)
            .and_then(|e| e.server.as_ref())
        {
            _ = server.notify::<DidCloseTextDocument>(DidCloseTextDocumentParams {
                text_document: TextDocumentIdentifier { uri: document.uri },
            });
        }
    }

    /// Stores a publish; returns the error and warning counts for markers.
    pub fn set_diagnostics(
        &mut self,
        path: PathBuf,
        items: Vec<lsp_types::Diagnostic>,
    ) -> (u32, u32) {
        use lsp_types::DiagnosticSeverity;
        let count = |severity| {
            items
                .iter()
                .filter(|item| item.severity == Some(severity))
                .count() as u32
        };
        let counts = (
            count(DiagnosticSeverity::ERROR),
            count(DiagnosticSeverity::WARNING),
        );
        self.generation += 1;
        self.diagnostics.insert(
            path,
            FileDiagnostics {
                items,
                generation: self.generation,
            },
        );
        counts
    }

    pub fn diagnostics(&self, path: &Path) -> Option<&FileDiagnostics> {
        self.diagnostics.get(path)
    }
}
