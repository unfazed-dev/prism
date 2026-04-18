//! Template registry — bundles all template assets at compile time.

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TemplateName {
    ClaudeMd,
    ContextMd,
    DirClaudeMd,
    DirContextMd,
    PrismMd,

    RulesGeneral,
    RulesService,
    RulesViewModel,
    RulesTest,
    RulesDocumentStandard,
    RulesIcm,

    RefsArchitecture,
    RefsSchema,
    RefsDependencies,
    RefsPattern,
    RefsDomainKnowledge,
}

impl TemplateName {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::ClaudeMd => "CLAUDE.md",
            Self::ContextMd => "CONTEXT.md",
            Self::DirClaudeMd => "dir-CLAUDE.md",
            Self::DirContextMd => "dir-CONTEXT.md",
            Self::PrismMd => "PRISM.md",
            Self::RulesGeneral => "rules/general-conventions.md",
            Self::RulesService => "rules/service-conventions.md",
            Self::RulesViewModel => "rules/viewmodel-conventions.md",
            Self::RulesTest => "rules/test-conventions.md",
            Self::RulesDocumentStandard => "rules/prism-document-standard.md",
            Self::RulesIcm => "rules/icm-conventions.md",
            Self::RefsArchitecture => "refs/architecture.md",
            Self::RefsSchema => "refs/schema.md",
            Self::RefsDependencies => "refs/dependencies.md",
            Self::RefsPattern => "refs/pattern.md",
            Self::RefsDomainKnowledge => "refs/domain-knowledge.md",
        }
    }

    pub fn output_filename(&self) -> &'static str {
        match self {
            Self::ClaudeMd | Self::DirClaudeMd => "CLAUDE.md",
            Self::ContextMd | Self::DirContextMd => "CONTEXT.md",
            Self::PrismMd => "PRISM.md",
            Self::RulesGeneral => "general-conventions.md",
            Self::RulesService => "service-conventions.md",
            Self::RulesViewModel => "viewmodel-conventions.md",
            Self::RulesTest => "test-conventions.md",
            Self::RulesDocumentStandard => "prism-document-standard.md",
            Self::RulesIcm => "icm-conventions.md",
            Self::RefsArchitecture => "architecture.md",
            Self::RefsSchema => "schema.md",
            Self::RefsDependencies => "dependencies.md",
            Self::RefsPattern => "pattern.md",
            Self::RefsDomainKnowledge => "domain-knowledge.md",
        }
    }

    pub fn all() -> &'static [TemplateName] {
        &[
            Self::ClaudeMd,
            Self::ContextMd,
            Self::DirClaudeMd,
            Self::DirContextMd,
            Self::PrismMd,
            Self::RulesGeneral,
            Self::RulesService,
            Self::RulesViewModel,
            Self::RulesTest,
            Self::RulesDocumentStandard,
            Self::RulesIcm,
            Self::RefsArchitecture,
            Self::RefsSchema,
            Self::RefsDependencies,
            Self::RefsPattern,
            Self::RefsDomainKnowledge,
        ]
    }

    pub fn rules() -> &'static [TemplateName] {
        &[
            Self::RulesGeneral,
            Self::RulesService,
            Self::RulesViewModel,
            Self::RulesTest,
            Self::RulesDocumentStandard,
            Self::RulesIcm,
        ]
    }

    pub fn refs() -> &'static [TemplateName] {
        &[
            Self::RefsArchitecture,
            Self::RefsSchema,
            Self::RefsDependencies,
            Self::RefsPattern,
            Self::RefsDomainKnowledge,
        ]
    }
}

pub fn get_template_source(name: TemplateName) -> &'static str {
    match name {
        TemplateName::ClaudeMd => include_str!("../../../../templates/CLAUDE.md.template"),
        TemplateName::ContextMd => include_str!("../../../../templates/CONTEXT.md.template"),
        TemplateName::DirClaudeMd => include_str!("../../../../templates/dir-CLAUDE.md.template"),
        TemplateName::DirContextMd => include_str!("../../../../templates/dir-CONTEXT.md.template"),
        TemplateName::PrismMd => include_str!("../../../../templates/PRISM.md.template"),
        TemplateName::RulesGeneral => {
            include_str!("../../../../templates/rules/general-conventions.md.template")
        }
        TemplateName::RulesService => {
            include_str!("../../../../templates/rules/service-conventions.md.template")
        }
        TemplateName::RulesViewModel => {
            include_str!("../../../../templates/rules/viewmodel-conventions.md.template")
        }
        TemplateName::RulesTest => {
            include_str!("../../../../templates/rules/test-conventions.md.template")
        }
        TemplateName::RulesDocumentStandard => {
            include_str!("../../../../templates/rules/prism-document-standard.md.template")
        }
        TemplateName::RulesIcm => {
            include_str!("../../../../templates/rules/icm-conventions.md.template")
        }
        TemplateName::RefsArchitecture => {
            include_str!("../../../../templates/refs/architecture.md.template")
        }
        TemplateName::RefsSchema => include_str!("../../../../templates/refs/schema.md.template"),
        TemplateName::RefsDependencies => {
            include_str!("../../../../templates/refs/dependencies.md.template")
        }
        TemplateName::RefsPattern => include_str!("../../../../templates/refs/pattern.md.template"),
        TemplateName::RefsDomainKnowledge => {
            include_str!("../../../../templates/refs/domain-knowledge.md.template")
        }
    }
}
