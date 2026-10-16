use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum CoreError {
    #[error("{path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("not a directory: {0}")]
    NotADirectory(PathBuf),
    #[error("path is outside the workspace: {0}")]
    OutsideWorkspace(PathBuf),
    #[error("no workspace is open")]
    NoWorkspace,
    #[error("not a UTF-8 text file (binary?): {0}")]
    NotUtf8(PathBuf),
    #[error("offset {0} is out of range or not on a character boundary")]
    InvalidOffset(usize),
    #[error("buffer has no file path; use save_as")]
    NoPath,
    #[error("buffer has unsaved changes")]
    Dirty,
    #[error("no such {0}")]
    NotFound(&'static str),
    #[error("terminal: {0}")]
    Terminal(String),
}

impl CoreError {
    pub(crate) fn io(path: impl Into<PathBuf>, source: std::io::Error) -> Self {
        Self::Io {
            path: path.into(),
            source,
        }
    }
}

pub type Result<T, E = CoreError> = std::result::Result<T, E>;
