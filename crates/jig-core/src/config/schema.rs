use std::collections::HashMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// Which config layer a value was loaded from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConfigSource {
    GlobalUser,
    TeamProject,
    PersonalLocal,
    CliFlag,
}

impl std::fmt::Display for ConfigSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::GlobalUser => write!(f, "~/.config/jig/config.yaml"),
            Self::TeamProject => write!(f, ".jig.yaml"),
            Self::PersonalLocal => write!(f, ".jig.local.yaml"),
            Self::CliFlag => write!(f, "CLI flag"),
        }
    }
}

/// Determines the approval UX for hooks and MCP servers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "tier")]
pub enum HookTrustTier {
    Full,
    Team,
    /// External skill — carries source URL for display in approval prompt.
    ExternalSkill { url: String },
    Personal,
}

/// The top-level jig config schema (all scopes share this format).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct JigConfig {
    pub schema: Option<u32>,
    pub profile: Option<Profile>,
    pub persona: Option<Persona>,
    pub context: Option<Context>,
    pub hooks: Option<Hooks>,
    pub extends: Option<Vec<String>>,
    pub token_budget: Option<TokenBudget>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Profile {
    pub skills: Option<SkillsConfig>,
    pub mcp: Option<HashMap<String, McpServer>>,
    pub settings: Option<Settings>,
    pub env: Option<HashMap<String, String>>,
    pub plugins: Option<Vec<PluginRef>>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct SkillsConfig {
    pub from_source: Option<HashMap<String, Vec<String>>>,
    pub local: Option<Vec<PathBuf>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpServer {
    #[serde(rename = "type")]
    pub server_type: Option<String>,
    pub command: Option<String>,
    pub args: Option<Vec<String>>,
    pub env: Option<HashMap<String, String>>,
    pub url: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    #[serde(rename = "allowedTools")]
    pub allowed_tools: Option<Vec<String>>,
    #[serde(rename = "disallowedTools")]
    pub disallowed_tools: Option<Vec<String>>,
    pub model: Option<String>,
    /// Explicit passthrough flags to claude CLI (allowlist-validated at assembly time).
    pub claude_flags: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginRef {
    pub name: String,
    pub marketplace: Option<String>,
    pub path: Option<PathBuf>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Persona {
    pub name: Option<String>,
    pub rules: Option<Vec<String>>,
    pub file: Option<PathBuf>,
    #[serde(rename = "ref")]
    pub ref_name: Option<String>,
    /// Only valid in `.jig.local.yaml`. Hard error in `.jig.yaml` or global config.
    pub extends: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Context {
    pub fragments: Option<Vec<ContextFragment>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextFragment {
    pub path: PathBuf,
    pub priority: Option<i32>,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Hooks {
    pub pre_launch: Option<Vec<HookEntry>>,
    pub post_exit: Option<Vec<HookEntry>>,
}

/// A hook entry: either exec array (direct, no injection) or command string with shell: true.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum HookEntry {
    Exec { exec: Vec<String> },
    Shell { command: String, shell: bool },
}

impl HookEntry {
    /// Returns the display string for this hook command.
    pub fn display_command(&self) -> &str {
        match self {
            Self::Exec { exec } => exec.first().map(String::as_str).unwrap_or(""),
            Self::Shell { command, .. } => command,
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct TokenBudget {
    pub warn_threshold: Option<u32>,
    pub hard_limit: Option<u32>,
}

/// A named template (used in template list).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Template {
    pub name: String,
    pub description: Option<String>,
    pub config: JigConfig,
}

/// A reference to a template by name (for CLI / TUI selection).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplateRef {
    pub name: String,
    pub description: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_minimal_config() {
        let yaml = "schema: 1\n";
        let config: JigConfig = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(config.schema, Some(1));
    }

    #[test]
    fn test_parse_hook_entry_exec_form() {
        let yaml = "exec: [\"notify-send\", \"done\"]\n";
        let hook: HookEntry = serde_yaml::from_str(yaml).unwrap();
        assert!(matches!(hook, HookEntry::Exec { .. }));
        assert_eq!(hook.display_command(), "notify-send");
    }

    #[test]
    fn test_parse_hook_entry_shell_form() {
        let yaml = "command: echo hello\nshell: true\n";
        let hook: HookEntry = serde_yaml::from_str(yaml).unwrap();
        assert!(matches!(hook, HookEntry::Shell { .. }));
        assert_eq!(hook.display_command(), "echo hello");
    }

    #[test]
    fn test_parse_mcp_server() {
        let yaml = "type: stdio\ncommand: npx\nargs: [\"-y\", \"some-mcp\"]\n";
        let server: McpServer = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(server.server_type.as_deref(), Some("stdio"));
        assert_eq!(server.command.as_deref(), Some("npx"));
        let args = server.args.unwrap();
        assert_eq!(args[0], "-y");
        assert_eq!(args[1], "some-mcp");
    }

    #[test]
    fn test_parse_full_config() {
        let yaml = "schema: 1\npersona:\n  name: strict-security\n  rules:\n    - Never run as root.\n";
        let config: JigConfig = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(config.schema, Some(1));
        let persona = config.persona.unwrap();
        assert_eq!(persona.name.as_deref(), Some("strict-security"));
        assert_eq!(persona.rules.unwrap().len(), 1);
    }

    #[test]
    fn test_config_source_display() {
        assert_eq!(ConfigSource::GlobalUser.to_string(), "~/.config/jig/config.yaml");
        assert_eq!(ConfigSource::TeamProject.to_string(), ".jig.yaml");
        assert_eq!(ConfigSource::PersonalLocal.to_string(), ".jig.local.yaml");
        assert_eq!(ConfigSource::CliFlag.to_string(), "CLI flag");
    }
}
