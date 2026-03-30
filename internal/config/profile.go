package config

// Profile represents a jig profile configuration.
type Profile struct {
	Name        string `yaml:"name"                    json:"name"`
	Description string `yaml:"description,omitempty"   json:"description,omitempty"`
	Extends     string `yaml:"extends,omitempty"       json:"extends,omitempty"`

	// Core settings → Claude CLI flags
	Model          string `yaml:"model,omitempty"           json:"model,omitempty"`
	Effort         string `yaml:"effort,omitempty"          json:"effort,omitempty"`
	PermissionMode string `yaml:"permission_mode,omitempty" json:"permission_mode,omitempty"`

	// System prompt
	SystemPrompt       string `yaml:"system_prompt,omitempty"        json:"system_prompt,omitempty"`
	AppendSystemPrompt string `yaml:"append_system_prompt,omitempty" json:"append_system_prompt,omitempty"`

	// Tool permissions
	AllowedTools    []string `yaml:"allowed_tools,omitempty"    json:"allowed_tools,omitempty"`
	DisallowedTools []string `yaml:"disallowed_tools,omitempty" json:"disallowed_tools,omitempty"`

	// Session agent
	SessionAgent string `yaml:"session_agent,omitempty" json:"session_agent,omitempty"`

	// MCP servers
	MCPServers []MCPServerEntry `yaml:"mcp_servers,omitempty" json:"mcp_servers,omitempty"`

	// Skills, agents, commands → symlinked into plugin dir
	Skills   []PathEntry `yaml:"skills,omitempty"   json:"skills,omitempty"`
	Agents   []PathEntry `yaml:"agents,omitempty"   json:"agents,omitempty"`
	Commands []PathEntry `yaml:"commands,omitempty" json:"commands,omitempty"`

	// Hooks
	Hooks map[string][]HookMatcher `yaml:"hooks,omitempty" json:"hooks,omitempty"`

	// Hook scripts to copy into plugin dir
	HookScripts []HookScript `yaml:"hook_scripts,omitempty" json:"hook_scripts,omitempty"`

	// Raw settings.json passthrough
	Settings map[string]any `yaml:"settings,omitempty" json:"settings,omitempty"`

	// Extra CLI flags
	ExtraFlags []string `yaml:"extra_flags,omitempty" json:"extra_flags,omitempty"`

	// Plugin isolation: full plugin enables
	EnabledPlugins map[string]bool `yaml:"enabled_plugins,omitempty" json:"enabled_plugins,omitempty"`

	// Plugin isolation: individual component selections from plugins
	PluginComponents map[string]PluginComponentSelection `yaml:"plugin_components,omitempty" json:"plugin_components,omitempty"`

	// Source tracking (not serialized)
	source   ProfileSource
	filePath string
}

// PluginComponentSelection specifies which individual components to load from a plugin.
type PluginComponentSelection struct {
	Agents     []string `yaml:"agents,omitempty"      json:"agents,omitempty"`
	Skills     []string `yaml:"skills,omitempty"      json:"skills,omitempty"`
	Commands   []string `yaml:"commands,omitempty"    json:"commands,omitempty"`
	MCPServers []string `yaml:"mcp_servers,omitempty" json:"mcp_servers,omitempty"`
}

// MCPServerEntry is either a reference to an existing MCP server or an inline definition.
type MCPServerEntry struct {
	Ref     string            `yaml:"ref,omitempty"`
	Name    string            `yaml:"name,omitempty"`
	Command string            `yaml:"command,omitempty"`
	Args    []string          `yaml:"args,omitempty"`
	Env     map[string]string `yaml:"env,omitempty"`
}

// PathEntry references a skill, agent, or command by path.
type PathEntry struct {
	Path string `yaml:"path"`
}

// HookMatcher matches tool names and applies hooks.
type HookMatcher struct {
	Matcher string     `yaml:"matcher"`
	Hooks   []HookDef  `yaml:"hooks"`
}

// HookDef defines a single hook action.
type HookDef struct {
	Type    string `yaml:"type"`
	Command string `yaml:"command"`
}

// HookScript references a script to copy into the plugin dir.
type HookScript struct {
	Path string `yaml:"path"`
	Dest string `yaml:"dest"`
}

// ProfileSource indicates where a profile was loaded from.
type ProfileSource int

const (
	SourceDefault ProfileSource = iota
	SourceGlobal
	SourceProject
	SourceShortcut
)

func (p *Profile) Source() ProfileSource { return p.source }
func (p *Profile) FilePath() string      { return p.filePath }
