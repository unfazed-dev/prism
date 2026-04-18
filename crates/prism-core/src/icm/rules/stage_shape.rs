//! `STAGE_FOLDER_SHAPE` — stage folders match `^\d{2}-[a-z0-9-]+$`,
//! sequential from `01`, no gaps.

use std::path::Path;

use super::list_stage_dirs;
use crate::icm::{IcmRule, IcmViolation};

pub fn check(project_root: &Path, out: &mut Vec<IcmViolation>) {
    let candidates = list_stage_dirs(project_root);
    if candidates.is_empty() {
        return;
    }
    let mut valid: Vec<(u32, String)> = Vec::new();
    for (path, name) in &candidates {
        match parse_stage(name) {
            Some(num) => valid.push((num, name.clone())),
            None => {
                out.push(IcmViolation::at_file(
                    IcmRule::StageFolderShape,
                    path.clone(),
                    format!(
                        "stage folder `{name}/` does not match `^\\d{{2}}-[a-z0-9-]+$`"
                    ),
                ));
            }
        }
    }
    if valid.is_empty() {
        return;
    }
    valid.sort_by_key(|(n, _)| *n);
    // starts at 01
    if valid[0].0 != 1 {
        out.push(IcmViolation::project(
            IcmRule::StageFolderShape,
            format!(
                "stage numbering must start at 01; found first stage `{}`",
                valid[0].1
            ),
        ));
    }
    for window in valid.windows(2) {
        let (prev_num, prev_name) = &window[0];
        let (cur_num, cur_name) = &window[1];
        if *cur_num != prev_num + 1 {
            out.push(IcmViolation::project(
                IcmRule::StageFolderShape,
                format!(
                    "stage numbering gap: `{prev_name}` → `{cur_name}` (expected {})",
                    prev_num + 1
                ),
            ));
        }
    }
}

fn parse_stage(name: &str) -> Option<u32> {
    if name.len() < 4 {
        return None;
    }
    let bytes = name.as_bytes();
    if !(bytes[0].is_ascii_digit() && bytes[1].is_ascii_digit() && bytes[2] == b'-') {
        return None;
    }
    let slug = &name[3..];
    if slug.is_empty() {
        return None;
    }
    for c in slug.chars() {
        let ok = c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-';
        if !ok {
            return None;
        }
    }
    name[..2].parse::<u32>().ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn mk_stages(dir: &Path, names: &[&str]) {
        for n in names {
            std::fs::create_dir_all(dir.join(n)).unwrap();
        }
    }

    #[test]
    fn sequential_stages_pass() {
        let dir = TempDir::new().unwrap();
        mk_stages(dir.path(), &["01-discovery", "02-exploration", "03-build"]);
        let mut v = Vec::new();
        check(dir.path(), &mut v);
        assert!(v.is_empty(), "{v:?}");
    }

    #[test]
    fn gap_is_flagged() {
        let dir = TempDir::new().unwrap();
        mk_stages(dir.path(), &["01-discovery", "03-build"]);
        let mut v = Vec::new();
        check(dir.path(), &mut v);
        assert_eq!(v.len(), 1);
        assert!(v[0].message.contains("gap"));
    }

    #[test]
    fn starting_above_one_is_flagged() {
        let dir = TempDir::new().unwrap();
        mk_stages(dir.path(), &["02-exploration"]);
        let mut v = Vec::new();
        check(dir.path(), &mut v);
        assert!(v.iter().any(|x| x.message.contains("must start at 01")));
    }

    #[test]
    fn underscore_separator_is_rejected() {
        let dir = TempDir::new().unwrap();
        mk_stages(dir.path(), &["01_discovery"]);
        let mut v = Vec::new();
        check(dir.path(), &mut v);
        assert!(v.iter().any(|x| x.rule == IcmRule::StageFolderShape));
    }

    #[test]
    fn uppercase_slug_is_rejected() {
        let dir = TempDir::new().unwrap();
        mk_stages(dir.path(), &["01-Discovery"]);
        let mut v = Vec::new();
        check(dir.path(), &mut v);
        assert!(v.iter().any(|x| x.rule == IcmRule::StageFolderShape));
    }

    #[test]
    fn parse_stage_accepts_hyphenated() {
        assert_eq!(parse_stage("01-discovery"), Some(1));
        assert_eq!(parse_stage("07-multi-word-stage"), Some(7));
        assert_eq!(parse_stage("01_discovery"), None);
        assert_eq!(parse_stage("1-discovery"), None);
        assert_eq!(parse_stage("01-"), None);
    }
}
