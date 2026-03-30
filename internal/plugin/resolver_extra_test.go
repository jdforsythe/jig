package plugin

import (
	"os"
	"path/filepath"
	"testing"
)

func TestResolve_MalformedJSON(t *testing.T) {
	homeDir := t.TempDir()
	t.Setenv("HOME", homeDir)

	pluginsDir := filepath.Join(homeDir, ".claude", "plugins")
	if err := os.MkdirAll(pluginsDir, 0755); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(filepath.Join(pluginsDir, "installed_plugins.json"), []byte("{not valid json"), 0644); err != nil {
		t.Fatal(err)
	}

	_, err := Resolve()
	if err == nil {
		t.Error("Resolve() expected error for malformed JSON, got nil")
	}
}

func TestResolve_MultipleInstalls_UsesFirst(t *testing.T) {
	homeDir := t.TempDir()
	t.Setenv("HOME", homeDir)

	installPathA := filepath.Join(homeDir, "cache", "plug", "1.0.0")
	installPathB := filepath.Join(homeDir, "cache", "plug", "2.0.0")
	makePluginCache(t, installPathA, []string{"agent-v1"}, nil, nil, nil)
	makePluginCache(t, installPathB, []string{"agent-v2"}, nil, nil, nil)

	writeInstalledPlugins(t, homeDir, InstalledPluginsFile{
		Version: 1,
		Plugins: map[string][]PluginInstall{
			"plug@market": {
				{InstallPath: installPathA, Version: "1.0.0"},
				{InstallPath: installPathB, Version: "2.0.0"},
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
	// First install record wins
	if len(infos[0].Components.Agents) != 1 || infos[0].Components.Agents[0] != "agent-v1" {
		t.Errorf("Agents = %v, want [agent-v1] (first install used)", infos[0].Components.Agents)
	}
}

func TestResolve_SkipsEmptyInstallList(t *testing.T) {
	homeDir := t.TempDir()
	t.Setenv("HOME", homeDir)

	writeInstalledPlugins(t, homeDir, InstalledPluginsFile{
		Version: 1,
		Plugins: map[string][]PluginInstall{
			"empty-plugin@market": {}, // no installs
		},
	})

	infos, err := Resolve()
	if err != nil {
		t.Fatalf("Resolve() error = %v", err)
	}
	if len(infos) != 0 {
		t.Errorf("Resolve() len = %d, want 0 (empty install list skipped)", len(infos))
	}
}

func TestInstallPathForKey_MalformedJSON(t *testing.T) {
	homeDir := t.TempDir()
	t.Setenv("HOME", homeDir)

	pluginsDir := filepath.Join(homeDir, ".claude", "plugins")
	if err := os.MkdirAll(pluginsDir, 0755); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(filepath.Join(pluginsDir, "installed_plugins.json"), []byte("{bad json"), 0644); err != nil {
		t.Fatal(err)
	}

	_, err := InstallPathForKey("any@market")
	if err == nil {
		t.Error("InstallPathForKey() expected error for malformed JSON, got nil")
	}
}

func TestListEntryNames_HiddenFilesFiltered(t *testing.T) {
	dir := t.TempDir()

	if err := os.WriteFile(filepath.Join(dir, "visible-agent.md"), []byte("ok"), 0644); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(filepath.Join(dir, ".hidden"), []byte("hidden"), 0644); err != nil {
		t.Fatal(err)
	}
	if err := os.MkdirAll(filepath.Join(dir, ".git"), 0755); err != nil {
		t.Fatal(err)
	}

	names := listEntryNames(dir)

	if len(names) != 1 {
		t.Fatalf("listEntryNames len = %d, want 1 (hidden entries filtered): %v", len(names), names)
	}
	if names[0] != "visible-agent.md" {
		t.Errorf("name = %q, want visible-agent.md", names[0])
	}
}

func TestListEntryNames_NonexistentDir(t *testing.T) {
	names := listEntryNames("/nonexistent/path/that/does/not/exist")
	if names != nil {
		t.Errorf("listEntryNames on nonexistent dir = %v, want nil", names)
	}
}

func TestReadMCPServerNames_Malformed(t *testing.T) {
	f, err := os.CreateTemp("", "mcp-*.json")
	if err != nil {
		t.Fatal(err)
	}
	defer os.Remove(f.Name())
	f.WriteString("{not valid json")
	f.Close()

	names := readMCPServerNames(f.Name())
	if names != nil {
		t.Errorf("readMCPServerNames (malformed) = %v, want nil", names)
	}
}

func TestReadMCPServerNames_NoMCPServersKey(t *testing.T) {
	f, err := os.CreateTemp("", "mcp-*.json")
	if err != nil {
		t.Fatal(err)
	}
	defer os.Remove(f.Name())
	f.WriteString(`{"otherKey": {}}`)
	f.Close()

	names := readMCPServerNames(f.Name())
	if len(names) != 0 {
		t.Errorf("readMCPServerNames (no mcpServers) = %v, want empty/nil", names)
	}
}

func TestReadMCPServerNames_NonexistentFile(t *testing.T) {
	names := readMCPServerNames("/nonexistent/.mcp.json")
	if names != nil {
		t.Errorf("readMCPServerNames (missing file) = %v, want nil", names)
	}
}

func TestExpandHome_WithTilde(t *testing.T) {
	homeDir := t.TempDir()
	t.Setenv("HOME", homeDir)

	result := expandHome("~/foo/bar")
	expected := filepath.Join(homeDir, "foo", "bar")
	if result != expected {
		t.Errorf("expandHome(~/foo/bar) = %q, want %q", result, expected)
	}
}

func TestExpandHome_NoTilde(t *testing.T) {
	result := expandHome("/absolute/path")
	if result != "/absolute/path" {
		t.Errorf("expandHome(/absolute/path) = %q, want unchanged", result)
	}
}

func TestExpandHome_TildeOnly(t *testing.T) {
	// "~" alone — no slash follows, should not expand
	result := expandHome("~")
	if result != "~" {
		t.Errorf("expandHome(~) = %q, want ~ (no expansion)", result)
	}
}

func TestExpandHome_RelativePath(t *testing.T) {
	result := expandHome("relative/path")
	if result != "relative/path" {
		t.Errorf("expandHome(relative/path) = %q, want unchanged", result)
	}
}

func TestScanComponents_HiddenFilesFiltered(t *testing.T) {
	dir := t.TempDir()

	// Visible agent dir
	if err := os.MkdirAll(filepath.Join(dir, "agents", "my-agent"), 0755); err != nil {
		t.Fatal(err)
	}
	// Hidden agent dir — should be filtered
	if err := os.MkdirAll(filepath.Join(dir, "agents", ".hidden-agent"), 0755); err != nil {
		t.Fatal(err)
	}

	c := scanComponents(dir)

	if len(c.Agents) != 1 {
		t.Fatalf("Agents len = %d, want 1: %v", len(c.Agents), c.Agents)
	}
	if c.Agents[0] != "my-agent" {
		t.Errorf("Agents[0] = %q, want my-agent", c.Agents[0])
	}
}
