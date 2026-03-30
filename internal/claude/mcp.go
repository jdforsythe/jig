package claude

import (
	"encoding/json"
	"os"
	"path/filepath"
)

// MCPIndex holds all known MCP server definitions found in .mcp.json files.
type MCPIndex struct {
	Servers map[string]MCPServerDef
}

// MCPServerDef is a single MCP server definition from .mcp.json.
type MCPServerDef struct {
	Command string            `json:"command"`
	Args    []string          `json:"args,omitempty"`
	Env     map[string]string `json:"env,omitempty"`
}

// mcpConfigFile is the structure of a .mcp.json file.
type mcpConfigFile struct {
	MCPServers map[string]MCPServerDef `json:"mcpServers"`
}

// BuildMCPIndex scans for .mcp.json files and builds an index of available servers.
// Searches: cwd/.mcp.json, ~/.claude/.mcp.json
func BuildMCPIndex(cwd string) (*MCPIndex, error) {
	index := &MCPIndex{
		Servers: make(map[string]MCPServerDef),
	}

	// Scan locations in order (later ones override earlier)
	locations := []string{}

	// Global: ~/.claude/.mcp.json
	if home, err := os.UserHomeDir(); err == nil {
		locations = append(locations, filepath.Join(home, ".claude", ".mcp.json"))
	}

	// Project: cwd/.mcp.json
	locations = append(locations, filepath.Join(cwd, ".mcp.json"))

	for _, path := range locations {
		if err := loadMCPFile(path, index); err != nil {
			continue // skip files that don't exist or can't be parsed
		}
	}

	return index, nil
}

func loadMCPFile(path string, index *MCPIndex) error {
	data, err := os.ReadFile(path)
	if err != nil {
		return err
	}

	var config mcpConfigFile
	if err := json.Unmarshal(data, &config); err != nil {
		return err
	}

	for name, def := range config.MCPServers {
		index.Servers[name] = def
	}
	return nil
}

// Resolve looks up an MCP server by name in the index.
func (idx *MCPIndex) Resolve(name string) (MCPServerDef, bool) {
	def, ok := idx.Servers[name]
	return def, ok
}
