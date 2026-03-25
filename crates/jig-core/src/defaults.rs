use crate::config::schema::{JigConfig, Persona, Profile, Settings, Template, TemplateRef};

/// Returns all built-in templates.
pub fn builtin_templates() -> Vec<Template> {
    vec![
        Template {
            name: "base".to_owned(),
            description: Some("Minimal base template with no extra tools".to_owned()),
            config: JigConfig::default(),
        },
        Template {
            name: "base-devops".to_owned(),
            description: Some("DevOps template with docker, k8s, terraform skills".to_owned()),
            config: JigConfig {
                schema: Some(1),
                profile: Some(Profile {
                    settings: Some(Settings {
                        allowed_tools: Some(vec![
                            "Bash".to_owned(),
                            "Edit".to_owned(),
                            "Read".to_owned(),
                        ]),
                        ..Default::default()
                    }),
                    ..Default::default()
                }),
                ..Default::default()
            },
        },
        Template {
            name: "base-frontend".to_owned(),
            description: Some("Frontend development template".to_owned()),
            config: JigConfig {
                schema: Some(1),
                profile: Some(Profile {
                    settings: Some(Settings {
                        allowed_tools: Some(vec![
                            "Bash".to_owned(),
                            "Edit".to_owned(),
                            "Read".to_owned(),
                            "Write".to_owned(),
                        ]),
                        ..Default::default()
                    }),
                    ..Default::default()
                }),
                ..Default::default()
            },
        },
        Template {
            name: "code-review".to_owned(),
            description: Some("Read-only code review template".to_owned()),
            config: JigConfig {
                schema: Some(1),
                profile: Some(Profile {
                    settings: Some(Settings {
                        allowed_tools: Some(vec![
                            "Read".to_owned(),
                            "Grep".to_owned(),
                            "Glob".to_owned(),
                        ]),
                        disallowed_tools: Some(vec![
                            "Bash".to_owned(),
                            "Edit".to_owned(),
                            "Write".to_owned(),
                        ]),
                        ..Default::default()
                    }),
                    ..Default::default()
                }),
                ..Default::default()
            },
        },
        Template {
            name: "data-science".to_owned(),
            description: Some("Data science and ML template".to_owned()),
            config: JigConfig::default(),
        },
        Template {
            name: "security-audit".to_owned(),
            description: Some("Security-focused read-only audit template".to_owned()),
            config: JigConfig {
                schema: Some(1),
                profile: Some(Profile {
                    settings: Some(Settings {
                        allowed_tools: Some(vec![
                            "Read".to_owned(),
                            "Grep".to_owned(),
                            "Glob".to_owned(),
                        ]),
                        disallowed_tools: Some(vec![
                            "Bash".to_owned(),
                            "Edit".to_owned(),
                            "Write".to_owned(),
                        ]),
                        ..Default::default()
                    }),
                    ..Default::default()
                }),
                ..Default::default()
            },
        },
        Template {
            name: "documentation".to_owned(),
            description: Some("Documentation writing template".to_owned()),
            config: JigConfig::default(),
        },
        Template {
            name: "api-design".to_owned(),
            description: Some("API design and review template".to_owned()),
            config: JigConfig::default(),
        },
        Template {
            name: "testing".to_owned(),
            description: Some("Testing and QA template".to_owned()),
            config: JigConfig::default(),
        },
    ]
}

