use std::path::PathBuf;

use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::config::schema::{ConfigSource, HookEntry, HookTrustTier};

/// A request for hook/MCP approval from the UI layer.
pub struct ApprovalRequest {
    pub tier: HookTrustTier,
    pub command: String,
    pub source_file: PathBuf,
    pub previous_command: Option<String>,
}

/// Decision returned from the UI layer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApprovalDecision {
    Approved,
    Denied,
    ApproveSession,
}

/// Trait for the approval UI — implemented by TerminalApprovalUi, TuiApprovalUi, MockApprovalUi.
/// Core defines the trait; the binary implements it.
pub trait ApprovalUi: Send {
    fn prompt_approval(&self, req: &ApprovalRequest) -> ApprovalDecision;
}

/// Error type for hook denial.
#[derive(Debug, Error)]
#[error("Hook denied: {command} (from {hook_source})")]
pub struct HookDeniedError {
    pub hook_source: String,
    pub command: String,
}

/// Runs hook approval checks for all pre_launch hooks.
pub fn run_hook_approvals(
    hooks: &[(HookEntry, ConfigSource)],
    ui: &dyn ApprovalUi,
    yes: bool,
) -> Result<(), HookDeniedError> {
    run_hook_approvals_inner(hooks, ui, yes, &approval_cache_path())
}

/// Inner implementation that accepts an explicit cache path.
/// Used by tests to inject a temp cache file without touching the real one.
fn run_hook_approvals_inner(
    hooks: &[(HookEntry, ConfigSource)],
    ui: &dyn ApprovalUi,
    yes: bool,
    cache_path: &std::path::Path,
) -> Result<(), HookDeniedError> {
    let approval_cache = load_approval_cache_from(cache_path);

    for (hook, source) in hooks {
        let command = hook.display_command().to_owned();
        let hash = sha256_command(&command);
        let tier = source_to_tier(*source, None);

        // Check cache first
        let is_cached = approval_cache.contains(&hash);

        if yes && is_cached {
            // --yes only auto-approves cached items
            continue;
        }

        if yes && !is_cached {
            // New items with --yes: still require manual approval for external hooks
            match tier {
                HookTrustTier::Full | HookTrustTier::Personal | HookTrustTier::Team => {
                    // Auto-approve for non-external tiers with --yes
                    continue;
                }
                HookTrustTier::ExternalSkill { .. } => {
                    // External hooks require explicit approval even with --yes
                }
            }
        }

        if is_cached {
            continue;
        }

        // Prompt user
        let req = ApprovalRequest {
            tier: tier.clone(),
            command: command.clone(),
            source_file: PathBuf::from(source.to_string()),
            previous_command: None,
        };

        match ui.prompt_approval(&req) {
            ApprovalDecision::Approved | ApprovalDecision::ApproveSession => {
                append_approval_cache_to(cache_path, &hash, &command, source);
            }
            ApprovalDecision::Denied => {
                return Err(HookDeniedError {
                    hook_source: source.to_string(),
                    command,
                });
            }
        }
    }

    Ok(())
}

fn source_to_tier(source: ConfigSource, _url: Option<&str>) -> HookTrustTier {
    match source {
        ConfigSource::GlobalUser => HookTrustTier::Full,
        ConfigSource::TeamProject => HookTrustTier::Team,
        ConfigSource::PersonalLocal => HookTrustTier::Personal,
        // CLI-sourced variants: template selection and explicit flags both carry Full trust
        // because the user directly initiated them at the command line.
        ConfigSource::CliFlag | ConfigSource::TemplateSelected | ConfigSource::ExplicitCliFlag => {
            HookTrustTier::Full
        }
    }
}

/// Computes SHA-256 of the command string (pre-expansion, as it appears in config).
pub fn sha256_command(command: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(command.as_bytes());
    format!("sha256:{}", hex::encode(hasher.finalize()))
}

fn approval_cache_path() -> PathBuf {
    home::home_dir()
        .unwrap_or_default()
        .join(".config")
        .join("jig")
        .join("state")
        .join("hook-approvals.jsonl")
}

