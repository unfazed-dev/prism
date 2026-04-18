//! `L0_EXISTS`, `L1_EXISTS`, `L2_ONE_PER_STAGE` — layer presence checks.

use std::path::Path;

use super::list_stage_dirs;
use crate::icm::{IcmRule, IcmViolation};

pub fn check(project_root: &Path, out: &mut Vec<IcmViolation>) {
    if !project_root.join("CLAUDE.md").is_file() {
        out.push(IcmViolation::project(
            IcmRule::L0Exists,
            "root CLAUDE.md is missing",
        ));
    }
    if !project_root.join("CONTEXT.md").is_file() {
        out.push(IcmViolation::project(
            IcmRule::L1Exists,
            "root CONTEXT.md is missing",
        ));
    }
    for (stage_path, stage_name) in list_stage_dirs(project_root) {
        let ctx = stage_path.join("CONTEXT.md");
        if !ctx.is_file() {
            out.push(IcmViolation::at_file(
                IcmRule::L2OnePerStage,
                ctx,
                format!("stage folder `{stage_name}/` is missing CONTEXT.md"),
            ));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn detects_missing_l0_and_l1() {
        let dir = TempDir::new().unwrap();
        let mut v = Vec::new();
        check(dir.path(), &mut v);
        let ids: Vec<&str> = v.iter().map(|r| r.rule.id()).collect();
        assert!(ids.contains(&"L0_EXISTS"));
        assert!(ids.contains(&"L1_EXISTS"));
    }

    #[test]
    fn clean_root_passes() {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join("CLAUDE.md"), "# x\n").unwrap();
        std::fs::write(dir.path().join("CONTEXT.md"), "# y\n").unwrap();
        let mut v = Vec::new();
        check(dir.path(), &mut v);
        assert!(v.is_empty());
    }

    #[test]
    fn stage_folder_without_context_md_is_flagged() {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join("CLAUDE.md"), "# x\n").unwrap();
        std::fs::write(dir.path().join("CONTEXT.md"), "# y\n").unwrap();
        std::fs::create_dir_all(dir.path().join("01-discovery")).unwrap();
        let mut v = Vec::new();
        check(dir.path(), &mut v);
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].rule, IcmRule::L2OnePerStage);
    }

    #[test]
    fn stage_folder_with_context_md_passes() {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join("CLAUDE.md"), "# x\n").unwrap();
        std::fs::write(dir.path().join("CONTEXT.md"), "# y\n").unwrap();
        let stage = dir.path().join("01-discovery");
        std::fs::create_dir_all(&stage).unwrap();
        std::fs::write(stage.join("CONTEXT.md"), "# inputs\n").unwrap();
        let mut v = Vec::new();
        check(dir.path(), &mut v);
        assert!(v.is_empty());
    }
}
