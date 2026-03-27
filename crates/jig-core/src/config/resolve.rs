use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use tracing::{debug, trace, warn};

use super::schema::{
    ConfigSource, Context, ContextFragment, HookEntry, Hooks, JigConfig, McpServer, Persona,
    Profile,
};
use super::validate::{validate_layer, ConfigError};

/// The result of merging all four config layers.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ResolvedConfig {
    pub template_name: Option<String>,
    pub persona_name: Option<String>,

    /// Merged MCP servers (original names, before conflict resolution).
    pub mcp_servers: HashMap<String, McpServer>,

    /// Union of allowed tools from all layers.
    pub allowed_tools: Vec<String>,
    /// Union of disallowed tools from all layers.
    pub disallowed_tools: Vec<String>,

    /// Union of skills (source → names).
    pub skills: HashMap<String, Vec<String>>,
    /// Local skill paths.
    pub local_skills: Vec<PathBuf>,

    /// Per-key env vars (higher specificity wins).
    pub env: HashMap<String, String>,

    /// Model override (highest layer wins).
    pub model: Option<String>,

    /// Context fragments, ordered by priority.
    pub context_fragments: Vec<ContextFragment>,

    /// All pre_launch hooks (all layers, in order global → team → local → cli).
    pub pre_launch_hooks: Vec<(HookEntry, ConfigSource)>,
    /// All post_exit hooks.
    pub post_exit_hooks: Vec<(HookEntry, ConfigSource)>,

    /// Composed persona (resolved from extends if present).
    pub persona: ResolvedPersona,

    /// Token budget settings.
    pub token_warn_threshold: Option<u32>,
    pub token_hard_limit: Option<u32>,

    /// Passthrough claude CLI flags (allowlist-validated).
    pub claude_flags: Vec<String>,

    /// Per-field provenance for --dry-run --json resolution_trace.
    pub resolution_trace: HashMap<String, String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ResolvedPersona {
    pub name: Option<String>,
    pub rules: Vec<String>,
    pub file: Option<PathBuf>,
}

/// Load and parse a config file from a path.
fn load_config_file(path: &Path) -> Option<JigConfig> {
    if !path.exists() {
        trace!("Config file not found, skipping: {}", path.display());
        return None;
    }
    trace!("Loading config from: {}", path.display());
    match std::fs::read_to_string(path) {
        Ok(contents) => match serde_yaml::from_str(&contents) {
            Ok(config) => {
                debug!("Loaded config from: {}", path.display());
                Some(config)
            }
            Err(e) => {
                warn!("Failed to parse config {}: {}", path.display(), e);
                None
            }
        },
        Err(e) => {
            warn!("Failed to read config {}: {}", path.display(), e);
            None
        }
    }
}

/// Finds the global config path: `~/.config/jig/config.yaml`.
fn global_config_path() -> Option<PathBuf> {
    home::home_dir().map(|h| h.join(".config").join("jig").join("config.yaml"))
}

/// Resolves config from four layers, merging them in order of precedence.
/// Validates each layer before merging.
///
/// # Errors
/// Returns `ConfigError` if any layer fails validation.
pub fn resolve_config(
    project_dir: &Path,
    cli_overrides: &CliOverrides,
) -> Result<ResolvedConfig, ConfigError> {
    let global_path = global_config_path().unwrap_or_else(|| PathBuf::from("/dev/null"));
    resolve_config_with_global_path(project_dir, cli_overrides, &global_path)
}