/// Built-in persona definitions.
pub fn builtin_personas() -> Vec<(String, Persona)> {
    vec![
        (
            "default".to_owned(),
            Persona {
                name: Some("default".to_owned()),
                rules: Some(vec![
                    "Be concise and direct.".to_owned(),
                    "Show code changes, not just explanations.".to_owned(),
                ]),
                ..Default::default()
            },
        ),
        (
            "strict-security".to_owned(),
            Persona {
                name: Some("strict-security".to_owned()),
                rules: Some(vec![
                    "Always check for security vulnerabilities before suggesting changes.".to_owned(),
                    "Never suggest running commands as root.".to_owned(),
                    "Flag any hardcoded credentials or secrets.".to_owned(),
                    "Prefer principle of least privilege.".to_owned(),
                ]),
                ..Default::default()
            },
        ),
        (
            "mentor".to_owned(),
            Persona {
                name: Some("mentor".to_owned()),
                rules: Some(vec![
                    "Explain your reasoning step by step.".to_owned(),
                    "Teach concepts, don't just provide answers.".to_owned(),
                    "Ask clarifying questions when requirements are ambiguous.".to_owned(),
                ]),
                ..Default::default()
            },
        ),
        (
            "pair-programmer".to_owned(),
            Persona {
                name: Some("pair-programmer".to_owned()),
                rules: Some(vec![
                    "Think out loud about your approach before coding.".to_owned(),
                    "Suggest alternatives when you see a better path.".to_owned(),
                    "Raise concerns about design decisions proactively.".to_owned(),
                ]),
                ..Default::default()
            },
        ),
        (
            "code-reviewer".to_owned(),
            Persona {
                name: Some("code-reviewer".to_owned()),
                rules: Some(vec![
                    "Look for bugs, not just style issues.".to_owned(),
                    "Check for edge cases and error handling.".to_owned(),
                    "Suggest tests for uncovered code paths.".to_owned(),
                ]),
                ..Default::default()
            },
        ),
        (
            "architect".to_owned(),
            Persona {
                name: Some("architect".to_owned()),
                rules: Some(vec![
                    "Think about long-term maintainability.".to_owned(),
                    "Consider scalability implications.".to_owned(),
                    "Identify coupling and suggest decoupling strategies.".to_owned(),
                ]),
                ..Default::default()
            },
        ),
        (
            "minimalist".to_owned(),
            Persona {
                name: Some("minimalist".to_owned()),
                rules: Some(vec![
                    "Prefer the simplest solution that works.".to_owned(),
                    "Avoid over-engineering and unnecessary abstraction.".to_owned(),
                    "YAGNI: You Aren't Gonna Need It.".to_owned(),
                ]),
                ..Default::default()
            },
        ),
        (
            "tdd".to_owned(),
            Persona {
                name: Some("tdd".to_owned()),
                rules: Some(vec![
                    "Write tests before implementation.".to_owned(),
                    "Red-green-refactor cycle.".to_owned(),
                    "Test behavior, not implementation details.".to_owned(),
                ]),
                ..Default::default()
            },
        ),
        (
            "docs-writer".to_owned(),
            Persona {
                name: Some("docs-writer".to_owned()),
                rules: Some(vec![
                    "Write for the reader, not the writer.".to_owned(),
                    "Use examples liberally.".to_owned(),
                    "Assume minimal context; explain everything.".to_owned(),
                ]),
                ..Default::default()
            },
        ),
        (
            "performance".to_owned(),
            Persona {
                name: Some("performance".to_owned()),
                rules: Some(vec![
                    "Measure before optimizing.".to_owned(),
                    "Identify the critical path.".to_owned(),
                    "Consider memory and CPU implications of every change.".to_owned(),
                ]),
                ..Default::default()
            },
        ),
    ]
}

/// Returns template refs (for listing without full config).
pub fn builtin_template_refs() -> Vec<TemplateRef> {
    builtin_templates()
        .into_iter()
        .map(|t| TemplateRef {
            name: t.name,
            description: t.description,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_builtin_template_count() {
        assert_eq!(builtin_templates().len(), 9);
    }

    #[test]
    fn test_builtin_persona_count() {
        assert_eq!(builtin_personas().len(), 10);
    }

    #[test]
    fn test_builtin_templates_have_names() {
        for t in builtin_templates() {
            assert!(!t.name.is_empty(), "template name should not be empty");
        }
    }

    #[test]
    fn test_builtin_templates_have_descriptions() {
        for t in builtin_templates() {
            assert!(t.description.is_some(), "template '{}' missing description", t.name);
        }
    }

    #[test]
    fn test_builtin_personas_have_rules() {
        for (name, persona) in builtin_personas() {
            assert!(
                persona.rules.as_ref().map(|r| !r.is_empty()).unwrap_or(false),
                "persona '{name}' should have at least one rule"
            );
        }
    }

    #[test]
    fn test_template_refs_match_templates() {
        let templates = builtin_templates();
        let refs = builtin_template_refs();
        assert_eq!(templates.len(), refs.len());
        for (t, r) in templates.iter().zip(refs.iter()) {
            assert_eq!(t.name, r.name);
            assert_eq!(t.description, r.description);
        }
    }

    #[test]
    fn test_template_names_unique() {
        let templates = builtin_templates();
        let mut names: std::collections::HashSet<&str> = std::collections::HashSet::new();
        for t in &templates {
            assert!(names.insert(t.name.as_str()), "duplicate template name: {}", t.name);
        }
    }

    #[test]
    fn test_persona_names_unique() {
        let personas = builtin_personas();
        let mut names: std::collections::HashSet<&str> = std::collections::HashSet::new();
        for (name, _) in &personas {
            assert!(names.insert(name.as_str()), "duplicate persona name: {name}");
        }
    }
}
