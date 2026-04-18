use std::env;
use std::path::PathBuf;

use anyhow::Context;
use prism_core::templates::scaffold::{scaffold_directory, ScaffoldOptions};
use prism_db::PrismDb;
use serde_json::{json, Value};

pub fn run() -> anyhow::Result<()> {
    let project_root = env::current_dir().context("read cwd")?;
    let prism_dir = project_root.join(".prism");
    std::fs::create_dir_all(&prism_dir)?;

    let db_path = prism_dir.join("prism.db");
    let _ = PrismDb::open(&db_path).context("init database")?;

    let claude_dir = project_root.join(".claude");
    std::fs::create_dir_all(&claude_dir)?;
    install_hooks(&claude_dir.join("settings.json"))?;

    let project_name = project_root
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("project")
        .to_string();

    let opts = ScaffoldOptions {
        directory_name: project_name.clone(),
        is_root: true,
        project_name: Some(project_name),
        ..Default::default()
    };
    let outcomes = scaffold_directory(&project_root, &opts)?;

    println!("PRISM initialized at {}", project_root.display());
    println!("  database: {}", db_path.display());
    println!("  scaffold: {} outcome(s)", outcomes.len());
    Ok(())
}

fn install_hooks(settings_path: &PathBuf) -> anyhow::Result<()> {
    let existing: Value = if settings_path.exists() {
        let s = std::fs::read_to_string(settings_path)?;
        serde_json::from_str(&s).unwrap_or(json!({}))
    } else {
        json!({})
    };

    let mut root = existing
        .as_object()
        .cloned()
        .unwrap_or_default();

    let prism_bin = which_prism();
    let hooks_obj = json!({
        "SessionStart": [{
            "matcher": "",
            "hooks": [{ "type": "command", "command": format!("{} hook session-start", prism_bin) }]
        }],
        "PostToolUse": [{
            "matcher": "Write|Edit",
            "hooks": [{ "type": "command", "command": format!("{} hook post-tool-use", prism_bin) }]
        }]
    });
    root.insert("hooks".to_string(), hooks_obj);

    let content = serde_json::to_string_pretty(&Value::Object(root))?;
    std::fs::write(settings_path, content)?;
    Ok(())
}

fn which_prism() -> String {
    env::var("PRISM_BIN").unwrap_or_else(|_| "prism".to_string())
}
