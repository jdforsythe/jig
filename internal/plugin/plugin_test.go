package plugin

import (
	"encoding/json"
	"os"
	"path/filepath"
	"sort"
	"testing"
)

// writeInstalledPlugins writes a test installed_plugins.json under homeDir.
func writeInstalledPlugins(t *testing.T, homeDir string, f InstalledPluginsFile) {
	t.Helper()
	dir := filepath.Join(homeDir, ".claude", "plugins")
	if err := os.MkdirAll(dir, 0755); err != nil {
		t.Fatal(err)
	}
	data, err := json.Marshal(f)
	if err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(filepath.Join(dir, "installed_plugins.json"), data, 0644); err != nil {
		t.Fatal(err)
	}
}

// makePluginCache creates a plugin cache directory with the given components.
func makePluginCache(t *testing.T, installPath string, agents, skills, commands []string, mcpServers map[string]any) {
	t.Helper()
	if err := os.MkdirAll(installPath, 0755); err != nil {
		t.Fatal(err)
	}
	for _, name := range agents {
		dir := filepath.Join(installPath, "agents", name)
		if err := os.MkdirAll(dir, 0755); err != nil {
			t.Fatal(err)
		}
	}
	for _, name := range skills {
		dir := filepath.Join(installPath, "skills", name)
		if err := os.MkdirAll(dir, 0755); err != nil {
			t.Fatal(err)
		}
	}
	for _, name := range commands {
		f := filepath.Join(installPath, "commands", name+".md")
		if err := os.MkdirAll(filepath.Dir(f), 0755); err != nil {
			t.Fatal(err)
		}
		if err := os.WriteFile(f, []byte("# "+name), 0644); err != nil {
			t.Fatal(err)
		}
	}
	if len(mcpServers) > 0 {
		data, err := json.Marshal(map[string]any{"mcpServers": mcpServers})
		if err != nil {
			t.Fatal(err)
		}
		if err := os.WriteFile(filepath.Join(installPath, ".mcp.json"), data, 0644); err != nil {
			t.Fatal(err)
		}
	}
}

func TestSplitKey(t *testing.T) {
	tests := []struct {
		key         string
		wantName    string
		wantMarket  string
	}{
		{"ss-engineering@sensource-claude-marketplace", "ss-engineering", "sensource-claude-marketplace"},
		{"forge@sensource-claude-marketplace", "forge", "sensource-claude-marketplace"},
		{"noscope", "noscope", ""},
		{"a@b@c", "a@b", "c"}, // last @ wins
	}
	for _, tt := range tests {
		name, market := splitKey(tt.key)
		if name != tt.wantName {
			t.Errorf("splitKey(%q) name = %q, want %q", tt.key, name, tt.wantName)
		}
		if market != tt.wantMarket {
			t.Errorf("splitKey(%q) marketplace = %q, want %q", tt.key, market, tt.wantMarket)
		}
	}
}

func TestResolve_NoFile(t *testing.T) {
	homeDir := t.TempDir()
	t.Setenv("HOME", homeDir)

	infos, err := Resolve()
	if err != nil {
		t.Fatalf("Resolve() error = %v, want nil", err)
	}
	if infos != nil {
		t.Errorf("Resolve() = %v, want nil", infos)
	}
}

func TestResolve_EmptyPlugins(t *testing.T) {
	homeDir := t.TempDir()
	t.Setenv("HOME", homeDir)

	writeInstalledPlugins(t, homeDir, InstalledPluginsFile{
		Version: 1,
		Plugins: map[string][]PluginInstall{},
	})

	infos, err := Resolve()
	if err != nil {
		t.Fatalf("Resolve() error = %v", err)
	}
	if len(infos) != 0 {
		t.Errorf("Resolve() len = %d, want 0", len(infos))
	}
}

func TestResolve_SinglePlugin(t *testing.T) {
	homeDir := t.TempDir()
	t.Setenv("HOME", homeDir)

	installPath := filepath.Join(homeDir, ".claude", "plugins", "cache", "sensource", "ss-engineering", "1.0.0")
	makePluginCache(t, installPath, []string{"repo-research-analyst", "pattern-recognition-specialist"}, []string{"commit"}, nil, nil)

	writeInstalledPlugins(t, homeDir, InstalledPluginsFile{
		Version: 1,
		Plugins: map[string][]PluginInstall{
			"ss-engineering@sensource": {
				{
					Scope:       "global",
					InstallPath: installPath,
					Version:     "1.0.0",
				},
			},
		},
	})

	infos, err := Resolve()
	if err != nil {
		t.Fatalf("Resolve() error = %v", err)
	}
	if len(infos) != 1 {
		t.Fatalf("Resolve() len = %d, want 1", len(infos))
	}

	pi := infos[0]
	if pi.Key != "ss-engineering@sensource" {
		t.Errorf("Key = %q, want %q", pi.Key, "ss-engineering@sensource")
	}
	if pi.Name != "ss-engineering" {
		t.Errorf("Name = %q, want %q", pi.Name, "ss-engineering")
	}
	if pi.Marketplace != "sensource" {
		t.Errorf("Marketplace = %q, want %q", pi.Marketplace, "sensource")
	}

	agents := pi.Components.Agents
	sort.Strings(agents)
	if len(agents) != 2 || agents[0] != "pattern-recognition-specialist" || agents[1] != "repo-research-analyst" {
		t.Errorf("Agents = %v, want [pattern-recognition-specialist repo-research-analyst]", agents)
	}

	if len(pi.Components.Skills) != 1 || pi.Components.Skills[0] != "commit" {
		t.Errorf("Skills = %v, want [commit]", pi.Components.Skills)
	}
}

