use std::env;
use std::path::{Path, PathBuf};

use anyhow::Context;
use prism_core::templates::scaffold::{
    scaffold_directory, scaffold_prism_dir, scaffold_rules, ScaffoldOptions,
};
use prism_db::document_registry::{upsert, DocumentRegistryRow};
use prism_db::PrismDb;
use serde_json::{json, Value};

pub fn run() -> anyhow::Result<()> {
    let project_root = env::current_dir().context("read cwd")?;
    let prism_dir = project_root.join(".prism");
    std::fs::create_dir_all(&prism_dir)?;

    let db_path = prism_dir.join("prism.db");
    let db = PrismDb::open(&db_path).context("init database")?;

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
        project_name: Some(project_name.clone()),
        ..Default::default()
    };
    let root_outcomes = scaffold_directory(&project_root, &opts)?;
    let prism_dir_files = scaffold_prism_dir(&project_root, &project_name)?;
    let rules_files = scaffold_rules(&project_root, &project_name)?;

    register_managed_files(&db, &project_root, &["CLAUDE.md", "CONTEXT.md"])?;
    register_paths(&db, &project_root, &prism_dir_files)?;
    register_paths(&db, &project_root, &rules_files)?;

    println!("PRISM initialized at {}", project_root.display());
    println!("  database: {}", db_path.display());
    println!(
        "  scaffolded: {} root + {} refs/prism + {} rules",
        root_outcomes.len(),
        prism_dir_files.len(),
        rules_files.len()
    );
    Ok(())
}

fn register_managed_files(
    db: &PrismDb,
    project_root: &Path,
    relative_paths: &[&str],
) -> anyhow::Result<()> {
    for rel in relative_paths {
        let abs = project_root.join(rel);
        if abs.exists() {
            upsert_doc(db, rel, &abs)?;
        }
    }
    Ok(())
}

fn register_paths(db: &PrismDb, project_root: &Path, paths: &[String]) -> anyhow::Result<()> {
    for abs_str in paths {
        let abs = Path::new(abs_str);
        let rel = abs
            .strip_prefix(project_root)
            .unwrap_or(abs)
            .to_string_lossy()
            .to_string();
        upsert_doc(db, &rel, abs)?;
    }
    Ok(())
}

fn upsert_doc(db: &PrismDb, rel_path: &str, abs_path: &Path) -> anyhow::Result<()> {
    let now = chrono::Utc::now().to_rfc3339();
    let title = abs_path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or(rel_path)
        .to_string();
    upsert(
        db.conn(),
        &DocumentRegistryRow {
            doc_id: rel_path.to_string(),
            title,
            description: None,
            doc_type: "derived".to_string(),
            layer: None,
            classification: "reference".to_string(),
            status: "active".to_string(),
            version: "0.1.0".to_string(),
            created_at: now.clone(),
            last_synced: now,
            last_synced_by: "prism start".to_string(),
            review_date: None,
            token_budget: None,
            token_estimate: None,
            source_hash: None,
            parent_dir: abs_path
                .parent()
                .and_then(|p| p.to_str())
                .map(|s| s.to_string()),
            origin: "scaffold".to_string(),
        },
    )?;
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
