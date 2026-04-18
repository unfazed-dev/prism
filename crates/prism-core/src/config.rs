//! PRISM configuration management.
//!
//! Defines [`PrismConfig`] which maps to the project-level `config.json` and provides
//! defaults, validation, and load logic matching the PRISM spec v2.1.

use crate::PrismError;
use serde::{Deserialize, Serialize};
use std::path::Path;

// ---------------------------------------------------------------------------
// Top-level config
// ---------------------------------------------------------------------------

/// Top-level PRISM configuration, typically loaded from `config.json`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrismConfig {
    /// Spec version (required).
    pub version: String,

    /// Whether PRISM automation (hooks) is enabled for this project.
    /// When `false`, all hooks return no-op immediately.
    /// Defaults to `true` for backward compatibility — missing field = enabled.
    #[serde(default = "default_true")]
    pub enabled: bool,

    /// External dependency configuration.
    #[serde(default)]
    pub dependencies: DependenciesConfig,

    /// Context layer token budgets.
    #[serde(default)]
    pub context: ContextConfig,

    /// Freshness scoring thresholds and TTL settings.
    #[serde(default)]
    pub freshness: FreshnessConfig,

    /// Checkpoint trigger rules.
    #[serde(default)]
    pub checkpoints: CheckpointConfig,

    /// Drift detection and reconciliation settings.
    #[serde(default)]
    pub drift: DriftConfig,

    /// Auto-sync behavior for document categories.
    #[serde(default)]
    pub auto_sync: AutoSyncConfig,

    /// Search delegation and scope settings.
    #[serde(default)]
    pub search: SearchConfig,

    /// LLM enrichment settings for CLAUDE.md content generation.
    #[serde(default)]
    pub enrichment: EnrichmentConfig,

    /// Project type and framework metadata.
    #[serde(default)]
    pub project: ProjectConfig,

    /// Discovery scan parameters.
    #[serde(default)]
    pub discovery: DiscoveryConfig,
}

fn default_true() -> bool {
    true
}

impl Default for PrismConfig {
    fn default() -> Self {
        Self {
            version: "0.1.0".to_string(),
            enabled: true,
            dependencies: DependenciesConfig::default(),
            context: ContextConfig::default(),
            freshness: FreshnessConfig::default(),
            checkpoints: CheckpointConfig::default(),
            drift: DriftConfig::default(),
            auto_sync: AutoSyncConfig::default(),
            search: SearchConfig::default(),
            project: ProjectConfig::default(),
            enrichment: EnrichmentConfig::default(),
            discovery: DiscoveryConfig::default(),
        }
    }
}

impl PrismConfig {
    /// Load a `PrismConfig` from a JSON file at `path`.
    pub fn load(path: &Path) -> Result<Self, PrismError> {
        let contents = std::fs::read_to_string(path)?;
        Self::load_from_str(&contents)
    }

    /// Parse a `PrismConfig` from a JSON string.
    pub fn load_from_str(json: &str) -> Result<Self, PrismError> {
        let cfg: PrismConfig = serde_json::from_str(json)?;
        Ok(cfg)
    }

    /// Save the config to a JSON file at `path`.
    pub fn save(&self, path: &Path) -> Result<(), PrismError> {
        let json = serde_json::to_string_pretty(self)?;
        std::fs::write(path, json)?;
        Ok(())
    }

