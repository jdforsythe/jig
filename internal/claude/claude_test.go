package claude

import (
	"encoding/json"
	"os"
	"path/filepath"
	"testing"

	"github.com/jdforsythe/jig/internal/config"
)

func TestBuildCLIArgs(t *testing.T) {
	tests := []struct {
		name        string
		profile     *config.Profile
		pluginDir   string
		settingsPath string
		passthrough []string
		wantContain []string
		wantOrder   [][2]string // pairs that must appear in this relative order
	}{
		{
			name: "minimal",
			profile: &config.Profile{
				Name: "test",
			},
			pluginDir:    "/tmp/jig-test",
			settingsPath: "/tmp/jig-test/jig-settings.json",
			wantContain: []string{
				"--settings", "/tmp/jig-test/jig-settings.json",
				"--plugin-dir", "/tmp/jig-test",
			},
		},
		{
			name: "settings before plugin-dir",
			profile: &config.Profile{
				Name: "test",
			},
			pluginDir:    "/tmp/jig-test",
			settingsPath: "/tmp/jig-test/jig-settings.json",
			wantContain:  []string{"--settings", "--plugin-dir"},
			wantOrder: [][2]string{
				{"--settings", "--plugin-dir"},
			},
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
			pluginDir:    "/tmp/jig-full",
			settingsPath: "/tmp/jig-full/jig-settings.json",
			wantContain: []string{
				"--settings", "/tmp/jig-full/jig-settings.json",
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
			pluginDir:    "/tmp/jig-pass",
			settingsPath: "/tmp/jig-pass/jig-settings.json",
			passthrough:  []string{"--verbose", "--no-color"},
			wantContain:  []string{"--verbose", "--no-color"},
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			args := BuildCLIArgs(tt.profile, tt.pluginDir, tt.settingsPath, tt.passthrough)

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

			// Check ordering constraints
			for _, pair := range tt.wantOrder {
				firstIdx, secondIdx := -1, -1
				for i, arg := range args {
					if arg == pair[0] && firstIdx < 0 {
						firstIdx = i
					}
					if arg == pair[1] && secondIdx < 0 {
						secondIdx = i
					}
				}
				if firstIdx < 0 {
					t.Errorf("order check: %q not found in args", pair[0])
				} else if secondIdx < 0 {
					t.Errorf("order check: %q not found in args", pair[1])
				} else if firstIdx >= secondIdx {
					t.Errorf("order check: %q (pos %d) should come before %q (pos %d)", pair[0], firstIdx, pair[1], secondIdx)
				}
			}
		})
	}
}

func TestGeneratePluginDir_AlwaysWritesSettings(t *testing.T) {
	mcpIndex := &MCPIndex{Servers: map[string]MCPServerDef{}}
	p := &config.Profile{Name: "test-isolation"}

	pluginDir, settingsPath, err := GeneratePluginDir(p, mcpIndex)
	if err != nil {
		t.Fatalf("GeneratePluginDir: %v", err)
	}
	defer os.RemoveAll(pluginDir)

	// jig-settings.json must always be written
	data, err := os.ReadFile(settingsPath)
	if err != nil {
		t.Fatalf("reading jig-settings.json: %v", err)
	}

	var settings map[string]any
	if err := json.Unmarshal(data, &settings); err != nil {
		t.Fatalf("parsing jig-settings.json: %v", err)
	}

	if _, ok := settings["enabledPlugins"]; !ok {
		t.Error("jig-settings.json missing enabledPlugins key")
	}
}

func TestGeneratePluginDir_EnabledPlugins(t *testing.T) {
	mcpIndex := &MCPIndex{Servers: map[string]MCPServerDef{}}
	p := &config.Profile{
		Name: "test-enabled",
		EnabledPlugins: map[string]bool{
			"forge@sensource": true,
		},
	}

	pluginDir, settingsPath, err := GeneratePluginDir(p, mcpIndex)
	if err != nil {
		t.Fatalf("GeneratePluginDir: %v", err)
	}
	defer os.RemoveAll(pluginDir)

	data, err := os.ReadFile(settingsPath)
	if err != nil {
		t.Fatalf("reading jig-settings.json: %v", err)
	}

	var settings map[string]any
	if err := json.Unmarshal(data, &settings); err != nil {
		t.Fatalf("parsing jig-settings.json: %v", err)
	}

	ep, ok := settings["enabledPlugins"].(map[string]any)
	if !ok {
		t.Fatal("enabledPlugins is not a map")
	}
	if ep["forge@sensource"] != true {
		t.Errorf("enabledPlugins[forge@sensource] = %v, want true", ep["forge@sensource"])
	}
}

