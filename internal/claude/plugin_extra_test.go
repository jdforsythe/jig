package claude

import (
	"encoding/json"
	"os"
	"path/filepath"
	"testing"

	"github.com/jdforsythe/jig/internal/config"
)

func TestGeneratePluginDir_SkillSymlinks(t *testing.T) {
	// Create real skill directory to symlink to
	skillDir := t.TempDir()
	skillPath := filepath.Join(skillDir, "my-skill")
	if err := os.MkdirAll(skillPath, 0755); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(filepath.Join(skillPath, "SKILL.md"), []byte("skill content"), 0644); err != nil {
		t.Fatal(err)
	}

	mcpIndex := &MCPIndex{Servers: map[string]MCPServerDef{}}
	p := &config.Profile{
		Name: "skill-test",
		Skills: []config.PathEntry{
			{Path: skillPath},
		},
	}

	dir, _, err := GeneratePluginDir(p, mcpIndex)
	if err != nil {
		t.Fatalf("GeneratePluginDir: %v", err)
	}
	defer os.RemoveAll(dir)

	symlink := filepath.Join(dir, "skills", "my-skill")
	info, err := os.Lstat(symlink)
	if err != nil {
		t.Fatalf("skills/my-skill not found: %v", err)
	}
	if info.Mode()&os.ModeSymlink == 0 {
		t.Error("skills/my-skill should be a symlink")
	}

	target, err := os.Readlink(symlink)
	if err != nil {
		t.Fatal(err)
	}
	if target != skillPath {
		t.Errorf("symlink target = %q, want %q", target, skillPath)
	}
}

func TestGeneratePluginDir_AgentSymlinks(t *testing.T) {
	agentDir := t.TempDir()
	agentFile := filepath.Join(agentDir, "reviewer.md")
	if err := os.WriteFile(agentFile, []byte("# Reviewer"), 0644); err != nil {
		t.Fatal(err)
	}

	mcpIndex := &MCPIndex{Servers: map[string]MCPServerDef{}}
	p := &config.Profile{
		Name: "agent-test",
		Agents: []config.PathEntry{
			{Path: agentFile},
		},
	}

	dir, _, err := GeneratePluginDir(p, mcpIndex)
	if err != nil {
		t.Fatalf("GeneratePluginDir: %v", err)
	}
	defer os.RemoveAll(dir)

	symlink := filepath.Join(dir, "agents", "reviewer.md")
	info, err := os.Lstat(symlink)
	if err != nil {
		t.Fatalf("agents/reviewer.md not found: %v", err)
	}
	if info.Mode()&os.ModeSymlink == 0 {
		t.Error("agents/reviewer.md should be a symlink")
	}
}

func TestGeneratePluginDir_CommandSymlinks(t *testing.T) {
	cmdDir := t.TempDir()
	cmdFile := filepath.Join(cmdDir, "deploy.md")
	if err := os.WriteFile(cmdFile, []byte("# Deploy"), 0644); err != nil {
		t.Fatal(err)
	}

	mcpIndex := &MCPIndex{Servers: map[string]MCPServerDef{}}
	p := &config.Profile{
		Name: "cmd-test",
		Commands: []config.PathEntry{
			{Path: cmdFile},
		},
	}

	dir, _, err := GeneratePluginDir(p, mcpIndex)
	if err != nil {
		t.Fatalf("GeneratePluginDir: %v", err)
	}
	defer os.RemoveAll(dir)

	symlink := filepath.Join(dir, "commands", "deploy.md")
	info, err := os.Lstat(symlink)
	if err != nil {
		t.Fatalf("commands/deploy.md not found: %v", err)
	}
	if info.Mode()&os.ModeSymlink == 0 {
		t.Error("commands/deploy.md should be a symlink")
	}
}

