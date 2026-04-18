//! Hook I/O protocol — stdin JSON parsing, stdout JSON formatting.
//!
//! Claude Code hooks receive JSON on stdin and write JSON to stdout.

use serde::{Deserialize, Serialize};
use std::io::{self, BufRead, Write};
use std::path::{Path, PathBuf};

/// Input received from Claude Code via stdin JSON.
#[derive(Debug, Clone, Deserialize)]
pub struct HookInput {
    /// The hook event name (e.g. "SessionStart", "PostToolUse").
    pub hook_event_name: String,
    /// The tool name that triggered the hook (for Pre/PostToolUse).
    pub tool_name: Option<String>,
    /// The tool input JSON (for Pre/PostToolUse).
    pub tool_input: Option<serde_json::Value>,
    /// The current session ID.
    pub session_id: Option<String>,
}

/// Output sent back to Claude Code via stdout JSON.
#[derive(Debug, Clone, Serialize, Default)]
pub struct HookOutput {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system_message: Option<String>,
}

impl HookOutput {
    /// Create an allow output with an optional system message to inject.
    pub fn allow(message: Option<String>) -> Self {
        Self {
            system_message: message,
        }
    }
}

/// Shared context for all hook handlers.
pub struct HookContext {
    pub project_root: PathBuf,
    pub session_id: String,
}

impl HookContext {
    /// Create a new hook context by walking up from `cwd` for `.prism/`.
    pub fn from_cwd(cwd: &Path, session_id: &str) -> Option<Self> {
        let mut current = cwd.to_path_buf();
        loop {
            if current.join(".prism").is_dir() {
                return Some(Self {
                    project_root: current,
                    session_id: session_id.to_string(),
                });
            }
            if !current.pop() {
                return None;
            }
        }
    }

    /// Create a hook context with explicit paths (fallback).
    pub fn new(project_root: PathBuf, session_id: &str) -> Self {
        Self {
            project_root,
            session_id: session_id.to_string(),
        }
    }
}

/// Extract the file path from a tool input JSON, if present.
pub fn extract_file_path(tool_input: &serde_json::Value) -> Option<String> {
    tool_input
        .get("file_path")
        .or_else(|| tool_input.get("path"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
}

/// Read hook input from stdin.
pub fn read_input() -> Result<HookInput, crate::hooks::HookError> {
    let stdin = io::stdin();
    let mut buf = String::new();
    for line in stdin.lock().lines() {
        buf.push_str(&line?);
    }
    Ok(serde_json::from_str(&buf)?)
}

/// Write hook output to stdout.
pub fn write_output(output: &HookOutput) -> Result<(), crate::hooks::HookError> {
    let json = serde_json::to_string(output)?;
    let mut stdout = io::stdout().lock();
    stdout.write_all(json.as_bytes())?;
    stdout.flush()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn hook_output_allow_empty_serializes_to_empty_object() {
        let json = serde_json::to_string(&HookOutput::allow(None)).unwrap();
        assert_eq!(json, "{}");
    }

    #[test]
    fn hook_output_allow_with_message_serializes_field() {
        let json = serde_json::to_string(&HookOutput::allow(Some("hi".into()))).unwrap();
        assert!(json.contains("\"system_message\":\"hi\""));
    }

    #[test]
    fn hook_input_deserializes_tool_event() {
        let json = r#"{
            "hook_event_name": "PostToolUse",
            "tool_name": "Write",
            "tool_input": {"file_path": "/tmp/test.md"},
            "session_id": "s1"
        }"#;
        let input: HookInput = serde_json::from_str(json).unwrap();
        assert_eq!(input.tool_name.as_deref(), Some("Write"));
        assert_eq!(
            extract_file_path(input.tool_input.as_ref().unwrap()).as_deref(),
            Some("/tmp/test.md")
        );
    }

    #[test]
    fn extract_file_path_from_path_key_fallback() {
        let v = serde_json::json!({"path": "/tmp/other.rs"});
        assert_eq!(extract_file_path(&v).as_deref(), Some("/tmp/other.rs"));
    }

    #[test]
    fn extract_file_path_missing_returns_none() {
        let v = serde_json::json!({"content": "x"});
        assert!(extract_file_path(&v).is_none());
    }

    #[test]
    fn extract_file_path_non_string_value_returns_none() {
        let v = serde_json::json!({"file_path": 123});
        assert!(extract_file_path(&v).is_none());
    }

    #[test]
    fn from_cwd_finds_prism_dir_walking_up() {
        let dir = TempDir::new().unwrap();
        std::fs::create_dir_all(dir.path().join(".prism")).unwrap();
        let sub = dir.path().join("src/deep");
        std::fs::create_dir_all(&sub).unwrap();
        let ctx = HookContext::from_cwd(&sub, "s").unwrap();
        assert_eq!(ctx.project_root, dir.path());
    }

    #[test]
    fn hook_context_new_sets_fields() {
        let ctx = HookContext::new(PathBuf::from("/tmp/p"), "sess");
        assert_eq!(ctx.project_root, PathBuf::from("/tmp/p"));
        assert_eq!(ctx.session_id, "sess");
    }
}
