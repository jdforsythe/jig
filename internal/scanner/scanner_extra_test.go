package scanner

import (
	"os"
	"path/filepath"
	"testing"
)

func TestScan_MCPServersDiscovered(t *testing.T) {
	homeDir := t.TempDir()
	cwd := t.TempDir()
	t.Setenv("HOME", homeDir)

	// Write a project .mcp.json
	mcpData := `{"mcpServers":{"server-x":{"command":"node"},"server-y":{"command":"python"}}}`
	if err := os.WriteFile(filepath.Join(cwd, ".mcp.json"), []byte(mcpData), 0644); err != nil {
		t.Fatal(err)
	}

	disc, err := Scan(cwd)
	if err != nil {
		t.Fatalf("Scan() error = %v", err)
	}

	nameSet := make(map[string]bool)
	for _, s := range disc.MCPServers {
		nameSet[s.Name] = true
	}
	if !nameSet["server-x"] || !nameSet["server-y"] {
		t.Errorf("MCPServers = %v, want server-x and server-y", disc.MCPServers)
	}
}

func TestScan_GlobalMCPServersMerged(t *testing.T) {
	homeDir := t.TempDir()
	cwd := t.TempDir()
	t.Setenv("HOME", homeDir)

	// Global .mcp.json
	globalDir := filepath.Join(homeDir, ".claude")
	if err := os.MkdirAll(globalDir, 0755); err != nil {
		t.Fatal(err)
	}
	globalMCP := `{"mcpServers":{"global-srv":{"command":"node"}}}`
	if err := os.WriteFile(filepath.Join(globalDir, ".mcp.json"), []byte(globalMCP), 0644); err != nil {
		t.Fatal(err)
	}

	// Project .mcp.json
	projMCP := `{"mcpServers":{"proj-srv":{"command":"python"}}}`
	if err := os.WriteFile(filepath.Join(cwd, ".mcp.json"), []byte(projMCP), 0644); err != nil {
		t.Fatal(err)
	}

	disc, err := Scan(cwd)
	if err != nil {
		t.Fatalf("Scan() error = %v", err)
	}

	nameSet := make(map[string]bool)
	for _, s := range disc.MCPServers {
		nameSet[s.Name] = true
	}
	if !nameSet["global-srv"] {
		t.Error("global-srv not found in MCPServers")
	}
	if !nameSet["proj-srv"] {
		t.Error("proj-srv not found in MCPServers")
	}
}

func TestScan_NoMCPFiles(t *testing.T) {
	homeDir := t.TempDir()
	cwd := t.TempDir()
	t.Setenv("HOME", homeDir)

	disc, err := Scan(cwd)
	if err != nil {
		t.Fatalf("Scan() error = %v", err)
	}
	if len(disc.MCPServers) != 0 {
		t.Errorf("MCPServers = %v, want empty", disc.MCPServers)
	}
}

func TestScanResources_HiddenFilesSkipped(t *testing.T) {
	dir := t.TempDir()

	// Visible file
	if err := os.WriteFile(filepath.Join(dir, "visible.md"), []byte("ok"), 0644); err != nil {
		t.Fatal(err)
	}
	// Hidden file — should be skipped
	if err := os.WriteFile(filepath.Join(dir, ".hidden.md"), []byte("hidden"), 0644); err != nil {
		t.Fatal(err)
	}
	// Hidden dir — should be skipped
	if err := os.MkdirAll(filepath.Join(dir, ".git"), 0755); err != nil {
		t.Fatal(err)
	}

	resources := scanResources("command", []string{dir}, []string{"project"})

	if len(resources) != 1 {
		t.Fatalf("len = %d, want 1 (hidden entries skipped)", len(resources))
	}
	if resources[0].Name != "visible" {
		t.Errorf("Name = %q, want visible", resources[0].Name)
	}
}

