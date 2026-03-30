package config

// Profile represents a jig profile configuration.
type Profile struct {
	Name        string `yaml:"name"`
	Description string `yaml:"description,omitempty"`
	Extends     string `yaml:"extends,omitempty"`

	// Core settings → Claude CLI flags
	Model          string `yaml:"model,omitempty"`
	Effort         string `yaml:"effort,omitempty"`
	PermissionMode string `yaml:"permission_mode,omitempty"`

	// System prompt
	SystemPrompt       string `yaml:"system_prompt,omitempty"`
	AppendSystemPrompt string `yaml:"append_system_prompt,omitempty"`

	// Tool permissions
	AllowedTools    []string `yaml:"allowed_tools,omitempty"`
	DisallowedTools []string `yaml:"disallowed_tools,omitempty"`

	// Session agent
	SessionAgent string `yaml:"session_agent,omitempty"`

	// MCP servers
	MCPServers []MCPServerEntry `yaml:"mcp_servers,omitempty"`

	// Skills, agents, commands → symlinked into plugin dir
	Skills   []PathEntry `yaml:"skills,omitempty"`
	Agents   []PathEntry `yaml:"agents,omitempty"`
	Commands []PathEntry `yaml:"commands,omitempty"`

	// Hooks
	Hooks map[string][]HookMatcher `yaml:"hooks,omitempty"`

	// Hook scripts to copy into plugin dir
	HookScripts []HookScript `yaml:"hook_scripts,omitempty"`

	// Raw settings.json passthrough
	Settings map[string]any `yaml:"settings,omitempty"`

	// Extra CLI flags
	ExtraFlags []string `yaml:"extra_flags,omitempty"`

	// Source tracking (not serialized)
	source   ProfileSource
	filePath string
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
