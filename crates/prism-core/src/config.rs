//! PRISM configuration — minimal v2 shape needed by `prism enrich`.

use crate::PrismError;
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrismConfig {
    pub version: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub enrichment: EnrichmentConfig,
    #[serde(default)]
    pub icm: IcmConfigOverrides,
}

/// Per-project ICM rule overrides. Defaults are spec-accurate.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct IcmConfigOverrides {
    /// When true, `NO_EM_DASH` is skipped.
    #[serde(default)]
    pub allow_em_dash: bool,
}

fn default_true() -> bool {
    true
}

impl Default for PrismConfig {
    fn default() -> Self {
        Self {
            version: "0.1.0".to_string(),
            enabled: true,
            enrichment: EnrichmentConfig::default(),
            icm: IcmConfigOverrides::default(),
        }
    }
}

impl PrismConfig {
    pub fn load(path: &Path) -> Result<Self, PrismError> {
        let contents = std::fs::read_to_string(path)?;
        Self::load_from_str(&contents)
    }

    pub fn load_from_str(json: &str) -> Result<Self, PrismError> {
        Ok(serde_json::from_str(json)?)
    }

    pub fn save(&self, path: &Path) -> Result<(), PrismError> {
        std::fs::write(path, serde_json::to_string_pretty(self)?)?;
        Ok(())
    }

    /// Walk up from `cwd` to find `.prism/config.json`. Fail-open: any error
    /// (missing, unreadable, malformed) returns `true`.
    pub fn is_enabled(cwd: &Path) -> bool {
        let mut current = cwd.to_path_buf();
        loop {
            let config_path = current.join(".prism/config.json");
            if config_path.exists() {
                return Self::load(&config_path).map(|c| c.enabled).unwrap_or(true);
            }
            if !current.pop() {
                return true;
            }
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum InitialScaffoldMode {
    TemplateOnly,
    HaikuBackground,
    #[default]
    HaikuBlocking,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnrichmentConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_batch_size")]
    pub batch_size: usize,
    #[serde(default = "default_max_retries")]
    pub max_retries: u32,
    #[serde(default)]
    pub initial_scaffold_mode: InitialScaffoldMode,
    #[serde(default)]
    pub autopilot: AutopilotConfig,
}

fn default_batch_size() -> usize {
    5
}

fn default_max_retries() -> u32 {
    3
}

impl Default for EnrichmentConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            batch_size: default_batch_size(),
            max_retries: default_max_retries(),
            initial_scaffold_mode: InitialScaffoldMode::default(),
            autopilot: AutopilotConfig::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutopilotConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_autopilot_model")]
    pub model: String,
    #[serde(default = "default_autopilot_timeout")]
    pub timeout_secs: u64,
    #[serde(default = "default_autopilot_allowed_tools")]
    pub allowed_tools: Vec<String>,
}

fn default_autopilot_model() -> String {
    "claude-haiku-4-5".to_string()
}

fn default_autopilot_timeout() -> u64 {
    120
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
            enabled: true,
            model: default_autopilot_model(),
            timeout_secs: default_autopilot_timeout(),
            allowed_tools: default_autopilot_allowed_tools(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn enabled_default_true() {
        assert!(PrismConfig::default().enabled);
    }

    #[test]
    fn enabled_missing_field_defaults_true() {
        let cfg = PrismConfig::load_from_str(r#"{"version":"0.1.0"}"#).unwrap();
        assert!(cfg.enabled);
    }

    #[test]
    fn enabled_explicit_false() {
        let cfg =
            PrismConfig::load_from_str(r#"{"version":"0.1.0","enabled":false}"#).unwrap();
        assert!(!cfg.enabled);
    }

    #[test]
    fn save_roundtrip() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("config.json");
        let cfg = PrismConfig {
            enabled: false,
            ..Default::default()
        };
        cfg.save(&path).unwrap();
        assert!(!PrismConfig::load(&path).unwrap().enabled);
    }

    #[test]
    fn is_enabled_no_prism_dir_fail_open() {
        let dir = TempDir::new().unwrap();
        assert!(PrismConfig::is_enabled(dir.path()));
    }

    #[test]
    fn is_enabled_walks_up_from_subdir() {
        let dir = TempDir::new().unwrap();
        std::fs::create_dir_all(dir.path().join(".prism")).unwrap();
        let sub = dir.path().join("src/deep");
        std::fs::create_dir_all(&sub).unwrap();
        PrismConfig {
            enabled: false,
            ..Default::default()
        }
        .save(&dir.path().join(".prism/config.json"))
        .unwrap();
        assert!(!PrismConfig::is_enabled(&sub));
    }

    #[test]
    fn is_enabled_corrupted_config_fails_open() {
        let dir = TempDir::new().unwrap();
        std::fs::create_dir_all(dir.path().join(".prism")).unwrap();
        std::fs::write(dir.path().join(".prism/config.json"), "not json").unwrap();
        assert!(PrismConfig::is_enabled(dir.path()));
    }
}
