use std::path::Path;

use serde::{Deserialize, Serialize};

use super::prompt::{compose_system_prompt, estimate_tokens};
use crate::config::resolve::{CliOverrides, ResolvedConfig, resolve_config};

/// Token count method.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TokenCountMethod {
    Heuristic,
    Tiktoken,
}

/// The boundary struct between jig-core and jig-tui.
/// Uses only stdlib types — no ratatui imports.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreviewData {
    pub token_count: usize,
    pub token_count_method: TokenCountMethod,
    pub skills: Vec<String>,
    pub permissions_summary: String,
    /// Plain text lines of the composed system prompt.
    pub system_prompt_lines: Vec<String>,
    pub worktree_warning: bool,
    pub active_concurrent_sessions: usize,
    pub template_name: Option<String>,
    pub persona_name: Option<String>,
}

/// Computes preview data (steps 1–3 of assembly pipeline only).
/// MUST NOT write any state — no temp dir, no MCP write.
// preview only: steps 1-3. No state mutations.
// NOTE: This module must not import mcp, stage, or executor.
pub fn compute_preview(
    project_dir: &Path,
    overrides: &CliOverrides,
) -> Result<PreviewData, crate::config::validate::ConfigError> {
    let resolved = resolve_config(project_dir, overrides)?;
    Ok(build_preview_data(&resolved, project_dir))
}

/// Builds `PreviewData` from an already-resolved config.
pub fn build_preview_data(resolved: &ResolvedConfig, project_dir: &Path) -> PreviewData {
    let system_prompt = compose_system_prompt(resolved, project_dir);
    let (token_count, _method) = estimate_tokens(&system_prompt);

    let skills: Vec<String> = resolved
        .skills
        .values()
        .flatten()
        .cloned()
        .collect();

    let allowed_count = resolved.allowed_tools.len();
    let permissions_summary = if allowed_count == 0 {
        "All tools allowed".to_owned()
    } else {
        format!("{allowed_count} tools allowed")
    };

    let system_prompt_lines: Vec<String> = system_prompt
        .lines()
        .map(str::to_owned)
        .collect();

    PreviewData {
        token_count,
        token_count_method: TokenCountMethod::Heuristic,
        skills,
        permissions_summary,
        system_prompt_lines,
        worktree_warning: false, // TODO: detect worktree
        active_concurrent_sessions: 0, // TODO: check refcounts
        template_name: resolved.template_name.clone(),
        persona_name: resolved.persona.name.clone(),
    }
}
