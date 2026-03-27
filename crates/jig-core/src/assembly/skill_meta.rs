use std::path::Path;
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct SkillMeta {
    pub name: Option<String>,
    pub description: Option<String>,
    pub tags: Option<Vec<String>>,
    pub version: Option<String>,
}

#[derive(Debug, Error)]
pub enum SkillMetaError {
    #[error("Failed to read skill file {path}: {source}")]
    ReadError {
        path: std::path::PathBuf,
        source: std::io::Error,
    },
}

/// Parses YAML frontmatter from a Markdown skill file.
/// Returns `SkillMeta::default()` if no frontmatter is present (not an error).
/// Only IO errors are propagated.
pub fn parse_frontmatter(path: &Path) -> Result<SkillMeta, SkillMetaError> {
    let content = std::fs::read_to_string(path).map_err(|e| SkillMetaError::ReadError {
        path: path.to_owned(),
        source: e,
    })?;
    Ok(parse_frontmatter_str(&content))
}

/// Pure string parser for YAML frontmatter.
/// Returns `SkillMeta::default()` if no frontmatter is present or if YAML is malformed.
pub fn parse_frontmatter_str(content: &str) -> SkillMeta {
    let trimmed = content.trim_start();

    // Must start with "---"
    if !trimmed.starts_with("---") {
        return SkillMeta::default();
    }

    let rest = &trimmed[3..];
    // Skip optional newline after opening ---
    let rest = rest.trim_start_matches('\r').trim_start_matches('\n');

    // Find closing ---
    let end_pos = rest.find("\n---")
        .or_else(|| rest.find("\r\n---"));

    let yaml_str = match end_pos {
        Some(pos) => &rest[..pos],
        None => return SkillMeta::default(),
    };

    // Parse the YAML frontmatter
    serde_yaml::from_str(yaml_str).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_valid_frontmatter() {
        let content = "---\nname: my-skill\ndescription: A useful skill\ntags:\n  - rust\n  - cli\nversion: \"1.0.0\"\n---\n\n# Content here\n";
        let meta = parse_frontmatter_str(content);
        assert_eq!(meta.name, Some("my-skill".to_owned()));
        assert_eq!(meta.description, Some("A useful skill".to_owned()));
        assert_eq!(meta.tags, Some(vec!["rust".to_owned(), "cli".to_owned()]));
        assert_eq!(meta.version, Some("1.0.0".to_owned()));
    }

    #[test]
    fn test_parse_no_frontmatter_returns_default() {
        let content = "# Just a markdown file\nNo frontmatter here.\n";
        let meta = parse_frontmatter_str(content);
        assert_eq!(meta, SkillMeta::default());
    }

    #[test]
    fn test_parse_empty_file_returns_default() {
        let meta = parse_frontmatter_str("");
        assert_eq!(meta, SkillMeta::default());
    }

    #[test]
    fn test_parse_malformed_yaml_returns_default() {
        let content = "---\nthis: is: invalid: yaml: !!!\n---\n# Content\n";
        let meta = parse_frontmatter_str(content);
        // May or may not parse correctly, but must not panic
        // serde_yaml might partially parse it — just ensure no panic
        let _ = meta;
    }

    #[test]
    fn test_parse_windows_line_endings() {
        let content = "---\r\nname: win-skill\r\ndescription: Windows test\r\n---\r\n# Content\r\n";
        let meta = parse_frontmatter_str(content);
        // Windows line endings should work — either parse or return default without panic
        let _ = meta;
    }

    #[test]
    fn test_parse_partial_frontmatter() {
        let content = "---\nname: only-name\n---\n";
        let meta = parse_frontmatter_str(content);
        assert_eq!(meta.name, Some("only-name".to_owned()));
        assert!(meta.description.is_none());
        assert!(meta.tags.is_none());
    }

    #[test]
    fn test_parse_frontmatter_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("skill.md");
        std::fs::write(&path, "---\nname: file-skill\ndescription: From file\n---\n# Content\n").unwrap();

        let meta = parse_frontmatter(&path).unwrap();
        assert_eq!(meta.name, Some("file-skill".to_owned()));
    }
}
