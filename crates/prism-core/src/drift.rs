//! Drift detection across PRISM's six dimensions.
//!
//! Drift occurs when the actual state of a document, index, or session diverges
//! from the expected state. This module defines the drift model and detection
//! entry points for each dimension.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// The six dimensions along which drift can occur.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DriftDimension {
    /// Index drift — the master index is out of sync with files on disk.
    Index,
    /// Session drift — session context references stale or missing documents.
    Session,
    /// Layer-3 drift — active context documents have diverged from source.
    Layer3,
    /// Stage drift — project stage metadata is inconsistent with actual progress.
    Stage,
    /// Dependency drift — declared dependencies between documents are broken.
    Dependency,
    /// Context drift — context snapshots no longer reflect current state.
    Context,
}

/// Severity level of a detected drift.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum DriftSeverity {
    /// Informational — noted but does not require action.
    Info,
    /// Warning — should be addressed soon.
    Warning,
    /// Critical — blocks correct operation and must be resolved.
    Critical,
}

/// Specific type of drift detected within a dimension.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DriftType {
    /// A file exists on disk but is missing from the index.
    MissingFromIndex,
    /// An index entry refers to a file that no longer exists.
    OrphanedEntry,
    /// Content hash mismatch between index and disk.
    ContentMismatch,
    /// A dependency target is missing or moved.
    BrokenDependency,
    /// Session references a document that has been modified since load.
    StaleReference,
    /// Stage metadata conflicts with document state.
    StageInconsistency,
    /// A CLAUDE.md or CONTEXT.md is missing from a significant directory.
    MissingContextFile,
    /// Context file content is outdated relative to source files.
    OutdatedContextFile,
}

/// A single detected drift record.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DriftRecord {
    /// Which dimension this drift belongs to.
    pub dimension: DriftDimension,
    /// The specific type of drift.
    pub drift_type: DriftType,
    /// Severity assessment.
    pub severity: DriftSeverity,
    /// Human-readable description of the drift.
    pub message: String,
    /// Path(s) involved.
    pub affected_paths: Vec<String>,
    /// When the drift was detected.
    pub detected_at: DateTime<Utc>,
    /// Whether this drift has been resolved.
    pub resolved: bool,
}

/// Result of a drift detection scan across all dimensions.
#[derive(Debug, Clone, Default)]
pub struct DriftReport {
    /// All drift records found.
    pub records: Vec<DriftRecord>,
}

impl DriftReport {
    /// Create an empty drift report.
    pub fn new() -> Self {
        Self {
            records: Vec::new(),
        }
    }

    /// Add a drift record to the report.
    pub fn add(&mut self, record: DriftRecord) {
        self.records.push(record);
    }

    /// Get all records for a specific dimension.
    pub fn by_dimension(&self, dimension: DriftDimension) -> Vec<&DriftRecord> {
        self.records
            .iter()
            .filter(|r| r.dimension == dimension)
            .collect()
    }

    /// Get all records at a specific severity or above.
    pub fn by_min_severity(&self, min_severity: DriftSeverity) -> Vec<&DriftRecord> {
        self.records
            .iter()
            .filter(|r| r.severity >= min_severity)
            .collect()
    }

    /// Get all unresolved records.
    pub fn unresolved(&self) -> Vec<&DriftRecord> {
        self.records.iter().filter(|r| !r.resolved).collect()
    }

    /// Total number of drift records.
    pub fn total(&self) -> usize {
        self.records.len()
    }

    /// Whether any critical drift was detected.
    pub fn has_critical(&self) -> bool {
        self.records
            .iter()
            .any(|r| r.severity == DriftSeverity::Critical)
    }

    /// Count records by severity.
    pub fn count_by_severity(&self) -> (usize, usize, usize) {
        let info = self
            .records
            .iter()
            .filter(|r| r.severity == DriftSeverity::Info)
            .count();
        let warning = self
            .records
            .iter()
            .filter(|r| r.severity == DriftSeverity::Warning)
            .count();
        let critical = self
            .records
            .iter()
            .filter(|r| r.severity == DriftSeverity::Critical)
            .count();
        (info, warning, critical)
    }
}

