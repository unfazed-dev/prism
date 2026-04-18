//! PRISM template rendering, scaffolding, and content generation.
//!
//! This crate bundles all PRISM template assets and provides:
//! - Template rendering via minijinja
//! - Directory scaffolding (CLAUDE.md + CONTEXT.md pairs)
//! - Content generation for routing tables, entity lists, etc.
//! - Frontmatter validation for L3b+ documents

pub mod content;
pub mod registry;
pub mod render;
pub mod scaffold;
pub mod validate;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum TemplateError {
    #[error("template rendering failed: {0}")]
    RenderError(String),

    #[error("template not found: {0}")]
    NotFound(String),

    #[error("invalid frontmatter: {0}")]
    InvalidFrontmatter(String),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("serialization error: {0}")]
    Serialization(String),
}
