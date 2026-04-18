//! Minijinja-based template rendering.

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
    render_template(registry::get_template_source(name), context)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn render_simple_template_interpolates() {
        let mut ctx = HashMap::new();
        ctx.insert("name", "PRISM");
        let result = render_template("Hello, {{ name }}!", &ctx).unwrap();
        assert_eq!(result, "Hello, PRISM!");
    }

    #[test]
    fn render_named_claude_md_with_context() {
        let mut ctx = HashMap::new();
        ctx.insert("project_name", "TestProject");
        ctx.insert("project_description", "A test project");
        let result = render_named(TemplateName::ClaudeMd, &ctx).unwrap();
        assert!(result.contains("TestProject"));
        assert!(result.contains("<!-- prism:managed -->"));
    }

    #[test]
    fn render_named_with_empty_context_uses_defaults() {
        let ctx: HashMap<String, String> = HashMap::new();
        let result = render_named(TemplateName::ClaudeMd, &ctx).unwrap();
        assert!(result.contains("<!-- prism:managed -->"));
    }

    #[test]
    fn all_registered_templates_render_without_error() {
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
    fn rules_list_matches_expected_count() {
        assert_eq!(TemplateName::rules().len(), 6);
    }

    #[test]
    fn refs_list_matches_expected_count() {
        assert_eq!(TemplateName::refs().len(), 5);
    }
}
