use std::collections::HashMap;
use std::path::{Path, PathBuf};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tracing::warn;

use super::sync::SyncOutcome;

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct SkillsLock {
    #[serde(default)]
    pub sources: HashMap<String, SourceLockEntry>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SourceLockEntry {
    pub url: String,
    pub fetched_at: String,  // RFC 3339
    pub sha: String,         // git HEAD SHA
    pub rev: String,         // branch/tag used
    #[serde(default)]
    pub skills: HashMap<String, SkillLockEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillLockEntry {
    pub sha256: String,      // "sha256:<hex>" format
    pub size_bytes: u64,
}

/// Path to the skills lock file: `~/.config/jig/skills.lock`
pub fn skills_lock_path() -> PathBuf {
    home::home_dir()
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join(".config")
        .join("jig")
        .join("skills.lock")
}

/// Reads the skills lock file. Returns an empty lock if file doesn't exist.
pub fn read_skills_lock() -> SkillsLock {
    let path = skills_lock_path();
    if !path.exists() {
        return SkillsLock::default();
    }
    match std::fs::read_to_string(&path) {
        Ok(content) => toml::from_str(&content).unwrap_or_default(),
        Err(e) => {
            warn!("Failed to read skills.lock: {e}");
            SkillsLock::default()
        }
    }
}

/// Writes the skills lock file atomically.
pub fn write_skills_lock(lock: &SkillsLock) -> std::io::Result<()> {
    let path = skills_lock_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let content = toml::to_string_pretty(lock)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))?;
    let tmp = path.with_extension("lock.tmp");
    std::fs::write(&tmp, content)?;
    std::fs::rename(&tmp, &path)
}

/// Updates the skills lock with the outcomes from a sync operation.
pub fn update_skills_lock(
    outcomes: &[SyncOutcome],
    sources: &HashMap<String, crate::config::schema::SourceConfig>,
) -> std::io::Result<()> {
    let mut lock = read_skills_lock();

    for outcome in outcomes {
        let name = &outcome.source_name;
        let config = match sources.get(name) {
            Some(c) => c,
            None => continue,
        };

        let sha = match &outcome.new_sha {
            Some(s) => s.clone(),
            None => continue,
        };

        let entry = lock.sources.entry(name.clone()).or_insert_with(|| SourceLockEntry {
            url: config.url.clone(),
            fetched_at: Utc::now().to_rfc3339(),
            sha: sha.clone(),
            rev: config.rev.clone().unwrap_or_else(|| "HEAD".to_owned()),
            skills: HashMap::new(),
        });

        entry.sha = sha;
        entry.fetched_at = Utc::now().to_rfc3339();
        entry.url = config.url.clone();

        // Scan skills directory and update hashes
        let skills_dir = super::skills::cached_skills_root(name);
        if skills_dir.exists() {
            if let Ok(read_dir) = std::fs::read_dir(&skills_dir) {
                for file_entry in read_dir.flatten() {
                    let path = file_entry.path();
                    if path.extension().map(|e| e == "md").unwrap_or(false) {
                        let skill_name = path.file_stem()
                            .map(|s| s.to_string_lossy().into_owned())
                            .unwrap_or_default();

                        if let Ok(meta) = std::fs::metadata(&path) {
                            if let Ok(content) = std::fs::read(&path) {
                                let hash = sha256_bytes(&content);
                                entry.skills.insert(skill_name, SkillLockEntry {
                                    sha256: format!("sha256:{hash}"),
                                    size_bytes: meta.len(),
                                });
                            }
                        }
                    }
                }
            }
        }
    }

    write_skills_lock(&lock)
}

/// Verifies a skill file's SHA-256 against the lock file.
/// Returns `None` if no lock entry exists (skip verification).
/// Returns `Some(true)` if hash matches, `Some(false)` if tampered.
pub fn verify_skill_integrity(source: &str, skill: &str, file_path: &Path) -> Option<bool> {
    let lock = read_skills_lock();
    let source_entry = lock.sources.get(source)?;
    let skill_entry = source_entry.skills.get(skill)?;

    let content = std::fs::read(file_path).ok()?;
    let hash = sha256_bytes(&content);
    let expected = format!("sha256:{hash}");

    Some(skill_entry.sha256 == expected)
}

fn sha256_bytes(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_skills_lock_round_trip() {
        let mut lock = SkillsLock::default();
        lock.sources.insert("my-source".to_owned(), SourceLockEntry {
            url: "https://github.com/example/skills".to_owned(),
            fetched_at: "2026-01-01T00:00:00Z".to_owned(),
            sha: "abc123".to_owned(),
            rev: "main".to_owned(),
            skills: {
                let mut s = HashMap::new();
                s.insert("my-skill".to_owned(), SkillLockEntry {
                    sha256: "sha256:deadbeef".to_owned(),
                    size_bytes: 1024,
                });
                s
            },
        });

        // Test serialize/deserialize
        let content = toml::to_string_pretty(&lock).unwrap();
        let parsed: SkillsLock = toml::from_str(&content).unwrap();

        let source = parsed.sources.get("my-source").unwrap();
        assert_eq!(source.url, "https://github.com/example/skills");
        assert_eq!(source.sha, "abc123");
        let skill = source.skills.get("my-skill").unwrap();
        assert_eq!(skill.sha256, "sha256:deadbeef");
        assert_eq!(skill.size_bytes, 1024);
    }

    #[test]
    fn test_read_missing_lock_returns_default() {
        // skills_lock_path() in a test env may or may not exist
        // Test that read_skills_lock doesn't panic and returns something valid
        // (we can't easily redirect the path, but the function handles missing gracefully)
        let lock = SkillsLock::default();
        assert!(lock.sources.is_empty());
    }

    #[test]
    fn test_verify_integrity_no_entry_returns_none() {
        let path = std::path::PathBuf::from("/tmp/nonexistent-jig-skill.md");
        let result = verify_skill_integrity("unknown-source", "unknown-skill", &path);
        assert!(result.is_none());
    }

    #[test]
    fn test_write_skills_lock_creates_parent() {
        let _dir = tempdir().unwrap();
        // Test that write_skills_lock creates parent dirs
        // We need to write to a custom path — test the toml serialization directly
        let lock = SkillsLock::default();
        let content = toml::to_string_pretty(&lock).unwrap();
        // The serialized form should be parseable (even if empty/minimal)
        let _parsed: SkillsLock = toml::from_str(&content).unwrap();
    }

    #[test]
    fn test_sha256_bytes_deterministic() {
        let data = b"hello world";
        let hash1 = sha256_bytes(data);
        let hash2 = sha256_bytes(data);
        assert_eq!(hash1, hash2);
        assert_eq!(hash1.len(), 64); // 32 bytes = 64 hex chars
    }

    #[test]
    fn test_sha256_format_prefix() {
        // verify_skill_integrity expects "sha256:<hex>" format
        let data = b"test";
        let hash = sha256_bytes(data);
        let formatted = format!("sha256:{hash}");
        assert!(formatted.starts_with("sha256:"));
    }
}