/// Inner implementation that accepts an explicit global config path.
/// Used by tests to inject a temp global config without touching the real one.
fn resolve_config_with_global_path(
    project_dir: &Path,
    cli_overrides: &CliOverrides,
    global_path: &Path,
) -> Result<ResolvedConfig, ConfigError> {
    let project_path = project_dir.join(".jig.yaml");
    let local_path = project_dir.join(".jig.local.yaml");

    // Parallel reads
    let (global_result, project_result, local_result) = std::thread::scope(|s| {
        let global = s.spawn(|| load_config_file(global_path));
        let project = s.spawn(|| load_config_file(&project_path));
        let local = s.spawn(|| load_config_file(&local_path));
        (global.join().unwrap_or(None), project.join().unwrap_or(None), local.join().unwrap_or(None))
    });

    // Validate each layer before merging
    if let Some(ref cfg) = global_result {
        validate_layer(cfg, ConfigSource::GlobalUser)?;
    }
    if let Some(ref cfg) = project_result {
        validate_layer(cfg, ConfigSource::TeamProject)?;
    }
    if let Some(ref cfg) = local_result {
        validate_layer(cfg, ConfigSource::PersonalLocal)?;
    }

    // Build layer stack: global (lowest) → project → local → cli (highest)
    // Template config is applied inside apply_cli_overrides so it sits above all
    // file-based layers — UI selections override .jig.yaml, not the other way around.
    let layers: Vec<(Option<JigConfig>, ConfigSource)> = vec![
        (global_result, ConfigSource::GlobalUser),
        (project_result, ConfigSource::TeamProject),
        (local_result, ConfigSource::PersonalLocal),
    ];

    let mut resolved = ResolvedConfig::default();
    let mut persona_layers: Vec<(Persona, ConfigSource)> = Vec::new();

    for (maybe_config, source) in &layers {
        let Some(config) = maybe_config else { continue };
        merge_layer(&mut resolved, config, *source, &mut persona_layers);
    }

    // Apply CLI overrides (highest priority)
    apply_cli_overrides(&mut resolved, cli_overrides, &mut persona_layers);

    // Resolve persona from layers
    resolved.persona = resolve_persona(&persona_layers);

    // Sort context fragments by priority (lower number = higher priority)
    resolved.context_fragments.sort_by_key(|f| f.priority.unwrap_or(100));

    Ok(resolved)
}

/// Merges one config layer into the resolved config.
fn merge_layer(
    resolved: &mut ResolvedConfig,
    config: &JigConfig,
    source: ConfigSource,
    persona_layers: &mut Vec<(Persona, ConfigSource)>,
) {
    if let Some(persona) = &config.persona {
        persona_layers.push((persona.clone(), source));
    }

    if let Some(profile) = &config.profile {
        merge_profile(resolved, profile, source);
    }

    if let Some(context) = &config.context {
        merge_context(resolved, context);
    }

    if let Some(hooks) = &config.hooks {
        merge_hooks(resolved, hooks, source);
    }

    if let Some(budget) = &config.token_budget {
        // Higher specificity wins for scalar fields
        if let Some(warn) = budget.warn_threshold {
            resolved.token_warn_threshold = Some(warn);
            resolved.resolution_trace.insert(
                "token_budget.warn_threshold".to_owned(),
                source.to_string(),
            );
        }
        if let Some(hard) = budget.hard_limit {
            resolved.token_hard_limit = Some(hard);
            resolved.resolution_trace.insert(
                "token_budget.hard_limit".to_owned(),
                source.to_string(),
            );
        }
    }
}