func TestScanResources_SourcesFallback(t *testing.T) {
	// When sources slice is shorter than dirs, extra dirs fall back to "user"
	dir1 := t.TempDir()
	dir2 := t.TempDir()

	if err := os.WriteFile(filepath.Join(dir1, "a.md"), []byte("a"), 0644); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(filepath.Join(dir2, "b.md"), []byte("b"), 0644); err != nil {
		t.Fatal(err)
	}

	// Only one source label provided for two dirs
	resources := scanResources("agent", []string{dir1, dir2}, []string{"custom"})

	srcMap := make(map[string]string)
	for _, r := range resources {
		srcMap[r.Name] = r.Source
	}
	if srcMap["a"] != "custom" {
		t.Errorf("a source = %q, want custom", srcMap["a"])
	}
	// dir2 has no corresponding source entry — falls back to "user"
	if srcMap["b"] != "user" {
		t.Errorf("b source = %q, want user (fallback)", srcMap["b"])
	}
}

func TestExtractMCPServerNames_NoKey(t *testing.T) {
	names := extractMCPServerNames(`{"otherKey":{}}`)
	if names != nil {
		t.Errorf("extractMCPServerNames (no mcpServers) = %v, want nil", names)
	}
}

func TestExtractMCPServerNames_EmptyInput(t *testing.T) {
	names := extractMCPServerNames("")
	if names != nil {
		t.Errorf("extractMCPServerNames (empty) = %v, want nil", names)
	}
}

func TestExtractMCPServerNames_EmptyServers(t *testing.T) {
	names := extractMCPServerNames(`{"mcpServers":{}}`)
	// Empty object — no server names
	if len(names) != 0 {
		t.Errorf("extractMCPServerNames (empty servers) = %v, want []", names)
	}
}

func TestExtractMCPServerNames_NestedValues(t *testing.T) {
	// Servers with nested config should still return top-level server names only
	json := `{"mcpServers":{"db":{"command":"npx","args":["sqlite"],"env":{"KEY":"val"}},"cache":{"command":"node"}}}`
	names := extractMCPServerNames(json)

	nameSet := make(map[string]bool)
	for _, n := range names {
		nameSet[n] = true
	}
	if !nameSet["db"] {
		t.Error("db not found in extracted names")
	}
	if !nameSet["cache"] {
		t.Error("cache not found in extracted names")
	}
	if len(names) != 2 {
		t.Errorf("got %d names, want 2: %v", len(names), names)
	}
}

func TestScan_UserLevelMCPFromClaudeJSON(t *testing.T) {
	homeDir := t.TempDir()
	cwd := t.TempDir()
	t.Setenv("HOME", homeDir)

	claudeJSON := `{"mcpServers":{"user-srv":{"command":"node"}}}`
	if err := os.WriteFile(filepath.Join(homeDir, ".claude.json"), []byte(claudeJSON), 0644); err != nil {
		t.Fatal(err)
	}

	disc, err := Scan(cwd)
	if err != nil {
		t.Fatalf("Scan() error = %v", err)
	}

	srcMap := make(map[string]string)
	for _, s := range disc.MCPServers {
		srcMap[s.Name] = s.Source
	}
	if srcMap["user-srv"] != "user" {
		t.Errorf("user-srv source = %q, want user", srcMap["user-srv"])
	}
}

func TestScan_ClaudeJSONTakesPriorityOverLegacy(t *testing.T) {
	homeDir := t.TempDir()
	cwd := t.TempDir()
	t.Setenv("HOME", homeDir)

	claudeJSON := `{"mcpServers":{"shared-srv":{"command":"node"}}}`
	if err := os.WriteFile(filepath.Join(homeDir, ".claude.json"), []byte(claudeJSON), 0644); err != nil {
		t.Fatal(err)
	}
	legacyDir := filepath.Join(homeDir, ".claude")
	if err := os.MkdirAll(legacyDir, 0755); err != nil {
		t.Fatal(err)
	}
	legacyMCP := `{"mcpServers":{"shared-srv":{"command":"python"}}}`
	if err := os.WriteFile(filepath.Join(legacyDir, ".mcp.json"), []byte(legacyMCP), 0644); err != nil {
		t.Fatal(err)
	}

	disc, err := Scan(cwd)
	if err != nil {
		t.Fatalf("Scan() error = %v", err)
	}

	count := 0
	for _, s := range disc.MCPServers {
		if s.Name == "shared-srv" {
			count++
		}
	}
	if count != 1 {
		t.Errorf("shared-srv appears %d times, want 1 (dedup across files)", count)
	}
}

