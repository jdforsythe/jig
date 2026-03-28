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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compute_preview_empty_project() {
        let dir = tempfile::tempdir().unwrap();
        let overrides = CliOverrides::default();
        let preview = compute_preview(dir.path(), &overrides).unwrap();
        assert_eq!(preview.token_count, 0);
        assert!(preview.system_prompt_lines.is_empty());
        assert!(preview.template_name.is_none());
    }

    #[test]
    fn test_compute_preview_with_template_override() {
        let dir = tempfile::tempdir().unwrap();
        let overrides = CliOverrides {
            template: Some("code-review".to_owned()),
            ..Default::default()
        };
        let preview = compute_preview(dir.path(), &overrides).unwrap();
        assert_eq!(preview.template_name.as_deref(), Some("code-review"));
        // code-review has allowed/disallowed tools → non-empty permissions
        assert!(!preview.permissions_summary.is_empty());
    }

    #[test]
    fn test_compute_preview_with_persona_rules() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join(".jig.yaml"),
            "schema: 1\npersona:\n  name: tester\n  rules:\n    - Be precise and thorough\n",
        ).unwrap();
        let overrides = CliOverrides::default();
        let preview = compute_preview(dir.path(), &overrides).unwrap();
        assert!(
            preview.system_prompt_lines.iter().any(|l| l.contains("Be precise")),
            "system_prompt_lines must contain persona rule text"
        );
        assert!(preview.token_count > 0, "persona rules must produce non-zero token count");
    }

    #[test]
    fn test_compute_preview_with_context_fragments() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("context.md"), "Important project context here.").unwrap();
        std::fs::write(
            dir.path().join(".jig.yaml"),
            "schema: 1\ncontext:\n  fragments:\n    - path: context.md\n      description: Project context\n",
        ).unwrap();
        let overrides = CliOverrides::default();
        let preview = compute_preview(dir.path(), &overrides).unwrap();
        assert!(
            preview.system_prompt_lines.iter().any(|l| l.contains("Important project context")),
            "system_prompt_lines must include context fragment content"
        );
        assert!(preview.token_count > 0);
    }

    #[test]
    fn test_build_preview_data_token_count_matches_heuristic() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join(".jig.yaml"),
            "schema: 1\npersona:\n  name: test\n  rules:\n    - A rule with exactly known length for testing\n",
        ).unwrap();
        let overrides = CliOverrides::default();
        let resolved = resolve_config(dir.path(), &overrides).unwrap();
        let preview = build_preview_data(&resolved, dir.path());
        // Token count should be system_prompt length / 4
        let prompt = super::super::prompt::compose_system_prompt(&resolved, dir.path());
        let expected = prompt.len() / 4;
        assert_eq!(preview.token_count, expected, "token count must match chars/4 heuristic");
    }
}
