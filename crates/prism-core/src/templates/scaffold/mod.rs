//! Directory scaffolding — creates CLAUDE.md + CONTEXT.md pairs.
//!
//! Implementation is split across submodules:
//! - `io` — `WriteOutcome`, managed-write guard, atomic rename helper.
//! - `options` — `ScaffoldOptions` + the internal template-context builder.
//!
//! Public API is unchanged; callers continue to import
//! `scaffold::ScaffoldOptions`, `scaffold::WriteOutcome`, and the four
//! `scaffold_*` functions below.

mod io;
mod options;

pub use io::WriteOutcome;
pub use options::ScaffoldOptions;

use std::collections::HashMap;
use std::path::Path;

use super::registry::TemplateName;
use super::render;
use super::TemplateError;

use self::io::write_managed_file;
use self::options::build_context;

/// Scaffold a directory with a CLAUDE.md + CONTEXT.md pair.
///
/// Uses root templates for the project root, directory templates for subdirectories.
/// Respects user-owned files (those without `<!-- prism:managed -->`) and skips
/// writes when content hasn't changed.
pub fn scaffold_directory(
    target_dir: &Path,
    options: &ScaffoldOptions,
) -> Result<Vec<WriteOutcome>, TemplateError> {
    std::fs::create_dir_all(target_dir)?;

    let ctx = build_context(target_dir, options);

    let (claude_tpl, context_tpl) = if options.is_root {
        (TemplateName::ClaudeMd, TemplateName::ContextMd)
    } else {
        (TemplateName::DirClaudeMd, TemplateName::DirContextMd)
    };

    let claude_content = render::render_named(claude_tpl, &ctx)?;
    let claude_path = target_dir.join("CLAUDE.md");
    let claude_outcome = write_managed_file(&claude_path, &claude_content)?;

    let mut outcomes = vec![claude_outcome];

    if !options.skip_context {
        let context_content = render::render_named(context_tpl, &ctx)?;
        let context_path = target_dir.join("CONTEXT.md");
        let context_outcome = write_managed_file(&context_path, &context_content)?;
        outcomes.push(context_outcome);
    }

    Ok(outcomes)
}

/// Scaffold the .prism directory structure.
///
/// Creates: .prism/PRISM.md, .prism/refs/, .prism/stages/
pub fn scaffold_prism_dir(
    project_root: &Path,
    project_name: &str,
) -> Result<Vec<String>, TemplateError> {
    let prism_dir = project_root.join(".prism");
    let refs_dir = prism_dir.join("refs");
    let stages_dir = prism_dir.join("stages");

    std::fs::create_dir_all(&refs_dir)?;
    std::fs::create_dir_all(&stages_dir)?;

    let mut created = Vec::new();

    let mut ctx: HashMap<String, serde_json::Value> = HashMap::new();
    ctx.insert(
        "project_name".into(),
        serde_json::Value::String(project_name.to_string()),
    );
    let prism_md = render::render_named(TemplateName::PrismMd, &ctx)?;
    let prism_md_path = prism_dir.join("PRISM.md");
    std::fs::write(&prism_md_path, &prism_md)?;
    created.push(prism_md_path.to_string_lossy().to_string());

    for tpl in TemplateName::refs() {
        let rendered = render::render_named(*tpl, &ctx)?;
        let path = refs_dir.join(tpl.output_filename());
        std::fs::write(&path, &rendered)?;
        created.push(path.to_string_lossy().to_string());
    }

    Ok(created)
}

