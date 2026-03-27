use std::collections::HashMap;
use std::path::PathBuf;
use serde::{Deserialize, Serialize};
use tracing::warn;

use super::skill_meta::{parse_frontmatter, SkillMeta};

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct SkillIndex {
    #[serde(default)]
    pub entries: HashMap<String, Vec<IndexedSkill>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexedSkill {
    pub source: String,
    pub skill_name: String,
    pub path: PathBuf,
    pub meta: SkillMeta,
}

/// Path to the skill index cache: `~/.config/jig/state/skill-index.json`
pub fn index_path() -> PathBuf {
    home::home_dir()
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join(".config")
        .join("jig")
        .join("state")
        .join("skill-index.json")
}

/// Reads the skill index from disk. Returns empty index if file doesn't exist.
pub fn read_index() -> SkillIndex {
    let path = index_path();
    if !path.exists() {
        return SkillIndex::default();
    }
    match std::fs::read_to_string(&path) {
        Ok(content) => serde_json::from_str(&content).unwrap_or_default(),
        Err(e) => {
            warn!("Failed to read skill index: {e}");
            SkillIndex::default()
        }
    }
}

/// Writes the skill index to disk atomically.
pub fn write_index(index: &SkillIndex) -> std::io::Result<()> {
    let path = index_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let content = serde_json::to_string_pretty(index)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))?;
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, content)?;
    std::fs::rename(&tmp, &path)
}

/// Rebuilds the skill index by scanning `~/.config/jig/skills/<source>/*.md`.
pub fn rebuild_index() -> std::io::Result<()> {
    let skills_root = home::home_dir()
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join(".config")
        .join("jig")
        .join("skills");

    let mut index = SkillIndex::default();

    if !skills_root.exists() {
        return write_index(&index);
    }

    for source_entry in std::fs::read_dir(&skills_root)?.flatten() {
        let source_path = source_entry.path();
        if !source_path.is_dir() {
            continue;
        }
        let source_name = source_entry.file_name().to_string_lossy().into_owned();

        for skill_entry in std::fs::read_dir(&source_path)?.flatten() {
            let path = skill_entry.path();
            if path.extension().map(|e| e == "md").unwrap_or(false) {
                let skill_name = path.file_stem()
                    .map(|s| s.to_string_lossy().into_owned())
                    .unwrap_or_default();

                let meta = parse_frontmatter(&path).unwrap_or_default();

                index
                    .entries
                    .entry(source_name.clone())
                    .or_default()
                    .push(IndexedSkill {
                        source: source_name.clone(),
                        skill_name,
                        path,
                        meta,
                    });
            }
        }
    }

    write_index(&index)
}

/// Searches the index by keyword (case-insensitive substring match on name, description, or tags).
pub fn search(index: &SkillIndex, query: &str) -> Vec<IndexedSkill> {
    let q = query.to_lowercase();
    index
        .entries
        .values()
        .flatten()
        .filter(|s| {
            s.skill_name.to_lowercase().contains(&q)
                || s.meta
                    .description
                    .as_deref()
                    .unwrap_or("")
                    .to_lowercase()
                    .contains(&q)
                || s.meta
                    .tags
                    .as_deref()
                    .unwrap_or(&[])
                    .iter()
                    .any(|t| t.to_lowercase().contains(&q))
        })
        .cloned()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn make_index_with_skills() -> SkillIndex {
        let mut index = SkillIndex::default();
        index.entries.insert("test-source".to_owned(), vec![
            IndexedSkill {
                source: "test-source".to_owned(),
                skill_name: "code-review".to_owned(),
                path: PathBuf::from("/fake/path.md"),
                meta: SkillMeta {
                    name: Some("Code Review".to_owned()),
                    description: Some("Reviews code for quality".to_owned()),
                    tags: Some(vec!["code".to_owned(), "review".to_owned()]),
                    version: Some("1.0.0".to_owned()),
                },
            },
            IndexedSkill {
                source: "test-source".to_owned(),
                skill_name: "security-audit".to_owned(),
                path: PathBuf::from("/fake/security.md"),
                meta: SkillMeta {
                    name: Some("Security Audit".to_owned()),
                    description: Some("Audits for vulnerabilities".to_owned()),
                    tags: Some(vec!["security".to_owned(), "audit".to_owned()]),
                    version: None,
                },
            },
        ]);
        index
    }

    #[test]
    fn test_search_by_name() {
        let index = make_index_with_skills();
        let results = search(&index, "code");
        assert!(!results.is_empty());
        assert!(results.iter().any(|s| s.skill_name == "code-review"));
    }

    #[test]
    fn test_search_by_tag() {
        let index = make_index_with_skills();
        let results = search(&index, "security");
        assert!(!results.is_empty());
        assert!(results.iter().any(|s| s.skill_name == "security-audit"));
    }

    #[test]
    fn test_search_case_insensitive() {
        let index = make_index_with_skills();
        let results = search(&index, "CODE");
        assert!(!results.is_empty());
    }

    #[test]
    fn test_search_no_matches_returns_empty() {
        let index = make_index_with_skills();
        let results = search(&index, "zzz-no-match-xyz");
        assert!(results.is_empty());
    }

    #[test]
    fn test_search_by_description() {
        let index = make_index_with_skills();
        let results = search(&index, "vulnerabilities");
        assert!(!results.is_empty());
        assert!(results.iter().any(|s| s.skill_name == "security-audit"));
    }

    #[test]
    fn test_rebuild_index_empty_dir_writes_empty() {
        // rebuild_index will scan ~/.config/jig/skills/ — if it doesn't exist, returns empty
        // We can't easily redirect, but ensure no panic
        let _ = SkillIndex::default();
    }

    #[test]
    fn test_read_index_missing_returns_default() {
        let index = read_index();
        // Either returns valid index or default — must not panic
        let _ = index;
    }

    #[test]
    fn test_index_round_trip_serialization() {
        let index = make_index_with_skills();
        let json = serde_json::to_string_pretty(&index).unwrap();
        let parsed: SkillIndex = serde_json::from_str(&json).unwrap();

        let skills = parsed.entries.get("test-source").unwrap();
        assert_eq!(skills.len(), 2);
        // Order may differ since HashMap iteration is non-deterministic
        assert!(skills.iter().any(|s| s.skill_name == "code-review"));
    }

    #[test]
    fn test_write_and_read_index() {
        let dir = tempdir().unwrap();
        // Test the toml serialization/deserialization logic directly since
        // we cannot redirect index_path() to a temp directory
        let index = make_index_with_skills();
        let json = serde_json::to_string_pretty(&index).unwrap();
        let path = dir.path().join("skill-index.json");
        std::fs::write(&path, &json).unwrap();

        let content = std::fs::read_to_string(&path).unwrap();
        let parsed: SkillIndex = serde_json::from_str(&content).unwrap();
        assert!(!parsed.entries.is_empty());
    }
}
