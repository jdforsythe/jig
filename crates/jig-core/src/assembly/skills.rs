use std::os::unix::fs::symlink;
use std::path::{Path, PathBuf};

use thiserror::Error;
use tracing::{debug, info};

#[derive(Debug, Error)]
pub enum SkillsError {
    #[error("Skill path resolves outside allowed directory: {path} is not under {allowed_root}")]
    PathJailViolation { path: PathBuf, allowed_root: PathBuf },

    #[error("Failed to create skill symlink {src} -> {dst}: {source}")]
    SymlinkError {
        src: PathBuf,
        dst: PathBuf,
        source: std::io::Error,
    },

    #[error("Failed to create temp directory: {0}")]
    TempDirError(#[from] std::io::Error),
}

/// Creates the session temp directory at `/tmp/jig-XXXXXX/` with mode 0700.
pub fn create_temp_dir() -> Result<tempfile::TempDir, SkillsError> {
    tempfile::Builder::new()
        .prefix("jig-")
        .tempdir()
        .map_err(SkillsError::TempDirError)
}

/// Validates that `target` is a canonical subdirectory of `allowed_root`.
fn path_jail_check(target: &Path, allowed_root: &Path) -> Result<(), SkillsError> {
    let canonical_target = std::fs::canonicalize(target).unwrap_or_else(|_| target.to_owned());
    let canonical_root = std::fs::canonicalize(allowed_root).unwrap_or_else(|_| allowed_root.to_owned());

    if !canonical_target.starts_with(&canonical_root) {
        return Err(SkillsError::PathJailViolation {
            path: canonical_target,
            allowed_root: canonical_root,
        });
    }
    Ok(())
}

/// Symlinks local skill paths into the temp directory.
/// Each skill is symlinked as `temp_dir/skills/<skill_name>`.
/// Path jail is enforced: target must be under `allowed_root`.
pub fn stage_local_skills(
    temp_dir: &Path,
    local_skills: &[PathBuf],
    allowed_root: &Path,
) -> Result<Vec<PathBuf>, SkillsError> {
    let skills_dir = temp_dir.join("skills");
    std::fs::create_dir_all(&skills_dir)?;

    let mut staged = Vec::new();

    for skill_path in local_skills {
        // Path jail check
        path_jail_check(skill_path, allowed_root)?;

        let name = skill_path
            .file_name()
            .unwrap_or(skill_path.as_os_str());
        let link_path = skills_dir.join(name);

        if link_path.exists() || link_path.is_symlink() {
            debug!("Skill symlink already exists: {}", link_path.display());
            staged.push(link_path);
            continue;
        }

        symlink(skill_path, &link_path).map_err(|e| SkillsError::SymlinkError {
            src: skill_path.clone(),
            dst: link_path.clone(),
            source: e,
        })?;

        debug!("Staged skill: {} -> {}", link_path.display(), skill_path.display());
        staged.push(link_path);
    }

    info!("Staged {} skill symlinks", staged.len());
    Ok(staged)
}

/// Returns the path to the cached skills directory for a given source.
pub fn cached_skills_root(source_name: &str) -> PathBuf {
    home::home_dir()
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join(".config")
        .join("jig")
        .join("skills")
        .join(source_name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_path_jail_allows_subpath() {
        let dir = tempfile::tempdir().unwrap();
        let sub = dir.path().join("skills").join("my_skill");
        std::fs::create_dir_all(&sub).unwrap();
        assert!(path_jail_check(&sub, dir.path()).is_ok());
    }

    #[test]
    fn test_path_jail_rejects_escape() {
        let dir = tempfile::tempdir().unwrap();
        let outside = PathBuf::from("/etc");
        assert!(path_jail_check(&outside, dir.path()).is_err());
    }

    // ─── Task 0.5: stage_local_skills tests ───────────────────────────────────

    #[test]
    fn test_stage_local_skills_creates_symlinks() {
        let project_dir = tempfile::tempdir().unwrap();
        let skill_dir = project_dir.path().join("my_skill");
        std::fs::create_dir_all(&skill_dir).unwrap();

        let temp_dir = tempfile::tempdir().unwrap();
        let local_skills = vec![skill_dir.clone()];

        stage_local_skills(temp_dir.path(), &local_skills, project_dir.path()).unwrap();

        let expected_link = temp_dir.path().join("skills").join("my_skill");
        assert!(expected_link.is_symlink(), "symlink must be created at skills/<name>");
    }

    #[test]
    fn test_stage_local_skills_returns_staged_paths() {
        let project_dir = tempfile::tempdir().unwrap();
        let skill_dir = project_dir.path().join("some_skill");
        std::fs::create_dir_all(&skill_dir).unwrap();

        let temp_dir = tempfile::tempdir().unwrap();
        let local_skills = vec![skill_dir.clone()];

        let staged = stage_local_skills(temp_dir.path(), &local_skills, project_dir.path()).unwrap();

        assert_eq!(staged.len(), 1, "one skill must be staged");
        assert!(staged[0].to_string_lossy().contains("some_skill"), "staged path must include skill name");
    }

    #[test]
    fn test_stage_local_skills_path_jail_blocks_escape() {
        let project_dir = tempfile::tempdir().unwrap();
        let temp_dir = tempfile::tempdir().unwrap();
        // Point outside the project dir
        let outside = PathBuf::from("/etc");
        let local_skills = vec![outside];

        let err = stage_local_skills(temp_dir.path(), &local_skills, project_dir.path())
            .expect_err("path outside project dir must be rejected");
        assert!(matches!(err, SkillsError::PathJailViolation { .. }));
    }
}