func TestGeneratePluginDir_BasicPluginJSON(t *testing.T) {
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

	pluginDir, settingsPath, err := GeneratePluginDir(p, mcpIndex)
	if err != nil {
		t.Fatalf("GeneratePluginDir: %v", err)
	}
	defer os.RemoveAll(pluginDir)

	if settingsPath == "" {
		t.Error("settingsPath is empty")
	}

	// Check plugin.json exists
	pluginJSON := filepath.Join(pluginDir, ".claude-plugin", "plugin.json")
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

	// Check .mcp.json
	mcpJSON := filepath.Join(pluginDir, ".mcp.json")
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

func TestGeneratePluginDir_PluginComponents(t *testing.T) {
	// Set up a fake plugin install directory
	pluginInstallDir := t.TempDir()

	// Create agents/skills in the plugin install dir
	agentPath := filepath.Join(pluginInstallDir, "agents", "my-agent.md")
	if err := os.MkdirAll(filepath.Dir(agentPath), 0755); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(agentPath, []byte("agent content"), 0644); err != nil {
		t.Fatal(err)
	}

	skillPath := filepath.Join(pluginInstallDir, "skills", "my-skill")
	if err := os.MkdirAll(skillPath, 0755); err != nil {
		t.Fatal(err)
	}

	// Write an installed_plugins.json pointing to our fake install dir
	homeDir := t.TempDir()
	t.Setenv("HOME", homeDir)
	pluginsDir := filepath.Join(homeDir, ".claude", "plugins")
	if err := os.MkdirAll(pluginsDir, 0755); err != nil {
		t.Fatal(err)
	}
	pluginsJSON := map[string]any{
		"version": 1,
		"plugins": map[string]any{
			"test-plugin@market": []map[string]any{
				{"scope": "global", "installPath": pluginInstallDir, "version": "1.0.0"},
			},
		},
	}
	data, _ := json.Marshal(pluginsJSON)
	if err := os.WriteFile(filepath.Join(pluginsDir, "installed_plugins.json"), data, 0644); err != nil {
		t.Fatal(err)
	}

	mcpIndex := &MCPIndex{Servers: map[string]MCPServerDef{}}
	p := &config.Profile{
		Name: "comp-test",
		PluginComponents: map[string]config.PluginComponentSelection{
			"test-plugin@market": {
				Agents: []string{"my-agent.md"},
				Skills: []string{"my-skill"},
			},
		},
	}

	dir, _, err := GeneratePluginDir(p, mcpIndex)
	if err != nil {
		t.Fatalf("GeneratePluginDir: %v", err)
	}
	defer os.RemoveAll(dir)

	// Check agent symlink was created
	agentSymlink := filepath.Join(dir, "agents", "my-agent.md")
	info, err := os.Lstat(agentSymlink)
	if err != nil {
		t.Fatalf("agent symlink not found: %v", err)
	}
	if info.Mode()&os.ModeSymlink == 0 {
		t.Error("agents/my-agent.md is not a symlink")
	}

	// Check skill symlink was created
	skillSymlink := filepath.Join(dir, "skills", "my-skill")
	info, err = os.Lstat(skillSymlink)
	if err != nil {
		t.Fatalf("skill symlink not found: %v", err)
	}
	if info.Mode()&os.ModeSymlink == 0 {
		t.Error("skills/my-skill is not a symlink")
	}
}

func TestGeneratePluginDir_EmptyPluginComponentsNoMCP(t *testing.T) {
	// When profile has no MCP servers and no plugin components, no .mcp.json should be written
	mcpIndex := &MCPIndex{Servers: map[string]MCPServerDef{}}
	p := &config.Profile{Name: "no-mcp"}

	dir, _, err := GeneratePluginDir(p, mcpIndex)
	if err != nil {
		t.Fatalf("GeneratePluginDir: %v", err)
	}
	defer os.RemoveAll(dir)

	if _, err := os.Stat(filepath.Join(dir, ".mcp.json")); !os.IsNotExist(err) {
		t.Error(".mcp.json should not exist when no MCP servers configured")
	}
}

func TestBuildMCPIndex(t *testing.T) {
	dir := t.TempDir()

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
