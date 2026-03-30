package plugin

// InstalledPluginsFile represents the structure of ~/.claude/plugins/installed_plugins.json.
type InstalledPluginsFile struct {
	Version int                        `json:"version"`
	Plugins map[string][]PluginInstall `json:"plugins"`
}

// PluginInstall describes a single installation record for a plugin.
type PluginInstall struct {
	Scope       string `json:"scope"`
	ProjectPath string `json:"projectPath,omitempty"`
	InstallPath string `json:"installPath"`
	Version     string `json:"version"`
}

// PluginInfo holds resolved information about an installed plugin from cache.
type PluginInfo struct {
	Key         string         // "ss-engineering@sensource-claude-marketplace"
	Name        string         // "ss-engineering"
	Marketplace string         // "sensource-claude-marketplace"
	Install     PluginInstall  // first install record
	Components  PluginComponents
}

// PluginComponents lists the available components within a plugin.
type PluginComponents struct {
	Agents     []string // names under agents/
	Skills     []string // names under skills/
	Commands   []string // names under commands/
	MCPServers []string // server keys from plugin's .mcp.json
}
