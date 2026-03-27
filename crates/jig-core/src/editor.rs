use std::collections::HashMap;
use std::path::{Path, PathBuf};
use serde::{Deserialize, Serialize};

use crate::config::schema::{
    Context, ContextFragment, HookEntry, Hooks, JigConfig, McpServer, Persona, Profile, Settings,
};
use crate::defaults::builtin_templates;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SaveScope {
    Global,   // ~/.config/jig/templates/<name>.yaml
    Project,  // .jig.yaml (writes template stanza)
    Local,    // .jig.local.yaml
}

/// In-memory draft of a template being edited in the TUI.
/// Pure data, no ratatui deps.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct EditorDraft {
    pub name: String,
    pub description: String,
    pub allowed_tools: Vec<String>,
    pub disallowed_tools: Vec<String>,
    pub model: Option<String>,
    pub persona_name: Option<String>,
    pub persona_rules: Vec<String>,
    pub mcp_servers: HashMap<String, McpServer>,
    pub context_fragments: Vec<ContextFragment>,
    pub pre_launch_hooks: Vec<HookEntry>,
    pub post_exit_hooks: Vec<HookEntry>,
    pub claude_flags: Vec<String>,
}

impl EditorDraft {
    /// Converts the draft to a JigConfig for serialization / preview.
    pub fn to_jig_config(&self) -> JigConfig {
        let settings = if self.allowed_tools.is_empty()
            && self.disallowed_tools.is_empty()
            && self.model.is_none()
            && self.claude_flags.is_empty()
        {
            None
        } else {
            Some(Settings {
                allowed_tools: if self.allowed_tools.is_empty() {
                    None
                } else {
                    Some(self.allowed_tools.clone())
                },
                disallowed_tools: if self.disallowed_tools.is_empty() {
                    None
                } else {
                    Some(self.disallowed_tools.clone())
                },
                model: self.model.clone(),
                claude_flags: if self.claude_flags.is_empty() {
                    None
                } else {
                    Some(self.claude_flags.clone())
                },
            })
        };

        let profile = if self.mcp_servers.is_empty() && settings.is_none() {
            None
        } else {
            Some(Profile {
                skills: None,
                mcp: if self.mcp_servers.is_empty() {
                    None
                } else {
                    Some(self.mcp_servers.clone())
                },
                settings,
                env: None,
                plugins: None,
                sources: None,
            })
        };

        let persona = if self.persona_name.is_some() || !self.persona_rules.is_empty() {
            Some(Persona {
                name: self.persona_name.clone(),
                rules: if self.persona_rules.is_empty() {
                    None
                } else {
                    Some(self.persona_rules.clone())
                },
                file: None,
                ref_name: None,
                extends: None,
            })
        } else {
            None
        };

        let hooks = if self.pre_launch_hooks.is_empty() && self.post_exit_hooks.is_empty() {
            None
        } else {
            Some(Hooks {
                pre_launch: if self.pre_launch_hooks.is_empty() {
                    None
                } else {
                    Some(self.pre_launch_hooks.clone())
                },
                post_exit: if self.post_exit_hooks.is_empty() {
                    None
                } else {
                    Some(self.post_exit_hooks.clone())
                },
            })
        };

        let context = if self.context_fragments.is_empty() {
            None
        } else {
            Some(Context {
                fragments: Some(self.context_fragments.clone()),
            })
        };

        JigConfig {
            schema: Some(1),
            profile,
            persona,
            context,
            hooks,
            extends: None,
            token_budget: None,
        }
    }

    /// Populates a draft from an existing JigConfig.
    pub fn from_jig_config(name: &str, config: &JigConfig) -> Self {
        let mut draft = Self { name: name.to_owned(), ..Self::default() };

        if let Some(profile) = &config.profile {
            if let Some(settings) = &profile.settings {
                if let Some(tools) = &settings.allowed_tools {
                    draft.allowed_tools = tools.clone();
                }
                if let Some(tools) = &settings.disallowed_tools {
                    draft.disallowed_tools = tools.clone();
                }
                draft.model = settings.model.clone();
                if let Some(flags) = &settings.claude_flags {
                    draft.claude_flags = flags.clone();
                }
            }
            if let Some(mcp) = &profile.mcp {
                draft.mcp_servers = mcp.clone();
            }
        }

        if let Some(persona) = &config.persona {
            draft.persona_name = persona.name.clone();
            if let Some(rules) = &persona.rules {
                draft.persona_rules = rules.clone();
            }
        }

        if let Some(context) = &config.context {
            if let Some(frags) = &context.fragments {
                draft.context_fragments = frags.clone();
            }
        }

        if let Some(hooks) = &config.hooks {
            if let Some(h) = &hooks.pre_launch {
                draft.pre_launch_hooks = h.clone();
            }
            if let Some(h) = &hooks.post_exit {
                draft.post_exit_hooks = h.clone();
            }
        }

        draft
    }

    /// Saves the draft as a named template YAML file.
    /// Returns the path written.
    pub fn save_as_template(&self, scope: SaveScope, project_dir: &Path) -> std::io::Result<PathBuf> {
        let path = match scope {
            SaveScope::Global => home::home_dir()
                .unwrap_or_else(|| PathBuf::from("/tmp"))
                .join(".config")
                .join("jig")
                .join("templates")
                .join(format!("{}.yaml", sanitize_name(&self.name))),
            SaveScope::Project => project_dir.join(".jig.yaml"),
            SaveScope::Local => project_dir.join(".jig.local.yaml"),
        };

        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let config = self.to_jig_config();
        let yaml = serde_yaml::to_string(&config)
            .map_err(|e| std::io::Error::other(e.to_string()))?;

        std::fs::write(&path, yaml)?;
        Ok(path)
    }
}

