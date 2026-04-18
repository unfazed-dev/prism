//! PRISM layer hierarchy and loading mechanisms.
//!
//! Defines the layered document architecture (L0–L4, Meta, Distributed) and
//! how each layer's documents are loaded into context.

use serde::{Deserialize, Serialize};

/// The PRISM document layer hierarchy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Layer {
    /// L0 — Foundation layer (project brief, tech stack).
    L0,
    /// L1 — Persistent context (architecture, patterns).
    L1,
    /// L2 — Session index and active documents list.
    L2,
    /// L3a — Active context loaded into the current session.
    L3a,
    /// L3b — Extended context available on demand.
    L3b,
    /// L4 — Ephemeral / scratch context.
    L4,
    /// Meta — Cross-cutting documents (decisions, goals, progress).
    Meta,
    /// Distributed — Documents embedded alongside source code (CLAUDE.md, CONTEXT.md).
    Distributed,
}

/// How a document at a given layer is loaded into an AI session.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum LoadingMechanism {
    /// Automatically loaded at session start.
    AutoLoad,
    /// Loaded on demand when referenced.
    OnDemand,
    /// Discovered and indexed but not loaded unless requested.
    Indexed,
    /// Never automatically loaded; manual reference only.
    Manual,
}

impl Layer {
    /// Get the default loading mechanism for this layer.
    pub fn default_loading_mechanism(&self) -> LoadingMechanism {
        match self {
            Layer::L0 => LoadingMechanism::AutoLoad,
            Layer::L1 => LoadingMechanism::AutoLoad,
            Layer::L2 => LoadingMechanism::AutoLoad,
            Layer::L3a => LoadingMechanism::OnDemand,
            Layer::L3b => LoadingMechanism::Indexed,
            Layer::L4 => LoadingMechanism::Manual,
            Layer::Meta => LoadingMechanism::OnDemand,
            Layer::Distributed => LoadingMechanism::AutoLoad,
        }
    }

    /// Get the default token budget for this layer.
    pub fn default_token_budget(&self) -> u32 {
        match self {
            Layer::L0 => 800,
            Layer::L1 => 300,
            Layer::L2 => 500,
            Layer::L3a => 2000,
            Layer::L3b => 2000,
            Layer::L4 => 4000,
            Layer::Meta => 1000,
            Layer::Distributed => 500,
        }
    }

    /// Whether this layer requires YAML frontmatter.
    ///
    /// Hot-path files (L0, L1, Distributed) use `<!-- prism:managed -->` markers
    /// with metadata stored in prism.db. Only L3b+ documents get full frontmatter.
    pub fn requires_frontmatter(&self) -> bool {
        matches!(self, Layer::L3a | Layer::L3b | Layer::L4 | Layer::Meta)
    }

    /// Whether this layer is auto-loaded at session start.
    pub fn is_auto_loaded(&self) -> bool {
        matches!(self.default_loading_mechanism(), LoadingMechanism::AutoLoad)
    }

    /// Get the sort order for layer priority (lower = higher priority).
    pub fn priority(&self) -> u8 {
        match self {
            Layer::L0 => 0,
            Layer::L1 => 1,
            Layer::L2 => 2,
            Layer::Distributed => 3,
            Layer::L3a => 4,
            Layer::Meta => 5,
            Layer::L3b => 6,
            Layer::L4 => 7,
        }
    }

    /// Parse a layer string like "L0", "L3b", "Meta", "Distributed" into a Layer.
    pub fn parse(s: &str) -> Option<Layer> {
        match s {
            "L0" => Some(Layer::L0),
            "L1" => Some(Layer::L1),
            "L2" => Some(Layer::L2),
            "L3a" => Some(Layer::L3a),
            "L3b" => Some(Layer::L3b),
            "L4" => Some(Layer::L4),
            "Meta" => Some(Layer::Meta),
            "Distributed" => Some(Layer::Distributed),
            _ => None,
        }
    }

    /// Convert to string representation.
    pub fn as_str(&self) -> &'static str {
        match self {
            Layer::L0 => "L0",
            Layer::L1 => "L1",
            Layer::L2 => "L2",
            Layer::L3a => "L3a",
            Layer::L3b => "L3b",
            Layer::L4 => "L4",
            Layer::Meta => "Meta",
            Layer::Distributed => "Distributed",
        }
    }
}

/// Classify a file path into a layer based on its location and name.
///
/// This is a heuristic classification — the definitive layer comes from
/// frontmatter or the document registry.
#[tracing::instrument(skip_all, fields(path))]
pub fn classify_path(path: &str) -> Layer {
    let normalized = path.replace('\\', "/");
    let parts: Vec<&str> = normalized.split('/').collect();
    let filename = parts.last().unwrap_or(&"");

    // .prism/ directory structure (check BEFORE Distributed to handle .prism/stages/*/CONTEXT.md)
    if parts.contains(&".prism") {
        // .prism/refs/ → L3b
        if parts.contains(&"refs") {
            return Layer::L3b;
        }
        // .prism/stages/ → L3a
        if parts.contains(&"stages") {
            return Layer::L3a;
        }
        // .prism/PRISM.md → Meta
        if *filename == "PRISM.md" {
            return Layer::Meta;
        }
        return Layer::L2;
    }

    // Distributed: CLAUDE.md or CONTEXT.md in any directory
    if *filename == "CLAUDE.md" || *filename == "CONTEXT.md" {
        // Root-level CLAUDE.md is L0, others are Distributed
        if parts.len() <= 2 {
            return Layer::L0;
        }
        return Layer::Distributed;
    }

    // .claude/rules/ → L1
    if parts.contains(&".claude") || parts.contains(&"rules") {
        return Layer::L1;
    }

    // Default to L3b for other markdown files
    if filename.ends_with(".md") {
        return Layer::L3b;
    }

    Layer::L4
}

/// Validate that the total token budget across all layers doesn't exceed the target.
#[tracing::instrument(skip_all)]
pub fn validate_token_budget(
    budgets: &[(Layer, u32)],
    total_target: u32,
) -> Result<(), (u32, u32)> {
    let total: u32 = budgets.iter().map(|(_, b)| b).sum();
    if total > total_target {
        Err((total, total_target))
    } else {
        Ok(())
    }
}