func TestResolve_MultiplePlugins(t *testing.T) {
	homeDir := t.TempDir()
	t.Setenv("HOME", homeDir)

	installA := filepath.Join(homeDir, "cache", "plugin-a", "1.0.0")
	installB := filepath.Join(homeDir, "cache", "plugin-b", "1.0.0")
	makePluginCache(t, installA, []string{"agent-a"}, nil, nil, nil)
	makePluginCache(t, installB, nil, []string{"skill-b"}, nil, nil)

	writeInstalledPlugins(t, homeDir, InstalledPluginsFile{
		Version: 1,
		Plugins: map[string][]PluginInstall{
			"plugin-a@market": {{InstallPath: installA, Version: "1.0.0"}},
			"plugin-b@market": {{InstallPath: installB, Version: "1.0.0"}},
		},
	})

	infos, err := Resolve()
	if err != nil {
		t.Fatalf("Resolve() error = %v", err)
	}
	if len(infos) != 2 {
		t.Fatalf("Resolve() len = %d, want 2", len(infos))
	}
}

func TestResolve_WithMCPServers(t *testing.T) {
	homeDir := t.TempDir()
	t.Setenv("HOME", homeDir)

	installPath := filepath.Join(homeDir, "cache", "plugin-mcp", "1.0.0")
	makePluginCache(t, installPath, nil, nil, nil, map[string]any{
		"my-server": map[string]any{"command": "node", "args": []string{"server.js"}},
	})

	writeInstalledPlugins(t, homeDir, InstalledPluginsFile{
		Version: 1,
		Plugins: map[string][]PluginInstall{
			"plugin-mcp@market": {{InstallPath: installPath, Version: "1.0.0"}},
		},
	})

	infos, err := Resolve()
	if err != nil {
		t.Fatalf("Resolve() error = %v", err)
	}
	if len(infos) != 1 {
		t.Fatalf("len = %d, want 1", len(infos))
	}
	if len(infos[0].Components.MCPServers) != 1 || infos[0].Components.MCPServers[0] != "my-server" {
		t.Errorf("MCPServers = %v, want [my-server]", infos[0].Components.MCPServers)
	}
}

func TestInstallPathForKey(t *testing.T) {
	homeDir := t.TempDir()
	t.Setenv("HOME", homeDir)

	installPath := filepath.Join(homeDir, "cache", "myplug", "1.0.0")

	writeInstalledPlugins(t, homeDir, InstalledPluginsFile{
		Version: 1,
		Plugins: map[string][]PluginInstall{
			"myplug@market": {{InstallPath: installPath, Version: "1.0.0"}},
		},
	})

	got, err := InstallPathForKey("myplug@market")
	if err != nil {
		t.Fatalf("InstallPathForKey error = %v", err)
	}
	if got != installPath {
		t.Errorf("InstallPathForKey = %q, want %q", got, installPath)
	}
}

func TestInstallPathForKey_NotFound(t *testing.T) {
	homeDir := t.TempDir()
	t.Setenv("HOME", homeDir)

	writeInstalledPlugins(t, homeDir, InstalledPluginsFile{
		Version: 1,
		Plugins: map[string][]PluginInstall{},
	})

	_, err := InstallPathForKey("nonexistent@market")
	if err == nil {
		t.Error("InstallPathForKey() expected error, got nil")
	}
}

func TestInstallPathForKey_NoFile(t *testing.T) {
	homeDir := t.TempDir()
	t.Setenv("HOME", homeDir)

	_, err := InstallPathForKey("any@market")
	if err == nil {
		t.Error("InstallPathForKey() expected error when no file, got nil")
	}
}

func TestScanComponents_EmptyDir(t *testing.T) {
	dir := t.TempDir()
	c := scanComponents(dir)
	if c.Agents != nil || c.Skills != nil || c.Commands != nil || c.MCPServers != nil {
		t.Errorf("scanComponents on empty dir: got non-nil slices: %+v", c)
	}
}

func TestScanComponents_WithComponents(t *testing.T) {
	dir := t.TempDir()
	makePluginCache(&testing.T{}, dir,
		[]string{"agent-one", "agent-two"},
		[]string{"skill-one"},
		[]string{"cmd-one"},
		map[string]any{"srv": map[string]any{"command": "node"}},
	)

	c := scanComponents(dir)

	agents := append([]string{}, c.Agents...)
	sort.Strings(agents)
	if len(agents) != 2 || agents[0] != "agent-one" || agents[1] != "agent-two" {
		t.Errorf("Agents = %v", agents)
	}
	if len(c.Skills) != 1 || c.Skills[0] != "skill-one" {
		t.Errorf("Skills = %v", c.Skills)
	}
	if len(c.Commands) != 1 || c.Commands[0] != "cmd-one.md" {
		t.Errorf("Commands = %v", c.Commands)
	}
	if len(c.MCPServers) != 1 || c.MCPServers[0] != "srv" {
		t.Errorf("MCPServers = %v", c.MCPServers)
	}
}
