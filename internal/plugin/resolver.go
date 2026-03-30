package plugin

import (
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"
	"strings"
)

// installedPluginsPath returns the path to the installed_plugins.json file.
func installedPluginsPath() (string, error) {
	home, err := os.UserHomeDir()
	if err != nil {
		return "", fmt.Errorf("getting home dir: %w", err)
	}
	return filepath.Join(home, ".claude", "plugins", "installed_plugins.json"), nil
}

// Resolve reads all installed plugins from cache and returns their info.
// Returns nil, nil if the installed_plugins.json does not exist.
func Resolve() ([]*PluginInfo, error) {
	path, err := installedPluginsPath()
	if err != nil {
		return nil, err
	}
	return resolveFromPath(path)
}

// resolveFromPath reads plugins from a specific installed_plugins.json path.
// Exported for testing purposes via the test helper.
func resolveFromPath(path string) ([]*PluginInfo, error) {
	data, err := os.ReadFile(path)
	if err != nil {
		if os.IsNotExist(err) {
			return nil, nil
		}
		return nil, fmt.Errorf("reading installed_plugins.json: %w", err)
	}

	var f InstalledPluginsFile
	if err := json.Unmarshal(data, &f); err != nil {
		return nil, fmt.Errorf("parsing installed_plugins.json: %w", err)
	}

	var infos []*PluginInfo
	for key, installs := range f.Plugins {
		if len(installs) == 0 {
			continue
		}
		install := installs[0] // use first (most recent) install record
		install.InstallPath = expandHome(install.InstallPath)

		name, marketplace := splitKey(key)
		info := &PluginInfo{
			Key:         key,
			Name:        name,
			Marketplace: marketplace,
			Install:     install,
			Components:  scanComponents(install.InstallPath),
		}
		infos = append(infos, info)
	}

	return infos, nil
}

// InstallPathForKey looks up the installPath for a plugin key.
func InstallPathForKey(key string) (string, error) {
	path, err := installedPluginsPath()
	if err != nil {
		return "", err
	}

	data, err := os.ReadFile(path)
	if err != nil {
		if os.IsNotExist(err) {
			return "", fmt.Errorf("plugin %q not found: installed_plugins.json does not exist", key)
		}
		return "", fmt.Errorf("reading installed_plugins.json: %w", err)
	}

	var f InstalledPluginsFile
	if err := json.Unmarshal(data, &f); err != nil {
		return "", fmt.Errorf("parsing installed_plugins.json: %w", err)
	}

	installs, ok := f.Plugins[key]
	if !ok || len(installs) == 0 {
		return "", fmt.Errorf("plugin %q not found in installed_plugins.json", key)
	}

	return expandHome(installs[0].InstallPath), nil
}

// splitKey splits "name@marketplace" into (name, marketplace).
// If there is no "@", returns (key, "").
func splitKey(key string) (name, marketplace string) {
	idx := strings.LastIndex(key, "@")
	if idx < 0 {
		return key, ""
	}
	return key[:idx], key[idx+1:]
}

// scanComponents reads the plugin's install directory and lists available components.
func scanComponents(installPath string) PluginComponents {
	return PluginComponents{
		Agents:     listEntryNames(filepath.Join(installPath, "agents")),
		Skills:     listEntryNames(filepath.Join(installPath, "skills")),
		Commands:   listEntryNames(filepath.Join(installPath, "commands")),
		MCPServers: readMCPServerNames(filepath.Join(installPath, ".mcp.json")),
	}
}

// listEntryNames returns the names of non-hidden entries in a directory.
func listEntryNames(dir string) []string {
	entries, err := os.ReadDir(dir)
	if err != nil {
		return nil
	}
	var names []string
	for _, e := range entries {
		if strings.HasPrefix(e.Name(), ".") {
			continue
		}
		names = append(names, e.Name())
	}
	return names
}

// readMCPServerNames parses a .mcp.json file and returns the server keys.
// Supports both the wrapped format {"mcpServers": {...}} used by user configs
// and the flat format {"serverName": {...}} used by some official plugins.
func readMCPServerNames(mcpPath string) []string {
	data, err := os.ReadFile(mcpPath)
	if err != nil {
		return nil
	}

	// Try wrapped format first: {"mcpServers": {...}}
	var wrapped struct {
		MCPServers map[string]json.RawMessage `json:"mcpServers"`
	}
	if err := json.Unmarshal(data, &wrapped); err == nil && len(wrapped.MCPServers) > 0 {
		var names []string
		for name := range wrapped.MCPServers {
			names = append(names, name)
		}
		return names
	}

	// Fall back to flat format: {"serverName": {...}}
	// Only count entries whose value is a non-empty JSON object, as a basic
	// sanity check that they look like MCP server definitions.
	var flat map[string]json.RawMessage
	if err := json.Unmarshal(data, &flat); err == nil {
		var names []string
		for name, val := range flat {
			var obj map[string]json.RawMessage
			if err := json.Unmarshal(val, &obj); err == nil && len(obj) > 0 {
				names = append(names, name)
			}
		}
		if len(names) > 0 {
			return names
		}
	}

	return nil
}

// expandHome expands a leading "~/" in a path to the user's home directory.
func expandHome(path string) string {
	if len(path) > 1 && path[0] == '~' && path[1] == '/' {
		home, err := os.UserHomeDir()
		if err == nil {
			return filepath.Join(home, path[2:])
		}
	}
	return path
}
