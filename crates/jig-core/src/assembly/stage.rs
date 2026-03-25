/// Assembly pipeline sequencer — max 200 lines.
/// All step logic lives in the module it belongs to.
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use thiserror::Error;

use super::executor::{ExecutorError, find_claude_binary, fork_and_exec};
use super::mcp::{McpError, cleanup_entries, write_atomic};
use super::permissions::rewrite_mcp_permissions;
use super::prompt::compose_system_prompt;
use super::skills::{SkillsError, create_temp_dir, stage_local_skills};
use crate::config::resolve::{CliOverrides, ResolvedConfig, resolve_config};
use crate::history::{HistoryEntry, record_exit, record_start};
use crate::security::approval::{ApprovalUi, run_hook_approvals};

#[derive(Debug, Error)]
pub enum AssemblyError {
    #[error("Config error: {0}")]
    Config(#[from] crate::config::validate::ConfigError),

    #[error("MCP error: {0}")]
    Mcp(#[from] McpError),

    #[error("Skills error: {0}")]
    Skills(#[from] SkillsError),

    #[error("Executor error: {0}")]
    Executor(#[from] ExecutorError),

    #[error("Hook denied: {hook_source} hook '{command}' was not approved")]
    HookDenied { hook_source: String, command: String },

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

/// Options for the assembly pipeline.
pub struct AssemblyOptions {
    pub project_dir: PathBuf,
    pub cli_overrides: CliOverrides,
    pub dry_run: bool,
    pub approval_ui: Box<dyn ApprovalUi>,
    pub yes: bool,              // only auto-approves cached items
    pub non_interactive: bool,
}

/// SessionGuard: holds all written state; Drop performs Category A cleanup.
struct SessionGuard {
    #[allow(dead_code)] // TempDir auto-cleans on Drop — it must be held alive
    temp_dir: Option<tempfile::TempDir>,
    mcp_written: bool,
    session_suffix: String,
    canonical_cwd: PathBuf,
    exit_outcome: Option<ExitOutcome>,
    session_id: String,
}

struct ExitOutcome {
    exit_code: i32,
}

impl Drop for SessionGuard {
    fn drop(&mut self) {
        // Category A: always run (even on error/panic)
        if self.mcp_written {
            if let Err(e) = cleanup_entries(&self.canonical_cwd, &self.session_suffix) {
                // Never panic inside Drop
                tracing::error!("MCP cleanup failed: {}", e);
            }
        }
        // temp_dir auto-cleans via TempDir::Drop when Some

        // Category B: clean exit only
        if let Some(outcome) = &self.exit_outcome {
            let _ = record_exit(&self.session_id, outcome.exit_code);
        }
    }
}

/// Runs the full 16-step assembly pipeline.
pub fn run_assembly(opts: AssemblyOptions) -> Result<i32, AssemblyError> {
    // Step 1: Detect environment
    let canonical_cwd = std::fs::canonicalize(&opts.project_dir)
        .unwrap_or_else(|_| opts.project_dir.clone());
    let pid = std::process::id();
    let session_id = uuid::Uuid::new_v4().to_string();

    // Step 2: Resolve config (all four layers)
    let resolved = resolve_config(&opts.project_dir, &opts.cli_overrides)?;

    // Step 6: Security approvals (hook trust tier evaluation)
    if !opts.dry_run {
        run_hook_approvals(
            &resolved.pre_launch_hooks,
            opts.approval_ui.as_ref(),
            opts.yes,
        )
        .map_err(|e| AssemblyError::HookDenied {
            hook_source: e.hook_source.clone(),
            command: e.command.clone(),
        })?;
    }
    // Step 7: Run pre_launch hooks (dry_run: show without running)
    if opts.dry_run {
        print_dry_run_hooks(&resolved.pre_launch_hooks);
    } else {
        // TODO: run hooks (Step 7 implementation)
    }

    if opts.dry_run {
        return run_dry_run(&resolved, &opts.project_dir, &canonical_cwd);
    }

    // Step 8: Stage temp dir
    let temp_dir = create_temp_dir()?;
    let temp_path = temp_dir.path().to_owned();

    // Step 9: Symlink skills into temp dir
    stage_local_skills(&temp_path, &resolved.local_skills, &opts.project_dir)?;

    // Step 10: Write MCP to ~/.claude.json (atomic, flock, conflict-detect, refcount)
    let mcp_result = if !resolved.mcp_servers.is_empty() {
        Some(write_atomic(&resolved.mcp_servers, &canonical_cwd, pid)?)
    } else {
        None
    };
    // Initialize SessionGuard after Step 10 — Drop runs Category A cleanup
    let mut guard = SessionGuard {
        temp_dir: Some(temp_dir),
        mcp_written: mcp_result.is_some(),
        session_suffix: mcp_result
            .as_ref()
            .map(|r| r.session_suffix.clone())
            .unwrap_or_default(),
        canonical_cwd: canonical_cwd.clone(),
        exit_outcome: None,
        session_id: session_id.clone(),
    };

    // Step 11: Build claude invocation flags
    let rename_map = mcp_result
        .as_ref()
        .map(|r| r.rename_map.clone())
        .unwrap_or_default();

    let claude_args = build_claude_args(&resolved, &temp_path, &rename_map, &opts.project_dir);

    // Step 12: Export env vars
    for (k, v) in &resolved.env {
        std::env::set_var(k, v);
    }

    // Step 13: Record session start in history.jsonl
    let entry = HistoryEntry::new_start(
        &session_id,
        resolved.template_name.as_deref(),
        resolved.persona.name.as_deref(),
        &canonical_cwd,
        &mcp_result
            .as_ref()
            .map(|r| {
                resolved
                    .mcp_servers
                    .keys()
                    .map(|n| {
                        r.rename_map.get(n).cloned().unwrap_or_else(|| n.clone())
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default(),
    );
    let _ = record_start(&entry);

    // Step 14: Find claude binary
    let claude_bin = find_claude_binary().ok_or(ExecutorError::ClaudeNotFound)?;

    // Steps 14–16: Fork, signal handling, waitpid
    let exit_code = fork_and_exec(&claude_bin, &claude_args)?;

    // After waitpid returns — set exit outcome for Category B cleanup
    guard.exit_outcome = Some(ExitOutcome { exit_code });

    // guard.drop() runs here:
    // - Category A: MCP cleanup (if refcount hits 0)
    // - Category B: post_exit hooks + history exit record
    Ok(exit_code)
}

/// Builds the claude CLI argument list.
fn build_claude_args(
    resolved: &ResolvedConfig,
    temp_dir: &Path,
    rename_map: &super::permissions::RenameMap,
    project_dir: &Path,
) -> Vec<String> {
    let mut args: Vec<String> = Vec::new();

    // System prompt file
    let prompt = compose_system_prompt(resolved, project_dir);
    if !prompt.is_empty() {
        let prompt_file = temp_dir.join("system-prompt.md");
        let _ = std::fs::write(&prompt_file, &prompt);
        args.push("--append-system-prompt-file".to_owned());
        args.push(prompt_file.to_string_lossy().into_owned());
    }

    // Skills directory (add-dir)
    let skills_dir = temp_dir.join("skills");
    if skills_dir.exists() {
        args.push("--add-dir".to_owned());
        args.push(skills_dir.to_string_lossy().into_owned());
    }

    // Allowed tools (with MCP permission rewrites)
    let allowed = rewrite_mcp_permissions(&resolved.allowed_tools, rename_map);
    if !allowed.is_empty() {
        args.push("--allowedTools".to_owned());
        args.push(allowed.join(","));
    }

    // Disallowed tools
    let disallowed = rewrite_mcp_permissions(&resolved.disallowed_tools, rename_map);
    if !disallowed.is_empty() {
        args.push("--disallowedTools".to_owned());
        args.push(disallowed.join(","));
    }

    // Model
    if let Some(model) = &resolved.model {
        args.push("--model".to_owned());
        args.push(model.clone());
    }

    // Passthrough flags (already allowlist-validated at config resolution time)
    args.extend(resolved.claude_flags.iter().cloned());

    args
}

/// Handles --dry-run: prints assembled command without forking.
fn run_dry_run(
    resolved: &ResolvedConfig,
    project_dir: &Path,
    canonical_cwd: &Path,
) -> Result<i32, AssemblyError> {
    let dummy_rename_map = HashMap::new();
    let dummy_temp = PathBuf::from("/tmp/jig-dry-run");

    let claude_args = build_claude_args(resolved, &dummy_temp, &dummy_rename_map, project_dir);

    println!("# Dry run — resolved claude invocation:");
    println!("claude {}", claude_args.join(" "));
    println!();
    println!("# Working directory: {}", canonical_cwd.display());

    if let Some(template) = &resolved.template_name {
        println!("# Template: {template}");
    }
    if let Some(persona) = &resolved.persona.name {
        println!("# Persona: {persona}");
    }
    if !resolved.mcp_servers.is_empty() {
        println!("# MCP servers: {}", resolved.mcp_servers.keys().cloned().collect::<Vec<_>>().join(", "));
    }

    Ok(0)
}

fn print_dry_run_hooks(hooks: &[(crate::config::schema::HookEntry, crate::config::schema::ConfigSource)]) {
    if hooks.is_empty() {
        return;
    }
    println!("# Hooks that would run:");
    for (hook, source) in hooks {
        println!("  [{source}] pre_launch: {}", hook.display_command());
    }
}
