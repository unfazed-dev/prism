//! SessionStart hook — runs once at the beginning of each Claude session.
//!
//! Responsibilities:
//! 1. Emit a brief status message (drift count + pending enrichment count)
//! 2. Hand off — CLI / skills drive scaffolding explicitly

use crate::hooks::protocol::{HookContext, HookOutput};
use crate::hooks::HookError;
use prism_db::{directive_log, PrismDb};

pub fn run(ctx: &HookContext) -> Result<HookOutput, HookError> {
    let db_path = ctx.project_root.join(".prism/prism.db");
    if !db_path.exists() {
        return Ok(HookOutput::allow(None));
    }

    let db = PrismDb::open(&db_path)?;

    let unresolved_drift = db.list_unresolved_drift()?;
    let pending_enrich = directive_log::count_by_state(
        db.conn(),
        directive_log::KIND_ENRICH,
        directive_log::STATE_PENDING,
    )?;

    let msg = if unresolved_drift.is_empty() && pending_enrich == 0 {
        None
    } else {
        Some(format!(
            "PRISM: {} unresolved drift, {} pending enrichment",
            unresolved_drift.len(),
            pending_enrich
        ))
    };

    Ok(HookOutput::allow(msg))
}
