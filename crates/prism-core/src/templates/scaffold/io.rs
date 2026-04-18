//! Managed-file I/O primitives for scaffolding.
//!
//! Split out of `mod.rs` during the prism-churn Phase 3 refactor. Groups the
//! concerns that live "between the caller and the filesystem": the
//! `WriteOutcome` type that describes what happened, the marker strings used
//! to detect managed vs user-owned vs enriched files, the guarded
//! write-if-changed helper, and an atomic rename-based writer.

use super::TemplateError;

pub(super) const MANAGED_MARKER: &str = "<!-- prism:managed -->";
pub(super) const ENRICHED_MARKER: &str = "<!-- prism:enriched -->";

/// Outcome of writing a single managed file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WriteOutcome {
    /// File did not exist — created.
    Created(String),
    /// File existed with `<!-- prism:managed -->` and content changed — updated.
    Updated(String),
    /// File existed but content is identical — no write needed.
    Unchanged(String),
    /// File exists without `<!-- prism:managed -->` marker — user owns it, not touched.
    UserOwned(String),
}

impl WriteOutcome {
    /// The file path this outcome refers to.
    pub fn path(&self) -> &str {
        match self {
            Self::Created(p) | Self::Updated(p) | Self::Unchanged(p) | Self::UserOwned(p) => p,
        }
    }

    /// Human-readable verb for logging.
    pub fn verb(&self) -> &'static str {
        match self {
            Self::Created(_) => "Created",
            Self::Updated(_) => "Updated",
            Self::Unchanged(_) => "Unchanged",
            Self::UserOwned(_) => "Skipped (user-owned)",
        }
    }

    /// Whether an actual write occurred.
    pub fn was_written(&self) -> bool {
        matches!(self, Self::Created(_) | Self::Updated(_))
    }
}

/// Write a managed file only if it would change, respecting user-owned and enriched files.
///
/// Writes are atomic — the content lands via a tmp sibling + rename so a
/// reader never sees a half-written managed file. A crash between the tmp
/// write and the rename leaves only the tmp behind; the session-start sweep
/// reconciles it against the pending hash row if one was registered.
pub(super) fn write_managed_file(
    path: &std::path::Path,
    new_content: &str,
) -> Result<WriteOutcome, TemplateError> {
    let path_str = path.to_string_lossy().to_string();

    if path.exists() {
        let existing = std::fs::read_to_string(path)?;

        if !existing.contains(MANAGED_MARKER) {
            return Ok(WriteOutcome::UserOwned(path_str));
        }

        if existing.contains(ENRICHED_MARKER) {
            return Ok(WriteOutcome::Unchanged(path_str));
        }

        if existing == new_content {
            return Ok(WriteOutcome::Unchanged(path_str));
        }

        atomic_write(path, new_content.as_bytes())?;
        Ok(WriteOutcome::Updated(path_str))
    } else {
        atomic_write(path, new_content.as_bytes())?;
        Ok(WriteOutcome::Created(path_str))
    }
}

/// Write `content` to `path` via a tmp sibling + rename. Duplicated from
/// `prism-db::atomic_write` to keep `prism-templates` free of DB dependencies
/// — the two are intentionally the same algorithm.
fn atomic_write(path: &std::path::Path, content: &[u8]) -> std::io::Result<()> {
    use std::io::Write;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let tmp_path = match path.file_name() {
        Some(name) => {
            let mut tmp = std::ffi::OsString::from(".");
            tmp.push(name);
            tmp.push(format!(".prism-tmp-{}", std::process::id()));
            path.with_file_name(tmp)
        }
        None => {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "atomic_write target has no file name",
            ))
        }
    };
    {
        let mut f = std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(&tmp_path)?;
        f.write_all(content)?;
        f.flush()?;
        f.sync_all()?;
    }
    match std::fs::rename(&tmp_path, path) {
        Ok(()) => Ok(()),
        Err(e) => {
            let _ = std::fs::remove_file(&tmp_path);
            Err(e)
        }
    }
}
