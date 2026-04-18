//! Project discovery and directory scanning.
//!
//! Scans a project directory to identify significant files, classify them,
//! and build an initial understanding of the project structure.
//!
//! Significance is determined by a scoring system based on:
//! - Whether the directory is in the `always_significant` list
//! - Number of files (more files = more significant)
//! - Presence of key files (package.json, Cargo.toml, etc.)
//! - Depth within the project tree

use crate::config::DiscoveryConfig;
use crate::PrismError;
use serde::{Deserialize, Serialize};
use std::path::Path;

/// Information about a discovered directory.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DirectoryInfo {
    /// Path relative to project root.
    pub path: String,
    /// Number of files in the directory (non-recursive).
    pub file_count: usize,
    /// Detected purpose/role of this directory.
    pub role: Option<String>,
    /// Whether this directory contains context-relevant files.
    pub is_significant: bool,
    /// Significance score (higher = more significant).
    pub score: u32,
}

/// The result of evaluating a file or directory's significance.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SignificanceResult {
    /// High significance — should be indexed and tracked.
    High,
    /// Medium significance — index but lower priority.
    Medium,
    /// Low significance — skip unless explicitly requested.
    Low,
    /// Excluded by configuration.
    Excluded,
}

/// Holds discovery results for context-relevant files.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextFileDiscovery {
    /// Discovered context files and their significance.
    pub files: Vec<(String, SignificanceResult)>,
    /// Discovered directories with metadata.
    pub directories: Vec<DirectoryInfo>,
    /// Total files scanned.
    pub total_scanned: usize,
    /// Count of scanned directories bucketed by significance score (0, 1, 2, ...).
    /// Populated post-scan; used by `/prism:status` to visualise sprawl pressure.
    #[serde(default)]
    pub score_distribution: std::collections::BTreeMap<u32, usize>,
}

/// Check if a directory name should be skipped during scanning.
#[tracing::instrument(skip_all, fields(name))]
pub fn should_skip_directory(name: &str, config: &DiscoveryConfig) -> bool {
    config.skip_directories.iter().any(|s| s == name)
}

/// Check if a directory name is in the always-significant list.
#[tracing::instrument(skip_all)]
pub fn is_always_significant(name: &str, config: &DiscoveryConfig) -> bool {
    config.always_significant.iter().any(|s| s == name)
}

/// Calculate the significance score for a directory.
///
/// Scoring:
/// - Always-significant directory: +5
/// - Contains CLAUDE.md or CONTEXT.md: +3
/// - Contains package.json, Cargo.toml, or similar: +2
/// - Each file up to 10: +1 each
/// - Depth penalty: -1 per level beyond max_depth
#[tracing::instrument(skip_all)]
pub fn calculate_significance(
    dir_name: &str,
    file_count: usize,
    depth: u32,
    has_context_file: bool,
    has_project_file: bool,
    config: &DiscoveryConfig,
) -> (u32, SignificanceResult) {
    let mut score: u32 = 0;

    if is_always_significant(dir_name, config) {
        score += 5;
    }

    if has_context_file {
        score += 3;
    }

    if has_project_file {
        score += 2;
    }

    score += (file_count as u32).min(10);

    // Depth penalty
    if depth > config.max_depth {
        let penalty = depth - config.max_depth;
        score = score.saturating_sub(penalty);
    }

    let result = if score >= config.significance_threshold + 4 {
        SignificanceResult::High
    } else if score >= config.significance_threshold {
        SignificanceResult::Medium
    } else {
        SignificanceResult::Low
    };

    (score, result)
}

/// Detect the role of a directory based on its name.
#[tracing::instrument(skip_all, fields(name))]
pub fn detect_directory_role(name: &str) -> Option<String> {
    let role = match name.to_lowercase().as_str() {
        "src" | "lib" => "source",
        "test" | "tests" | "spec" | "specs" => "tests",
        "docs" | "doc" | "documentation" => "documentation",
        "config" | "configs" | "configuration" => "configuration",
        "scripts" | "bin" => "scripts",
        "public" | "static" | "assets" => "assets",
        "migrations" | "migrate" => "migrations",
        "models" | "entities" => "models",
        "controllers" | "handlers" | "routes" => "handlers",
        "services" | "service" => "services",
        "utils" | "helpers" | "common" | "shared" => "utilities",
        "views" | "templates" | "pages" | "screens" => "views",
        "components" | "widgets" => "components",
        "middleware" | "interceptors" => "middleware",
        "types" | "interfaces" | "schemas" => "types",
        "api" => "api",
        "supabase" => "backend",
        "app" => "application",
        _ => return None,
    };
    Some(role.to_string())
}

/// Check if a file is a project manifest (indicates a package/module root).
#[tracing::instrument(skip_all)]
pub fn is_project_file(filename: &str) -> bool {
    matches!(
        filename,
        "package.json"
            | "Cargo.toml"
            | "pubspec.yaml"
            | "pyproject.toml"
            | "setup.py"
            | "go.mod"
            | "build.gradle"
            | "pom.xml"
            | "Gemfile"
            | "composer.json"
            | "Makefile"
            | "CMakeLists.txt"
    )
}

