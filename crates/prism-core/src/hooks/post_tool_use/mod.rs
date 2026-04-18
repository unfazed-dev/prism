//! PostToolUse hook — runs after Write/Edit tool calls.
//!
//! Responsibilities:
//! 1. Update the `file_hashes` row for the written file
//! 2. Walk ancestor dirs to nearest registered CLAUDE.md; record drift and
//!    enqueue an ENRICH directive targeting that dir.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use crate::hashing::hash_file;
use crate::hooks::protocol::{extract_file_path, HookContext, HookInput, HookOutput};
use crate::hooks::HookError;
use prism_db::{directive_log, doc_drift, document_registry, PrismDb};

pub fn run(input: &HookInput, ctx: &HookContext) -> Result<HookOutput, HookError> {
    let Some(tool_input) = input.tool_input.as_ref() else {
        return Ok(HookOutput::allow(None));
    };
    let Some(rel_path) = extract_file_path(tool_input) else {
        return Ok(HookOutput::allow(None));
    };

    let db_path = ctx.project_root.join(".prism/prism.db");
    if !db_path.exists() {
        return Ok(HookOutput::allow(None));
    }

    let db = PrismDb::open(&db_path)?;
    let abs_path = if Path::new(&rel_path).is_absolute() {
        Path::new(&rel_path).to_path_buf()
    } else {
        ctx.project_root.join(&rel_path)
    };

    let mut content_changed = false;
    if let Ok(bytes) = std::fs::read(&abs_path) {
        let new_hash = hash_file(&bytes).hex;
        let prior = db.get_file_hash(&rel_path)?;
        content_changed = prior.as_ref().map(|r| r.hash != new_hash).unwrap_or(false);
        db.upsert_file_hash(&rel_path, &new_hash)?;
    }

    // Skip re-entrancy: edits to managed docs themselves should not trigger drift.
    if rel_path.ends_with("CLAUDE.md") || rel_path.ends_with("CONTEXT.md") {
        return Ok(HookOutput::allow(None));
    }

    let registered_claude_dirs = load_registered_claude_dirs(&db)?;
    let Some(parent) = abs_path.parent() else {
        return Ok(HookOutput::allow(None));
    };

    let Some(target_dir) =
        nearest_registered_ancestor(parent, &ctx.project_root, &registered_claude_dirs)
    else {
        return Ok(HookOutput::allow(None));
    };

    let rel_dir = target_dir
        .strip_prefix(&ctx.project_root)
        .unwrap_or(&target_dir)
        .to_string_lossy()
        .to_string();
    let rel_dir = if rel_dir.is_empty() { ".".to_string() } else { rel_dir };
    let affected_doc = if rel_dir == "." {
        "CLAUDE.md".to_string()
    } else {
        format!("{}/CLAUDE.md", rel_dir)
    };

    if content_changed {
        doc_drift::insert(
            db.conn(),
            &doc_drift::DocDriftRow {
                drift_id: None,
                session_id: ctx.session_id.clone(),
                detected_turn: 0,
                affected_doc,
                drift_type: "OutdatedContextFile".to_string(),
                severity: "warning".to_string(),
                description: format!("Source edited: {}", rel_path),
                resolved: false,
                resolved_by: None,
                resolved_at: None,
            },
        )?;
    }
    let already_queued = matches!(
        directive_log::latest_for_target(db.conn(), &rel_dir, directive_log::KIND_ENRICH)?,
        Some(row) if row.state == directive_log::STATE_PENDING
    );
    if !already_queued {
        directive_log::insert(
            db.conn(),
            &directive_log::DirectiveLogRow {
                id: None,
                kind: directive_log::KIND_ENRICH.into(),
                target_path: rel_dir,
                session_id: ctx.session_id.clone(),
                emitted_at: chrono::Utc::now().timestamp(),
                completed_at: None,
                retry_count: 0,
                state: directive_log::STATE_PENDING.into(),
                source: directive_log::SOURCE_DIRECTIVE.into(),
                priority: directive_log::priority::NORMAL,
            },
        )?;
    }

    Ok(HookOutput::allow(None))
}

/// Collect absolute parent dirs of every registered CLAUDE.md in the registry.
fn load_registered_claude_dirs(db: &PrismDb) -> Result<HashSet<PathBuf>, HookError> {
    let rows = document_registry::list_all(db.conn())?;
    let mut out = HashSet::new();
    for row in rows {
        if !row.doc_id.ends_with("CLAUDE.md") {
            continue;
        }
        if let Some(parent_dir) = row.parent_dir.as_ref() {
            out.insert(PathBuf::from(parent_dir));
        }
    }
    Ok(out)
}

/// Walk ancestors from `start` up to (and including) `project_root`; return the
/// first ancestor that matches a registered CLAUDE.md parent dir.
fn nearest_registered_ancestor(
    start: &Path,
    project_root: &Path,
    registered: &HashSet<PathBuf>,
) -> Option<PathBuf> {
    let mut cur: Option<&Path> = Some(start);
    while let Some(dir) = cur {
        if registered.contains(dir) {
            return Some(dir.to_path_buf());
        }
        if dir == project_root {
            break;
        }
        cur = dir.parent();
    }
    None
}
