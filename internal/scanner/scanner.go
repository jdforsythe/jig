package scanner

import (
	"os"
	"path/filepath"
	"strings"

	"github.com/jforsythe/jig/internal/plugin"
)

// Discovery holds all discovered resources.
type Discovery struct {
	MCPServers []ServerInfo
	Skills     []ResourceInfo
	Agents     []ResourceInfo
	Commands   []ResourceInfo
	Plugins    []*plugin.PluginInfo
}

// ServerInfo describes a discovered MCP server.
type ServerInfo struct {
	Name   string
	Source string // file it was found in
}

// ResourceInfo describes a discovered skill, agent, or command.
type ResourceInfo struct {
	Name   string
	Path   string
	Type   string // "skill", "agent", "command"
	Source string // "user", "project", or plugin key (e.g. "ss-engineering@marketplace")
}

// Scan discovers all available resources from user, project, and plugin configs.
func Scan(cwd string) (*Discovery, error) {
	d := &Discovery{}

	home, _ := os.UserHomeDir()

	// Scan for MCP servers in .mcp.json files
	d.MCPServers = scanMCPServers(cwd, home)

	// Scan for skills
	d.Skills = scanResources("skill", []string{
		filepath.Join(home, ".claude", "skills"),
		filepath.Join(cwd, ".claude", "skills"),
	}, []string{"user", "project"})

	// Scan for agents
	d.Agents = scanResources("agent", []string{
		filepath.Join(home, ".claude", "agents"),
		filepath.Join(cwd, ".claude", "agents"),
	}, []string{"user", "project"})

	// Scan for commands
	d.Commands = scanResources("command", []string{
		filepath.Join(home, ".claude", "commands"),
		filepath.Join(cwd, ".claude", "commands"),
	}, []string{"user", "project"})

	// Scan for installed plugins (non-fatal)
	plugins, _ := plugin.Resolve()
	d.Plugins = plugins

	return d, nil
}

func scanMCPServers(cwd, home string) []ServerInfo {
	var servers []ServerInfo
	paths := []string{
		filepath.Join(home, ".claude", ".mcp.json"),
		filepath.Join(cwd, ".mcp.json"),
	}

	for _, p := range paths {
		data, err := os.ReadFile(p)
		if err != nil {
			continue
		}

		names := extractMCPServerNames(string(data))
		for _, name := range names {
			servers = append(servers, ServerInfo{Name: name, Source: p})
		}
	}
	return servers
}

func extractMCPServerNames(jsonStr string) []string {
	// Simple string-based extraction to avoid import cycle with claude package
	var names []string
	idx := strings.Index(jsonStr, `"mcpServers"`)
	if idx < 0 {
		return nil
	}

	rest := jsonStr[idx:]
	braceIdx := strings.Index(rest, "{")
	if braceIdx < 0 {
		return nil
	}
	rest = rest[braceIdx+1:]

	depth := 1
	i := 0
	for i < len(rest) && depth > 0 {
		switch rest[i] {
		case '{':
			depth++
		case '}':
			depth--
		case '"':
			if depth == 1 {
				end := strings.Index(rest[i+1:], `"`)
				if end >= 0 {
					key := rest[i+1 : i+1+end]
					names = append(names, key)
					i = i + 1 + end
				}
			}
		}
		i++
	}

	return names
}

func scanResources(typ string, dirs []string, sources []string) []ResourceInfo {
	var resources []ResourceInfo
	seen := make(map[string]bool)

	for dirIdx, dir := range dirs {
		source := "user"
		if dirIdx < len(sources) {
			source = sources[dirIdx]
		}

		entries, err := os.ReadDir(dir)
		if err != nil {
			continue
		}

		for _, e := range entries {
			name := e.Name()
			if strings.HasPrefix(name, ".") {
				continue
			}

			baseName := strings.TrimSuffix(name, filepath.Ext(name))
			if seen[baseName] {
				continue
			}
			seen[baseName] = true

			resources = append(resources, ResourceInfo{
				Name:   baseName,
				Path:   filepath.Join(dir, name),
				Type:   typ,
				Source: source,
			})
		}
	}

	return resources
}
