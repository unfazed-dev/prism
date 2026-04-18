//! Content generation for dynamic document sections.
//!
//! These functions generate markdown content fragments that get inserted
//! into templates or existing documents.

use serde::Serialize;

/// A row in a routing table.
#[derive(Debug, Clone, Serialize)]
pub struct RoutingEntry {
    pub directory: String,
    pub purpose: String,
    pub has_claude_md: bool,
    pub has_context_md: bool,
}

/// A row in an entity list.
#[derive(Debug, Clone, Serialize)]
pub struct EntityEntry {
    pub id: String,
    pub path: String,
    pub layer: String,
    pub classification: String,
    pub status: String,
}

/// A row in a freshness table.
#[derive(Debug, Clone, Serialize)]
pub struct FreshnessEntry {
    pub path: String,
    pub score: String,
    pub last_read: u32,
    pub ttl: u32,
    pub ratio: f64,
}

/// A row in a decision list.
#[derive(Debug, Clone, Serialize)]
pub struct DecisionEntry {
    pub id: String,
    pub scope: String,
    pub decision: String,
    pub active: bool,
}

/// Generate a routing table mapping directories to their purpose.
pub fn generate_routing_table(entries: &[RoutingEntry]) -> String {
    if entries.is_empty() {
        return "*No directories catalogued.*".to_string();
    }

    let mut table = String::from("| Directory | Purpose | CLAUDE.md | CONTEXT.md |\n");
    table.push_str("|-----------|---------|-----------|------------|\n");
    for entry in entries {
        let claude = if entry.has_claude_md { "yes" } else { "no" };
        let context = if entry.has_context_md { "yes" } else { "no" };
        table.push_str(&format!(
            "| `{}` | {} | {} | {} |\n",
            entry.directory, entry.purpose, claude, context
        ));
    }
    table
}

/// Generate an entity list of tracked documents.
pub fn generate_entity_list(entries: &[EntityEntry]) -> String {
    if entries.is_empty() {
        return "*No documents tracked.*".to_string();
    }

    let mut table = String::from("| ID | Path | Layer | Classification | Status |\n");
    table.push_str("|-----|------|-------|----------------|--------|\n");
    for entry in entries {
        table.push_str(&format!(
            "| {} | `{}` | {} | {} | {} |\n",
            entry.id, entry.path, entry.layer, entry.classification, entry.status
        ));
    }
    table
}

/// Generate a freshness table showing document ages and staleness.
pub fn generate_freshness_table(entries: &[FreshnessEntry]) -> String {
    if entries.is_empty() {
        return "*No freshness data.*".to_string();
    }

    let mut table = String::from("| Document | Score | Last Read | TTL | Ratio |\n");
    table.push_str("|----------|-------|-----------|-----|-------|\n");
    for entry in entries {
        table.push_str(&format!(
            "| `{}` | {} | Turn {} | {} | {:.2} |\n",
            entry.path, entry.score, entry.last_read, entry.ttl, entry.ratio
        ));
    }
    table
}

/// Generate a decision list from decision records.
pub fn generate_decision_list(entries: &[DecisionEntry]) -> String {
    if entries.is_empty() {
        return "*No decisions recorded.*".to_string();
    }

    let mut table = String::from("| ID | Scope | Decision | Active |\n");
    table.push_str("|----|-------|----------|--------|\n");
    for entry in entries {
        let active = if entry.active { "yes" } else { "no" };
        table.push_str(&format!(
            "| {} | `{}` | {} | {} |\n",
            entry.id, entry.scope, entry.decision, active
        ));
    }
    table
}

/// Generate a drift summary string from counts.
pub fn generate_drift_summary(info: usize, warning: usize, critical: usize) -> String {
    if info == 0 && warning == 0 && critical == 0 {
        return "No drift detected.".to_string();
    }

    let mut parts = Vec::new();
    if critical > 0 {
        parts.push(format!("{critical} critical"));
    }
    if warning > 0 {
        parts.push(format!("{warning} warning"));
    }
    if info > 0 {
        parts.push(format!("{info} info"));
    }
    format!("Drift detected: {}", parts.join(", "))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_routing_table_empty() {
        assert_eq!(generate_routing_table(&[]), "*No directories catalogued.*");
    }

    #[test]
    fn test_routing_table_with_entries() {
        let entries = vec![
            RoutingEntry {
                directory: "src".to_string(),
                purpose: "Source code".to_string(),
                has_claude_md: true,
                has_context_md: true,
            },
            RoutingEntry {
                directory: "tests".to_string(),
                purpose: "Test files".to_string(),
                has_claude_md: false,
                has_context_md: false,
            },
        ];
        let table = generate_routing_table(&entries);
        assert!(table.contains("| `src`"));
        assert!(table.contains("| yes | yes |"));
        assert!(table.contains("| `tests`"));
        assert!(table.contains("| no | no |"));
    }

    #[test]
    fn test_entity_list_empty() {
        assert_eq!(generate_entity_list(&[]), "*No documents tracked.*");
    }

    #[test]
    fn test_entity_list_with_entries() {
        let entries = vec![EntityEntry {
            id: "doc-001".to_string(),
            path: "CLAUDE.md".to_string(),
            layer: "L0".to_string(),
            classification: "core".to_string(),
            status: "active".to_string(),
        }];
        let table = generate_entity_list(&entries);
        assert!(table.contains("doc-001"));
        assert!(table.contains("`CLAUDE.md`"));
    }

    #[test]
    fn test_freshness_table_empty() {
        assert_eq!(generate_freshness_table(&[]), "*No freshness data.*");
    }

    #[test]
    fn test_freshness_table_with_entries() {
        let entries = vec![FreshnessEntry {
            path: "arch.md".to_string(),
            score: "Fresh".to_string(),
            last_read: 10,
            ttl: 30,
            ratio: 0.33,
        }];
        let table = generate_freshness_table(&entries);
        assert!(table.contains("`arch.md`"));
        assert!(table.contains("Turn 10"));
        assert!(table.contains("0.33"));
    }

    #[test]
    fn test_decision_list_empty() {
        assert_eq!(generate_decision_list(&[]), "*No decisions recorded.*");
    }

    #[test]
    fn test_decision_list_with_entries() {
        let entries = vec![DecisionEntry {
            id: "d-001".to_string(),
            scope: "project".to_string(),
            decision: "Use Rust".to_string(),
            active: true,
        }];
        let table = generate_decision_list(&entries);
        assert!(table.contains("d-001"));
        assert!(table.contains("Use Rust"));
        assert!(table.contains("| yes |"));
    }

    #[test]
    fn test_drift_summary_none() {
        assert_eq!(generate_drift_summary(0, 0, 0), "No drift detected.");
    }

    #[test]
    fn test_drift_summary_mixed() {
        let summary = generate_drift_summary(2, 1, 3);
        assert!(summary.contains("3 critical"));
        assert!(summary.contains("1 warning"));
        assert!(summary.contains("2 info"));
    }

    #[test]
    fn test_drift_summary_critical_only() {
        let summary = generate_drift_summary(0, 0, 1);
        assert_eq!(summary, "Drift detected: 1 critical");
    }
}
