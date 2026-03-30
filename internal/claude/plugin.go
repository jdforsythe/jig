package claude

import (
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"

	"github.com/jforsythe/jig/internal/config"
)

// PluginManifest is the minimal plugin.json for Claude's plugin system.
type PluginManifest struct {
	Name        string `json:"name"`
	Description string `json:"description"`
	Version     string `json:"version"`
}

// GeneratePluginDir creates a temporary plugin directory from a resolved profile.
func GeneratePluginDir(p *config.Profile, mcpIndex *MCPIndex) (string, error) {
	// Create temp dir
	tmpDir, err := os.MkdirTemp("", "jig-")
	if err != nil {
		return "", fmt.Errorf("creating temp dir: %w", err)
	}

	pluginDir := filepath.Join(tmpDir, ".claude-plugin")
	if err := os.MkdirAll(pluginDir, 0755); err != nil {
		return "", fmt.Errorf("creating plugin dir: %w", err)
	}

	// Write plugin.json
	manifest := PluginManifest{
		Name:        fmt.Sprintf("jig-%s", p.Name),
		Description: p.Description,
		Version:     "1.0.0",
	}
	if err := writeJSON(filepath.Join(pluginDir, "plugin.json"), manifest); err != nil {
		return "", fmt.Errorf("writing plugin.json: %w", err)
	}

	// Write .mcp.json if there are MCP servers
	if len(p.MCPServers) > 0 {
		if err := writeMCPConfig(tmpDir, p, mcpIndex); err != nil {
			return "", fmt.Errorf("writing .mcp.json: %w", err)
		}
	}

	// Symlink skills
	if len(p.Skills) > 0 {
		skillsDir := filepath.Join(tmpDir, "skills")
		if err := os.MkdirAll(skillsDir, 0755); err != nil {
			return "", err
		}
		for _, s := range p.Skills {
			src := expandPath(s.Path)
			name := filepath.Base(src)
			if err := os.Symlink(src, filepath.Join(skillsDir, name)); err != nil {
				return "", fmt.Errorf("symlinking skill %s: %w", s.Path, err)
			}
		}
	}

	// Symlink agents
	if len(p.Agents) > 0 {
		agentsDir := filepath.Join(tmpDir, "agents")
		if err := os.MkdirAll(agentsDir, 0755); err != nil {
			return "", err
		}
		for _, a := range p.Agents {
			src := expandPath(a.Path)
			name := filepath.Base(src)
			if err := os.Symlink(src, filepath.Join(agentsDir, name)); err != nil {
				return "", fmt.Errorf("symlinking agent %s: %w", a.Path, err)
			}
		}
	}

	// Symlink commands
	if len(p.Commands) > 0 {
		commandsDir := filepath.Join(tmpDir, "commands")
		if err := os.MkdirAll(commandsDir, 0755); err != nil {
			return "", err
		}
		for _, c := range p.Commands {
			src := expandPath(c.Path)
			name := filepath.Base(src)
			if err := os.Symlink(src, filepath.Join(commandsDir, name)); err != nil {
				return "", fmt.Errorf("symlinking command %s: %w", c.Path, err)
			}
		}
	}

	// Write hooks
	if len(p.Hooks) > 0 {
		hooksDir := filepath.Join(tmpDir, "hooks")
		if err := os.MkdirAll(hooksDir, 0755); err != nil {
			return "", err
		}
		if err := writeJSON(filepath.Join(hooksDir, "hooks.json"), p.Hooks); err != nil {
			return "", fmt.Errorf("writing hooks.json: %w", err)
		}
	}

	// Copy hook scripts
	for _, hs := range p.HookScripts {
		src := expandPath(hs.Path)
		dest := filepath.Join(tmpDir, hs.Dest)
		if err := os.MkdirAll(filepath.Dir(dest), 0755); err != nil {
			return "", err
		}
		if err := copyFile(src, dest); err != nil {
			return "", fmt.Errorf("copying hook script %s: %w", hs.Path, err)
		}
	}

	// Write settings.json if there are passthrough settings
	if len(p.Settings) > 0 {
		if err := writeJSON(filepath.Join(pluginDir, "settings.json"), p.Settings); err != nil {
			return "", fmt.Errorf("writing settings.json: %w", err)
		}
	}

	return tmpDir, nil
}

func writeMCPConfig(dir string, p *config.Profile, mcpIndex *MCPIndex) error {
	servers := make(map[string]MCPServerDef)

	for _, entry := range p.MCPServers {
		if entry.Ref != "" {
			// Resolve from index
			def, ok := mcpIndex.Resolve(entry.Ref)
			if !ok {
				return fmt.Errorf("MCP server ref %q not found in any .mcp.json", entry.Ref)
			}
			servers[entry.Ref] = def
		} else {
			// Inline definition
			servers[entry.Name] = MCPServerDef{
				Command: entry.Command,
				Args:    entry.Args,
				Env:     entry.Env,
			}
		}
	}

	mcpConfig := mcpConfigFile{MCPServers: servers}
	return writeJSON(filepath.Join(dir, ".mcp.json"), mcpConfig)
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
