use std::env;

use anyhow::Context;
use serde_json::Value;

pub fn run() -> anyhow::Result<()> {
    let settings_path = env::current_dir()?.join(".claude/settings.json");
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
