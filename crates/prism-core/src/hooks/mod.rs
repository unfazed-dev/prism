//! Claude Code lifecycle hook handlers.
//!
//! Two events wired in v2:
//! - `SessionStart` — discover drift, enqueue enrichment, scaffold missing pairs
//! - `PostToolUse` — update file hashes, enqueue enrichment for edits

pub mod post_tool_use;
pub mod protocol;
pub mod session_start;

/// Typed error for hook handlers.
#[derive(Debug, thiserror::Error)]
pub enum HookError {
    #[error("database error: {0}")]
    Db(#[from] prism_db::DbError),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("protocol error: {0}")]
    Protocol(String),

    #[error("parse error: {0}")]
    Parse(String),
}

impl From<serde_json::Error> for HookError {
    fn from(e: serde_json::Error) -> Self {
        HookError::Parse(e.to_string())
    }
}

impl From<crate::PrismError> for HookError {
    fn from(e: crate::PrismError) -> Self {
        match e {
            crate::PrismError::Io(io) => HookError::Io(io),
            other => HookError::Protocol(other.to_string()),
        }
    }
}
