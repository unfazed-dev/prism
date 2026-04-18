use super::TemplateError;

/// Validate YAML frontmatter for L3b+ documents.
///
/// Checks that the required fields (id, title, status, owner, updated)
/// are present and well-formed in the frontmatter block.
pub fn validate_frontmatter(frontmatter: &str) -> Result<(), TemplateError> {
    let value: serde_yaml_ng::Value = serde_yaml_ng::from_str(frontmatter)
        .map_err(|e| TemplateError::InvalidFrontmatter(e.to_string()))?;

    let mapping = value.as_mapping().ok_or_else(|| {
        TemplateError::InvalidFrontmatter("frontmatter must be a YAML mapping".into())
    })?;

    let required_fields = ["id", "title", "status", "owner", "updated"];
    for field in required_fields {
        if !mapping.contains_key(serde_yaml_ng::Value::String(field.to_string())) {
            return Err(TemplateError::InvalidFrontmatter(format!(
                "missing required field: {field}"
            )));
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_frontmatter() {
        let fm = r#"
id: doc-001
title: Test Document
status: active
owner: evan
updated: 2026-04-11
"#;
        assert!(validate_frontmatter(fm).is_ok());
    }

    #[test]
    fn test_missing_field() {
        let fm = r#"
id: doc-001
title: Test Document
"#;
        assert!(validate_frontmatter(fm).is_err());
    }
}