/// Check if a file is a context file that PRISM manages.
#[tracing::instrument(skip_all)]
pub fn is_context_file(filename: &str) -> bool {
    filename == "CLAUDE.md" || filename == "CONTEXT.md"
}

/// Scan a project directory and return discovery results.
///
/// This function requires a [`crate::FileSystem`] implementation to abstract I/O.
/// For the trait-based version, see the hook handlers that call this.
///
/// # Examples
///
/// ```no_run
/// use prism_core::config::DiscoveryConfig;
/// use prism_core::discovery;
/// use std::path::Path;
///
/// let config = DiscoveryConfig::default();
/// let result = discovery::scan_project(Path::new("."), &config).unwrap();
/// println!("Scanned {} files", result.total_scanned);
/// ```
#[tracing::instrument(skip(root, config), fields(root = %root.display()))]
pub fn scan_project(
    root: &Path,
    config: &DiscoveryConfig,
) -> Result<ContextFileDiscovery, PrismError> {
    let mut discovery = ContextFileDiscovery {
        files: Vec::new(),
        directories: Vec::new(),
        total_scanned: 0,
        score_distribution: std::collections::BTreeMap::new(),
    };

    scan_directory_recursive(root, root, 0, config, &mut discovery)?;

    // Bucket the score histogram so callers (e.g. /prism:status) can
    // visualise how many directories sit at each score level and which are
    // near the significance threshold.
    for dir in &discovery.directories {
        *discovery.score_distribution.entry(dir.score).or_insert(0) += 1;
    }

    Ok(discovery)
}

fn scan_directory_recursive(
    current: &Path,
    root: &Path,
    depth: u32,
    config: &DiscoveryConfig,
    discovery: &mut ContextFileDiscovery,
) -> Result<(), PrismError> {
    if depth > config.max_depth + 2 {
        return Ok(());
    }

    let entries = match std::fs::read_dir(current) {
        Ok(entries) => entries,
        Err(_) => return Ok(()), // Skip unreadable directories
    };

    let mut files = Vec::new();
    let mut subdirs = Vec::new();
    let mut has_context_file = false;
    let mut has_project_file = false;

    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();

        if path.is_dir() {
            if !should_skip_directory(&name, config) && !name.starts_with('.') {
                subdirs.push(path);
            }
        } else {
            discovery.total_scanned += 1;
            if is_context_file(&name) {
                has_context_file = true;
                let rel_path = path.strip_prefix(root).unwrap_or(&path);
                discovery.files.push((
                    rel_path.to_string_lossy().to_string(),
                    SignificanceResult::High,
                ));
            }
            if is_project_file(&name) {
                has_project_file = true;
            }
            files.push(name);
        }
    }

    let dir_name = current
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();

    if depth >= config.min_depth {
        let (score, significance) = calculate_significance(
            &dir_name,
            files.len(),
            depth,
            has_context_file,
            has_project_file,
            config,
        );

        if significance != SignificanceResult::Low || has_context_file {
            let rel_path = current.strip_prefix(root).unwrap_or(current);
            discovery.directories.push(DirectoryInfo {
                path: rel_path.to_string_lossy().to_string(),
                file_count: files.len(),
                role: detect_directory_role(&dir_name),
                is_significant: matches!(
                    significance,
                    SignificanceResult::High | SignificanceResult::Medium
                ),
                score,
            });
        }
    }

    for subdir in subdirs {
        scan_directory_recursive(&subdir, root, depth + 1, config, discovery)?;
    }

    Ok(())
}

#[cfg(test)]
mod risk3_tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn score_distribution_populated_from_scanned_dirs() {
        // Assemble a project with one "always_significant" dir + a couple of
        // borderline dirs so the distribution has at least two buckets.
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        std::fs::create_dir_all(root.join("src")).unwrap();
        std::fs::write(root.join("src/lib.rs"), "fn main(){}").unwrap();
        std::fs::write(root.join("src/mod.rs"), "pub mod x;").unwrap();
        std::fs::create_dir_all(root.join("notes")).unwrap();
        std::fs::write(root.join("notes/a.md"), "x").unwrap();

        let scan = scan_project(root, &DiscoveryConfig::default()).unwrap();
        assert!(
            !scan.score_distribution.is_empty(),
            "distribution must have at least one bucket"
        );
        let total: usize = scan.score_distribution.values().sum();
        assert_eq!(
            total,
            scan.directories.len(),
            "distribution total must match directory count"
        );
    }

    #[test]
    fn lazy_doc_pair_creation_default_false() {
        let cfg = DiscoveryConfig::default();
        assert!(!cfg.lazy_doc_pair_creation);
    }

    #[test]
    fn expanded_skip_list_covers_python_caches() {
        let cfg = DiscoveryConfig::default();
        for expected in [
            ".venv",
            ".tox",
            ".mypy_cache",
            ".pytest_cache",
            ".ruff_cache",
        ] {
            assert!(
                cfg.skip_directories.iter().any(|s| s == expected),
                "skip list missing {expected}"
            );
        }
    }
}