fn sanitize_name(name: &str) -> String {
    name.chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '-'
            }
        })
        .collect()
}

/// Builds preview data from an EditorDraft without requiring a full assembly pipeline.
/// Returns a minimal PreviewData derived from the draft content.
pub fn resolve_draft_preview(draft: &EditorDraft) -> crate::assembly::preview::PreviewData {
    use crate::assembly::preview::{PreviewData, TokenCountMethod};

    let config = draft.to_jig_config();

    let token_count: usize = {
        let mut total = 0usize;
        if let Some(persona) = &config.persona {
            if let Some(rules) = &persona.rules {
                for rule in rules {
                    total += rule.len() / 4;
                }
            }
        }
        if let Some(ctx) = &config.context {
            if let Some(frags) = &ctx.fragments {
                total += frags.len() * 500; // estimate per fragment
            }
        }
        total
    };

    let permissions_summary = {
        let mut parts = Vec::new();
        if let Some(profile) = &config.profile {
            if let Some(settings) = &profile.settings {
                if let Some(tools) = &settings.allowed_tools {
                    parts.push(format!("allow: {}", tools.join(", ")));
                }
                if let Some(tools) = &settings.disallowed_tools {
                    parts.push(format!("deny: {}", tools.join(", ")));
                }
            }
        }
        parts.join(" | ")
    };

    let system_prompt_lines: Vec<String> = {
        let mut lines = Vec::new();
        if let Some(persona) = &config.persona {
            if let Some(name) = &persona.name {
                lines.push(format!("Persona: {name}"));
            }
            if let Some(rules) = &persona.rules {
                for rule in rules {
                    lines.push(rule.clone());
                }
            }
        }
        lines
    };

    PreviewData {
        token_count,
        token_count_method: TokenCountMethod::Heuristic,
        skills: Vec::new(),
        permissions_summary,
        system_prompt_lines,
        worktree_warning: false,
        active_concurrent_sessions: 0,
        template_name: Some(draft.name.clone()),
        persona_name: draft.persona_name.clone(),
    }
}

/// Loads a draft for editing an existing built-in or user template.
pub fn load_draft_for_template(name: &str) -> EditorDraft {
    // Check built-in templates first — extract via JigConfig
    if let Some(template) = builtin_templates().into_iter().find(|t| t.name == name) {
        let mut draft = EditorDraft::from_jig_config(name, &template.config);
        if let Some(desc) = template.description {
            draft.description = desc;
        }
        return draft;
    }

    // Return empty draft with the name set
    EditorDraft { name: name.to_owned(), ..EditorDraft::default() }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_editor_draft_to_jig_config_round_trips() {
        let mut draft = EditorDraft::default();
        draft.name = "test-template".to_owned();
        draft.allowed_tools = vec!["Read".to_owned(), "Bash".to_owned()];
        draft.model = Some("claude-opus".to_owned());
        draft.persona_rules = vec!["Be precise.".to_owned()];

        let config = draft.to_jig_config();

        // Round trip back via from_jig_config
        let round_tripped = EditorDraft::from_jig_config("test-template", &config);
        assert_eq!(round_tripped.allowed_tools, draft.allowed_tools);
        assert_eq!(round_tripped.model, draft.model);
        assert_eq!(round_tripped.persona_rules, draft.persona_rules);
    }

    #[test]
    fn test_save_as_template_writes_valid_yaml() {
        let dir = tempdir().unwrap();
        let mut draft = EditorDraft::default();
        draft.name = "my-template".to_owned();
        draft.allowed_tools = vec!["Read".to_owned()];

        let path = draft.save_as_template(SaveScope::Project, dir.path()).unwrap();
        assert!(path.exists());

        let content = std::fs::read_to_string(&path).unwrap();
        // Must be parseable as YAML
        let _: serde_yaml::Value = serde_yaml::from_str(&content).expect("must be valid YAML");
    }

    #[test]
    fn test_save_as_template_creates_parent_dirs() {
        let dir = tempdir().unwrap();
        let mut draft = EditorDraft::default();
        draft.name = "test".to_owned();

        let path = draft.save_as_template(SaveScope::Project, dir.path()).unwrap();
        assert!(path.parent().unwrap().exists());
    }

    #[test]
    fn test_sanitize_name_removes_special_chars() {
        assert_eq!(sanitize_name("my template!"), "my-template-");
        assert_eq!(sanitize_name("valid-name_123"), "valid-name_123");
    }

    #[test]
    fn test_empty_draft_produces_minimal_config() {
        let draft = EditorDraft::default();
        let config = draft.to_jig_config();
        // Empty draft should produce a config with only schema set
        assert_eq!(config.schema, Some(1));
        assert!(config.profile.is_none());
        assert!(config.persona.is_none());
        assert!(config.context.is_none());
        assert!(config.hooks.is_none());
    }

    #[test]
    fn test_resolve_draft_preview_returns_permissions_summary() {
        let mut draft = EditorDraft::default();
        draft.allowed_tools = vec!["Read".to_owned(), "Bash".to_owned()];
        let preview = resolve_draft_preview(&draft);
        assert!(preview.permissions_summary.contains("Read"));
        assert!(preview.permissions_summary.contains("Bash"));
    }

    #[test]
    fn test_load_draft_for_builtin_template() {
        // "base-devops" has allowed_tools defined
        let draft = load_draft_for_template("base-devops");
        assert_eq!(draft.name, "base-devops");
        assert!(!draft.allowed_tools.is_empty(), "base-devops should have allowed_tools");
    }

    #[test]
    fn test_load_draft_for_unknown_template_returns_default() {
        let draft = load_draft_for_template("nonexistent-template-xyz");
        assert_eq!(draft.name, "nonexistent-template-xyz");
        assert!(draft.allowed_tools.is_empty());
    }
}