    /// Check if PRISM is enabled by walking up from `cwd` to find `.prism/config.json`.
    /// Fail-open: if config can't be found or loaded, returns `true` so a broken
    /// config never silently disables PRISM.
    pub fn is_enabled(cwd: &Path) -> bool {
        let mut current = cwd.to_path_buf();
        loop {
            let config_path = current.join(".prism/config.json");
            if config_path.exists() {
                return match Self::load(&config_path) {
                    Ok(cfg) => cfg.enabled,
                    Err(_) => true, // fail-open
                };
            }
            if !current.pop() {
                return true; // no .prism/ found anywhere, fail-open
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Sub-configs
// ---------------------------------------------------------------------------

/// Configuration for external tool dependencies.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DependenciesConfig {
    /// Whether context-mode is installed.
    pub context_mode_installed: bool,
    /// Minimum required context-mode version.
    pub context_mode_version: String,
    /// Actual cached version detected on disk, if any.
    #[serde(default)]
    pub context_mode_cached_version: Option<String>,
}

impl Default for DependenciesConfig {
    fn default() -> Self {
        Self {
            context_mode_installed: true,
            context_mode_version: "1.0.15".to_string(),
            context_mode_cached_version: None,
        }
    }
}

/// Token budgets for each context layer.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextConfig {
    /// Token budget for Layer 0 (system prompt).
    pub layer0_token_budget: u32,
    /// Token budget for Layer 1 (project brief).
    pub layer1_token_budget: u32,
    /// Token budget for Layer 2 (active context index).
    pub layer2_token_budget: u32,
    /// Maximum tokens per stage for Layer 3 documents.
    pub layer3_max_per_stage: u32,
    /// Maximum tokens per stage for Layer 4 ephemeral items.
    pub layer4_max_per_stage: u32,
    /// Total token target per task across all layers.
    pub total_target_per_task: u32,
}

impl Default for ContextConfig {
    fn default() -> Self {
        Self {
            layer0_token_budget: 800,
            layer1_token_budget: 300,
            layer2_token_budget: 500,
            layer3_max_per_stage: 2000,
            layer4_max_per_stage: 4000,
            total_target_per_task: 8000,
        }
    }
}

/// Freshness scoring thresholds and TTL settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FreshnessConfig {
    /// Default time-to-live in turns before an item is considered stale.
    pub default_ttl_turns: u32,
    /// Ratio above which an item is classified as stale.
    pub stale_threshold: f64,
    /// Ratio above which an item is classified as aging.
    pub aging_threshold: f64,
    /// Whether TTL adjusts based on access frequency.
    pub adaptive_ttl: bool,
}

impl Default for FreshnessConfig {
    fn default() -> Self {
        Self {
            default_ttl_turns: 30,
            stale_threshold: 1.0,
            aging_threshold: 0.7,
            adaptive_ttl: true,
        }
    }
}

/// Rules for when PRISM should create checkpoints.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckpointConfig {
    /// Number of turns between automatic checkpoints.
    pub interval_turns: u32,
    /// Create a checkpoint when the active goal changes.
    pub on_goal_switch: bool,
    /// Create a checkpoint before destructive operations.
    pub on_destructive_ops: bool,
    /// Create a checkpoint after context compaction.
    pub after_compaction: bool,
}

impl Default for CheckpointConfig {
    fn default() -> Self {
        Self {
            interval_turns: 25,
            on_goal_switch: true,
            on_destructive_ops: true,
            after_compaction: true,
        }
    }
}

/// Drift detection and reconciliation settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DriftConfig {
    /// Run drift reconciliation automatically at session start.
    pub auto_reconcile_on_session_start: bool,
    /// Minimum occurrences before a pattern is extracted.
    pub pattern_extraction_threshold: u32,
    /// Surface critical drift immediately rather than deferring.
    pub critical_drift_immediate_surface: bool,
}

impl Default for DriftConfig {
    fn default() -> Self {
        Self {
            auto_reconcile_on_session_start: true,
            pattern_extraction_threshold: 3,
            critical_drift_immediate_surface: true,
        }
    }
}

/// Auto-sync behavior for different document categories.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutoSyncConfig {
    /// Automatically fix derived documents when source changes.
    pub derived_auto_fix: bool,
    /// Propose updates to semi-derived documents at commit time.
    pub semi_derived_propose_at_commit: bool,
    /// Only warn (don't auto-fix) for authored documents.
    pub authored_warn_only: bool,
    /// Enable the pre-commit hook for sync checks.
    pub pre_commit_hook_enabled: bool,
    /// Sync documents when a session ends.
    pub sync_on_session_end: bool,
}

impl Default for AutoSyncConfig {
    fn default() -> Self {
        Self {
            derived_auto_fix: true,
            semi_derived_propose_at_commit: true,
            authored_warn_only: true,
            pre_commit_hook_enabled: true,
            sync_on_session_end: false,
        }
    }
}

/// Search delegation and scope settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchConfig {
    /// Tool name to delegate search queries to.
    pub delegate_to: String,
    /// Include Layer 3 document references in search results.
    pub include_layer3_refs: bool,
    /// Restrict search scope to the active goal.
    pub scope_to_active_goal: bool,
}

impl Default for SearchConfig {
    fn default() -> Self {
        Self {
            delegate_to: "ctx_search".to_string(),
            include_layer3_refs: true,
            scope_to_active_goal: true,
        }
    }
}

