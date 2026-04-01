package claude

import (
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"
	"strings"

	"github.com/jdforsythe/jig/internal/config"
	"github.com/jdforsythe/jig/internal/plugin"
)

// PluginManifest is the minimal plugin.json for Claude's plugin system.
type PluginManifest struct {
	Name        string `json:"name"`
	Description string `json:"description"`
	Version     string `json:"version"`
}

// GeneratePluginDir creates a temporary plugin directory from a resolved profile.
// It always writes a jig-settings.json for plugin isolation.
// Returns the plugin dir path and the settings file path.
func GeneratePluginDir(p *config.Profile, mcpIndex *MCPIndex) (pluginDir, settingsPath string, err error) {
	// Create temp dir
	tmpDir, err := os.MkdirTemp("", "jig-")
	if err != nil {
		return "", "", fmt.Errorf("creating temp dir: %w", err)
	}

	pluginDir = tmpDir

	pluginManifestDir := filepath.Join(tmpDir, ".claude-plugin")
	if err = os.MkdirAll(pluginManifestDir, 0755); err != nil {
		return "", "", fmt.Errorf("creating plugin dir: %w", err)
	}

	// Write plugin.json
	manifest := PluginManifest{
		Name:        fmt.Sprintf("jig-%s", p.Name),
		Description: p.Description,
		Version:     "1.0.0",
	}
	if err = writeJSON(filepath.Join(pluginManifestDir, "plugin.json"), manifest); err != nil {
		return "", "", fmt.Errorf("writing plugin.json: %w", err)
	}

	// Always write jig-settings.json for plugin isolation.
	// enabledPlugins controls which globally-installed plugins are active.
	// Explicitly set all installed plugins to false unless the profile enables them,
	// so an empty profile doesn't inadvertently inherit all global plugins.
	enabledPlugins := make(map[string]bool)
	for k, v := range p.EnabledPlugins {
		enabledPlugins[k] = v
	}
	if installedPlugins, resolveErr := plugin.Resolve(); resolveErr == nil {
		for _, pi := range installedPlugins {
			if _, ok := enabledPlugins[pi.Key]; !ok {
				enabledPlugins[pi.Key] = false
			}
		}
	}
	settingsContent := map[string]any{"enabledPlugins": enabledPlugins}
	settingsPath = filepath.Join(tmpDir, "jig-settings.json")
	if err = writeJSON(settingsPath, settingsContent); err != nil {
		return "", "", fmt.Errorf("writing jig-settings.json: %w", err)
	}

	// Collect MCP servers: start with profile's declared servers
	mcpServers := make(map[string]MCPServerDef)

	// Build MCP servers from profile declarations
	if len(p.MCPServers) > 0 {
		for _, entry := range p.MCPServers {
			if entry.Ref != "" {
				def, ok := mcpIndex.Resolve(entry.Ref)
				if !ok {
					return "", "", fmt.Errorf("MCP server ref %q not found in any .mcp.json", entry.Ref)
				}
				mcpServers[entry.Ref] = def
			} else {
				mcpServers[entry.Name] = MCPServerDef{
					Command: entry.Command,
					Args:    entry.Args,
					Env:     entry.Env,
				}
			}
		}
	}

	// Symlink skills from profile
	if len(p.Skills) > 0 {
		skillsDir := filepath.Join(tmpDir, "skills")
		if err = os.MkdirAll(skillsDir, 0755); err != nil {
			return "", "", err
		}
		for _, s := range p.Skills {
			src := expandPath(s.Path)
			name, nameErr := safeBaseName(src)
			if nameErr != nil {
				return "", "", fmt.Errorf("invalid skill path %q: %w", s.Path, nameErr)
			}
			if err = os.Symlink(src, filepath.Join(skillsDir, name)); err != nil {
				return "", "", fmt.Errorf("symlinking skill %s: %w", s.Path, err)
			}
		}
	}

	// Symlink agents from profile
	if len(p.Agents) > 0 {
		agentsDir := filepath.Join(tmpDir, "agents")
		if err = os.MkdirAll(agentsDir, 0755); err != nil {
			return "", "", err
		}
		for _, a := range p.Agents {
			src := expandPath(a.Path)
			name, nameErr := safeBaseName(src)
			if nameErr != nil {
				return "", "", fmt.Errorf("invalid agent path %q: %w", a.Path, nameErr)
			}
			if err = os.Symlink(src, filepath.Join(agentsDir, name)); err != nil {
				return "", "", fmt.Errorf("symlinking agent %s: %w", a.Path, err)
			}
		}
	}

	// Symlink commands from profile
	if len(p.Commands) > 0 {
		commandsDir := filepath.Join(tmpDir, "commands")
		if err = os.MkdirAll(commandsDir, 0755); err != nil {
			return "", "", err
		}
		for _, c := range p.Commands {
			src := expandPath(c.Path)
			name, nameErr := safeBaseName(src)
			if nameErr != nil {
				return "", "", fmt.Errorf("invalid command path %q: %w", c.Path, nameErr)
			}
			if err = os.Symlink(src, filepath.Join(commandsDir, name)); err != nil {
				return "", "", fmt.Errorf("symlinking command %s: %w", c.Path, err)
			}
		}
	}

	// Symlink individual plugin components from plugin_components
	if len(p.PluginComponents) > 0 {
		for pluginKey, sel := range p.PluginComponents {
			installPath, resolveErr := plugin.InstallPathForKey(pluginKey)
			if resolveErr != nil {
				return "", "", fmt.Errorf("resolving plugin %q: %w", pluginKey, resolveErr)
			}

			if len(sel.Agents) > 0 {
				agentsDir := filepath.Join(tmpDir, "agents")
				if err = os.MkdirAll(agentsDir, 0755); err != nil {
					return "", "", err
				}
				for _, name := range sel.Agents {
					safeName, nameErr := safeComponentName(name)
					if nameErr != nil {
						return "", "", fmt.Errorf("invalid plugin agent name %q for %q: %w", name, pluginKey, nameErr)
					}
					src := filepath.Join(installPath, "agents", safeName)
					dest := filepath.Join(agentsDir, safeName)
					if _, statErr := os.Lstat(dest); os.IsNotExist(statErr) {
						if err = os.Symlink(src, dest); err != nil {
							return "", "", fmt.Errorf("symlinking plugin agent %s/%s: %w", pluginKey, safeName, err)
						}
					}
				}
			}

			if len(sel.Skills) > 0 {
				skillsDir := filepath.Join(tmpDir, "skills")
				if err = os.MkdirAll(skillsDir, 0755); err != nil {
					return "", "", err
				}
				for _, name := range sel.Skills {
					safeName, nameErr := safeComponentName(name)
					if nameErr != nil {
						return "", "", fmt.Errorf("invalid plugin skill name %q for %q: %w", name, pluginKey, nameErr)
					}
					src := filepath.Join(installPath, "skills", safeName)
					dest := filepath.Join(skillsDir, safeName)
					if _, statErr := os.Lstat(dest); os.IsNotExist(statErr) {
						if err = os.Symlink(src, dest); err != nil {
							return "", "", fmt.Errorf("symlinking plugin skill %s/%s: %w", pluginKey, safeName, err)
						}
					}
				}
			}

			if len(sel.Commands) > 0 {
				commandsDir := filepath.Join(tmpDir, "commands")
				if err = os.MkdirAll(commandsDir, 0755); err != nil {
					return "", "", err
				}
				for _, name := range sel.Commands {
					safeName, nameErr := safeComponentName(name)
					if nameErr != nil {
						return "", "", fmt.Errorf("invalid plugin command name %q for %q: %w", name, pluginKey, nameErr)
					}
					src := filepath.Join(installPath, "commands", safeName)
					dest := filepath.Join(commandsDir, safeName)
					if _, statErr := os.Lstat(dest); os.IsNotExist(statErr) {
						if err = os.Symlink(src, dest); err != nil {
							return "", "", fmt.Errorf("symlinking plugin command %s/%s: %w", pluginKey, safeName, err)
						}
					}
				}
			}

			// Merge MCP servers from plugin
			if len(sel.MCPServers) > 0 {
				pluginMCPPath := filepath.Join(installPath, ".mcp.json")
				pluginMCP, readErr := readMCPFile(pluginMCPPath)
				if readErr != nil {
					return "", "", fmt.Errorf("reading plugin MCP config for %q: %w", pluginKey, readErr)
				}
				for _, serverName := range sel.MCPServers {
					def, ok := pluginMCP[serverName]
					if !ok {
						return "", "", fmt.Errorf("MCP server %q not found in plugin %q", serverName, pluginKey)
					}
					mcpServers[serverName] = def
				}
			}
		}
	}

	// Write .mcp.json if we have any servers
	if len(mcpServers) > 0 {
		mcpConfig := mcpConfigFile{MCPServers: mcpServers}
		if err = writeJSON(filepath.Join(tmpDir, ".mcp.json"), mcpConfig); err != nil {
			return "", "", fmt.Errorf("writing .mcp.json: %w", err)
		}
	}

	// Write hooks
	if len(p.Hooks) > 0 {
		hooksDir := filepath.Join(tmpDir, "hooks")
		if err = os.MkdirAll(hooksDir, 0755); err != nil {
			return "", "", err
		}
		if err = writeJSON(filepath.Join(hooksDir, "hooks.json"), p.Hooks); err != nil {
			return "", "", fmt.Errorf("writing hooks.json: %w", err)
		}
	}

	// Copy hook scripts
	for _, hs := range p.HookScripts {
		src := expandPath(hs.Path)
		dest, joinErr := safeJoinWithin(tmpDir, hs.Dest)
		if joinErr != nil {
			return "", "", fmt.Errorf("invalid hook script destination %q: %w", hs.Dest, joinErr)
		}
		if err = os.MkdirAll(filepath.Dir(dest), 0755); err != nil {
			return "", "", err
		}
		if err = copyFile(src, dest); err != nil {
			return "", "", fmt.Errorf("copying hook script %s: %w", hs.Path, err)
		}
	}

	// Write settings.json if there are passthrough settings
	if len(p.Settings) > 0 {
		if err = writeJSON(filepath.Join(pluginManifestDir, "settings.json"), p.Settings); err != nil {
			return "", "", fmt.Errorf("writing settings.json: %w", err)
		}
	}

	return pluginDir, settingsPath, nil
}

