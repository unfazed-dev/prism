//! `STAGE_CONTEXT_SECTIONS`, `INPUTS_TABLE_COLUMNS` — stage CONTEXT.md shape.

use std::path::Path;

use super::list_stage_dirs;
use crate::icm::{IcmRule, IcmViolation};

const REQUIRED_SECTIONS: [&str; 3] = ["Inputs", "Process", "Outputs"];
const INPUTS_COLUMNS: [&str; 4] = ["Source", "File/Location", "Section/Scope", "Why"];

pub fn check_project(project_root: &Path, out: &mut Vec<IcmViolation>) {
    for (stage_path, _) in list_stage_dirs(project_root) {
        let ctx = stage_path.join("CONTEXT.md");
        check_file(project_root, &ctx, out);
    }
}

pub fn check_file(project_root: &Path, abs: &Path, out: &mut Vec<IcmViolation>) {
    if !is_stage_context_md(project_root, abs) {
        return;
    }
    let Ok(content) = std::fs::read_to_string(abs) else {
        return;
    };
    let rel = abs.strip_prefix(project_root).unwrap_or(abs).to_path_buf();
    check_sections(&content, &rel, out);
    check_inputs_table(&content, &rel, out);
}

fn is_stage_context_md(project_root: &Path, abs: &Path) -> bool {
    if abs.file_name().and_then(|n| n.to_str()) != Some("CONTEXT.md") {
        return false;
    }
    let Some(parent) = abs.parent() else {
        return false;
    };
    let Ok(rel_parent) = parent.strip_prefix(project_root) else {
        return false;
    };
    let Some(name) = rel_parent.file_name().and_then(|n| n.to_str()) else {
        return false;
    };
    name.len() >= 3
        && name.as_bytes()[0].is_ascii_digit()
        && name.as_bytes()[1].is_ascii_digit()
        && name.as_bytes()[2] == b'-'
}

fn check_sections(content: &str, rel: &Path, out: &mut Vec<IcmViolation>) {
    for required in REQUIRED_SECTIONS {
        if !has_heading(content, required) {
            out.push(IcmViolation::at_file(
                IcmRule::StageContextSections,
                rel.to_path_buf(),
                format!("missing heading `## {required}`"),
            ));
        }
    }
}

fn has_heading(content: &str, name: &str) -> bool {
    for line in content.lines() {
        let t = line.trim_start_matches('#').trim();
        if t.eq_ignore_ascii_case(name) && line.trim_start().starts_with('#') {
            return true;
        }
    }
    false
}

fn check_inputs_table(content: &str, rel: &Path, out: &mut Vec<IcmViolation>) {
    // Find the first markdown table header line after a line starting with
    // `## Inputs` (case-insensitive). If no table exists after the heading,
    // the section is prose-only and we skip.
    let mut in_inputs = false;
    for (i, line) in content.lines().enumerate() {
        let trimmed = line.trim_start_matches('#').trim();
        if line.trim_start().starts_with('#') && trimmed.eq_ignore_ascii_case("Inputs") {
            in_inputs = true;
            continue;
        }
        if in_inputs {
            if line.trim_start().starts_with('#') {
                // next heading reached; no table found
                return;
            }
            if line.starts_with('|') {
                let cols: Vec<String> = line
                    .split('|')
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect();
                if cols.len() != INPUTS_COLUMNS.len()
                    || !cols
                        .iter()
                        .zip(INPUTS_COLUMNS.iter())
                        .all(|(a, b)| a.eq_ignore_ascii_case(b))
                {
                    out.push(IcmViolation::at_line(
                        IcmRule::InputsTableColumns,
                        rel.to_path_buf(),
                        i + 1,
                        format!(
                            "Inputs table columns must be `{} | {} | {} | {}`, found `{}`",
                            INPUTS_COLUMNS[0],
                            INPUTS_COLUMNS[1],
                            INPUTS_COLUMNS[2],
                            INPUTS_COLUMNS[3],
                            cols.join(" | ")
                        ),
                    ));
                }
                return;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn stage_dir(root: &Path) -> std::path::PathBuf {
        let s = root.join("01-discovery");
        std::fs::create_dir_all(&s).unwrap();
        s
    }

    #[test]
    fn missing_outputs_heading_is_flagged() {
        let dir = TempDir::new().unwrap();
        let ctx = stage_dir(dir.path()).join("CONTEXT.md");
        std::fs::write(
            &ctx,
            "# discovery\n\n## Inputs\n\n## Process\n",
        )
        .unwrap();
        let mut v = Vec::new();
        check_file(dir.path(), &ctx, &mut v);
        assert_eq!(v.len(), 1);
        assert!(v[0].message.contains("Outputs"));
    }

    #[test]
    fn all_three_headings_pass() {
        let dir = TempDir::new().unwrap();
        let ctx = stage_dir(dir.path()).join("CONTEXT.md");
        std::fs::write(
            &ctx,
            "# discovery\n\n## Inputs\n\n## Process\n\n## Outputs\n",
        )
        .unwrap();
        let mut v = Vec::new();
        check_file(dir.path(), &ctx, &mut v);
        assert!(v.is_empty(), "{v:?}");
    }

    #[test]
    fn non_stage_context_md_is_skipped() {
        let dir = TempDir::new().unwrap();
        let ctx = dir.path().join("CONTEXT.md");
        std::fs::write(&ctx, "# routing\n").unwrap();
        let mut v = Vec::new();
        check_file(dir.path(), &ctx, &mut v);
        assert!(v.is_empty());
    }

    #[test]
    fn inputs_table_wrong_columns_is_flagged() {
        let dir = TempDir::new().unwrap();
        let ctx = stage_dir(dir.path()).join("CONTEXT.md");
        let body = "# s\n\n## Inputs\n\n| From | Where | Why |\n|---|---|---|\n\n## Process\n\n## Outputs\n";
        std::fs::write(&ctx, body).unwrap();
        let mut v = Vec::new();
        check_file(dir.path(), &ctx, &mut v);
        assert!(v.iter().any(|x| x.rule == IcmRule::InputsTableColumns));
    }

    #[test]
    fn inputs_table_correct_columns_passes() {
        let dir = TempDir::new().unwrap();
        let ctx = stage_dir(dir.path()).join("CONTEXT.md");
        let body = "# s\n\n## Inputs\n\n| Source | File/Location | Section/Scope | Why |\n|---|---|---|---|\n\n## Process\n\n## Outputs\n";
        std::fs::write(&ctx, body).unwrap();
        let mut v = Vec::new();
        check_file(dir.path(), &ctx, &mut v);
        assert!(v.iter().all(|x| x.rule != IcmRule::InputsTableColumns));
    }

    #[test]
    fn inputs_section_without_table_is_ok() {
        let dir = TempDir::new().unwrap();
        let ctx = stage_dir(dir.path()).join("CONTEXT.md");
        let body = "# s\n\n## Inputs\n\nNone.\n\n## Process\n\n## Outputs\n";
        std::fs::write(&ctx, body).unwrap();
        let mut v = Vec::new();
        check_file(dir.path(), &ctx, &mut v);
        assert!(v.iter().all(|x| x.rule != IcmRule::InputsTableColumns));
    }
}
