package scanner

import (
	"os"
	"path/filepath"
	"testing"
)

func TestScan_EmptyDirs(t *testing.T) {
	homeDir := t.TempDir()
	cwd := t.TempDir()
	t.Setenv("HOME", homeDir)

	disc, err := Scan(cwd)
	if err != nil {
		t.Fatalf("Scan() error = %v", err)
	}
	if disc == nil {
		t.Fatal("Scan() returned nil")
	}
	// No resources expected (dirs don't exist)
	if len(disc.Skills) != 0 {
		t.Errorf("Skills: got %d, want 0", len(disc.Skills))
	}
	if len(disc.Agents) != 0 {
		t.Errorf("Agents: got %d, want 0", len(disc.Agents))
	}
	if len(disc.Commands) != 0 {
		t.Errorf("Commands: got %d, want 0", len(disc.Commands))
	}
}

func TestScan_SourceField(t *testing.T) {
	homeDir := t.TempDir()
	cwd := t.TempDir()
	t.Setenv("HOME", homeDir)

	// Create a user agent
	userAgents := filepath.Join(homeDir, ".claude", "agents")
	if err := os.MkdirAll(userAgents, 0755); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(filepath.Join(userAgents, "user-agent.md"), []byte("user agent"), 0644); err != nil {
		t.Fatal(err)
	}

	// Create a project agent
	projAgents := filepath.Join(cwd, ".claude", "agents")
	if err := os.MkdirAll(projAgents, 0755); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(filepath.Join(projAgents, "proj-agent.md"), []byte("proj agent"), 0644); err != nil {
		t.Fatal(err)
	}

	disc, err := Scan(cwd)
	if err != nil {
		t.Fatalf("Scan() error = %v", err)
	}

	sourceMap := make(map[string]string)
	for _, a := range disc.Agents {
		sourceMap[a.Name] = a.Source
	}

	if sourceMap["user-agent"] != "user" {
		t.Errorf("user-agent source = %q, want %q", sourceMap["user-agent"], "user")
	}
	if sourceMap["proj-agent"] != "project" {
		t.Errorf("proj-agent source = %q, want %q", sourceMap["proj-agent"], "project")
	}
}

func TestScan_SkillsAndCommands(t *testing.T) {
	homeDir := t.TempDir()
	cwd := t.TempDir()
	t.Setenv("HOME", homeDir)

	// Create a user skill
	userSkills := filepath.Join(homeDir, ".claude", "skills")
	if err := os.MkdirAll(filepath.Join(userSkills, "my-skill"), 0755); err != nil {
		t.Fatal(err)
	}

	// Create a project command
	projCommands := filepath.Join(cwd, ".claude", "commands")
	if err := os.MkdirAll(projCommands, 0755); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(filepath.Join(projCommands, "deploy.md"), []byte("deploy"), 0644); err != nil {
		t.Fatal(err)
	}

	disc, err := Scan(cwd)
	if err != nil {
		t.Fatalf("Scan() error = %v", err)
	}

	if len(disc.Skills) != 1 || disc.Skills[0].Name != "my-skill" {
		t.Errorf("Skills = %v, want [{my-skill ...}]", disc.Skills)
	}
	if disc.Skills[0].Source != "user" {
		t.Errorf("skill source = %q, want user", disc.Skills[0].Source)
	}

	if len(disc.Commands) != 1 || disc.Commands[0].Name != "deploy" {
		t.Errorf("Commands = %v, want [{deploy ...}]", disc.Commands)
	}
	if disc.Commands[0].Source != "project" {
		t.Errorf("command source = %q, want project", disc.Commands[0].Source)
	}
}

func TestScan_DeduplicatesAcrossSources(t *testing.T) {
	homeDir := t.TempDir()
	cwd := t.TempDir()
	t.Setenv("HOME", homeDir)

	// Same agent name in both user and project dirs
	userAgents := filepath.Join(homeDir, ".claude", "agents")
	projAgents := filepath.Join(cwd, ".claude", "agents")
	for _, dir := range []string{userAgents, projAgents} {
		if err := os.MkdirAll(dir, 0755); err != nil {
			t.Fatal(err)
		}
		if err := os.WriteFile(filepath.Join(dir, "shared.md"), []byte("agent"), 0644); err != nil {
			t.Fatal(err)
		}
	}

	disc, err := Scan(cwd)
	if err != nil {
		t.Fatalf("Scan() error = %v", err)
	}

	// Should only appear once (user version wins since it's scanned first)
	if len(disc.Agents) != 1 {
		t.Errorf("Agents len = %d, want 1 (deduplication)", len(disc.Agents))
	}
	if disc.Agents[0].Source != "user" {
		t.Errorf("Agents[0].Source = %q, want user (user wins in dedup)", disc.Agents[0].Source)
	}
}

func TestScanResources_Sources(t *testing.T) {
	dir1 := t.TempDir()
	dir2 := t.TempDir()

	if err := os.WriteFile(filepath.Join(dir1, "a.md"), []byte("a"), 0644); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(filepath.Join(dir2, "b.md"), []byte("b"), 0644); err != nil {
		t.Fatal(err)
	}

	resources := scanResources("agent", []string{dir1, dir2}, []string{"user", "project"})

	if len(resources) != 2 {
		t.Fatalf("len = %d, want 2", len(resources))
	}

	srcMap := make(map[string]string)
	for _, r := range resources {
		srcMap[r.Name] = r.Source
	}
	if srcMap["a"] != "user" {
		t.Errorf("a source = %q, want user", srcMap["a"])
	}
	if srcMap["b"] != "project" {
		t.Errorf("b source = %q, want project", srcMap["b"])
	}
}

func TestExtractMCPServerNames(t *testing.T) {
	json := `{"mcpServers":{"server-a":{},"server-b":{}}}`
	names := extractMCPServerNames(json)

	nameSet := make(map[string]bool)
	for _, n := range names {
		nameSet[n] = true
	}

	if !nameSet["server-a"] || !nameSet["server-b"] {
		t.Errorf("extractMCPServerNames = %v, want [server-a server-b]", names)
	}
}