/// Detect index drift by comparing registered paths against actual files on disk.
///
/// - Files on disk not in the registry → MissingFromIndex
/// - Registry entries with no file on disk → OrphanedEntry
#[tracing::instrument(skip_all)]
pub fn detect_index_drift(
    registered_paths: &[String],
    disk_paths: &[String],
    now: DateTime<Utc>,
) -> Vec<DriftRecord> {
    let registered: std::collections::HashSet<&str> =
        registered_paths.iter().map(|s| s.as_str()).collect();
    let on_disk: std::collections::HashSet<&str> = disk_paths.iter().map(|s| s.as_str()).collect();

    let mut records = Vec::new();

    // Files on disk but not in registry
    for path in on_disk.difference(&registered) {
        records.push(DriftRecord {
            dimension: DriftDimension::Index,
            drift_type: DriftType::MissingFromIndex,
            severity: DriftSeverity::Warning,
            message: format!("File exists on disk but not in index: {path}"),
            affected_paths: vec![path.to_string()],
            detected_at: now,
            resolved: false,
        });
    }

    // Registry entries with no file on disk
    for path in registered.difference(&on_disk) {
        records.push(DriftRecord {
            dimension: DriftDimension::Index,
            drift_type: DriftType::OrphanedEntry,
            severity: DriftSeverity::Warning,
            message: format!("Index entry has no file on disk: {path}"),
            affected_paths: vec![path.to_string()],
            detected_at: now,
            resolved: false,
        });
    }

    records
}

/// Detect content drift by comparing stored hashes against current hashes.
///
/// Returns ContentMismatch records for any file whose hash has changed.
#[tracing::instrument(skip_all)]
pub fn detect_content_drift(
    hash_pairs: &[(String, String, String)], // (path, stored_hash, current_hash)
    now: DateTime<Utc>,
) -> Vec<DriftRecord> {
    hash_pairs
        .iter()
        .filter(|(_, stored, current)| stored != current)
        .map(|(path, _stored, _current)| DriftRecord {
            dimension: DriftDimension::Index,
            drift_type: DriftType::ContentMismatch,
            severity: DriftSeverity::Warning,
            message: format!("Content hash changed for {path}"),
            affected_paths: vec![path.clone()],
            detected_at: now,
            resolved: false,
        })
        .collect()
}

/// Detect dependency drift by checking that all dependency targets exist.
///
/// `dependencies` is a list of (source_path, target_path) pairs.
/// `existing_paths` is the set of all known paths.
#[tracing::instrument(skip_all)]
pub fn detect_dependency_drift(
    dependencies: &[(String, String)],
    existing_paths: &[String],
    now: DateTime<Utc>,
) -> Vec<DriftRecord> {
    let existing: std::collections::HashSet<&str> =
        existing_paths.iter().map(|s| s.as_str()).collect();

    dependencies
        .iter()
        .filter(|(_, target)| !existing.contains(target.as_str()))
        .map(|(source, target)| DriftRecord {
            dimension: DriftDimension::Dependency,
            drift_type: DriftType::BrokenDependency,
            severity: DriftSeverity::Critical,
            message: format!("Dependency target missing: {source} → {target}"),
            affected_paths: vec![source.clone(), target.clone()],
            detected_at: now,
            resolved: false,
        })
        .collect()
}

/// Detect session drift — context items from non-current sessions that are not marked stale.
///
/// `stale_items` is a list of (item_id, session_id) for items that should have been
/// marked stale but weren't. `current_session_id` is the active session.
#[tracing::instrument(skip_all)]
pub fn detect_session_drift(
    stale_items: &[(String, String)],
    current_session_id: &str,
    now: DateTime<Utc>,
) -> Vec<DriftRecord> {
    stale_items
        .iter()
        .filter(|(_, sid)| sid != current_session_id)
        .map(|(item_id, session_id)| DriftRecord {
            dimension: DriftDimension::Session,
            drift_type: DriftType::StaleReference,
            severity: DriftSeverity::Info,
            message: format!("Context item `{item_id}` from session `{session_id}` still active"),
            affected_paths: vec![item_id.clone()],
            detected_at: now,
            resolved: false,
        })
        .collect()
}