func TestGeneratePluginDir_HooksJSON(t *testing.T) {
	mcpIndex := &MCPIndex{Servers: map[string]MCPServerDef{}}
	p := &config.Profile{
		Name: "hooks-test",
		Hooks: map[string][]config.HookMatcher{
			"PostToolUse": {
				{
					Matcher: "Edit|Write",
					Hooks:   []config.HookDef{{Type: "command", Command: "prettier --write"}},
				},
			},
		},
	}

	dir, _, err := GeneratePluginDir(p, mcpIndex)
	if err != nil {
		t.Fatalf("GeneratePluginDir: %v", err)
	}
	defer os.RemoveAll(dir)

	hooksJSON := filepath.Join(dir, "hooks", "hooks.json")
	data, err := os.ReadFile(hooksJSON)
	if err != nil {
		t.Fatalf("hooks/hooks.json not found: %v", err)
	}

	var hooks map[string]any
	if err := json.Unmarshal(data, &hooks); err != nil {
		t.Fatalf("parsing hooks.json: %v", err)
	}
	if _, ok := hooks["PostToolUse"]; !ok {
		t.Error("hooks.json missing PostToolUse key")
	}
}

func TestGeneratePluginDir_SettingsPassthrough(t *testing.T) {
	mcpIndex := &MCPIndex{Servers: map[string]MCPServerDef{}}
	p := &config.Profile{
		Name: "settings-test",
		Settings: map[string]any{
			"customKey":  "customValue",
			"anotherKey": 42,
		},
	}

	dir, _, err := GeneratePluginDir(p, mcpIndex)
	if err != nil {
		t.Fatalf("GeneratePluginDir: %v", err)
	}
	defer os.RemoveAll(dir)

	settingsJSON := filepath.Join(dir, ".claude-plugin", "settings.json")
	data, err := os.ReadFile(settingsJSON)
	if err != nil {
		t.Fatalf(".claude-plugin/settings.json not found: %v", err)
	}

	var settings map[string]any
	if err := json.Unmarshal(data, &settings); err != nil {
		t.Fatalf("parsing settings.json: %v", err)
	}
	if settings["customKey"] != "customValue" {
		t.Errorf("customKey = %v, want customValue", settings["customKey"])
	}
}

func TestGeneratePluginDir_NoSettingsFile_WhenEmpty(t *testing.T) {
	// When profile.Settings is nil/empty, .claude-plugin/settings.json should NOT be written
	mcpIndex := &MCPIndex{Servers: map[string]MCPServerDef{}}
	p := &config.Profile{Name: "no-settings"}

	dir, _, err := GeneratePluginDir(p, mcpIndex)
	if err != nil {
		t.Fatalf("GeneratePluginDir: %v", err)
	}
	defer os.RemoveAll(dir)

	settingsJSON := filepath.Join(dir, ".claude-plugin", "settings.json")
	if _, err := os.Stat(settingsJSON); !os.IsNotExist(err) {
		t.Error(".claude-plugin/settings.json should not exist when profile.Settings is empty")
	}
}

func TestGeneratePluginDir_HookScripts(t *testing.T) {
	// Create a script to copy
	scriptDir := t.TempDir()
	scriptPath := filepath.Join(scriptDir, "validate.sh")
	if err := os.WriteFile(scriptPath, []byte("#!/bin/bash\necho ok"), 0755); err != nil {
		t.Fatal(err)
	}

	mcpIndex := &MCPIndex{Servers: map[string]MCPServerDef{}}
	p := &config.Profile{
		Name: "hookscript-test",
		HookScripts: []config.HookScript{
			{Path: scriptPath, Dest: "hooks/validate.sh"},
		},
	}

	dir, _, err := GeneratePluginDir(p, mcpIndex)
	if err != nil {
		t.Fatalf("GeneratePluginDir: %v", err)
	}
	defer os.RemoveAll(dir)

	dest := filepath.Join(dir, "hooks", "validate.sh")
	data, err := os.ReadFile(dest)
	if err != nil {
		t.Fatalf("hooks/validate.sh not copied: %v", err)
	}
	if string(data) != "#!/bin/bash\necho ok" {
		t.Errorf("script content mismatch: %q", string(data))
	}
}

