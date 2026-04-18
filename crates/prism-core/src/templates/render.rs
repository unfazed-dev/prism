//! Template rendering via minijinja.
//!
//! Provides both raw template string rendering and registry-based rendering.

use minijinja::Environment;
use serde::Serialize;

use super::registry::{self, TemplateName};
use super::TemplateError;

/// Render a raw template string with the given context data.
pub fn render_template<T: Serialize>(
    template_source: &str,
    context: &T,
) -> Result<String, TemplateError> {
    let mut env = Environment::new();
    env.set_undefined_behavior(minijinja::UndefinedBehavior::Lenient);
    env.add_template("tpl", template_source)
        .map_err(|e| TemplateError::RenderError(e.to_string()))?;

    let tpl = env
        .get_template("tpl")
        .map_err(|e| TemplateError::RenderError(e.to_string()))?;

    tpl.render(context)
        .map_err(|e| TemplateError::RenderError(e.to_string()))
}

/// Render a named template from the registry with the given context data.
pub fn render_named<T: Serialize>(
    name: TemplateName,
    context: &T,
) -> Result<String, TemplateError> {
    let source = registry::get_template_source(name);
    render_template(source, context)
}

/// Create a minijinja Environment pre-loaded with all PRISM templates.
///
/// Useful when rendering multiple templates in sequence to avoid
/// re-parsing template sources each time.
pub fn create_environment() -> Result<Environment<'static>, TemplateError> {
    let mut env = Environment::new();
    env.set_undefined_behavior(minijinja::UndefinedBehavior::Lenient);

    for name in TemplateName::all() {
        let source = registry::get_template_source(*name);
        env.add_template(name.as_str(), source)
            .map_err(|e| TemplateError::RenderError(format!("{}: {}", name.as_str(), e)))?;
    }

    Ok(env)
}

/// Render a template from a pre-built environment.
pub fn render_from_env<T: Serialize>(
    env: &Environment<'_>,
    template_name: &str,
    context: &T,
) -> Result<String, TemplateError> {
    let tpl = env
        .get_template(template_name)
        .map_err(|e| TemplateError::NotFound(e.to_string()))?;

    tpl.render(context)
        .map_err(|e| TemplateError::RenderError(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn test_render_simple_template() {
        let mut ctx = HashMap::new();
        ctx.insert("name", "PRISM");
        let result = render_template("Hello, {{ name }}!", &ctx).unwrap();
        assert_eq!(result, "Hello, PRISM!");
    }

    #[test]
    fn test_render_named_claude_md() {
        let mut ctx = HashMap::new();
        ctx.insert("project_name", "TestProject");
        ctx.insert("project_description", "A test project");
        let result = render_named(TemplateName::ClaudeMd, &ctx).unwrap();
        assert!(result.contains("TestProject"));
        assert!(result.contains("<!-- prism:managed -->"));
    }

    #[test]
    fn test_render_named_with_defaults() {
        // Render with empty context — should use default values
        let ctx: HashMap<String, String> = HashMap::new();
        let result = render_named(TemplateName::ClaudeMd, &ctx).unwrap();
        assert!(result.contains("<!-- prism:managed -->"));
        assert!(result.contains("Project description"));
    }

    #[test]
    fn test_create_environment_loads_all() {
        let env = create_environment().unwrap();
        for name in TemplateName::all() {
            assert!(
                env.get_template(name.as_str()).is_ok(),
                "Template {} should be loadable",
                name.as_str()
            );
        }
    }

    #[test]
    fn test_render_from_env() {
        let env = create_environment().unwrap();
        let mut ctx = HashMap::new();
        ctx.insert("project_name", "EnvTest");
        let result = render_from_env(&env, "PRISM.md", &ctx).unwrap();
        assert!(result.contains("EnvTest"));
    }

    #[test]
    fn test_all_templates_render_without_error() {
        let ctx: HashMap<String, String> = HashMap::new();
        for name in TemplateName::all() {
            let result = render_named(*name, &ctx);
            assert!(
                result.is_ok(),
                "Template {} failed to render: {:?}",
                name.as_str(),
                result.err()
            );
        }
    }

    #[test]
    fn test_all_template_output_filenames() {
        for name in TemplateName::all() {
            let filename = name.output_filename();
            assert!(
                filename.ends_with(".md"),
                "Template {} output filename should end with .md, got {}",
                name.as_str(),
                filename
            );
        }
    }

    #[test]
    fn test_find_template_by_name() {
        assert_eq!(
            super::registry::find_template("CLAUDE.md").unwrap(),
            TemplateName::ClaudeMd
        );
        assert_eq!(
            super::registry::find_template("PRISM.md").unwrap(),
            TemplateName::PrismMd
        );
    }

    #[test]
    fn test_find_template_by_output_filename() {
        // "general-conventions.md" is an output_filename for RulesGeneral
        assert_eq!(
            super::registry::find_template("general-conventions.md").unwrap(),
            TemplateName::RulesGeneral
        );
    }

    #[test]
    fn test_find_template_not_found() {
        let result = super::registry::find_template("nonexistent.md");
        assert!(result.is_err());
    }

    #[test]
    fn test_rules_list() {
        let rules = TemplateName::rules();
        assert_eq!(rules.len(), 5);
    }

    #[test]
    fn test_refs_list() {
        let refs = TemplateName::refs();
        assert_eq!(refs.len(), 5);
    }

    #[test]
    fn test_get_template_source_all() {
        for name in TemplateName::all() {
            let source = super::registry::get_template_source(*name);
            assert!(
                !source.is_empty(),
                "Template {} source should not be empty",
                name.as_str()
            );
        }
    }

    #[test]
    fn test_render_from_env_not_found() {
        let env = create_environment().unwrap();
        let ctx: HashMap<String, String> = HashMap::new();
        let result = render_from_env(&env, "nonexistent-template", &ctx);
        assert!(result.is_err());
    }

    #[test]
    fn test_all_templates_produce_valid_markdown() {
        let ctx: HashMap<String, String> = HashMap::new();
        for name in TemplateName::all() {
            let rendered = render_named(*name, &ctx).unwrap();
            // Basic markdown validity: should contain at least one heading
            assert!(
                rendered.contains('#') || rendered.contains("---"),
                "Template {} should produce markdown with headings or frontmatter",
                name.as_str()
            );
        }
    }
}
