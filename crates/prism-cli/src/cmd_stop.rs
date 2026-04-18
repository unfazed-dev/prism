use std::env;
use std::path::Path;

use anyhow::Context;
use serde_json::Value;

pub fn run() -> anyhow::Result<()> {
    run_at(&env::current_dir()?)
}

pub fn run_at(project_root: &Path) -> anyhow::Result<()> {
    let settings_path = project_root.join(".claude/settings.json");
    if !settings_path.exists() {
        println!("No .claude/settings.json found; nothing to stop.");
        return Ok(());
    }

    let content = std::fs::read_to_string(&settings_path).context("read settings")?;
    let mut value: Value = serde_json::from_str(&content)?;

    if let Some(obj) = value.as_object_mut() {
        obj.remove("hooks");
    }

    let out = serde_json::to_string_pretty(&value)?;
    std::fs::write(&settings_path, out)?;
    println!("PRISM hooks removed from {}", settings_path.display());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn no_settings_file_is_noop() {
        let dir = TempDir::new().unwrap();
        run_at(dir.path()).unwrap();
        assert!(!dir.path().join(".claude/settings.json").exists());
    }

    #[test]
    fn removes_hooks_key_preserves_rest() {
        let dir = TempDir::new().unwrap();
        let settings_path = dir.path().join(".claude/settings.json");
        std::fs::create_dir_all(settings_path.parent().unwrap()).unwrap();
        std::fs::write(
            &settings_path,
            r#"{"hooks":{"SessionStart":[{"hooks":[]}]},"model":"sonnet"}"#,
        )
        .unwrap();
        run_at(dir.path()).unwrap();
        let after = std::fs::read_to_string(&settings_path).unwrap();
        let parsed: Value = serde_json::from_str(&after).unwrap();
        assert!(parsed.get("hooks").is_none());
        assert_eq!(parsed.get("model").and_then(|v| v.as_str()), Some("sonnet"));
    }

    #[test]
    fn settings_without_hooks_is_left_intact() {
        let dir = TempDir::new().unwrap();
        let settings_path = dir.path().join(".claude/settings.json");
        std::fs::create_dir_all(settings_path.parent().unwrap()).unwrap();
        std::fs::write(&settings_path, r#"{"model":"sonnet"}"#).unwrap();
        run_at(dir.path()).unwrap();
        let after = std::fs::read_to_string(&settings_path).unwrap();
        let parsed: Value = serde_json::from_str(&after).unwrap();
        assert_eq!(parsed.get("model").and_then(|v| v.as_str()), Some("sonnet"));
    }

    #[test]
    fn malformed_settings_errors() {
        let dir = TempDir::new().unwrap();
        let settings_path = dir.path().join(".claude/settings.json");
        std::fs::create_dir_all(settings_path.parent().unwrap()).unwrap();
        std::fs::write(&settings_path, "not json").unwrap();
        assert!(run_at(dir.path()).is_err());
    }
}
