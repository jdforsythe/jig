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
    let approval_cache = load_approval_cache();

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
                append_approval_cache(&hash, &command, source);
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
        ConfigSource::CliFlag => HookTrustTier::Full,
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

fn load_approval_cache() -> std::collections::HashSet<String> {
    let path = approval_cache_path();
    let Ok(contents) = std::fs::read_to_string(&path) else {
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

fn append_approval_cache(hash: &str, command: &str, source: &ConfigSource) {
    use std::io::Write;

    let path = approval_cache_path();
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
        .open(&path)
    {
        let _ = writeln!(file, "{}", record);
    }
}
