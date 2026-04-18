//! `NO_EM_DASH` — the ICM spec bans em dashes in prose.

use std::path::Path;

use super::{is_markdown, list_stage_dirs};
use crate::icm::{IcmRule, IcmSettings, IcmViolation};

const EM_DASH: char = '\u{2014}';

pub fn check_project(project_root: &Path, settings: IcmSettings, out: &mut Vec<IcmViolation>) {
    if settings.allow_em_dash {
        return;
    }
    for rel in [
        "CLAUDE.md",
        "CONTEXT.md",
    ] {
        let p = project_root.join(rel);
        if p.is_file() {
            check_file(project_root, &p, settings, out);
        }
    }
    for (stage_path, _) in list_stage_dirs(project_root) {
        let ctx = stage_path.join("CONTEXT.md");
        if ctx.is_file() {
            check_file(project_root, &ctx, settings, out);
        }
    }
}

pub fn check_file(
    project_root: &Path,
    abs: &Path,
    settings: IcmSettings,
    out: &mut Vec<IcmViolation>,
) {
    if settings.allow_em_dash {
        return;
    }
    if !is_markdown(abs) || !abs.is_file() {
        return;
    }
    let Ok(content) = std::fs::read_to_string(abs) else {
        return;
    };
    let rel = abs.strip_prefix(project_root).unwrap_or(abs).to_path_buf();
    for (i, line) in content.lines().enumerate() {
        if line.contains(EM_DASH) {
            out.push(IcmViolation::at_line(
                IcmRule::NoEmDash,
                rel.clone(),
                i + 1,
                "em dash (U+2014) is banned by the ICM spec — use hyphen-minus or rephrase",
            ));
            break;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn em_dash_is_flagged() {
        let dir = TempDir::new().unwrap();
        let f = dir.path().join("CLAUDE.md");
        std::fs::write(&f, "# root\n\nHere \u{2014} something.\n").unwrap();
        let mut v = Vec::new();
        check_file(dir.path(), &f, IcmSettings::default(), &mut v);
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].rule, IcmRule::NoEmDash);
        assert_eq!(v[0].line, Some(3));
    }

    #[test]
    fn hyphen_minus_passes() {
        let dir = TempDir::new().unwrap();
        let f = dir.path().join("CLAUDE.md");
        std::fs::write(&f, "# root\n\nHere - something.\n").unwrap();
        let mut v = Vec::new();
        check_file(dir.path(), &f, IcmSettings::default(), &mut v);
        assert!(v.is_empty());
    }

    #[test]
    fn allow_em_dash_setting_disables_rule() {
        let dir = TempDir::new().unwrap();
        let f = dir.path().join("CLAUDE.md");
        std::fs::write(&f, "# root\n\nHere \u{2014} something.\n").unwrap();
        let mut v = Vec::new();
        check_file(
            dir.path(),
            &f,
            IcmSettings {
                allow_em_dash: true,
            },
            &mut v,
        );
        assert!(v.is_empty());
    }
}