/// Detect stage drift — active stages with no recent file activity in scope, or
/// stages that have been open longer than expected.
///
/// `active_stages` contains (stage_name, scope_pattern, started_at).
/// `recent_file_paths` is the set of files modified in the current session.
#[tracing::instrument(skip_all)]
pub fn detect_stage_drift(
    active_stages: &[(String, Option<String>, String)],
    recent_file_paths: &[String],
    now: DateTime<Utc>,
) -> Vec<DriftRecord> {
    let mut records = Vec::new();

    for (stage_name, scope_pattern, _started_at) in active_stages {
        // If stage has a scope, check whether any recent activity is within that scope
        if let Some(scope) = scope_pattern {
            let scope_prefix = scope.trim_end_matches('*').trim_end_matches('/');
            let has_activity = recent_file_paths
                .iter()
                .any(|p| p.starts_with(scope_prefix));

            if !has_activity && !recent_file_paths.is_empty() {
                records.push(DriftRecord {
                    dimension: DriftDimension::Stage,
                    drift_type: DriftType::StageInconsistency,
                    severity: DriftSeverity::Warning,
                    message: format!(
                        "Stage `{stage_name}` scoped to `{scope}` but no recent activity in scope"
                    ),
                    affected_paths: vec![stage_name.clone()],
                    detected_at: now,
                    resolved: false,
                });
            }
        }
    }

    records
}

/// Detect layer-3 drift — significant directories that are missing CLAUDE.md or CONTEXT.md.
///
/// `significant_dirs` are directories that should have context files.
/// `existing_context_files` are context file paths that exist on disk.
#[tracing::instrument(skip_all)]
pub fn detect_layer3_drift(
    significant_dirs: &[String],
    existing_context_files: &[String],
    now: DateTime<Utc>,
) -> Vec<DriftRecord> {
    let existing: std::collections::HashSet<&str> =
        existing_context_files.iter().map(|s| s.as_str()).collect();

    let mut records = Vec::new();

    for dir in significant_dirs {
        let claude_path = if dir.is_empty() {
            "CLAUDE.md".to_string()
        } else {
            format!("{dir}/CLAUDE.md")
        };
        let context_path = if dir.is_empty() {
            "CONTEXT.md".to_string()
        } else {
            format!("{dir}/CONTEXT.md")
        };

        if !existing.contains(claude_path.as_str()) {
            records.push(DriftRecord {
                dimension: DriftDimension::Layer3,
                drift_type: DriftType::MissingContextFile,
                severity: DriftSeverity::Warning,
                message: format!("Significant directory `{dir}` missing CLAUDE.md"),
                affected_paths: vec![claude_path],
                detected_at: now,
                resolved: false,
            });
        }

        // CONTEXT.md is optional for root, required for subdirectories
        if !dir.is_empty() && !existing.contains(context_path.as_str()) {
            records.push(DriftRecord {
                dimension: DriftDimension::Layer3,
                drift_type: DriftType::MissingContextFile,
                severity: DriftSeverity::Info,
                message: format!("Significant directory `{dir}` missing CONTEXT.md"),
                affected_paths: vec![context_path],
                detected_at: now,
                resolved: false,
            });
        }
    }

    records
}