func TestGeneratePluginDir_PluginComponentMCPMerge(t *testing.T) {
	// Set up a fake plugin install dir with .mcp.json
	pluginInstallDir := t.TempDir()
	pluginMCP := `{"mcpServers":{"plugin-db":{"command":"python","args":["db.py"]},"plugin-cache":{"command":"node","args":["cache.js"]}}}`
	if err := os.WriteFile(filepath.Join(pluginInstallDir, ".mcp.json"), []byte(pluginMCP), 0644); err != nil {
		t.Fatal(err)
	}

	// Set up installed_plugins.json
	homeDir := t.TempDir()
	t.Setenv("HOME", homeDir)
	pluginsDir := filepath.Join(homeDir, ".claude", "plugins")
	if err := os.MkdirAll(pluginsDir, 0755); err != nil {
		t.Fatal(err)
	}
	installedJSON := map[string]any{
		"version": 1,
		"plugins": map[string]any{
			"mcp-plugin@market": []map[string]any{
				{"scope": "global", "installPath": pluginInstallDir, "version": "1.0.0"},
			},
		},
	}
	data, _ := json.Marshal(installedJSON)
	if err := os.WriteFile(filepath.Join(pluginsDir, "installed_plugins.json"), data, 0644); err != nil {
		t.Fatal(err)
	}

	mcpIndex := &MCPIndex{Servers: map[string]MCPServerDef{}}
	p := &config.Profile{
		Name: "plugin-mcp-test",
		PluginComponents: map[string]config.PluginComponentSelection{
			"mcp-plugin@market": {
				MCPServers: []string{"plugin-db"},
			},
		},
	}

	dir, _, err := GeneratePluginDir(p, mcpIndex)
	if err != nil {
		t.Fatalf("GeneratePluginDir: %v", err)
	}
	defer os.RemoveAll(dir)

	// .mcp.json should be written with the plugin server
	mcpJSON := filepath.Join(dir, ".mcp.json")
	raw, err := os.ReadFile(mcpJSON)
	if err != nil {
		t.Fatalf(".mcp.json not found: %v", err)
	}

	var mcpConfig mcpConfigFile
	if err := json.Unmarshal(raw, &mcpConfig); err != nil {
		t.Fatalf("parsing .mcp.json: %v", err)
	}
	if _, ok := mcpConfig.MCPServers["plugin-db"]; !ok {
		t.Error(".mcp.json missing plugin-db server from plugin_components")
	}
	// plugin-cache was NOT selected, should not be present
	if _, ok := mcpConfig.MCPServers["plugin-cache"]; ok {
		t.Error(".mcp.json should not contain plugin-cache (not selected)")
	}
}

func TestGeneratePluginDir_ExpandsTilde(t *testing.T) {
	homeDir := t.TempDir()
	t.Setenv("HOME", homeDir)

	// Create an agent file under fake home
	agentDir := filepath.Join(homeDir, ".claude", "agents")
	if err := os.MkdirAll(agentDir, 0755); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(filepath.Join(agentDir, "home-agent.md"), []byte("agent"), 0644); err != nil {
		t.Fatal(err)
	}

	mcpIndex := &MCPIndex{Servers: map[string]MCPServerDef{}}
	p := &config.Profile{
		Name: "tilde-test",
		Agents: []config.PathEntry{
			{Path: "~/.claude/agents/home-agent.md"},
		},
	}

	dir, _, err := GeneratePluginDir(p, mcpIndex)
	if err != nil {
		t.Fatalf("GeneratePluginDir: %v", err)
	}
	defer os.RemoveAll(dir)

	symlink := filepath.Join(dir, "agents", "home-agent.md")
	if _, err := os.Lstat(symlink); err != nil {
		t.Fatalf("symlink not created for tilde path: %v", err)
	}
}

func TestExpandPath_Tilde(t *testing.T) {
	homeDir := t.TempDir()
	t.Setenv("HOME", homeDir)

	result := expandPath("~/foo/bar")
	expected := filepath.Join(homeDir, "foo", "bar")
	if result != expected {
		t.Errorf("expandPath(~/foo/bar) = %q, want %q", result, expected)
	}
}

func TestExpandPath_NoTilde(t *testing.T) {
	result := expandPath("/absolute/path")
	if result != "/absolute/path" {
		t.Errorf("expandPath(/absolute/path) = %q, want unchanged", result)
	}

	result = expandPath("relative/path")
	if result != "relative/path" {
		t.Errorf("expandPath(relative/path) = %q, want unchanged", result)
	}
}

func TestExpandPath_TildeOnly(t *testing.T) {
	// "~" alone (no trailing slash) — should not expand
	result := expandPath("~")
	if result != "~" {
		t.Errorf("expandPath(~) = %q, want ~ (no expansion without trailing /)", result)
	}
}
