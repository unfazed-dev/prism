//! prism-core — scaffold, drift detection, and Haiku enrichment for CLAUDE.md pairs.

pub mod command_runner;
pub mod config;
pub mod enrich;
pub mod hashing;
pub mod hooks;
pub mod icm;
pub mod templates;

#[derive(Debug, thiserror::Error)]
pub enum PrismError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("{0}")]
    Other(String),
}