/// Project type and framework metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectConfig {
    /// Maps to JSON key `"type"`.
    #[serde(rename = "type")]
    pub project_type: String,
    /// Primary framework (e.g. "axum", "react") or "auto" for detection.
    pub framework: String,
    /// Backend technology or "auto" for detection.
    pub backend: String,
}

impl Default for ProjectConfig {
    fn default() -> Self {
        Self {
            project_type: "auto".to_string(),
            framework: "auto".to_string(),
            backend: "auto".to_string(),
        }
    }
}

/// How initial CLAUDE.md / CONTEXT.md content is generated for newly
/// scaffolded directories.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum InitialScaffoldMode {
    /// Minijinja template output only. No Haiku call.
    TemplateOnly,
    /// Template placeholder now, queue a high-priority ENRICH directive so
    /// the background autopilot replaces it ASAP. Does not block `prism start`.
    HaikuBackground,
    /// Call `claude -p` synchronously, wait for richer output before writing.
    /// Falls back to template + queued directive on failure.
    #[default]
    HaikuBlocking,
}

impl InitialScaffoldMode {
    /// Stable short identifier for CLI flags and JSON.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::TemplateOnly => "template-only",
            Self::HaikuBackground => "haiku-background",
            Self::HaikuBlocking => "haiku-blocking",
        }
    }
}

/// LLM enrichment settings for CLAUDE.md content generation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnrichmentConfig {
    /// Whether LLM enrichment is enabled.
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// Number of directories to enrich per session start.
    #[serde(default = "default_batch_size")]
    pub batch_size: usize,
    /// How many sessions a directive can be re-emitted before it is abandoned.
    #[serde(default = "default_max_retries")]
    pub max_retries: u32,
    /// After this many retries, the directive wording escalates to URGENT.
    #[serde(default = "default_escalate_after")]
    pub escalate_after: u32,
    /// How initial scaffold files are generated.
    #[serde(default)]
    pub initial_scaffold_mode: InitialScaffoldMode,
    /// Optional autonomous enrichment via `claude -p` headless mode.
    #[serde(default)]
    pub autopilot: AutopilotConfig,
}

fn default_batch_size() -> usize {
    5
}

fn default_max_retries() -> u32 {
    3
}

fn default_escalate_after() -> u32 {
    2
}

impl Default for EnrichmentConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            batch_size: 5,
            max_retries: default_max_retries(),
            escalate_after: default_escalate_after(),
            initial_scaffold_mode: InitialScaffoldMode::default(),
            autopilot: AutopilotConfig::default(),
        }
    }
}

/// Autonomous enrichment settings — runs `claude -p` per pending directory.
///
/// Defaults to disabled. When enabled, the `prism enrich` command spawns one
/// headless Claude session per directory using a small/fast model (Haiku),
/// restricted to `Read`/`Edit`/`Glob`/`Grep` tools.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutopilotConfig {
    /// Master switch. Defaults to `true` so first-run `/prism:start` enriches
    /// scaffolded files inline via Haiku. Set to `false` in
    /// `.prism/config.json` for offline / hook-only / no-subprocess mode;
    /// then `prism enrich` refuses to run and `HaikuBlocking` scaffolding
    /// falls through to queue-only behavior.
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// Model id passed to `claude --model`. Default is Haiku 4.5.
    #[serde(default = "default_autopilot_model")]
    pub model: String,
    /// Whether `prism start` should kick off `prism enrich --all` in the
    /// background when a backlog is detected.
    #[serde(default)]
    pub run_at_session_start: bool,
    /// Maximum concurrent `claude -p` subprocesses.
    #[serde(default = "default_autopilot_concurrent")]
    pub max_concurrent: u32,
    /// Per-directory subprocess timeout in seconds.
    #[serde(default = "default_autopilot_timeout")]
    pub timeout_secs: u64,
    /// Tools to permit inside the headless session.
    #[serde(default = "default_autopilot_allowed_tools")]
    pub allowed_tools: Vec<String>,
    /// Set to `true` once the "autopilot-disabled, new default is on" hint has
    /// been emitted to the user. Prevents the same message from re-printing on
    /// every `/prism:start` of a legacy project whose config.json has
    /// `enabled: false` left over from the pre-default-on era. Written back by
    /// `cmd_start` the first time the hint fires.
    #[serde(default)]
    pub hint_shown: bool,
    /// Hard cap on directories enriched per `prism enrich` invocation.
    /// Autopilot halts cleanly once this many directories have been spawned,
    /// preventing a pathological queue from running the full table in one go.
    #[serde(default = "default_autopilot_max_session_directories")]
    pub max_session_directories: usize,
    /// Soft cap on estimated USD spend per session. Enforced only when
    /// token parsing is wired in — today `enrichment_runs.cost_estimate_usd`
    /// rows are populated with 0.0 and the cap never triggers.
    #[serde(default = "default_autopilot_max_session_usd")]
    pub max_session_usd: f64,
    /// Path (project-relative) where `prism enrich` writes live progress
    /// after each directory completes. `/prism:status` reads this file to
    /// render the autopilot panel.
    #[serde(default = "default_autopilot_progress_file")]
    pub progress_file: String,
}

