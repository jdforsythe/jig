use std::collections::HashMap;
use std::path::Path;
use std::process::Command;
use thiserror::Error;
use tracing::info;

use super::skills::cached_skills_root;
use crate::config::schema::SourceConfig;

#[derive(Debug, Error)]
pub enum SyncError {
    #[error("git is not available in PATH. Install git to use `jig sync`.")]
    GitNotFound,

    #[error("git clone failed for source '{source_name}': {stderr}")]
    CloneFailed { source_name: String, stderr: String },

    #[error("git fetch failed for source '{source_name}': {stderr}")]
    FetchFailed { source_name: String, stderr: String },

    #[error("Source '{source_name}' is out of date (--frozen mode). Run `jig sync` to update.")]
    FrozenOutOfDate { source_name: String },

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

pub struct SyncOptions {
    pub frozen: bool,
    pub check: bool,
}

#[derive(Debug)]
pub enum SyncAction {
    Cloned,
    Updated { from_sha: String },
    AlreadyUpToDate,
    BehindCheck { local_sha: String, remote_sha: String },
    UpToDateCheck,
    SkippedNoUrl,
}

#[derive(Debug)]
pub struct SyncOutcome {
    pub source_name: String,
    pub action: SyncAction,
    pub new_sha: Option<String>,
}

/// Syncs all configured sources.
pub fn sync_sources(
    sources: &HashMap<String, SourceConfig>,
    opts: &SyncOptions,
) -> Result<Vec<SyncOutcome>, SyncError> {
    // Verify git is available
    if !git_available() {
        return Err(SyncError::GitNotFound);
    }

    let mut outcomes = Vec::new();

    for (name, config) in sources {
        let dest = cached_skills_root(name);
        let outcome = sync_single_source(name, config, &dest, opts)?;
        outcomes.push(outcome);
    }

    Ok(outcomes)
}

fn sync_single_source(
    name: &str,
    config: &SourceConfig,
    dest: &Path,
    opts: &SyncOptions,
) -> Result<SyncOutcome, SyncError> {
    if dest.join(".git").exists() {
        // Already cloned — fetch or check
        if opts.check {
            return check_staleness(name, dest);
        }
        if opts.frozen {
            return check_frozen(name, dest);
        }
        update_source(name, config, dest)
    } else {
        // First time — clone
        if opts.check {
            return Ok(SyncOutcome {
                source_name: name.to_owned(),
                action: SyncAction::SkippedNoUrl,
                new_sha: None,
            });
        }
        if opts.frozen {
            return Err(SyncError::FrozenOutOfDate { source_name: name.to_owned() });
        }
        clone_source(name, config, dest)
    }
}

pub fn git_available() -> bool {
    Command::new("git")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn clone_source(name: &str, config: &SourceConfig, dest: &Path) -> Result<SyncOutcome, SyncError> {
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let mut cmd = Command::new("git");
    cmd.arg("clone").arg("--no-recurse-submodules");

    if let Some(rev) = &config.rev {
        cmd.arg("--branch").arg(rev);
    }

    cmd.arg(&config.url).arg(dest);

    info!("Cloning {} from {}", name, config.url);

    let output = cmd.output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        return Err(SyncError::CloneFailed { source_name: name.to_owned(), stderr });
    }

    let new_sha = local_sha(dest);
    Ok(SyncOutcome {
        source_name: name.to_owned(),
        action: SyncAction::Cloned,
        new_sha,
    })
}

fn update_source(name: &str, config: &SourceConfig, dest: &Path) -> Result<SyncOutcome, SyncError> {
    let old_sha = local_sha(dest);

    // git fetch origin --no-recurse-submodules
    let output = Command::new("git")
        .args(["fetch", "origin", "--no-recurse-submodules"])
        .current_dir(dest)
        .output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        return Err(SyncError::FetchFailed { source_name: name.to_owned(), stderr });
    }

    // git reset --hard origin/HEAD (or rev)
    let target = config.rev.as_deref().map(|r| format!("origin/{r}"))
        .unwrap_or_else(|| "origin/HEAD".to_owned());

    Command::new("git")
        .args(["reset", "--hard", &target])
        .current_dir(dest)
        .output()?;

    let new_sha = local_sha(dest);

    let action = if old_sha == new_sha {
        SyncAction::AlreadyUpToDate
    } else {
        SyncAction::Updated { from_sha: old_sha.unwrap_or_default() }
    };

    Ok(SyncOutcome {
        source_name: name.to_owned(),
        action,
        new_sha,
    })
}

fn check_staleness(name: &str, dest: &Path) -> Result<SyncOutcome, SyncError> {
    // Fetch quietly to see if there are upstream changes
    Command::new("git")
        .args(["fetch", "origin", "--no-recurse-submodules", "--dry-run"])
        .current_dir(dest)
        .output()
        .ok(); // Ignore errors in check mode

    let local = local_sha(dest);
    let remote = remote_sha(dest);

    let action = if local == remote || remote.is_none() {
        SyncAction::UpToDateCheck
    } else {
        SyncAction::BehindCheck {
            local_sha: local.clone().unwrap_or_default(),
            remote_sha: remote.unwrap_or_default(),
        }
    };

    Ok(SyncOutcome {
        source_name: name.to_owned(),
        action,
        new_sha: local,
    })
}

fn check_frozen(name: &str, dest: &Path) -> Result<SyncOutcome, SyncError> {
    // In frozen mode, check if we're behind without fetching
    let local = local_sha(dest);
    let remote = remote_sha(dest);

    if remote.is_some() && local != remote {
        return Err(SyncError::FrozenOutOfDate { source_name: name.to_owned() });
    }

    Ok(SyncOutcome {
        source_name: name.to_owned(),
        action: SyncAction::AlreadyUpToDate,
        new_sha: local,
    })
}

/// Returns the local HEAD SHA for a cloned source.
pub fn local_sha(dest: &Path) -> Option<String> {
    Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(dest)
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_owned())
}

fn remote_sha(dest: &Path) -> Option<String> {
    Command::new("git")
        .args(["rev-parse", "@{u}"])
        .current_dir(dest)
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sync_sources_empty_map_returns_empty() {
        let sources: HashMap<String, SourceConfig> = HashMap::new();
        let opts = SyncOptions { frozen: false, check: false };

        // Skip if git not available
        if !git_available() {
            return;
        }

        let result = sync_sources(&sources, &opts).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn test_git_available_returns_bool() {
        // Just verify it doesn't panic
        let _ = git_available();
    }

    #[test]
    fn test_local_sha_nonexistent_dir_returns_none() {
        let path = std::path::PathBuf::from("/tmp/nonexistent-jig-test-xyz");
        let sha = local_sha(&path);
        assert!(sha.is_none());
    }

    // Git integration tests — gated on JIG_RUN_GIT_TESTS env var
    #[test]
    #[cfg_attr(not(feature = "integration-tests"), ignore)]
    fn test_clone_and_update_source() {
        if std::env::var("JIG_RUN_GIT_TESTS").is_err() {
            return;
        }
        // Would need a real git remote — integration test placeholder
    }
}