fn merge_profile(resolved: &mut ResolvedConfig, profile: &Profile, source: ConfigSource) {
    // MCP: union (new entries added, layer-scoped replace handled separately)
    if let Some(mcp) = &profile.mcp {
        for (name, server) in mcp {
            resolved.mcp_servers.insert(name.clone(), server.clone());
            resolved.resolution_trace.insert(
                format!("mcp.{name}"),
                source.to_string(),
            );
        }
    }

    // Skills: union (additive)
    if let Some(skills) = &profile.skills {
        if let Some(from_source) = &skills.from_source {
            for (src_name, skill_list) in from_source {
                let entry = resolved.skills.entry(src_name.clone()).or_default();
                for skill in skill_list {
                    if !entry.contains(skill) {
                        entry.push(skill.clone());
                    }
                }
            }
        }
        if let Some(local) = &skills.local {
            for path in local {
                if !resolved.local_skills.contains(path) {
                    resolved.local_skills.push(path.clone());
                }
            }
        }
    }

    // Settings: union for tool lists, last-wins for scalar
    if let Some(settings) = &profile.settings {
        if let Some(allowed) = &settings.allowed_tools {
            for tool in allowed {
                if !resolved.allowed_tools.contains(tool) {
                    resolved.allowed_tools.push(tool.clone());
                }
            }
        }
        if let Some(disallowed) = &settings.disallowed_tools {
            for tool in disallowed {
                if !resolved.disallowed_tools.contains(tool) {
                    resolved.disallowed_tools.push(tool.clone());
                }
            }
        }
        if let Some(model) = &settings.model {
            resolved.model = Some(model.clone());
            resolved.resolution_trace.insert("settings.model".to_owned(), source.to_string());
        }
        if let Some(flags) = &settings.claude_flags {
            // Passthrough flags — validation deferred to assembly
            for flag in flags {
                if !resolved.claude_flags.contains(flag) {
                    resolved.claude_flags.push(flag.clone());
                }
            }
        }
    }

    // Env: higher specificity wins per key
    if let Some(env) = &profile.env {
        for (k, v) in env {
            resolved.env.insert(k.clone(), v.clone());
            resolved.resolution_trace.insert(format!("env.{k}"), source.to_string());
        }
    }
}

fn merge_context(resolved: &mut ResolvedConfig, context: &Context) {
    if let Some(fragments) = &context.fragments {
        for fragment in fragments {
            resolved.context_fragments.push(fragment.clone());
        }
    }
}

fn merge_hooks(resolved: &mut ResolvedConfig, hooks: &Hooks, source: ConfigSource) {
    if let Some(pre) = &hooks.pre_launch {
        for hook in pre {
            resolved.pre_launch_hooks.push((hook.clone(), source));
        }
    }
    if let Some(post) = &hooks.post_exit {
        for hook in post {
            resolved.post_exit_hooks.push((hook.clone(), source));
        }
    }
}

/// CLI overrides struct (subset of JigConfig fields that can come from CLI flags).
#[derive(Debug, Default, Clone)]
pub struct CliOverrides {
    pub template: Option<String>,
    pub persona: Option<String>,
    pub model: Option<String>,
}

fn apply_cli_overrides(
    resolved: &mut ResolvedConfig,
    overrides: &CliOverrides,
    persona_layers: &mut Vec<(Persona, ConfigSource)>,
) {
    // Step 1: apply template config first (lower CLI priority — TemplateSelected).
    // This lets individual explicit CLI flags overwrite template scalar values below.
    if let Some(template_name) = &overrides.template {
        resolved.template_name = Some(template_name.clone());
        resolved.resolution_trace.insert(
            "template".to_owned(),
            ConfigSource::TemplateSelected.to_string(),
        );
        // Merge the template's embedded config at TemplateSelected priority so it
        // overrides all file-based layers (.jig.yaml, .jig.local.yaml), but remains
        // below explicit individual CLI flags applied in step 2.
        if let Some(template) = crate::defaults::builtin_templates()
            .into_iter()
            .find(|t| &t.name == template_name)
        {
            merge_layer(resolved, &template.config, ConfigSource::TemplateSelected, persona_layers);
        }
    }

    // Step 2: apply individual explicit CLI flag scalars (ExplicitCliFlag — highest priority).
    // These overwrite anything set by the template merge above.
    if let Some(persona_name) = &overrides.persona {
        // Add a synthetic persona layer for CLI-specified persona
        persona_layers.push((
            Persona {
                ref_name: Some(persona_name.clone()),
                ..Default::default()
            },
            ConfigSource::ExplicitCliFlag,
        ));
        resolved.resolution_trace.insert(
            "persona".to_owned(),
            ConfigSource::ExplicitCliFlag.to_string(),
        );
    }
    if let Some(model) = &overrides.model {
        resolved.model = Some(model.clone());
        resolved.resolution_trace.insert(
            "settings.model".to_owned(),
            ConfigSource::ExplicitCliFlag.to_string(),
        );
    }
}

