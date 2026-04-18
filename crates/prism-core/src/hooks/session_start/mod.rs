//! SessionStart hook — runs once at the beginning of each Claude session.
//!
//! Responsibilities:
//! 1. Emit a brief status message (drift count + pending enrichment count)
//! 2. Hand off — CLI / skills drive scaffolding explicitly

use crate::hooks::protocol::{HookContext, HookOutput};
use crate::hooks::HookError;
use prism_db::{directive_log, doc_drift, PrismDb};

pub fn run(ctx: &HookContext) -> Result<HookOutput, HookError> {
    let db_path = ctx.project_root.join(".prism/prism.db");
    if !db_path.exists() {
        return Ok(HookOutput::allow(None));
    }

    let db = PrismDb::open(&db_path)?;

    let source_drift =
        doc_drift::count_unresolved_by_type(db.conn(), doc_drift::DRIFT_TYPE_OUTDATED)?;
    let icm_violations =
        doc_drift::count_unresolved_by_type(db.conn(), doc_drift::DRIFT_TYPE_ICM)?;
    let pending_enrich = directive_log::count_by_state(
        db.conn(),
        directive_log::KIND_ENRICH,
        directive_log::STATE_PENDING,
    )?;
    let pending_fix = directive_log::count_by_state(
        db.conn(),
        directive_log::KIND_FIX_ICM,
        directive_log::STATE_PENDING,
    )?;

    let any = source_drift + icm_violations + pending_enrich + pending_fix;
    let msg = if any == 0 {
        None
    } else {
        Some(format!(
            "PRISM: {source_drift} source drift, {icm_violations} ICM violation(s), \
             {pending_enrich} pending enrich, {pending_fix} pending fix"
        ))
    };

    Ok(HookOutput::allow(msg))
}
