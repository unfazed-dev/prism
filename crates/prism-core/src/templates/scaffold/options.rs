//! Scaffold options and template-context construction.
//!
//! Split out of `mod.rs` during the prism-churn Phase 3 refactor. Groups the
//! caller-facing configuration type (`ScaffoldOptions`) and the internal
//! helper that turns those options into a minijinja template context.

use std::collections::HashMap;
use std::path::Path;

/// Options for scaffolding a directory.
#[derive(Debug, Clone, Default)]
pub struct ScaffoldOptions {
    /// Name of the directory being scaffolded.
    pub directory_name: String,
    /// Purpose/description of the directory.
    pub description: Option<String>,
    /// Whether this is the project root (uses root templates vs dir templates).
    pub is_root: bool,
    /// Project name (for root scaffolding).
    pub project_name: Option<String>,
    /// Project description (for root scaffolding).
    pub project_description: Option<String>,
    /// Tech stack items as a list (for root CLAUDE.md template iteration).
    pub tech_stack: Option<Vec<String>>,
    /// Human-readable purpose statement (for dir CLAUDE.md).
    pub purpose: Option<String>,
    /// Key files list: [{name, description}] for template iteration.
    pub key_files: Option<Vec<HashMap<String, String>>>,
    /// Key subdirectories: [{name, description, has_context_file}] for template iteration.
    pub key_subdirs: Option<Vec<HashMap<String, String>>>,
    /// Human-readable dependencies summary.
    pub dependencies_list: Option<String>,
    /// Skip CONTEXT.md generation (useful when CONTEXT.md is managed by session hooks).
    pub skip_context: bool,
}

/// Build template context from ScaffoldOptions.
///
/// Uses `serde_json::Value` so templates can iterate arrays via `{% for f in key_files %}`.
pub(super) fn build_context(
    target_dir: &Path,
    options: &ScaffoldOptions,
) -> HashMap<String, serde_json::Value> {
    let mut ctx: HashMap<String, serde_json::Value> = HashMap::new();

    ctx.insert(
        "directory_name".into(),
        serde_json::Value::String(options.directory_name.clone()),
    );
    ctx.insert(
        "directory_path".into(),
        serde_json::Value::String(target_dir.to_string_lossy().to_string()),
    );

    if let Some(ref desc) = options.description {
        ctx.insert(
            "directory_description".into(),
            serde_json::Value::String(desc.clone()),
        );
    }
    if let Some(ref name) = options.project_name {
        ctx.insert(
            "project_name".into(),
            serde_json::Value::String(name.clone()),
        );
    }
    if let Some(ref desc) = options.project_description {
        ctx.insert(
            "project_description".into(),
            serde_json::Value::String(desc.clone()),
        );
    }
    if let Some(ref ts) = options.tech_stack {
        ctx.insert(
            "tech_stack".into(),
            serde_json::to_value(ts).unwrap_or_default(),
        );
    }
    if let Some(ref p) = options.purpose {
        ctx.insert("purpose".into(), serde_json::Value::String(p.clone()));
    }
    if let Some(ref kf) = options.key_files {
        ctx.insert(
            "key_files".into(),
            serde_json::to_value(kf).unwrap_or_default(),
        );
    }
    if let Some(ref ks) = options.key_subdirs {
        ctx.insert(
            "key_subdirs".into(),
            serde_json::to_value(ks).unwrap_or_default(),
        );
    }
    if let Some(ref dl) = options.dependencies_list {
        ctx.insert(
            "dependencies_list".into(),
            serde_json::Value::String(dl.clone()),
        );
    }

    ctx
}