// readMCPFile reads a .mcp.json file and returns the server map.
func readMCPFile(path string) (map[string]MCPServerDef, error) {
	data, err := os.ReadFile(path)
	if err != nil {
		return nil, err
	}
	var f mcpConfigFile
	if err := json.Unmarshal(data, &f); err != nil {
		return nil, err
	}
	return f.MCPServers, nil
}

func writeJSON(path string, v any) error {
	data, err := json.MarshalIndent(v, "", "  ")
	if err != nil {
		return err
	}
	return os.WriteFile(path, data, 0644)
}

func copyFile(src, dst string) error {
	data, err := os.ReadFile(src)
	if err != nil {
		return err
	}
	return os.WriteFile(dst, data, 0755)
}

func expandPath(path string) string {
	if len(path) > 1 && path[0] == '~' && path[1] == '/' {
		home, err := os.UserHomeDir()
		if err == nil {
			return filepath.Join(home, path[2:])
		}
	}
	return path
}

func safeBaseName(path string) (string, error) {
	name := filepath.Base(path)
	return safeComponentName(name)
}

func safeComponentName(name string) (string, error) {
	if name == "" || name == "." || name == ".." {
		return "", fmt.Errorf("name must not be empty, '.' or '..'")
	}
	if filepath.IsAbs(name) {
		return "", fmt.Errorf("absolute paths are not allowed")
	}
	if filepath.Base(name) != name {
		return "", fmt.Errorf("path separators are not allowed")
	}
	if strings.Contains(name, "\\") || strings.Contains(name, "/") {
		return "", fmt.Errorf("path separators are not allowed")
	}
	return name, nil
}

func safeJoinWithin(baseDir, rel string) (string, error) {
	if rel == "" {
		return "", fmt.Errorf("destination must not be empty")
	}
	if filepath.IsAbs(rel) {
		return "", fmt.Errorf("absolute destination paths are not allowed")
	}

	cleanRel := filepath.Clean(rel)
	if cleanRel == "." || cleanRel == ".." || strings.HasPrefix(cleanRel, ".."+string(filepath.Separator)) {
		return "", fmt.Errorf("destination escapes plugin directory")
	}

	baseClean := filepath.Clean(baseDir)
	dest := filepath.Join(baseClean, cleanRel)
	relToBase, err := filepath.Rel(baseClean, dest)
	if err != nil {
		return "", err
	}
	if relToBase == ".." || strings.HasPrefix(relToBase, ".."+string(filepath.Separator)) {
		return "", fmt.Errorf("destination escapes plugin directory")
	}

	return dest, nil
}