fn default_autopilot_model() -> String {
    "claude-haiku-4-5".to_string()
}

fn default_autopilot_concurrent() -> u32 {
    2
}

fn default_autopilot_timeout() -> u64 {
    120
}

fn default_autopilot_max_session_directories() -> usize {
    50
}

fn default_autopilot_max_session_usd() -> f64 {
    0.50
}

fn default_autopilot_progress_file() -> String {
    ".prism/autopilot.progress.json".to_string()
}

fn default_autopilot_allowed_tools() -> Vec<String> {
    vec![
        "Read".to_string(),
        "Write".to_string(),
        "Edit".to_string(),
        "Glob".to_string(),
        "Grep".to_string(),
    ]
}

impl Default for AutopilotConfig {
    fn default() -> Self {
        Self {
            // On by default so first-run `/prism:start` produces Haiku-generated
            // CLAUDE.md/CONTEXT.md content inline, not just templates. Haiku cost
            // per directory is cheap enough to absorb on fresh-install. Users who
            // want hook-only / offline mode set this to `false` in `.prism/config.json`.
            enabled: true,
            model: default_autopilot_model(),
            run_at_session_start: false,
            max_concurrent: default_autopilot_concurrent(),
            timeout_secs: default_autopilot_timeout(),
            allowed_tools: default_autopilot_allowed_tools(),
            hint_shown: false,
            max_session_directories: default_autopilot_max_session_directories(),
            max_session_usd: default_autopilot_max_session_usd(),
            progress_file: default_autopilot_progress_file(),
        }
    }
}

/// Discovery scan parameters for project structure analysis.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveryConfig {
    /// Minimum file count for a directory to be considered significant.
    pub significance_threshold: u32,
    /// Minimum directory depth to scan.
    pub min_depth: u32,
    /// Maximum directory depth to scan.
    pub max_depth: u32,
    /// Directory names always treated as significant.
    pub always_significant: Vec<String>,
    /// Directory names to skip during scanning.
    pub skip_directories: Vec<String>,
    /// Run a discovery scan when a new session starts.
    pub scan_on_session_start: bool,
    /// Re-scan when structural file changes are detected.
    pub scan_on_structural_change: bool,
    /// Opt-in: skip proactive CLAUDE.md/CONTEXT.md scaffolding during
    /// `sync-docs`. When true, pairs are created only when the user actually
    /// edits a file inside a significant directory (post_tool_use hook
    /// enqueues a directive), avoiding doc-pair sprawl in projects with many
    /// borderline-significant subdirectories.
    #[serde(default)]
    pub lazy_doc_pair_creation: bool,
}

