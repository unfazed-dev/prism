//! End-to-end integration tests for the `prism` CLI binary.
//!
//! Drives real subprocess invocations against a tempdir project root:
//!   1. `prism start` — scaffolds docs, creates db, installs hooks.
//!   2. `prism hook post-tool-use` — simulates an Edit hook via stdin JSON.
//!   3. `prism status` — asserts managed / drift / pending counts.
//!   4. `prism stop` — tears down.
//!
//! `prism enrich` is NOT exercised because it requires the `claude` CLI on PATH.

use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

use assert_cmd::prelude::*;
use predicates::str;
use tempfile::TempDir;

fn prism(project_root: &Path) -> Command {
    let mut cmd = Command::cargo_bin("prism").expect("prism binary");
    cmd.current_dir(project_root);
    cmd
}

fn run_hook(project_root: &Path, event: &str, stdin_json: &str) -> std::process::Output {
    let mut child = prism(project_root)
        .args(["hook", event])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn prism hook");
    child
        .stdin
        .as_mut()
        .expect("stdin")
        .write_all(stdin_json.as_bytes())
        .expect("write stdin");
    child.wait_with_output().expect("hook output")
}

#[test]
fn start_status_hook_pipeline() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();

    // prism start
    prism(root)
        .arg("start")
        .assert()
        .success()
        .stdout(str::contains("PRISM initialized"));

    assert!(root.join(".prism/prism.db").exists());
    assert!(root.join("CLAUDE.md").exists());
    assert!(root.join("CONTEXT.md").exists());
    assert!(root.join(".claude/settings.json").exists());

    // status immediately after start: docs>0, zero drift, zero enrich
    prism(root)
        .arg("status")
        .assert()
        .success()
        .stdout(str::contains("managed docs:"))
        .stdout(str::contains("unresolved drift:  0"))
        .stdout(str::contains("pending enrich:    0"));

    // create a source file, then simulate PostToolUse hook twice
    let src_rel = "src/foo.rs";
    std::fs::create_dir_all(root.join("src")).unwrap();
    std::fs::write(root.join(src_rel), "fn one() {}\n").unwrap();

    let hook_json = format!(
        r#"{{"hook_event_name":"PostToolUse","tool_name":"Write","tool_input":{{"file_path":"{}"}},"session_id":"test-session"}}"#,
        src_rel
    );

    let out1 = run_hook(root, "post-tool-use", &hook_json);
    assert!(out1.status.success(), "first hook failed: {:?}", out1);

    // rewrite with different content so post_tool_use sees content_changed=true
    std::fs::write(root.join(src_rel), "fn two() {}\n").unwrap();
    let out2 = run_hook(root, "post-tool-use", &hook_json);
    assert!(out2.status.success(), "second hook failed: {:?}", out2);

    // status after edits: 1 drift row, 1 pending enrich directive
    prism(root)
        .arg("status")
        .assert()
        .success()
        .stdout(str::contains("unresolved drift:  1"))
        .stdout(str::contains("pending enrich:    1"));

    // session-start hook should succeed and produce valid JSON output
    let session_json = r#"{"hook_event_name":"SessionStart","session_id":"test-session"}"#;
    let out = run_hook(root, "session-start", session_json);
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.starts_with('{') && stdout.ends_with('}'), "not JSON: {stdout}");

    // prism stop removes the hooks block from settings.json
    prism(root).arg("stop").assert().success();
    let settings: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(root.join(".claude/settings.json")).unwrap(),
    )
    .unwrap();
    assert!(
        settings.get("hooks").is_none(),
        "hooks block should be removed: {settings}"
    );

    // Assert schema budget: <= 6 tables (plan verification bullet).
    let conn = rusqlite::Connection::open(root.join(".prism/prism.db")).unwrap();
    let table_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert!(table_count <= 6, "schema budget exceeded: {table_count} tables");
}

#[test]
fn hook_is_noop_without_prism_dir() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    // no `prism start` run — .prism/ is absent
    let hook_json = r#"{"hook_event_name":"PostToolUse","tool_name":"Write","tool_input":{"file_path":"x.rs"},"session_id":"s"}"#;
    let out = run_hook(root, "post-tool-use", hook_json);
    assert!(out.status.success());
    // emits empty JSON allow with no system_message
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert_eq!(stdout.trim(), "{}");
}

#[test]
fn enrich_without_claude_cli_bails() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    prism(root).arg("start").assert().success();

    // Force a pending directive so `enrich` reaches the preflight.
    let src_rel = "src/foo.rs";
    std::fs::create_dir_all(root.join("src")).unwrap();
    std::fs::write(root.join(src_rel), "fn x() {}\n").unwrap();
    let hook_json = format!(
        r#"{{"hook_event_name":"PostToolUse","tool_name":"Write","tool_input":{{"file_path":"{}"}},"session_id":"s"}}"#,
        src_rel
    );
    let h = run_hook(root, "post-tool-use", &hook_json);
    assert!(h.status.success());

    // Invoke with an empty PATH so `claude` is unreachable.
    let output = prism(root)
        .arg("enrich")
        .env("PATH", "")
        .output()
        .expect("enrich output");
    assert!(
        !output.status.success(),
        "enrich should bail without claude on PATH; stdout={:?}",
        String::from_utf8_lossy(&output.stdout)
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("claude") && stderr.contains("PATH"),
        "expected actionable claude/PATH error, got: {stderr}"
    );
}

#[test]
fn status_on_uninitialized_project() {
    let tmp = TempDir::new().unwrap();
    prism(tmp.path())
        .arg("status")
        .assert()
        .success()
        .stdout(str::contains("PRISM not initialized"));
}
