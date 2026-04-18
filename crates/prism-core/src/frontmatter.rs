//! YAML frontmatter parsing and validation.
//!
//! Extracts, parses, and validates the YAML frontmatter block from PRISM
//! markdown documents. Frontmatter carries metadata such as layer, tags,
//! dependencies, and freshness overrides.
//!
//! Only L3b+ documents get full YAML frontmatter. Hot-path files (L0/L1/distributed)
//! use `<!-- prism:managed -->` markers with metadata stored in prism.db.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Parsed YAML frontmatter from a PRISM document.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Frontmatter {
    /// Document title.
    pub title: Option<String>,
    /// Layer assignment (e.g. "L0", "L1", "L3b", "Meta").
    pub layer: Option<String>,
    /// Classification tag (e.g. "architecture", "decision", "reference").
    pub classification: Option<String>,
    /// Freeform tags.
    #[serde(default)]
    pub tags: Vec<String>,
    /// Document IDs that this document depends on.
    #[serde(default)]
    pub dependencies: Vec<String>,
    /// Scope path for decision/goal matching.
    pub scope: Option<String>,
    /// Manual freshness override in turns.
    pub freshness_override_turns: Option<u32>,
    /// Additional key-value metadata.
    #[serde(default, flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

/// Errors specific to frontmatter parsing and validation.
#[derive(Debug, Clone, thiserror::Error)]
pub enum FrontmatterError {
    /// The document is missing opening or closing `---` delimiters.
    #[error("Missing frontmatter delimiters")]
    MissingDelimiters,

    /// The YAML between the delimiters failed to parse.
    #[error("Invalid YAML in frontmatter: {message}")]
    InvalidYaml {
        /// Parser error message.
        message: String,
    },

    /// A field required by the PRISM spec is absent.
    #[error("Required field missing: {field}")]
    RequiredFieldMissing {
        /// Name of the missing field.
        field: String,
    },

    /// A field is present but contains an invalid value.
    #[error("Invalid field value for '{field}': {message}")]
    InvalidFieldValue {
        /// Name of the invalid field.
        field: String,
        /// Description of why the value is invalid.
        message: String,
    },
}

/// Extract and parse YAML frontmatter from a markdown document string.
///
/// Expects the document to begin with `---` and contain a closing `---`.
/// Returns `Ok(None)` if no frontmatter block is found.
/// Returns `Err` if delimiters are malformed or YAML is invalid.
///
/// # Examples
///
/// ```
/// use prism_core::frontmatter::parse_frontmatter;
///
/// let doc = "---\ntitle: Design\nlayer: L3b\nclassification: architecture\n---\n# Design\n";
/// let fm = parse_frontmatter(doc).unwrap().unwrap();
/// assert_eq!(fm.title.as_deref(), Some("Design"));
/// assert_eq!(fm.layer.as_deref(), Some("L3b"));
///
/// // No frontmatter returns None
/// let plain = "# Just markdown\n";
/// assert!(parse_frontmatter(plain).unwrap().is_none());
/// ```
#[tracing::instrument(skip(content), fields(len = content.len()))]
pub fn parse_frontmatter(content: &str) -> Result<Option<Frontmatter>, FrontmatterError> {
    let trimmed = content.trim_start();

    // No frontmatter if the document doesn't start with ---
    if !trimmed.starts_with("---") {
        return Ok(None);
    }

    // Find the closing ---
    let after_opening = &trimmed[3..];
    // Skip the rest of the opening line (could be "---\n" or "--- \n")
    let after_newline = match after_opening.find('\n') {
        Some(pos) => &after_opening[pos + 1..],
        None => return Err(FrontmatterError::MissingDelimiters),
    };

    let closing_pos = find_closing_delimiter(after_newline);
    let yaml_content = match closing_pos {
        Some(pos) => &after_newline[..pos],
        None => return Err(FrontmatterError::MissingDelimiters),
    };

    if yaml_content.trim().is_empty() {
        return Ok(Some(Frontmatter::default()));
    }

    // unsafe-libyaml (underneath serde_yaml_ng) can panic on certain malformed
    // inputs; catch the panic and convert it to an error so hook handlers
    // never see a crash.
    let yaml_owned = yaml_content.to_string();
    let parse_result =
        std::panic::catch_unwind(|| serde_yaml_ng::from_str::<Frontmatter>(&yaml_owned));

    let fm = match parse_result {
        Ok(Ok(fm)) => fm,
        Ok(Err(e)) => {
            return Err(FrontmatterError::InvalidYaml {
                message: e.to_string(),
            });
        }
        Err(_) => {
            return Err(FrontmatterError::InvalidYaml {
                message: "YAML parser panicked (upstream libyml bug)".to_string(),
            });
        }
    };

    Ok(Some(fm))
}

