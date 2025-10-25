use std::path::PathBuf;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum SaccadeError {
    #[error("I/O error: {source} (path: {path})")]
    Io {
        source: std::io::Error,
        path: PathBuf,
    },

    #[error("Invalid configuration: {field} = {value} ({reason})")]
    InvalidConfig {
        field: String,
        value: String,
        reason: String,
    },

    #[error("File too large: {path} ({size} bytes, max {max})")]
    FileTooLarge { path: PathBuf, size: u64, max: u64 },

    #[error("Git not available but --git-only was requested")]
    GitRequired,

    #[error("Not inside a Git repository")]
    NotInGitRepo,

    #[error("Repomix failed: {stderr}")]
    RepomixFailed { stderr: String },

    #[error("JSON serialization error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Regex error: {0}")]
    Regex(#[from] regex::Error),

    #[error("Generic error: {0}")]
    Other(String),
}

pub type Result<T> = std::result::Result<T, SaccadeError>;

// Allow `?` on std::io::Error by converting to SaccadeError::Io with unknown path.
// Call sites that *know* the path should still map_err with a real path where possible.
impl From<std::io::Error> for SaccadeError {
    fn from(source: std::io::Error) -> Self {
        SaccadeError::Io {
            source,
            path: PathBuf::from("<unknown>"),
        }
    }
}

// Gracefully convert WalkDir errors to a generic error.
// (They don't always carry a clean io::Error we can extract.)
impl From<walkdir::Error> for SaccadeError {
    fn from(e: walkdir::Error) -> Self {
        SaccadeError::Other(e.to_string())
    }
}