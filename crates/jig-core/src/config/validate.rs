use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use miette::Diagnostic;
use thiserror::Error;

use super::schema::{ConfigSource, JigConfig, Template};

#[derive(Debug, Error, Diagnostic)]
pub enum ConfigError {
    #[error("persona.extends is only allowed in .jig.local.yaml (PersonalLocal scope), found in {config_source}")]
    PersonaExtendsInWrongScope { config_source: ConfigSource },

    #[error("Circular extends detected: {cycle}")]
    CircularExtends { cycle: String },

    #[error("Template '{name}' referenced in extends not found (available: {available})")]
    MissingExtends { name: String, available: String },

    #[error("Schema version {found} is not supported (minimum: {minimum})")]
    UnsupportedSchema { found: u32, minimum: u32 },

    #[error("Config parse error in {path}: {message}")]
    ParseError { path: PathBuf, message: String },

    #[error("Hook requires shell: true to use command string form. Use exec: [\"cmd\", \"arg\"] for injection-safe execution.")]
    HookShellRequired,
}

/// Validates per-layer constraints BEFORE merging.
/// Must be called immediately after deserializing each config file.
pub fn validate_layer(config: &JigConfig, source: ConfigSource) -> Result<(), ConfigError> {
    // persona.extends is only allowed in PersonalLocal
    if let Some(persona) = &config.persona {
        if persona.extends.is_some() && source != ConfigSource::PersonalLocal {
            return Err(ConfigError::PersonaExtendsInWrongScope { config_source: source });
        }
    }

    Ok(())
}

/// Detects cycles in template extends chains using DFS with grey/white/black visited sets.
///
/// Colors:
/// - White (absent): not visited
/// - Grey (in set): currently being visited (on DFS stack)
/// - Black (in set): fully visited, no cycle
pub fn detect_extends_cycle(
    templates: &HashMap<String, Template>,
) -> Result<(), ConfigError> {
    let mut grey: HashSet<String> = HashSet::new();
    let mut black: HashSet<String> = HashSet::new();

    for name in templates.keys() {
        if !black.contains(name) {
            let mut path = Vec::new();
            dfs_extends(name, templates, &mut grey, &mut black, &mut path)?;
        }
    }
    Ok(())
}

fn dfs_extends(
    name: &str,
    templates: &HashMap<String, Template>,
    grey: &mut HashSet<String>,
    black: &mut HashSet<String>,
    path: &mut Vec<String>,
) -> Result<(), ConfigError> {
    grey.insert(name.to_owned());
    path.push(name.to_owned());

    let extends = templates
        .get(name)
        .and_then(|t| t.config.extends.as_ref())
        .cloned()
        .unwrap_or_default();

    for base in &extends {
        if grey.contains(base) {
            // Found cycle — build cycle string
            let cycle_start = path.iter().position(|n| n == base).unwrap_or(0);
            let mut cycle_path = path[cycle_start..].to_vec();
            cycle_path.push(base.clone());
            return Err(ConfigError::CircularExtends {
                cycle: cycle_path.join(" → "),
            });
        }
        if !black.contains(base) {
            if !templates.contains_key(base) {
                let available = templates.keys().cloned().collect::<Vec<_>>().join(", ");
                return Err(ConfigError::MissingExtends {
                    name: base.clone(),
                    available,
                });
            }
            dfs_extends(base, templates, grey, black, path)?;
        }
    }

    path.pop();
    grey.remove(name);
    black.insert(name.to_owned());
    Ok(())
}

/// Detects cycles in persona extends (single-parent, only in PersonalLocal).
pub fn detect_persona_cycle(extends_chain: &[String]) -> Result<(), ConfigError> {
    let mut seen = HashSet::new();
    for name in extends_chain {
        if !seen.insert(name) {
            return Err(ConfigError::CircularExtends {
                cycle: extends_chain.join(" → "),
            });
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::schema::{JigConfig, Persona, Template};

    fn make_template(name: &str, extends: Vec<&str>) -> Template {
        Template {
            name: name.to_owned(),
            description: None,
            config: JigConfig {
                extends: if extends.is_empty() {
                    None
                } else {
                    Some(extends.iter().map(|s| s.to_string()).collect())
                },
                ..Default::default()
            },
        }
    }

    #[test]
    fn test_validate_layer_rejects_extends_in_team_config() {
        let config = JigConfig {
            persona: Some(Persona {
                extends: Some("base".to_owned()),
                ..Default::default()
            }),
            ..Default::default()
        };
        let result = validate_layer(&config, ConfigSource::TeamProject);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), ConfigError::PersonaExtendsInWrongScope { .. }));
    }

    #[test]
    fn test_validate_layer_allows_extends_in_personal_local() {
        let config = JigConfig {
            persona: Some(Persona {
                extends: Some("base".to_owned()),
                ..Default::default()
            }),
            ..Default::default()
        };
        assert!(validate_layer(&config, ConfigSource::PersonalLocal).is_ok());
    }

    #[test]
    fn test_validate_layer_allows_extends_absent() {
        let config = JigConfig::default();
        assert!(validate_layer(&config, ConfigSource::TeamProject).is_ok());
        assert!(validate_layer(&config, ConfigSource::GlobalUser).is_ok());
    }

    #[test]
    fn test_detect_extends_no_cycle() {
        let mut templates = HashMap::new();
        templates.insert("a".to_owned(), make_template("a", vec!["b"]));
        templates.insert("b".to_owned(), make_template("b", vec![]));
        assert!(detect_extends_cycle(&templates).is_ok());
    }

    #[test]
    fn test_detect_extends_direct_cycle() {
        let mut templates = HashMap::new();
        templates.insert("a".to_owned(), make_template("a", vec!["b"]));
        templates.insert("b".to_owned(), make_template("b", vec!["a"]));
        let result = detect_extends_cycle(&templates);
        assert!(matches!(result, Err(ConfigError::CircularExtends { .. })));
    }

    #[test]
    fn test_detect_extends_self_cycle() {
        let mut templates = HashMap::new();
        templates.insert("a".to_owned(), make_template("a", vec!["a"]));
        let result = detect_extends_cycle(&templates);
        assert!(matches!(result, Err(ConfigError::CircularExtends { .. })));
    }

    #[test]
    fn test_detect_extends_missing_base() {
        let mut templates = HashMap::new();
        templates.insert("a".to_owned(), make_template("a", vec!["nonexistent"]));
        let result = detect_extends_cycle(&templates);
        assert!(matches!(result, Err(ConfigError::MissingExtends { .. })));
    }

    #[test]
    fn test_detect_persona_cycle_no_cycle() {
        let chain = vec!["local".to_owned(), "project".to_owned(), "base".to_owned()];
        assert!(detect_persona_cycle(&chain).is_ok());
    }

    #[test]
    fn test_detect_persona_cycle_with_cycle() {
        let chain = vec!["a".to_owned(), "b".to_owned(), "a".to_owned()];
        let result = detect_persona_cycle(&chain);
        assert!(matches!(result, Err(ConfigError::CircularExtends { .. })));
    }
}