/// Scaffold the .claude/rules directory with convention files.
pub fn scaffold_rules(
    project_root: &Path,
    project_name: &str,
) -> Result<Vec<String>, TemplateError> {
    let rules_dir = project_root.join(".claude").join("rules");
    std::fs::create_dir_all(&rules_dir)?;

    let mut ctx: HashMap<String, serde_json::Value> = HashMap::new();
    ctx.insert(
        "project_name".into(),
        serde_json::Value::String(project_name.to_string()),
    );

    let mut created = Vec::new();

    for tpl in TemplateName::rules() {
        let rendered = render::render_named(*tpl, &ctx)?;
        let path = rules_dir.join(tpl.output_filename());
        std::fs::write(&path, &rendered)?;
        created.push(path.to_string_lossy().to_string());
    }

    Ok(created)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_scaffold_root_directory() {
        let dir = TempDir::new().unwrap();
        let opts = ScaffoldOptions {
            directory_name: "myproject".to_string(),
            is_root: true,
            project_name: Some("MyProject".to_string()),
            project_description: Some("A cool project".to_string()),
            ..Default::default()
        };
        let outcomes = scaffold_directory(dir.path(), &opts).unwrap();
        assert_eq!(outcomes.len(), 2);
        assert!(outcomes
            .iter()
            .all(|o| matches!(o, WriteOutcome::Created(_))));
        assert!(dir.path().join("CLAUDE.md").exists());
        assert!(dir.path().join("CONTEXT.md").exists());

        let claude = std::fs::read_to_string(dir.path().join("CLAUDE.md")).unwrap();
        assert!(claude.contains("MyProject"));
        assert!(claude.contains("<!-- prism:managed -->"));
    }

    #[test]
    fn test_scaffold_subdirectory() {
        let dir = TempDir::new().unwrap();
        let subdir = dir.path().join("src/services");
        let opts = ScaffoldOptions {
            directory_name: "services".to_string(),
            description: Some("Business logic services".to_string()),
            is_root: false,
            ..Default::default()
        };
        let outcomes = scaffold_directory(&subdir, &opts).unwrap();
        assert_eq!(outcomes.len(), 2);
        assert!(outcomes
            .iter()
            .all(|o| matches!(o, WriteOutcome::Created(_))));
        assert!(subdir.join("CLAUDE.md").exists());
        assert!(subdir.join("CONTEXT.md").exists());

        let claude = std::fs::read_to_string(subdir.join("CLAUDE.md")).unwrap();
        assert!(claude.contains("services"));
    }

    #[test]
    fn test_scaffold_skips_unchanged() {
        let dir = TempDir::new().unwrap();
        let opts = ScaffoldOptions {
            directory_name: "myproject".to_string(),
            is_root: true,
            project_name: Some("MyProject".to_string()),
            ..Default::default()
        };
        let first = scaffold_directory(dir.path(), &opts).unwrap();
        assert!(first.iter().all(|o| matches!(o, WriteOutcome::Created(_))));

        let second = scaffold_directory(dir.path(), &opts).unwrap();
        assert!(
            second
                .iter()
                .all(|o| matches!(o, WriteOutcome::Unchanged(_))),
            "Identical content should produce Unchanged outcomes"
        );
    }

    #[test]
    fn test_scaffold_updates_on_drift() {
        let dir = TempDir::new().unwrap();
        let opts = ScaffoldOptions {
            directory_name: "myproject".to_string(),
            is_root: true,
            project_name: Some("MyProject".to_string()),
            ..Default::default()
        };
        scaffold_directory(dir.path(), &opts).unwrap();

        let opts2 = ScaffoldOptions {
            directory_name: "myproject".to_string(),
            is_root: true,
            project_name: Some("MyProject".to_string()),
            project_description: Some("Now with a description".to_string()),
            ..Default::default()
        };
        let outcomes = scaffold_directory(dir.path(), &opts2).unwrap();
        assert!(
            outcomes
                .iter()
                .any(|o| matches!(o, WriteOutcome::Updated(_))),
            "Changed content should produce Updated outcome"
        );
    }

    #[test]
    fn test_scaffold_skip_context() {
        let dir = TempDir::new().unwrap();
        let opts = ScaffoldOptions {
            directory_name: "myproject".to_string(),
            is_root: true,
            project_name: Some("MyProject".to_string()),
            skip_context: true,
            ..Default::default()
        };
        let outcomes = scaffold_directory(dir.path(), &opts).unwrap();
        assert_eq!(outcomes.len(), 1, "Only CLAUDE.md should be scaffolded");
        assert!(dir.path().join("CLAUDE.md").exists());
        assert!(
            !dir.path().join("CONTEXT.md").exists(),
            "CONTEXT.md should not be created when skip_context is true"
        );
    }

    #[test]
    fn test_scaffold_skip_context_preserves_existing_context_md() {
        let dir = TempDir::new().unwrap();
        let rich_context = "<!-- prism:managed -->\n# Context\nActive goal: Build auth system\n";
        std::fs::write(dir.path().join("CONTEXT.md"), rich_context).unwrap();

        let opts = ScaffoldOptions {
            directory_name: "myproject".to_string(),
            is_root: true,
            project_name: Some("MyProject".to_string()),
            skip_context: true,
            ..Default::default()
        };
        scaffold_directory(dir.path(), &opts).unwrap();

        let content = std::fs::read_to_string(dir.path().join("CONTEXT.md")).unwrap();
        assert_eq!(
            content, rich_context,
            "CONTEXT.md should not be modified when skip_context is true"
        );
    }

    #[test]
    fn test_scaffold_preserves_enriched_claude_md() {
        let dir = TempDir::new().unwrap();
        let enriched = "<!-- prism:managed -->\n<!-- prism:enriched -->\n# src\n\n> Core domain logic\n\n## Purpose\n\nImplements the main business rules.\n";
        std::fs::write(dir.path().join("CLAUDE.md"), enriched).unwrap();

        let opts = ScaffoldOptions {
            directory_name: "src".to_string(),
            is_root: false,
            purpose: Some("Source code — generic template".to_string()),
            ..Default::default()
        };
        let outcomes = scaffold_directory(dir.path(), &opts).unwrap();

        let claude_outcome = outcomes
            .iter()
            .find(|o| o.path().contains("CLAUDE.md"))
            .unwrap();
        assert!(
            matches!(claude_outcome, WriteOutcome::Unchanged(_)),
            "Enriched CLAUDE.md should not be overwritten by template"
        );

        let content = std::fs::read_to_string(dir.path().join("CLAUDE.md")).unwrap();
        assert!(content.contains("Implements the main business rules"));
        assert!(!content.contains("generic template"));
    }

    #[test]
    fn test_scaffold_preserves_user_owned() {
        let dir = TempDir::new().unwrap();
        std::fs::write(
            dir.path().join("CLAUDE.md"),
            "# My Custom CLAUDE.md\nHand-written content.",
        )
        .unwrap();

        let opts = ScaffoldOptions {
            directory_name: "myproject".to_string(),
            is_root: true,
            project_name: Some("MyProject".to_string()),
            ..Default::default()
        };
        let outcomes = scaffold_directory(dir.path(), &opts).unwrap();

        let claude_outcome = outcomes
            .iter()
            .find(|o| o.path().contains("CLAUDE.md"))
            .unwrap();
        assert!(
            matches!(claude_outcome, WriteOutcome::UserOwned(_)),
            "User-owned CLAUDE.md should not be overwritten"
        );

        let content = std::fs::read_to_string(dir.path().join("CLAUDE.md")).unwrap();
        assert!(content.contains("Hand-written content."));
    }

    #[test]
    fn test_scaffold_with_enriched_content() {
        let dir = TempDir::new().unwrap();
        let opts = ScaffoldOptions {
            directory_name: "my-app".to_string(),
            is_root: true,
            project_name: Some("my-app".to_string()),
            tech_stack: Some(vec![
                "Rust 2021".to_string(),
                "SQLite via rusqlite".to_string(),
                "clap CLI".to_string(),
            ]),
            ..Default::default()
        };
        let _created = scaffold_directory(dir.path(), &opts).unwrap();
        let claude = std::fs::read_to_string(dir.path().join("CLAUDE.md")).unwrap();
        assert!(
            claude.contains("Rust 2021"),
            "Tech stack should appear in CLAUDE.md"
        );
    }

    #[test]
    fn test_scaffold_subdir_with_purpose_and_key_files() {
        let dir = TempDir::new().unwrap();
        let subdir = dir.path().join("src");
        let opts = ScaffoldOptions {
            directory_name: "src".to_string(),
            is_root: false,
            purpose: Some("Source code — primary implementation".to_string()),
            key_files: Some(vec![
                {
                    let mut m = HashMap::new();
                    m.insert("name".to_string(), "main.rs".to_string());
                    m.insert("description".to_string(), "Entry point".to_string());
                    m
                },
                {
                    let mut m = HashMap::new();
                    m.insert("name".to_string(), "lib.rs".to_string());
                    m.insert("description".to_string(), "Library root".to_string());
                    m
                },
            ]),
            ..Default::default()
        };
        let _created = scaffold_directory(&subdir, &opts).unwrap();
        let claude = std::fs::read_to_string(subdir.join("CLAUDE.md")).unwrap();
        assert!(claude.contains("Source code"), "Purpose should appear");
        assert!(claude.contains("main.rs"), "Key files should be listed");
        assert!(
            claude.contains("Entry point"),
            "File descriptions should appear"
        );
    }

    #[test]
    fn test_scaffold_prism_dir() {
        let dir = TempDir::new().unwrap();
        let created = scaffold_prism_dir(dir.path(), "TestProject").unwrap();

        assert!(dir.path().join(".prism/PRISM.md").exists());
        assert!(dir.path().join(".prism/refs").is_dir());
        assert!(dir.path().join(".prism/stages").is_dir());
        assert!(dir.path().join(".prism/refs/architecture.md").exists());
        assert!(dir.path().join(".prism/refs/schema.md").exists());
        assert!(dir.path().join(".prism/refs/dependencies.md").exists());
        assert!(dir.path().join(".prism/refs/pattern.md").exists());
        assert!(dir
            .path()
            .join(".prism/refs/domain-knowledge.md")
            .exists());

        assert_eq!(created.len(), 6);
    }

    #[test]
    fn test_scaffold_rules() {
        let dir = TempDir::new().unwrap();
        let created = scaffold_rules(dir.path(), "TestProject").unwrap();

        assert!(dir.path().join(".claude/rules").is_dir());
        assert!(dir
            .path()
            .join(".claude/rules/general-conventions.md")
            .exists());
        assert!(dir
            .path()
            .join(".claude/rules/test-conventions.md")
            .exists());
        assert!(dir
            .path()
            .join(".claude/rules/prism-document-standard.md")
            .exists());
        assert!(dir
            .path()
            .join(".claude/rules/icm-conventions.md")
            .exists());
        assert_eq!(created.len(), 6);
    }

    #[test]
    fn test_scaffold_creates_files() {
        let dir = TempDir::new().unwrap();
        let opts = ScaffoldOptions {
            directory_name: "test".to_string(),
            is_root: false,
            ..Default::default()
        };
        scaffold_directory(dir.path(), &opts).unwrap();
        assert!(dir.path().join("CLAUDE.md").exists());
        assert!(dir.path().join("CONTEXT.md").exists());
    }
}
