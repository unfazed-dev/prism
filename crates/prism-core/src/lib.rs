//! prism-core — doc discovery, drift detection, scaffold, enrichment.

pub mod config;
pub mod discovery;
pub mod document;
pub mod freshness;
pub mod frontmatter;
pub mod hashing;
pub mod hierarchy;

use std::path::{Path, PathBuf};

pub trait FileSystem: Send + Sync {
    fn read_to_string(&self, path: &Path) -> Result<String, PrismError>;
    fn write(&self, path: &Path, contents: &str) -> Result<(), PrismError>;
    fn exists(&self, path: &Path) -> bool;
    fn list_dir(&self, dir: &Path) -> Result<Vec<PathBuf>, PrismError>;
    fn glob(&self, root: &Path, pattern: &str) -> Result<Vec<PathBuf>, PrismError>;
    fn metadata(&self, path: &Path) -> Result<FileMetadata, PrismError>;
}

#[derive(Debug, Clone)]
pub struct FileMetadata {
    pub size_bytes: u64,
    pub modified: Option<chrono::DateTime<chrono::Utc>>,
}

pub trait HashProvider: Send + Sync {
    fn hash_bytes(&self, data: &[u8]) -> String;
    fn hash_file(&self, fs: &dyn FileSystem, path: &Path) -> Result<String, PrismError>;
}

#[derive(Debug, thiserror::Error)]
pub enum PrismError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("YAML error: {0}")]
    Yaml(#[from] serde_yaml_ng::Error),

    #[error("Config error: {message}")]
    Config { message: String },

    #[error("Frontmatter error: {message}")]
    Frontmatter { message: String },

    #[error("Drift detection error: {message}")]
    Drift { message: String },

    #[error("Discovery error: {message}")]
    Discovery { message: String },

    #[error("Document error: {message}")]
    Document { message: String },

    #[error("Hash error: {message}")]
    Hash { message: String },

    #[error("Not found: {path}")]
    NotFound { path: String },

    #[error("{0}")]
    Other(String),
}