impl Default for DiscoveryConfig {
    fn default() -> Self {
        Self {
            // Risk 3 (sprawl control): raised from 3 to 5. At threshold=3 any
            // directory with a manifest + a couple of files tipped into
            // "significant", flooding the managed-doc set. Threshold=5 keeps
            // hot paths (`src`, `tests`, `supabase`, `app` via
            // `always_significant`, each scoring ≥ 5) while letting boilerplate
            // sibling dirs stay unmanaged.
            significance_threshold: 5,
            min_depth: 1,
            max_depth: 4,
            always_significant: vec![
                "lib".to_string(),
                "src".to_string(),
                "test".to_string(),
                "supabase".to_string(),
                "app".to_string(),
            ],
            skip_directories: vec![
                "node_modules".to_string(),
                "build".to_string(),
                "target".to_string(),
                "dist".to_string(),
                "out".to_string(),
                ".dart_tool".to_string(),
                ".git".to_string(),
                ".next".to_string(),
                "__pycache__".to_string(),
                "coverage".to_string(),
                "vendor".to_string(),
                // Risk 3: expanded defaults for Python/venv + cache dirs that
                // produce hundreds of borderline-significant entries.
                ".venv".to_string(),
                "venv".to_string(),
                "env".to_string(),
                ".tox".to_string(),
                ".mypy_cache".to_string(),
                ".pytest_cache".to_string(),
                ".ruff_cache".to_string(),
                ".gradle".to_string(),
                ".idea".to_string(),
                ".vscode".to_string(),
            ],
            scan_on_session_start: true,
            scan_on_structural_change: true,
            lazy_doc_pair_creation: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_enabled_defaults_to_true() {
        let cfg = PrismConfig::default();
        assert!(cfg.enabled);
    }

    #[test]
    fn test_enabled_backward_compat_missing_field() {
        let json = r#"{"version":"2.1.0"}"#;
        let cfg = PrismConfig::load_from_str(json).unwrap();
        assert!(cfg.enabled);
    }

    #[test]
    fn test_enabled_explicit_false() {
        let json = r#"{"version":"2.1.0","enabled":false}"#;
        let cfg = PrismConfig::load_from_str(json).unwrap();
        assert!(!cfg.enabled);
    }

    #[test]
    fn test_save_roundtrip_preserves_enabled() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("config.json");

        let cfg = PrismConfig {
            enabled: false,
            ..Default::default()
        };
        cfg.save(&path).unwrap();

        let loaded = PrismConfig::load(&path).unwrap();
        assert!(!loaded.enabled);
    }

    #[test]
    fn test_save_roundtrip_preserves_all_fields() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("config.json");

        let cfg = PrismConfig {
            enabled: false,
            freshness: FreshnessConfig {
                default_ttl_turns: 99,
                ..Default::default()
            },
            discovery: DiscoveryConfig {
                max_depth: 7,
                ..Default::default()
            },
            ..Default::default()
        };
        cfg.save(&path).unwrap();

        let loaded = PrismConfig::load(&path).unwrap();
        assert!(!loaded.enabled);
        assert_eq!(loaded.freshness.default_ttl_turns, 99);
        assert_eq!(loaded.discovery.max_depth, 7);
    }

    #[test]
    fn test_is_enabled_no_prism_dir() {
        let dir = TempDir::new().unwrap();
        // No .prism/ — fail-open returns true
        assert!(PrismConfig::is_enabled(dir.path()));
    }

    #[test]
    fn test_is_enabled_true_when_enabled() {
        let dir = TempDir::new().unwrap();
        std::fs::create_dir_all(dir.path().join(".prism")).unwrap();
        let cfg = PrismConfig::default();
        cfg.save(&dir.path().join(".prism/config.json")).unwrap();
        assert!(PrismConfig::is_enabled(dir.path()));
    }

    #[test]
    fn test_is_enabled_false_when_disabled() {
        let dir = TempDir::new().unwrap();
        std::fs::create_dir_all(dir.path().join(".prism")).unwrap();
        let cfg = PrismConfig {
            enabled: false,
            ..Default::default()
        };
        cfg.save(&dir.path().join(".prism/config.json")).unwrap();
        assert!(!PrismConfig::is_enabled(dir.path()));
    }

    #[test]
    fn test_is_enabled_walks_up_from_subdirectory() {
        let dir = TempDir::new().unwrap();
        std::fs::create_dir_all(dir.path().join(".prism")).unwrap();
        let subdir = dir.path().join("src/deep/nested");
        std::fs::create_dir_all(&subdir).unwrap();

        let cfg = PrismConfig {
            enabled: false,
            ..Default::default()
        };
        cfg.save(&dir.path().join(".prism/config.json")).unwrap();

        assert!(!PrismConfig::is_enabled(&subdir));
    }

    #[test]
    fn test_is_enabled_corrupted_config_fails_open() {
        let dir = TempDir::new().unwrap();
        std::fs::create_dir_all(dir.path().join(".prism")).unwrap();
        std::fs::write(dir.path().join(".prism/config.json"), "not json").unwrap();
        // Corrupted config → fail-open → true
        assert!(PrismConfig::is_enabled(dir.path()));
    }
}
