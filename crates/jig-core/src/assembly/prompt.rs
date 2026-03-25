use std::path::Path;

use tracing::debug;

use crate::config::resolve::ResolvedConfig;

/// Composes the full system prompt from the resolved config.
/// Returns the prompt as a String.
pub fn compose_system_prompt(resolved: &ResolvedConfig, project_dir: &Path) -> String {
    let mut parts: Vec<String> = Vec::new();

    // Persona rules
    if !resolved.persona.rules.is_empty() {
        if let Some(name) = &resolved.persona.name {
            parts.push(format!("<!-- jig persona: {name} -->"));
        }
        for rule in &resolved.persona.rules {
            parts.push(rule.clone());
        }
    }

    // Persona file content
    if let Some(file_path) = &resolved.persona.file {
        let path = if file_path.is_absolute() {
            file_path.clone()
        } else {
            project_dir.join(file_path)
        };
        match std::fs::read_to_string(&path) {
            Ok(content) => {
                debug!("Including persona file: {}", path.display());
                parts.push(content);
            }
            Err(e) => {
                tracing::warn!("Could not read persona file {}: {}", path.display(), e);
            }
        }
    }

    // Context fragments (already sorted by priority)
    for fragment in &resolved.context_fragments {
        let path = if fragment.path.is_absolute() {
            fragment.path.clone()
        } else {
            project_dir.join(&fragment.path)
        };
        match std::fs::read_to_string(&path) {
            Ok(content) => {
                debug!("Including context fragment: {}", path.display());
                if let Some(desc) = &fragment.description {
                    parts.push(format!("<!-- context: {desc} -->"));
                }
                parts.push(content);
            }
            Err(e) => {
                tracing::warn!("Could not read context fragment {}: {}", path.display(), e);
            }
        }
    }

    parts.join("\n\n")
}

/// Estimates token count using the character heuristic (chars / 4).
/// Returns (count, method) where method is "heuristic".
pub fn estimate_tokens(text: &str) -> (usize, &'static str) {
    let count = text.len() / 4;
    (count, "heuristic")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_estimate_tokens() {
        let text = "a".repeat(400);
        let (count, method) = estimate_tokens(&text);
        assert_eq!(count, 100);
        assert_eq!(method, "heuristic");
    }
}