func TestScan_MCPSourceLabels(t *testing.T) {
	homeDir := t.TempDir()
	cwd := t.TempDir()
	t.Setenv("HOME", homeDir)

	claudeJSON := `{"mcpServers":{"user-srv":{"command":"node"}}}`
	if err := os.WriteFile(filepath.Join(homeDir, ".claude.json"), []byte(claudeJSON), 0644); err != nil {
		t.Fatal(err)
	}
	projMCP := `{"mcpServers":{"proj-srv":{"command":"python"}}}`
	if err := os.WriteFile(filepath.Join(cwd, ".mcp.json"), []byte(projMCP), 0644); err != nil {
		t.Fatal(err)
	}

	disc, err := Scan(cwd)
	if err != nil {
		t.Fatalf("Scan() error = %v", err)
	}

	srcMap := make(map[string]string)
	for _, s := range disc.MCPServers {
		srcMap[s.Name] = s.Source
	}
	if srcMap["user-srv"] != "user" {
		t.Errorf("user-srv source = %q, want user (not a file path)", srcMap["user-srv"])
	}
	if srcMap["proj-srv"] != "project" {
		t.Errorf("proj-srv source = %q, want project (not a file path)", srcMap["proj-srv"])
	}
}

func TestScan_MCPDeduplicatesAcrossFiles(t *testing.T) {
	homeDir := t.TempDir()
	cwd := t.TempDir()
	t.Setenv("HOME", homeDir)

	claudeJSON := `{"mcpServers":{"shared-srv":{"command":"node"}}}`
	if err := os.WriteFile(filepath.Join(homeDir, ".claude.json"), []byte(claudeJSON), 0644); err != nil {
		t.Fatal(err)
	}
	projMCP := `{"mcpServers":{"shared-srv":{"command":"python"}}}`
	if err := os.WriteFile(filepath.Join(cwd, ".mcp.json"), []byte(projMCP), 0644); err != nil {
		t.Fatal(err)
	}

	disc, err := Scan(cwd)
	if err != nil {
		t.Fatalf("Scan() error = %v", err)
	}

	if len(disc.MCPServers) != 1 {
		t.Errorf("MCPServers len = %d, want 1 (dedup across files)", len(disc.MCPServers))
	}
}

func TestScan_LegacyUserMCPStillWorks(t *testing.T) {
	homeDir := t.TempDir()
	cwd := t.TempDir()
	t.Setenv("HOME", homeDir)

	legacyDir := filepath.Join(homeDir, ".claude")
	if err := os.MkdirAll(legacyDir, 0755); err != nil {
		t.Fatal(err)
	}
	legacyMCP := `{"mcpServers":{"legacy-srv":{"command":"node"}}}`
	if err := os.WriteFile(filepath.Join(legacyDir, ".mcp.json"), []byte(legacyMCP), 0644); err != nil {
		t.Fatal(err)
	}

	disc, err := Scan(cwd)
	if err != nil {
		t.Fatalf("Scan() error = %v", err)
	}

	srcMap := make(map[string]string)
	for _, s := range disc.MCPServers {
		srcMap[s.Name] = s.Source
	}
	if srcMap["legacy-srv"] != "user" {
		t.Errorf("legacy-srv source = %q, want user", srcMap["legacy-srv"])
	}
}
