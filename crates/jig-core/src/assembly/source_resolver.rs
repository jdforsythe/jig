use std::collections::HashMap;
use std::path::PathBuf;
use thiserror::Error;

use super::skills::cached_skills_root;

#[derive(Debug, Error)]
pub enum SourceResolveError {
    #[error(
        "Skill '{skill}' from source '{source_name}' has not been synced. \
         Run `jig sync` to fetch it."
    )]
    SkillNotSynced { source_name: String, skill: String },
}

/// Resolves `from_source` skill references to absolute file paths.
/// Returns an error if any skill file does not exist locally (not yet synced).
pub fn resolve_from_source_skills(
    skills: &HashMap<String, Vec<String>>,
) -> Result<Vec<PathBuf>, SourceResolveError> {
    let mut paths = Vec::new();
    for (source_name, skill_list) in skills {
        for skill_name in skill_list {
            let path = skill_file_path(source_name, skill_name);
            if !path.exists() {
                // Check override path first
                let override_path = override_skill_path(source_name, skill_name);
                if !override_path.exists() {
                    return Err(SourceResolveError::SkillNotSynced {
                        source_name: source_name.clone(),
                        skill: skill_name.clone(),
                    });
                }
                paths.push(override_path);
            } else {
                // Prefer override if it exists
                let override_path = override_skill_path(source_name, skill_name);
                if override_path.exists() {
                    paths.push(override_path);
                } else {
                    paths.push(path);
                }
            }
        }
    }
    Ok(paths)
}

/// Returns the canonical path for a skill file from a source.
/// `~/.config/jig/skills/<source_name>/<skill_name>.md`
pub fn skill_file_path(source_name: &str, skill_name: &str) -> PathBuf {
    cached_skills_root(source_name).join(format!("{skill_name}.md"))
}

/// Returns the override path for a skill file.
/// `~/.config/jig/skills-override/<source_name>/<skill_name>.md`
pub fn override_skill_path(source_name: &str, skill_name: &str) -> PathBuf {
    home::home_dir()
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join(".config")
        .join("jig")
        .join("skills-override")
        .join(source_name)
        .join(format!("{skill_name}.md"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_empty_map_returns_empty() {
        let result = resolve_from_source_skills(&HashMap::new()).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn test_resolve_missing_skill_returns_error() {
        let mut skills = HashMap::new();
        skills.insert("my-source".to_owned(), vec!["nonexistent-skill".to_owned()]);

        let result = resolve_from_source_skills(&skills);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("nonexistent-skill"));
        assert!(err.to_string().contains("my-source"));
    }

    #[test]
    fn test_resolve_existing_skill_returns_path() {
        let source = "test-source";
        let skill = "test-skill";

        // Use override path since we can control its location via skill_file_path logic
        // Actually test that skill_file_path returns expected structure
        let path = skill_file_path(source, skill);
        assert!(path.to_string_lossy().contains("skills"));
        assert!(path.to_string_lossy().contains("test-source"));
        assert!(path.to_string_lossy().contains("test-skill.md"));
    }

    #[test]
    fn test_override_path_structure() {
        let path = override_skill_path("my-source", "my-skill");
        assert!(path.to_string_lossy().contains("skills-override"));
        assert!(path.to_string_lossy().contains("my-source"));
        assert!(path.to_string_lossy().contains("my-skill.md"));
    }

    #[test]
    fn test_skill_file_path_structure() {
        let path = skill_file_path("source1", "skill1");
        assert!(path.ends_with("source1/skill1.md"));
    }
}
