pub mod budgets;
pub mod layer_existence;
pub mod sections;
pub mod stage_shape;
pub mod style;

use std::path::Path;

/// Walk the project root for top-level directory entries that match the ICM
/// stage-folder shape. Used by several rules.
pub(crate) fn list_stage_dirs(project_root: &Path) -> Vec<(std::path::PathBuf, String)> {
    let Ok(rd) = std::fs::read_dir(project_root) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for entry in rd.flatten() {
        let Ok(ft) = entry.file_type() else { continue };
        if !ft.is_dir() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().to_string();
        // only consider candidate stage dirs: begin with two digits then separator
        if name.len() >= 3
            && name.as_bytes()[0].is_ascii_digit()
            && name.as_bytes()[1].is_ascii_digit()
        {
            out.push((entry.path(), name));
        }
    }
    out.sort_by(|a, b| a.1.cmp(&b.1));
    out
}

/// True when the file is a managed `.md` (CLAUDE.md, CONTEXT.md, or template-derived).
pub(crate) fn is_markdown(abs: &Path) -> bool {
    abs.extension()
        .and_then(|e| e.to_str())
        .map(|ext| ext.eq_ignore_ascii_case("md"))
        .unwrap_or(false)
}
