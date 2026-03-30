package claude

import (
	"os"
	"path/filepath"
	"testing"
)

func TestBuildMCPIndex_GlobalAndProject(t *testing.T) {
	homeDir := t.TempDir()
	cwd := t.TempDir()
	t.Setenv("HOME", homeDir)

	// Global MCP config
	globalMCP := `{"mcpServers":{"global-server":{"command":"node","args":["global.js"]},"shared-server":{"command":"node","args":["global-shared.js"]}}}`
	globalMCPPath := filepath.Join(homeDir, ".claude", ".mcp.json")
	if err := os.MkdirAll(filepath.Dir(globalMCPPath), 0755); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(globalMCPPath, []byte(globalMCP), 0644); err != nil {
		t.Fatal(err)
	}

	// Project MCP config (overrides shared-server)
	projMCP := `{"mcpServers":{"project-server":{"command":"node","args":["project.js"]},"shared-server":{"command":"python","args":["project-shared.py"]}}}`
	if err := os.WriteFile(filepath.Join(cwd, ".mcp.json"), []byte(projMCP), 0644); err != nil {
		t.Fatal(err)
	}

	index, err := BuildMCPIndex(cwd)
	if err != nil {
		t.Fatalf("BuildMCPIndex() error = %v", err)
	}

	// Both global-only and project-only servers should be present
	if _, ok := index.Resolve("global-server"); !ok {
		t.Error("global-server not found in index")
	}
	if _, ok := index.Resolve("project-server"); !ok {
		t.Error("project-server not found in index")
	}

	// Project overrides global for shared-server
	shared, ok := index.Resolve("shared-server")
	if !ok {
		t.Fatal("shared-server not found in index")
	}
	if shared.Command != "python" {
		t.Errorf("shared-server.Command = %q, want python (project overrides global)", shared.Command)
	}
}

func TestBuildMCPIndex_GlobalOnly(t *testing.T) {
	homeDir := t.TempDir()
	cwd := t.TempDir()
	t.Setenv("HOME", homeDir)

	globalMCPPath := filepath.Join(homeDir, ".claude", ".mcp.json")
	if err := os.MkdirAll(filepath.Dir(globalMCPPath), 0755); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(globalMCPPath, []byte(`{"mcpServers":{"my-server":{"command":"node"}}}`), 0644); err != nil {
		t.Fatal(err)
	}

	index, err := BuildMCPIndex(cwd)
	if err != nil {
		t.Fatalf("BuildMCPIndex() error = %v", err)
	}

	if _, ok := index.Resolve("my-server"); !ok {
		t.Error("my-server not found in index from global config")
	}
}

func TestBuildMCPIndex_NeitherExists(t *testing.T) {
	homeDir := t.TempDir()
	cwd := t.TempDir()
	t.Setenv("HOME", homeDir)

	index, err := BuildMCPIndex(cwd)
	if err != nil {
		t.Fatalf("BuildMCPIndex() error = %v (should succeed with empty index)", err)
	}
	if len(index.Servers) != 0 {
		t.Errorf("expected empty index, got %v", index.Servers)
	}
}

func TestMCPIndex_ResolveNotFound(t *testing.T) {
	index := &MCPIndex{Servers: map[string]MCPServerDef{
		"existing": {Command: "node"},
	}}

	_, ok := index.Resolve("nonexistent")
	if ok {
		t.Error("Resolve() should return false for nonexistent server")
	}
}

func TestBuildMCPIndex_MalformedJSON(t *testing.T) {
	cwd := t.TempDir()

	// Malformed project .mcp.json — should be skipped, not fail
	if err := os.WriteFile(filepath.Join(cwd, ".mcp.json"), []byte("{not valid json"), 0644); err != nil {
		t.Fatal(err)
	}

	index, err := BuildMCPIndex(cwd)
	if err != nil {
		t.Fatalf("BuildMCPIndex() error = %v (malformed file should be skipped)", err)
	}
	if len(index.Servers) != 0 {
		t.Errorf("expected empty index after malformed file, got %v", index.Servers)
	}
}

func TestBuildMCPIndex_EnvVars(t *testing.T) {
	cwd := t.TempDir()

	mcpData := `{"mcpServers":{"db":{"command":"npx","args":["@mcp/sqlite"],"env":{"DB_PATH":"./data.db","SECRET":"abc"}}}}`
	if err := os.WriteFile(filepath.Join(cwd, ".mcp.json"), []byte(mcpData), 0644); err != nil {
		t.Fatal(err)
	}

	index, err := BuildMCPIndex(cwd)
	if err != nil {
		t.Fatalf("BuildMCPIndex() error = %v", err)
	}

	def, ok := index.Resolve("db")
	if !ok {
		t.Fatal("db not found")
	}
	if def.Env["DB_PATH"] != "./data.db" {
		t.Errorf("DB_PATH = %q, want ./data.db", def.Env["DB_PATH"])
	}
	if def.Env["SECRET"] != "abc" {
		t.Errorf("SECRET = %q, want abc", def.Env["SECRET"])
	}
}
