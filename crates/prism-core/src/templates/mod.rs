//! PRISM template rendering + directory scaffolding.

pub mod registry;
pub mod render;
pub mod scaffold;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum TemplateError {
    #[error("template rendering failed: {0}")]
    RenderError(String),

    #[error("template not found: {0}")]
    NotFound(String),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
}
