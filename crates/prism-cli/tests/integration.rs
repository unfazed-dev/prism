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
        .stdout(str::contains("source drift:      0"))
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
        .stdout(str::contains("source drift:      1"))
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
fn icm_lint_clean_project_succeeds() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    prism(root).arg("start").assert().success();
    prism(root)
        .arg("lint")
        .assert()
        .success()
        .stdout(str::contains("ICM: clean"));
}

#[test]
fn icm_lint_detects_missing_context_md() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    prism(root).arg("start").assert().success();
    std::fs::remove_file(root.join("CONTEXT.md")).unwrap();

    let output = prism(root).arg("lint").output().expect("lint output");
    assert!(!output.status.success(), "lint should exit non-zero");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("L1_EXISTS"), "stdout: {stdout}");
}

#[test]
fn icm_violation_detected_and_queued_by_hook() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    prism(root).arg("start").assert().success();

    // Seed a clearly-violating stage CONTEXT.md: missing Inputs/Process/Outputs,
    // em dash present, 100-line body.
    let stage = root.join("01-discovery");
    std::fs::create_dir_all(&stage).unwrap();
    let rel = "01-discovery/CONTEXT.md";
    let body: String = (0..100)
        .map(|_| "body \u{2014} line\n")
        .collect::<String>();
    std::fs::write(root.join(rel), format!("# bad\n{body}")).unwrap();

    let hook_json = format!(
        r#"{{"hook_event_name":"PostToolUse","tool_name":"Write","tool_input":{{"file_path":"{}"}},"session_id":"s"}}"#,
        rel
    );
    let out = run_hook(root, "post-tool-use", &hook_json);
    assert!(out.status.success(), "hook failed: {out:?}");

    // status should reflect at least one icm violation and one pending fix
    let stdout = String::from_utf8(
        prism(root)
            .arg("status")
            .output()
            .expect("status output")
            .stdout,
    )
    .unwrap();
    assert!(stdout.contains("icm violations:"), "{stdout}");
    assert!(stdout.contains("pending fix:"), "{stdout}");
    assert!(
        !stdout.contains("icm violations:    0"),
        "expected >0 icm violations: {stdout}"
    );
    assert!(
        !stdout.contains("pending fix:       0"),
        "expected >0 pending fix: {stdout}"
    );
}

#[test]
fn session_start_surfaces_icm_violations_in_message() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    prism(root).arg("start").assert().success();

    // Seed one ICM violation via a managed-md edit.
    std::fs::create_dir_all(root.join("01-discovery")).unwrap();
    let rel = "01-discovery/CONTEXT.md";
    std::fs::write(root.join(rel), "# bad\n").unwrap();
    let hook_json = format!(
        r#"{{"hook_event_name":"PostToolUse","tool_name":"Write","tool_input":{{"file_path":"{}"}},"session_id":"s"}}"#,
        rel
    );
    let out = run_hook(root, "post-tool-use", &hook_json);
    assert!(out.status.success());

    // Session-start should now emit a message referencing ICM state + pending fix.
    let session_json = r#"{"hook_event_name":"SessionStart","session_id":"s"}"#;
    let out = run_hook(root, "session-start", session_json);
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("ICM violation") && stdout.contains("pending fix"),
        "session-start message missing ICM surface: {stdout}"
    );
}

#[test]
fn duplicate_hook_fires_do_not_accumulate_drift_rows() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    prism(root).arg("start").assert().success();

    std::fs::create_dir_all(root.join("01-discovery")).unwrap();
    let rel = "01-discovery/CONTEXT.md";
    std::fs::write(root.join(rel), "# bad\n").unwrap();
    let hook_json = format!(
        r#"{{"hook_event_name":"PostToolUse","tool_name":"Write","tool_input":{{"file_path":"{}"}},"session_id":"s"}}"#,
        rel
    );
    for _ in 0..3 {
        let out = run_hook(root, "post-tool-use", &hook_json);
        assert!(out.status.success());
    }

    let conn = rusqlite::Connection::open(root.join(".prism/prism.db")).unwrap();
    let distinct: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM doc_drift WHERE drift_type = 'IcmViolation' AND resolved = 0",
            [],
            |r| r.get(0),
        )
        .unwrap();
    let grouped: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM (SELECT DISTINCT affected_doc, description FROM doc_drift WHERE drift_type = 'IcmViolation' AND resolved = 0)",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(
        distinct, grouped,
        "expected one row per (doc, description); got {distinct} rows vs {grouped} distinct"
    );
}

#[test]
fn allow_em_dash_config_disables_em_dash_rule() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    prism(root).arg("start").assert().success();
    std::fs::create_dir_all(root.join(".prism")).unwrap();
    std::fs::write(
        root.join(".prism/config.json"),
        r#"{"version":"0.1.0","icm":{"allow_em_dash":true}}"#,
    )
    .unwrap();

    // Introduce em dash into root CONTEXT.md; no other violations.
    std::fs::write(
        root.join("CONTEXT.md"),
        "# routing\n\nPoints \u{2014} to stages.\n",
    )
    .unwrap();

    let output = prism(root).arg("lint").output().expect("lint output");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success(),
        "lint should pass when allow_em_dash=true; stdout={stdout}"
    );
    assert!(stdout.contains("ICM: clean"), "{stdout}");
}

#[test]
fn prism_fix_without_claude_cli_bails() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    prism(root).arg("start").assert().success();

    // Seed one violating managed md + hook to queue a FIX_ICM directive.
    std::fs::create_dir_all(root.join("01-discovery")).unwrap();
    let rel = "01-discovery/CONTEXT.md";
    std::fs::write(root.join(rel), "# bad\n").unwrap();
    let hook_json = format!(
        r#"{{"hook_event_name":"PostToolUse","tool_name":"Write","tool_input":{{"file_path":"{}"}},"session_id":"s"}}"#,
        rel
    );
    let h = run_hook(root, "post-tool-use", &hook_json);
    assert!(h.status.success());

    let output = prism(root)
        .arg("fix")
        .env("PATH", "")
        .output()
        .expect("fix output");
    assert!(
        !output.status.success(),
        "fix should bail without claude on PATH"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("claude") && stderr.contains("PATH"), "{stderr}");
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
