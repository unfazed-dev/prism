//! PostToolUse hook — runs after Write/Edit tool calls.
//!
//! Responsibilities:
//! 1. Update the `file_hashes` row for the written file
//! 2. Enqueue an ENRICH directive when a CLAUDE.md / CONTEXT.md pair lives in
//!    the edited file's directory but has placeholder content

use std::path::Path;

use crate::hashing::hash_file;
use crate::hooks::protocol::{extract_file_path, HookContext, HookInput, HookOutput};
use crate::hooks::HookError;
use prism_db::{directive_log, PrismDb};

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

    if let Ok(bytes) = std::fs::read(&abs_path) {
        let hash = hash_file(&bytes).hex;
        db.upsert_file_hash(&rel_path, &hash)?;
    }

    if let Some(parent) = abs_path.parent() {
        let rel_dir = parent
            .strip_prefix(&ctx.project_root)
            .unwrap_or(parent)
            .to_string_lossy()
            .to_string();
        let claude_path = parent.join("CLAUDE.md");
        if claude_path.exists() {
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
                        target_path: rel_dir.clone(),
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
        }
    }

    Ok(HookOutput::allow(None))
}
