use std::path::PathBuf;

use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::config::schema::{ConfigSource, HookEntry, HookTrustTier, McpServer};

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

/// Computes SHA-256 of `mcp:{name}:{command}:{url}` for stable approval caching.
pub fn mcp_server_hash(name: &str, server: &McpServer) -> String {
    let command = server.command.as_deref().unwrap_or("");
    let url = server.url.as_deref().unwrap_or("");
    let input = format!("mcp:{name}:{command}:{url}");
    let mut hasher = Sha256::new();
    hasher.update(input.as_bytes());
    format!("sha256:{}", hex::encode(hasher.finalize()))
}

/// Runs MCP server approval checks for all servers on first use.
pub fn run_mcp_approvals(
    mcp_servers: &std::collections::HashMap<String, McpServer>,
    resolution_trace: &std::collections::HashMap<String, String>,
    ui: &dyn ApprovalUi,
    yes: bool,
) -> Result<(), HookDeniedError> {
    run_mcp_approvals_inner(mcp_servers, resolution_trace, ui, yes, &approval_cache_path())
}

/// Inner implementation that accepts an explicit cache path for testability.
fn run_mcp_approvals_inner(
    mcp_servers: &std::collections::HashMap<String, McpServer>,
    resolution_trace: &std::collections::HashMap<String, String>,
    ui: &dyn ApprovalUi,
    yes: bool,
    cache_path: &std::path::Path,
) -> Result<(), HookDeniedError> {
    let approval_cache = load_approval_cache_from(cache_path);

    for (name, server) in mcp_servers {
        let hash = mcp_server_hash(name, server);

        // Check cache first
        let is_cached = approval_cache.contains(&hash);

        if is_cached {
            continue;
        }

        // Determine tier from resolution_trace
        let trace_key = format!("mcp.{name}");
        let tier = match resolution_trace.get(&trace_key).map(String::as_str) {
            Some(src) if src.contains("~/.config/jig/config.yaml") => HookTrustTier::Full,
            Some(src) if src.contains(".jig.local.yaml") => HookTrustTier::Personal,
            _ => HookTrustTier::Team,
        };

        // Auto-approve non-external tiers with --yes
        if yes {
            match tier {
                HookTrustTier::Full | HookTrustTier::Personal | HookTrustTier::Team => continue,
                HookTrustTier::ExternalSkill { .. } => {}
            }
        }

        // Build human-readable description
        let desc = if let Some(cmd) = &server.command {
            format!("stdio: {cmd}")
        } else if let Some(url) = &server.url {
            format!("sse: {url}")
        } else {
            "unknown transport".to_owned()
        };
        let command_display = format!("MCP server '{name}' ({desc})");

        let source_file = PathBuf::from(
            resolution_trace
                .get(&trace_key)
                .map(String::as_str)
                .unwrap_or(".jig.yaml"),
        );

        let req = ApprovalRequest {
            tier: tier.clone(),
            command: command_display.clone(),
            source_file,
            previous_command: None,
        };

        match ui.prompt_approval(&req) {
            ApprovalDecision::Approved | ApprovalDecision::ApproveSession => {
                // Use a synthetic ConfigSource for the cache record label
                let src_label = resolution_trace
                    .get(&trace_key)
                    .map(String::as_str)
                    .unwrap_or(".jig.yaml");
                append_approval_cache_entry(cache_path, &hash, &command_display, src_label);
            }
            ApprovalDecision::Denied => {
                let hook_source = resolution_trace
                    .get(&trace_key)
                    .cloned()
                    .unwrap_or_else(|| ".jig.yaml".to_owned());
                return Err(HookDeniedError {
                    hook_source,
                    command: command_display,
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
    append_approval_cache_entry(path, hash, command, &source.to_string());
}

fn append_approval_cache_entry(
    path: &std::path::Path,
    hash: &str,
    command: &str,
    source_label: &str,
) {
    use std::io::Write;

    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }

    let record = serde_json::json!({
        "command_hash": hash,
        "command": command,
        "source": source_label,
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

    // ── MCP approval tests ────────────────────────────────────────────────────

    use crate::config::schema::McpServer;

    fn make_stdio_server(command: &str) -> McpServer {
        McpServer {
            server_type: Some("stdio".to_owned()),
            command: Some(command.to_owned()),
            args: None,
            env: None,
            url: None,
        }
    }

    fn make_sse_server(url: &str) -> McpServer {
        McpServer {
            server_type: Some("sse".to_owned()),
            command: None,
            args: None,
            env: None,
            url: Some(url.to_owned()),
        }
    }

    #[test]
    fn test_mcp_server_hash_is_stable() {
        let server = make_stdio_server("npx");
        let h1 = mcp_server_hash("my-server", &server);
        let h2 = mcp_server_hash("my-server", &server);
        assert_eq!(h1, h2, "same input must produce same hash");
        assert!(h1.starts_with("sha256:"), "hash must have sha256: prefix");
    }

    #[test]
    fn test_mcp_server_hash_differs_for_different_name() {
        let server = make_stdio_server("npx");
        let h1 = mcp_server_hash("server-a", &server);
        let h2 = mcp_server_hash("server-b", &server);
        assert_ne!(h1, h2, "different names must produce different hashes");
    }

    #[test]
    fn test_mcp_approval_cache_hit_skips_prompt() {
        let dir = tempfile::tempdir().unwrap();
        let cache_path = dir.path().join("approvals.jsonl");

        let server = make_stdio_server("npx");
        let hash = mcp_server_hash("my-server", &server);

        // Pre-populate the cache
        let line = serde_json::json!({
            "command_hash": hash,
            "command": "MCP server 'my-server' (stdio: npx)",
            "source": ".jig.yaml",
        });
        std::fs::write(&cache_path, format!("{line}\n")).unwrap();

        struct PanicUi;
        impl ApprovalUi for PanicUi {
            fn prompt_approval(&self, _req: &ApprovalRequest) -> ApprovalDecision {
                panic!("prompt_approval must not be called on a cache hit");
            }
        }

        let mut servers = std::collections::HashMap::new();
        servers.insert("my-server".to_owned(), server);
        let trace = std::collections::HashMap::new();

        let result = run_mcp_approvals_inner(&servers, &trace, &PanicUi, false, &cache_path);
        assert!(result.is_ok(), "cache hit must succeed without prompting");
    }

    #[test]
    fn test_mcp_approval_denied_returns_error() {
        let dir = tempfile::tempdir().unwrap();
        let cache_path = dir.path().join("approvals.jsonl");

        let server = make_sse_server("https://example.com/mcp");
        let ui = MockApprovalUi(std::sync::Mutex::new(vec![ApprovalDecision::Denied]));

        let mut servers = std::collections::HashMap::new();
        servers.insert("risky-server".to_owned(), server);
        let trace = std::collections::HashMap::new();

        let result = run_mcp_approvals_inner(&servers, &trace, &ui, false, &cache_path);
        assert!(result.is_err(), "denied decision must return error");
        let err = result.unwrap_err();
        assert!(
            err.command.contains("risky-server"),
            "error command must name the server: {}",
            err.command
        );
    }
}
