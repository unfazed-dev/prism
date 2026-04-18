//! ICM (Interpreted Context Methodology) validator — pure, no IO side effects
//! beyond reading project files.
//!
//! Canonical spec: <https://github.com/RinDig/Interpreted-Context-Methdology>
//! (note repo name typo: "Methdology"). Canonical rule text lives in
//! `_core/CONVENTIONS.md`.

use std::path::{Path, PathBuf};

pub mod rules;

/// Stable identifiers for each lint rule. Formatted as strings in db rows
/// and user-facing output.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum IcmRule {
    L0Exists,
    L1Exists,
    L2OnePerStage,
    StageFolderShape,
    ContextLineBudget,
    StageContextSections,
    InputsTableColumns,
    NoEmDash,
}

impl IcmRule {
    pub fn id(&self) -> &'static str {
        match self {
            Self::L0Exists => "L0_EXISTS",
            Self::L1Exists => "L1_EXISTS",
            Self::L2OnePerStage => "L2_ONE_PER_STAGE",
            Self::StageFolderShape => "STAGE_FOLDER_SHAPE",
            Self::ContextLineBudget => "CONTEXT_LINE_BUDGET",
            Self::StageContextSections => "STAGE_CONTEXT_SECTIONS",
            Self::InputsTableColumns => "INPUTS_TABLE_COLUMNS",
            Self::NoEmDash => "NO_EM_DASH",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IcmViolation {
    pub rule: IcmRule,
    pub file: Option<PathBuf>,
    pub line: Option<usize>,
    pub message: String,
}

impl IcmViolation {
    pub fn project(rule: IcmRule, message: impl Into<String>) -> Self {
        Self {
            rule,
            file: None,
            line: None,
            message: message.into(),
        }
    }

    pub fn at_file(rule: IcmRule, file: PathBuf, message: impl Into<String>) -> Self {
        Self {
            rule,
            file: Some(file),
            line: None,
            message: message.into(),
        }
    }

    pub fn at_line(rule: IcmRule, file: PathBuf, line: usize, message: impl Into<String>) -> Self {
        Self {
            rule,
            file: Some(file),
            line: Some(line),
            message: message.into(),
        }
    }
}

#[derive(Debug, Clone)]
pub enum Scope {
    /// Run every applicable rule across the project.
    Project,
    /// Run only rules that apply to a single file.
    File(PathBuf),
}

/// Project-level config for optional rule relaxations. Defaults are
/// spec-accurate (strict).
#[derive(Debug, Clone, Copy, Default)]
pub struct IcmSettings {
    /// When true, `NO_EM_DASH` is skipped.
    pub allow_em_dash: bool,
}

/// Load `IcmSettings` from `.prism/config.json`, walking up from `project_root`
/// if absent. Fail-open on any error: default (strict) settings.
pub fn load_settings(project_root: &Path) -> IcmSettings {
    let cfg_path = project_root.join(".prism/config.json");
    if !cfg_path.exists() {
        return IcmSettings::default();
    }
    match crate::config::PrismConfig::load(&cfg_path) {
        Ok(cfg) => IcmSettings {
            allow_em_dash: cfg.icm.allow_em_dash,
        },
        Err(_) => IcmSettings::default(),
    }
}

/// Run the validator. Returns violations sorted by (file, line, rule id) for
/// stable output in tests and CLI.
pub fn validate_icm(project_root: &Path, scope: &Scope, settings: IcmSettings) -> Vec<IcmViolation> {
    let mut out = Vec::new();
    match scope {
        Scope::Project => {
            rules::layer_existence::check(project_root, &mut out);
            rules::stage_shape::check(project_root, &mut out);
            rules::budgets::check_project(project_root, &mut out);
            rules::sections::check_project(project_root, &mut out);
            rules::style::check_project(project_root, settings, &mut out);
        }
        Scope::File(rel) => {
            let abs = if rel.is_absolute() {
                rel.clone()
            } else {
                project_root.join(rel)
            };
            rules::budgets::check_file(project_root, &abs, &mut out);
            rules::sections::check_file(project_root, &abs, &mut out);
            rules::style::check_file(project_root, &abs, settings, &mut out);
        }
    }
    out.sort_by(|a, b| {
        a.file
            .as_deref()
            .cmp(&b.file.as_deref())
            .then(a.line.cmp(&b.line))
            .then(a.rule.id().cmp(b.rule.id()))
    });
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn rule_ids_are_stable() {
        assert_eq!(IcmRule::L0Exists.id(), "L0_EXISTS");
        assert_eq!(IcmRule::NoEmDash.id(), "NO_EM_DASH");
    }

    #[test]
    fn clean_project_has_no_violations() {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join("CLAUDE.md"), "# root\n").unwrap();
        std::fs::write(dir.path().join("CONTEXT.md"), "# routing\n").unwrap();
        let v = validate_icm(dir.path(), &Scope::Project, IcmSettings::default());
        assert!(v.is_empty(), "unexpected violations: {v:?}");
    }

    #[test]
    fn load_settings_defaults_strict_when_config_absent() {
        let dir = TempDir::new().unwrap();
        let s = load_settings(dir.path());
        assert!(!s.allow_em_dash);
    }

    #[test]
    fn load_settings_respects_allow_em_dash_override() {
        let dir = TempDir::new().unwrap();
        std::fs::create_dir_all(dir.path().join(".prism")).unwrap();
        std::fs::write(
            dir.path().join(".prism/config.json"),
            r#"{"version":"0.1.0","icm":{"allow_em_dash":true}}"#,
        )
        .unwrap();
        let s = load_settings(dir.path());
        assert!(s.allow_em_dash);
    }

    #[test]
    fn load_settings_fails_open_on_malformed_config() {
        let dir = TempDir::new().unwrap();
        std::fs::create_dir_all(dir.path().join(".prism")).unwrap();
        std::fs::write(dir.path().join(".prism/config.json"), "not json").unwrap();
        let s = load_settings(dir.path());
        assert!(!s.allow_em_dash);
    }

    #[test]
    fn missing_both_layers_reports_both() {
        let dir = TempDir::new().unwrap();
        let v = validate_icm(dir.path(), &Scope::Project, IcmSettings::default());
        let ids: Vec<&str> = v.iter().map(|r| r.rule.id()).collect();
        assert!(ids.contains(&"L0_EXISTS"));
        assert!(ids.contains(&"L1_EXISTS"));
    }
}