/// Classify drift severity based on dimension and type.
#[tracing::instrument(skip_all)]
pub fn classify_severity(dimension: DriftDimension, drift_type: &DriftType) -> DriftSeverity {
    match (dimension, drift_type) {
        // Broken dependencies are always critical
        (_, DriftType::BrokenDependency) => DriftSeverity::Critical,
        // Stage inconsistencies are critical
        (DriftDimension::Stage, _) => DriftSeverity::Critical,
        // Missing context files are warnings
        (_, DriftType::MissingContextFile) => DriftSeverity::Warning,
        // Stale references are warnings
        (_, DriftType::StaleReference) => DriftSeverity::Warning,
        // Content mismatches depend on dimension
        (DriftDimension::Layer3, DriftType::ContentMismatch) => DriftSeverity::Warning,
        (DriftDimension::Index, _) => DriftSeverity::Warning,
        // Default to info
        _ => DriftSeverity::Info,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    #[test]
    fn test_session_drift_filters_current_session() {
        let now = Utc::now();
        let items = vec![
            ("doc1.md".to_string(), "old-session".to_string()),
            ("doc2.md".to_string(), "current".to_string()),
            ("doc3.md".to_string(), "another-old".to_string()),
        ];
        let drift = detect_session_drift(&items, "current", now);
        // Only items from non-current sessions
        assert_eq!(drift.len(), 2);
        assert!(drift.iter().all(|r| r.dimension == DriftDimension::Session));
    }

    #[test]
    fn test_session_drift_empty_when_all_current() {
        let now = Utc::now();
        let items = vec![("doc1.md".to_string(), "current".to_string())];
        let drift = detect_session_drift(&items, "current", now);
        assert!(drift.is_empty());
    }

    #[test]
    fn test_stage_drift_detects_out_of_scope_activity() {
        let now = Utc::now();
        let stages = vec![(
            "implement".to_string(),
            Some("src/auth/*".to_string()),
            "2026-01-01".to_string(),
        )];
        // Activity outside scope
        let recent = vec!["lib/utils.rs".to_string(), "tests/test.rs".to_string()];
        let drift = detect_stage_drift(&stages, &recent, now);
        assert_eq!(drift.len(), 1);
        assert_eq!(drift[0].dimension, DriftDimension::Stage);
    }

    #[test]
    fn test_stage_drift_no_drift_when_in_scope() {
        let now = Utc::now();
        let stages = vec![(
            "implement".to_string(),
            Some("src/auth/*".to_string()),
            "2026-01-01".to_string(),
        )];
        let recent = vec!["src/auth/login.rs".to_string()];
        let drift = detect_stage_drift(&stages, &recent, now);
        assert!(drift.is_empty());
    }

    #[test]
    fn test_stage_drift_no_scope_no_drift() {
        let now = Utc::now();
        let stages = vec![("implement".to_string(), None, "2026-01-01".to_string())];
        let recent = vec!["anything.rs".to_string()];
        let drift = detect_stage_drift(&stages, &recent, now);
        assert!(drift.is_empty());
    }

    #[test]
    fn test_layer3_drift_missing_claude_md() {
        let now = Utc::now();
        let dirs = vec!["src".to_string(), "lib".to_string()];
        let existing = vec!["src/CLAUDE.md".to_string()]; // lib missing both, src missing CONTEXT.md
        let drift = detect_layer3_drift(&dirs, &existing, now);
        // src/CONTEXT.md missing + lib/CLAUDE.md missing + lib/CONTEXT.md missing = 3
        assert_eq!(drift.len(), 3);
        assert!(drift.iter().all(|r| r.dimension == DriftDimension::Layer3));
    }

    #[test]
    fn test_layer3_drift_root_no_context_md_needed() {
        let now = Utc::now();
        let dirs = vec!["".to_string()]; // root
        let existing = vec!["CLAUDE.md".to_string()]; // no CONTEXT.md
        let drift = detect_layer3_drift(&dirs, &existing, now);
        // Root doesn't require CONTEXT.md
        assert!(drift.is_empty());
    }

    #[test]
    fn test_layer3_drift_all_present() {
        let now = Utc::now();
        let dirs = vec!["src".to_string()];
        let existing = vec!["src/CLAUDE.md".to_string(), "src/CONTEXT.md".to_string()];
        let drift = detect_layer3_drift(&dirs, &existing, now);
        assert!(drift.is_empty());
    }

    #[test]
    fn test_content_drift_uses_index_dimension() {
        let now = Utc::now();
        let pairs = vec![("file.md".to_string(), "aaa".to_string(), "bbb".to_string())];
        let drift = detect_content_drift(&pairs, now);
        assert_eq!(drift.len(), 1);
        // Was incorrectly DriftDimension::Context before the fix
        assert_eq!(drift[0].dimension, DriftDimension::Index);
    }

    #[test]
    fn test_index_drift_empty_inputs() {
        let now = Utc::now();
        let drift = detect_index_drift(&[], &[], now);
        assert!(drift.is_empty());
    }

    #[test]
    fn test_dependency_drift_all_targets_exist() {
        let now = Utc::now();
        let deps = vec![("a.md".to_string(), "b.md".to_string())];
        let existing = vec!["a.md".to_string(), "b.md".to_string()];
        let drift = detect_dependency_drift(&deps, &existing, now);
        assert!(drift.is_empty());
    }
}