/// Resolves the final persona from the stack of persona layers.
/// The highest-priority persona wins entirely — UNLESS it has `extends`.
fn resolve_persona(layers: &[(Persona, ConfigSource)]) -> ResolvedPersona {
    // Find the winning (highest priority) persona
    let Some((winning_persona, _source)) = layers.last() else {
        return ResolvedPersona::default();
    };

    if let Some(extends_name) = &winning_persona.extends {
        // Find the named persona in earlier layers
        let base = layers
            .iter()
            .rev()
            .skip(1) // skip the winning layer itself
            .find(|(p, _)| p.name.as_deref() == Some(extends_name.as_str()))
            .map(|(p, _)| p);

        if let Some(base_persona) = base {
            // Merge: base rules first, then winning rules appended
            let mut rules = base_persona.rules.clone().unwrap_or_default();
            if let Some(extra) = &winning_persona.rules {
                rules.extend(extra.iter().cloned());
            }
            return ResolvedPersona {
                name: winning_persona.name.clone().or_else(|| base_persona.name.clone()),
                rules,
                file: winning_persona.file.clone().or_else(|| base_persona.file.clone()),
            };
        }
    }

    // No extends — use winning persona directly, but merge built-in rules if name matches
    let effective_name = winning_persona.name.clone().or_else(|| winning_persona.ref_name.clone());

    let builtin_rules: Vec<String> = effective_name
        .as_deref()
        .and_then(|n| {
            crate::defaults::builtin_personas()
                .into_iter()
                .find(|(key, _)| key == n)
                .and_then(|(_, p)| p.rules)
        })
        .unwrap_or_default();

    let user_rules = winning_persona.rules.clone().unwrap_or_default();

    // Built-in rules first, then user-provided rules appended on top
    let mut rules = builtin_rules;
    rules.extend(user_rules);

    ResolvedPersona {
        name: effective_name,
        rules,
        file: winning_persona.file.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_empty_config() {
        let dir = tempfile::tempdir().unwrap();
        let overrides = CliOverrides::default();
        let resolved = resolve_config(dir.path(), &overrides).unwrap();
        assert!(resolved.mcp_servers.is_empty());
        assert!(resolved.allowed_tools.is_empty());
    }

    #[test]
    fn test_template_config_applied_when_selected() {
        // Regression: template name was stored as metadata but its embedded JigConfig
        // (allowed/disallowed tools, etc.) was never merged into the resolved config.
        let dir = tempfile::tempdir().unwrap();
        let overrides = CliOverrides {
            template: Some("code-review".to_owned()),
            ..Default::default()
        };
        let resolved = resolve_config(dir.path(), &overrides).unwrap();
        assert_eq!(resolved.template_name.as_deref(), Some("code-review"));
        assert!(resolved.disallowed_tools.contains(&"Bash".to_owned()), "template disallowed_tools must be applied");
        assert!(resolved.allowed_tools.contains(&"Read".to_owned()), "template allowed_tools must be applied");
    }

    #[test]
    fn test_security_audit_template_config_applied() {
        let dir = tempfile::tempdir().unwrap();
        let overrides = CliOverrides {
            template: Some("security-audit".to_owned()),
            ..Default::default()
        };
        let resolved = resolve_config(dir.path(), &overrides).unwrap();
        assert!(resolved.disallowed_tools.contains(&"Bash".to_owned()));
        assert!(resolved.disallowed_tools.contains(&"Edit".to_owned()));
        assert!(resolved.allowed_tools.contains(&"Grep".to_owned()));
    }

    #[test]
    fn test_project_config_additively_extends_template() {
        // Tool lists are additive (union), so .jig.yaml can add tools on top of the
        // template — but the template's own restrictions (applied at CLI priority) are
        // still in the disallowed list and will win over the allowed list at runtime.
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join(".jig.yaml"), "schema: 1\nprofile:\n  settings:\n    allowedTools:\n      - Bash\n").unwrap();
        let overrides = CliOverrides {
            template: Some("code-review".to_owned()),
            ..Default::default()
        };
        let resolved = resolve_config(dir.path(), &overrides).unwrap();
        // .jig.yaml adds Bash to allowed (additive union)
        assert!(resolved.allowed_tools.contains(&"Bash".to_owned()));
        // Template's allowed tools are still present
        assert!(resolved.allowed_tools.contains(&"Read".to_owned()));
        // Template's disallowed tools are applied (template wins over project for these)
        assert!(resolved.disallowed_tools.contains(&"Bash".to_owned()));
    }

    #[test]
    fn test_resolve_with_project_config() {
        let dir = tempfile::tempdir().unwrap();
        let config = r"
schema: 1
profile:
  settings:
    allowedTools:
      - Bash
      - Edit
";
        std::fs::write(dir.path().join(".jig.yaml"), config).unwrap();
        let overrides = CliOverrides::default();
        let resolved = resolve_config(dir.path(), &overrides).unwrap();
        assert!(resolved.allowed_tools.contains(&"Bash".to_owned()));
        assert!(resolved.allowed_tools.contains(&"Edit".to_owned()));
    }

    #[test]
    fn test_persona_extends_validation_rejected_in_team_config() {
        use crate::config::validate::validate_layer;
        let config = JigConfig {
            persona: Some(Persona {
                extends: Some("base".to_owned()),
                ..Default::default()
            }),
            ..Default::default()
        };
        let result = validate_layer(&config, ConfigSource::TeamProject);
        assert!(result.is_err());
    }

    #[test]
    fn test_persona_extends_allowed_in_personal_local() {
        use crate::config::validate::validate_layer;
        let config = JigConfig {
            persona: Some(Persona {
                extends: Some("base".to_owned()),
                ..Default::default()
            }),
            ..Default::default()
        };
        let result = validate_layer(&config, ConfigSource::PersonalLocal);
        assert!(result.is_ok());
    }

    #[test]
    fn test_builtin_rules_merged_when_name_matches() {
        // Reproduces the playbook step 6 bug: name: pair-programmer with user rules
        // should get built-in pair-programmer rules prepended, not just user rules.
        let layers = vec![(
            Persona {
                name: Some("pair-programmer".to_owned()),
                rules: Some(vec![
                    "This is the myproject repo. Main language is TypeScript.".to_owned(),
                    "Always run tests after changes.".to_owned(),
                ]),
                ..Default::default()
            },
            ConfigSource::TeamProject,
        )];
        let resolved = resolve_persona(&layers);
        // Built-in rules must be present
        assert!(
            resolved.rules.iter().any(|r| r.contains("Think out loud")),
            "expected built-in pair-programmer rules, got: {:?}",
            resolved.rules
        );
        // User rules must also be present
        assert!(resolved.rules.iter().any(|r| r.contains("Always run tests")));
        // Built-in rules come first
        let builtin_idx = resolved.rules.iter().position(|r| r.contains("Think out loud")).unwrap();
        let user_idx = resolved.rules.iter().position(|r| r.contains("Always run tests")).unwrap();
        assert!(builtin_idx < user_idx, "built-in rules should precede user rules");
    }

    #[test]
    fn test_unknown_persona_name_uses_only_user_rules() {
        let layers = vec![(
            Persona {
                name: Some("my-custom-persona".to_owned()),
                rules: Some(vec!["Custom rule.".to_owned()]),
                ..Default::default()
            },
            ConfigSource::TeamProject,
        )];
        let resolved = resolve_persona(&layers);
        assert_eq!(resolved.rules, vec!["Custom rule."]);
        assert_eq!(resolved.name.as_deref(), Some("my-custom-persona"));
    }

    #[test]
    fn test_cli_ref_name_merges_builtin_rules() {
        // -p pair-programmer from CLI should also get built-in rules
        let layers = vec![(
            Persona {
                ref_name: Some("pair-programmer".to_owned()),
                ..Default::default()
            },
            ConfigSource::CliFlag,
        )];
        let resolved = resolve_persona(&layers);
        assert!(
            resolved.rules.iter().any(|r| r.contains("Think out loud")),
            "CLI -p flag should load built-in rules, got: {:?}",
            resolved.rules
        );
    }

    // ─── Task 0.1: Five-layer config precedence tests ───────────────────────────

    /// Helper: write a YAML config file to path.
    fn write_yaml(path: &std::path::Path, yaml: &str) {
        std::fs::write(path, yaml).unwrap();
    }

    #[test]
    fn test_five_layer_precedence_scalar() {
        let dir = tempfile::tempdir().unwrap();
        let global_dir = tempfile::tempdir().unwrap();
        let global_cfg = global_dir.path().join("config.yaml");

        // Layer 1 (lowest): global
        write_yaml(&global_cfg, "schema: 1\nprofile:\n  settings:\n    model: from-global\n");
        // Layer 2: project
        write_yaml(
            &dir.path().join(".jig.yaml"),
            "schema: 1\nprofile:\n  settings:\n    model: from-project\n",
        );
        // Layer 3: local
        write_yaml(
            &dir.path().join(".jig.local.yaml"),
            "schema: 1\nprofile:\n  settings:\n    model: from-local\n",
        );
        // Layer 4 (highest): CLI
        let cli = CliOverrides { model: Some("from-cli".to_owned()), ..Default::default() };

        let resolved = resolve_config_with_global_path(dir.path(), &cli, &global_cfg).unwrap();
        assert_eq!(resolved.model.as_deref(), Some("from-cli"), "CLI must win");

        // Drop CLI → local wins
        let no_cli = CliOverrides::default();
        let resolved = resolve_config_with_global_path(dir.path(), &no_cli, &global_cfg).unwrap();
        assert_eq!(resolved.model.as_deref(), Some("from-local"), "local must beat project and global");

        // Drop local → project wins
        std::fs::remove_file(dir.path().join(".jig.local.yaml")).unwrap();
        let resolved = resolve_config_with_global_path(dir.path(), &no_cli, &global_cfg).unwrap();
        assert_eq!(resolved.model.as_deref(), Some("from-project"), "project must beat global");

        // Drop project → global wins
        std::fs::remove_file(dir.path().join(".jig.yaml")).unwrap();
        let resolved = resolve_config_with_global_path(dir.path(), &no_cli, &global_cfg).unwrap();
        assert_eq!(resolved.model.as_deref(), Some("from-global"), "global must be the floor");
    }

    #[test]
    fn test_mcp_servers_union_across_layers() {
        let dir = tempfile::tempdir().unwrap();
        let global_dir = tempfile::tempdir().unwrap();
        let global_cfg = global_dir.path().join("config.yaml");

        write_yaml(
            &global_cfg,
            "schema: 1\nprofile:\n  mcp:\n    server_a:\n      type: stdio\n      command: cmd_a\n",
        );
        write_yaml(
            &dir.path().join(".jig.yaml"),
            "schema: 1\nprofile:\n  mcp:\n    server_b:\n      type: stdio\n      command: cmd_b\n",
        );

        let resolved = resolve_config_with_global_path(dir.path(), &CliOverrides::default(), &global_cfg).unwrap();
        assert!(resolved.mcp_servers.contains_key("server_a"), "global MCP server must be present");
        assert!(resolved.mcp_servers.contains_key("server_b"), "project MCP server must be present");
    }

    #[test]
    fn test_skills_union_across_layers() {
        let dir = tempfile::tempdir().unwrap();
        let global_dir = tempfile::tempdir().unwrap();
        let global_cfg = global_dir.path().join("config.yaml");

        write_yaml(
            &global_cfg,
            "schema: 1\nprofile:\n  skills:\n    local:\n      - ./skill_a\n",
        );
        write_yaml(
            &dir.path().join(".jig.yaml"),
            "schema: 1\nprofile:\n  skills:\n    local:\n      - ./skill_b\n",
        );

        let resolved = resolve_config_with_global_path(dir.path(), &CliOverrides::default(), &global_cfg).unwrap();
        let skill_strs: Vec<String> = resolved.local_skills.iter()
            .map(|p| p.to_string_lossy().into_owned())
            .collect();
        assert!(skill_strs.iter().any(|s| s.contains("skill_a")), "global skill must be in union");
        assert!(skill_strs.iter().any(|s| s.contains("skill_b")), "project skill must be in union");
    }

    #[test]
    fn test_env_per_key_last_wins() {
        let dir = tempfile::tempdir().unwrap();
        let global_dir = tempfile::tempdir().unwrap();
        let global_cfg = global_dir.path().join("config.yaml");

        write_yaml(
            &global_cfg,
            "schema: 1\nprofile:\n  env:\n    FOO: global\n    BAR: global\n",
        );
        write_yaml(
            &dir.path().join(".jig.yaml"),
            "schema: 1\nprofile:\n  env:\n    FOO: project\n",
        );

        let resolved = resolve_config_with_global_path(dir.path(), &CliOverrides::default(), &global_cfg).unwrap();
        assert_eq!(resolved.env.get("FOO").map(String::as_str), Some("project"), "project FOO must override global");
        assert_eq!(resolved.env.get("BAR").map(String::as_str), Some("global"), "BAR only in global must survive");
    }

    // ─── Config precedence fix: ExplicitCliFlag > TemplateSelected ──────────────

    /// Regression test: `jig -t code-review --model claude-opus` must use `claude-opus`.
    /// The "base" template has no model set, so any model the template might inject
    /// must not win over an explicit --model flag.
    #[test]
    fn test_explicit_cli_model_overrides_template_model() {
        let dir = tempfile::tempdir().unwrap();
        // Use "base" template (JigConfig::default — no model set) combined with an
        // explicit --model CLI flag to verify ExplicitCliFlag beats TemplateSelected.
        let overrides = CliOverrides {
            template: Some("base".to_owned()),
            model: Some("claude-opus".to_owned()),
            ..Default::default()
        };
        let resolved = resolve_config(dir.path(), &overrides).unwrap();
        assert_eq!(
            resolved.model.as_deref(),
            Some("claude-opus"),
            "explicit --model must win over template config"
        );
        // Resolution trace must reflect ExplicitCliFlag, not TemplateSelected
        let trace_source = resolved.resolution_trace.get("settings.model").map(String::as_str);
        assert_eq!(
            trace_source,
            Some(ConfigSource::ExplicitCliFlag.to_string().as_str()),
            "settings.model trace must be ExplicitCliFlag, got: {:?}",
            trace_source
        );
    }

    /// Verify that when a template is selected, template-sourced values in the
    /// resolution_trace are tagged as TemplateSelected, not ExplicitCliFlag.
    #[test]
    fn test_template_selected_source_tracking() {
        let dir = tempfile::tempdir().unwrap();
        // "code-review" has MCP-free config but adds disallowed/allowed tools —
        // those go through merge_layer with TemplateSelected source.
        // We check the "template" key in the trace which is set explicitly.
        let overrides = CliOverrides {
            template: Some("code-review".to_owned()),
            ..Default::default()
        };
        let resolved = resolve_config(dir.path(), &overrides).unwrap();
        let template_trace = resolved.resolution_trace.get("template").map(String::as_str);
        assert_eq!(
            template_trace,
            Some(ConfigSource::TemplateSelected.to_string().as_str()),
            "template trace must be TemplateSelected, got: {:?}",
            template_trace
        );
        // Confirm no "settings.model" trace entry — code-review has no model field
        assert!(
            !resolved.resolution_trace.contains_key("settings.model"),
            "code-review template sets no model; settings.model should not appear in trace"
        );
    }
}
