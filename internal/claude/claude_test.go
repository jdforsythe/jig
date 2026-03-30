package claude

import (
	"encoding/json"
	"os"
	"path/filepath"
	"testing"

	"github.com/jforsythe/jig/internal/config"
)

func TestBuildCLIArgs(t *testing.T) {
	tests := []struct {
		name        string
		profile     *config.Profile
		pluginDir   string
		passthrough []string
		wantContain []string
	}{
		{
			name: "minimal",
			profile: &config.Profile{
				Name: "test",
			},
			pluginDir:   "/tmp/jig-test",
			wantContain: []string{"--plugin-dir", "/tmp/jig-test"},
		},
		{
			name: "full flags",
			profile: &config.Profile{
				Name:               "full",
				Model:              "opus",
				Effort:             "high",
				PermissionMode:     "plan",
				AppendSystemPrompt: "Be careful.",
				AllowedTools:       []string{"Read", "Grep"},
				DisallowedTools:    []string{"Bash(rm:*)"},
				SessionAgent:       "reviewer",
			},
			pluginDir: "/tmp/jig-full",
			wantContain: []string{
				"--plugin-dir", "/tmp/jig-full",
				"--model", "opus",
				"--effort", "high",
				"--permission-mode", "plan",
				"--append-system-prompt", "Be careful.",
				"--allowedTools", "Read", "Grep",
				"--disallowedTools", "Bash(rm:*)",
				"--agent", "reviewer",
			},
		},
		{
			name: "with passthrough",
			profile: &config.Profile{
				Name:  "test",
				Model: "sonnet",
			},
			pluginDir:   "/tmp/jig-pass",
			passthrough: []string{"--verbose", "--no-color"},
			wantContain: []string{"--verbose", "--no-color"},
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			args := BuildCLIArgs(tt.profile, tt.pluginDir, tt.passthrough)

			for _, want := range tt.wantContain {
				found := false
				for _, arg := range args {
					if arg == want {
						found = true
						break
					}
				}
				if !found {
					t.Errorf("args missing %q, got %v", want, args)
				}
			}
		})
	}
}

func TestGeneratePluginDir(t *testing.T) {
	mcpIndex := &MCPIndex{
		Servers: map[string]MCPServerDef{
			"github": {Command: "npx", Args: []string{"@mcp/github"}},
		},
	}

	p := &config.Profile{
		Name:        "test-plugin",
		Description: "Test plugin generation",
		MCPServers: []config.MCPServerEntry{
			{Ref: "github"},
			{Name: "custom", Command: "node", Args: []string{"server.js"}},
		},
	}

	dir, err := GeneratePluginDir(p, mcpIndex)
	if err != nil {
		t.Fatalf("GeneratePluginDir: %v", err)
	}
	defer os.RemoveAll(dir)

	// Check plugin.json exists
	pluginJSON := filepath.Join(dir, ".claude-plugin", "plugin.json")
	data, err := os.ReadFile(pluginJSON)
	if err != nil {
		t.Fatalf("reading plugin.json: %v", err)
	}

	var manifest PluginManifest
	if err := json.Unmarshal(data, &manifest); err != nil {
		t.Fatalf("parsing plugin.json: %v", err)
	}
	if manifest.Name != "jig-test-plugin" {
		t.Errorf("manifest.Name: got %q, want %q", manifest.Name, "jig-test-plugin")
	}

	// Check .mcp.json exists
	mcpJSON := filepath.Join(dir, ".mcp.json")
	data, err = os.ReadFile(mcpJSON)
	if err != nil {
		t.Fatalf("reading .mcp.json: %v", err)
	}

	var mcpConfig mcpConfigFile
	if err := json.Unmarshal(data, &mcpConfig); err != nil {
		t.Fatalf("parsing .mcp.json: %v", err)
	}
	if _, ok := mcpConfig.MCPServers["github"]; !ok {
		t.Error("mcp config missing github server")
	}
	if _, ok := mcpConfig.MCPServers["custom"]; !ok {
		t.Error("mcp config missing custom server")
	}
}

func TestBuildMCPIndex(t *testing.T) {
	dir := t.TempDir()

	// Write a .mcp.json file
	mcpData := `{"mcpServers":{"test-server":{"command":"node","args":["server.js"]}}}`
	if err := os.WriteFile(filepath.Join(dir, ".mcp.json"), []byte(mcpData), 0644); err != nil {
		t.Fatal(err)
	}

	index, err := BuildMCPIndex(dir)
	if err != nil {
		t.Fatalf("BuildMCPIndex: %v", err)
	}

	def, ok := index.Resolve("test-server")
	if !ok {
		t.Fatal("test-server not found in index")
	}
	if def.Command != "node" {
		t.Errorf("Command: got %q, want %q", def.Command, "node")
	}
}
