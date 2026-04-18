//! `CONTEXT_LINE_BUDGET` — CONTEXT.md ≤ 80 lines; reference files ≤ 200 lines.

use std::path::{Path, PathBuf};

use super::{is_markdown, list_stage_dirs};
use crate::icm::{IcmRule, IcmViolation};

pub const CONTEXT_MAX_LINES: usize = 80;
pub const REFERENCE_MAX_LINES: usize = 200;

pub fn check_project(project_root: &Path, out: &mut Vec<IcmViolation>) {
    // Root CONTEXT.md
    check_file(project_root, &project_root.join("CONTEXT.md"), out);
    // Per-stage CONTEXT.md
    for (stage_path, _) in list_stage_dirs(project_root) {
        check_file(project_root, &stage_path.join("CONTEXT.md"), out);
    }
    // Reference material: every `.md` file under `.prism/refs/` or `refs/`
    for refs_dir in [project_root.join("refs"), project_root.join(".prism/refs")] {
        if !refs_dir.is_dir() {
            continue;
        }
        for entry in walk_md(&refs_dir) {
            check_file(project_root, &entry, out);
        }
    }
}

pub fn check_file(project_root: &Path, abs: &Path, out: &mut Vec<IcmViolation>) {
    if !is_markdown(abs) || !abs.is_file() {
        return;
    }
    let Ok(content) = std::fs::read_to_string(abs) else {
        return;
    };
    let line_count = content.lines().count();
    let rel = abs.strip_prefix(project_root).unwrap_or(abs).to_path_buf();
    let is_context = abs
        .file_name()
        .and_then(|n| n.to_str())
        .map(|n| n.eq_ignore_ascii_case("CONTEXT.md"))
        .unwrap_or(false);
    let is_reference = rel_under(&rel, &PathBuf::from("refs"))
        || rel_under(&rel, &PathBuf::from(".prism/refs"));
    let (limit, label) = if is_context {
        (CONTEXT_MAX_LINES, "CONTEXT.md")
    } else if is_reference {
        (REFERENCE_MAX_LINES, "reference file")
    } else {
        return;
    };
    if line_count > limit {
        out.push(IcmViolation::at_file(
            IcmRule::ContextLineBudget,
            rel,
            format!("{label} has {line_count} lines (limit {limit})"),
        ));
    }
}

fn rel_under(rel: &Path, parent: &Path) -> bool {
    rel.components()
        .zip(parent.components())
        .all(|(a, b)| a == b)
        && rel.components().count() > parent.components().count()
}

fn walk_md(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        let Ok(rd) = std::fs::read_dir(&d) else {
            continue;
        };
        for entry in rd.flatten() {
            let path = entry.path();
            let Ok(ft) = entry.file_type() else { continue };
            if ft.is_dir() {
                stack.push(path);
            } else if is_markdown(&path) {
                out.push(path);
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn small_context_md_passes() {
        let dir = TempDir::new().unwrap();
        let ctx = dir.path().join("CONTEXT.md");
        std::fs::write(&ctx, "# tiny\n").unwrap();
        let mut v = Vec::new();
        check_file(dir.path(), &ctx, &mut v);
        assert!(v.is_empty());
    }

    #[test]
    fn oversized_context_md_is_flagged() {
        let dir = TempDir::new().unwrap();
        let ctx = dir.path().join("CONTEXT.md");
        let body: String = (0..120).map(|_| "line\n").collect();
        std::fs::write(&ctx, body).unwrap();
        let mut v = Vec::new();
        check_file(dir.path(), &ctx, &mut v);
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].rule, IcmRule::ContextLineBudget);
    }

    #[test]
    fn oversized_reference_file_is_flagged() {
        let dir = TempDir::new().unwrap();
        let refs = dir.path().join("refs");
        std::fs::create_dir_all(&refs).unwrap();
        let f = refs.join("big.md");
        let body: String = (0..250).map(|_| "l\n").collect();
        std::fs::write(&f, body).unwrap();
        let mut v = Vec::new();
        check_file(dir.path(), &f, &mut v);
        assert_eq!(v.len(), 1);
        assert!(v[0].message.contains("reference"));
    }

    #[test]
    fn non_context_non_ref_md_is_ignored_by_budget() {
        let dir = TempDir::new().unwrap();
        let f = dir.path().join("CLAUDE.md");
        let body: String = (0..500).map(|_| "l\n").collect();
        std::fs::write(&f, body).unwrap();
        let mut v = Vec::new();
        check_file(dir.path(), &f, &mut v);
        assert!(v.is_empty(), "CLAUDE.md has no hard line limit: {v:?}");
    }
}