fn load_approval_cache_from(path: &std::path::Path) -> std::collections::HashSet<String> {
    let Ok(contents) = std::fs::read_to_string(path) else {
        return Default::default();
    };

    contents
        .lines()
        .filter_map(|line| {
            serde_json::from_str::<serde_json::Value>(line)
                .ok()
                .and_then(|v| v["command_hash"].as_str().map(str::to_owned))
        })
        .collect()
}

fn append_approval_cache_to(
    path: &std::path::Path,
    hash: &str,
    command: &str,
    source: &ConfigSource,
) {
    use std::io::Write;

    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }

    let record = serde_json::json!({
        "command_hash": hash,
        "command": command,
        "source": source.to_string(),
        "approved_at": chrono::Utc::now().to_rfc3339(),
        "last_used_at": chrono::Utc::now().to_rfc3339(),
    });

    if let Ok(mut file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
    {
        let _ = writeln!(file, "{}", record);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::schema::HookEntry;

    struct MockApprovalUi(std::sync::Mutex<Vec<ApprovalDecision>>);

    impl ApprovalUi for MockApprovalUi {
        fn prompt_approval(&self, _req: &ApprovalRequest) -> ApprovalDecision {
            self.0.lock().unwrap().remove(0)
        }
    }

    fn make_hook(cmd: &str) -> (HookEntry, ConfigSource) {
        (HookEntry::Exec { exec: vec![cmd.to_owned()] }, ConfigSource::TeamProject)
    }

    #[test]
    fn test_sha256_command_is_stable() {
        let h1 = sha256_command("foo");
        let h2 = sha256_command("foo");
        assert_eq!(h1, h2, "same input must produce same hash");
        assert!(h1.starts_with("sha256:"), "hash must have sha256: prefix");
    }

    #[test]
    fn test_sha256_command_differs_for_different_input() {
        let h1 = sha256_command("foo");
        let h2 = sha256_command("bar");
        assert_ne!(h1, h2, "different inputs must produce different hashes");
    }

    #[test]
    fn test_approval_cache_hit_skips_prompt() {
        let dir = tempfile::tempdir().unwrap();
        let cache_path = dir.path().join("approvals.jsonl");

        // Pre-populate the cache with the hash of our command
        let cmd = "notify-send done";
        let hash = sha256_command(cmd);
        let line = serde_json::json!({
            "command_hash": hash,
            "command": cmd,
            "source": "team_project",
        });
        std::fs::write(&cache_path, format!("{line}\n")).unwrap();

        // MockApprovalUi that panics if called — cache hit must skip prompt
        struct PanicUi;
        impl ApprovalUi for PanicUi {
            fn prompt_approval(&self, _req: &ApprovalRequest) -> ApprovalDecision {
                panic!("prompt_approval must not be called on a cache hit");
            }
        }

        let hooks = vec![make_hook(cmd)];
        let result = run_hook_approvals_inner(&hooks, &PanicUi, false, &cache_path);
        assert!(result.is_ok(), "cache hit must succeed without prompting");
    }

    #[test]
    fn test_approval_cache_miss_prompts_once() {
        let dir = tempfile::tempdir().unwrap();
        let cache_path = dir.path().join("approvals.jsonl");
        // Cache does not exist — fresh miss

        let ui = MockApprovalUi(std::sync::Mutex::new(vec![ApprovalDecision::Approved]));
        let hooks = vec![make_hook("some-command")];
        let result = run_hook_approvals_inner(&hooks, &ui, false, &cache_path);
        assert!(result.is_ok(), "approved decision must succeed");
        // Cache should now exist with the entry
        assert!(cache_path.exists(), "approval must be persisted to cache");
        let contents = std::fs::read_to_string(&cache_path).unwrap();
        assert!(contents.contains("some-command"), "approved command must be in cache");
    }

    #[test]
    fn test_approval_denied_returns_error() {
        let dir = tempfile::tempdir().unwrap();
        let cache_path = dir.path().join("approvals.jsonl");

        let ui = MockApprovalUi(std::sync::Mutex::new(vec![ApprovalDecision::Denied]));
        let hooks = vec![make_hook("dangerous-script")];
        let result = run_hook_approvals_inner(&hooks, &ui, false, &cache_path);
        assert!(result.is_err(), "denied decision must return error");
        let err = result.unwrap_err();
        assert!(err.command.contains("dangerous-script"), "error must name the command");
    }
}