/// Extract just the body content (everything after frontmatter).
/// Returns the full content if no frontmatter is found.
#[tracing::instrument(skip(content), fields(len = content.len()))]
pub fn extract_body(content: &str) -> &str {
    let trimmed = content.trim_start();

    if !trimmed.starts_with("---") {
        return content;
    }

    let after_opening = &trimmed[3..];
    let after_newline = match after_opening.find('\n') {
        Some(pos) => &after_opening[pos + 1..],
        None => return content,
    };

    match find_closing_delimiter(after_newline) {
        Some(pos) => {
            let remainder = &after_newline[pos + 3..];
            // Skip the newline after closing ---
            remainder.strip_prefix('\n').unwrap_or(remainder)
        }
        None => content,
    }
}

/// Validate a parsed [`Frontmatter`] against PRISM requirements for L3b+ documents.
///
/// Required fields for L3b+ documents: title, layer, classification.
/// Returns a list of all validation errors found.
#[tracing::instrument(skip(fm))]
pub fn validate_frontmatter(fm: &Frontmatter) -> Result<(), Vec<FrontmatterError>> {
    let mut errors = Vec::new();

    if fm.title.is_none() {
        errors.push(FrontmatterError::RequiredFieldMissing {
            field: "title".to_string(),
        });
    }

    if fm.layer.is_none() {
        errors.push(FrontmatterError::RequiredFieldMissing {
            field: "layer".to_string(),
        });
    }

    if fm.classification.is_none() {
        errors.push(FrontmatterError::RequiredFieldMissing {
            field: "classification".to_string(),
        });
    }

    // Validate layer value if present
    if let Some(ref layer) = fm.layer {
        let valid_layers = ["L0", "L1", "L2", "L3a", "L3b", "L4", "Meta", "Distributed"];
        if !valid_layers.contains(&layer.as_str()) {
            errors.push(FrontmatterError::InvalidFieldValue {
                field: "layer".to_string(),
                message: format!("must be one of: {}", valid_layers.join(", ")),
            });
        }
    }

    // Validate classification value if present
    if let Some(ref classification) = fm.classification {
        let valid_classifications = [
            "core",
            "architecture",
            "api",
            "guide",
            "reference",
            "decision",
            "goal",
            "progress",
            "template",
            "other",
        ];
        if !valid_classifications.contains(&classification.to_lowercase().as_str()) {
            errors.push(FrontmatterError::InvalidFieldValue {
                field: "classification".to_string(),
                message: format!("must be one of: {}", valid_classifications.join(", ")),
            });
        }
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

/// Check if a document contains the PRISM managed marker.
#[tracing::instrument(skip(content))]
pub fn has_prism_marker(content: &str) -> bool {
    content.contains("<!-- prism:managed -->")
}

/// Find the position of the closing `---` delimiter in a string.
/// The closing delimiter must appear at the start of a line.
fn find_closing_delimiter(content: &str) -> Option<usize> {
    // Check if content starts with ---
    if content.starts_with("---") {
        return Some(0);
    }

    // Search for \n--- pattern
    let mut search_from = 0;
    loop {
        match content[search_from..].find("\n---") {
            Some(pos) => {
                let absolute_pos = search_from + pos + 1; // +1 to skip the \n
                                                          // Verify it's a clean delimiter (followed by newline, EOF, or whitespace)
                let after = &content[absolute_pos + 3..];
                if after.is_empty()
                    || after.starts_with('\n')
                    || after.starts_with('\r')
                    || after.starts_with(' ')
                {
                    return Some(absolute_pos);
                }
                search_from = absolute_pos + 3;
            }
            None => return None,
        }
    }
}
