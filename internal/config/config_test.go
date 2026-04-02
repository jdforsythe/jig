package config

import (
	"os"
	"path/filepath"
	"testing"

	"gopkg.in/yaml.v3"
	"strings"
)

func TestValidate(t *testing.T) {
	tests := []struct {
		name    string
		profile Profile
		wantErr bool
	}{
		{
			name:    "valid minimal",
			profile: Profile{Name: "test"},
			wantErr: false,
		},
		{
			name:    "empty name",
			profile: Profile{},
			wantErr: true,
		},
		{
			name:    "invalid name format",
			profile: Profile{Name: "Test_Profile"},
			wantErr: true,
		},
		{
			name:    "valid full profile",
			profile: Profile{Name: "code-review", Model: "opus", Effort: "high", PermissionMode: "plan"},
			wantErr: false,
		},
		{
			name:    "invalid model",
			profile: Profile{Name: "test", Model: "gpt-4"},
			wantErr: true,
		},
		{
			name:    "invalid effort",
			profile: Profile{Name: "test", Effort: "extreme"},
			wantErr: true,
		},
		{
			name:    "invalid permission mode",
			profile: Profile{Name: "test", PermissionMode: "yolo"},
			wantErr: true,
		},
		{
			name: "mcp server with ref",
			profile: Profile{
				Name:       "test",
				MCPServers: []MCPServerEntry{{Ref: "github"}},
			},
			wantErr: false,
		},
		{
			name: "mcp server inline",
			profile: Profile{
				Name:       "test",
				MCPServers: []MCPServerEntry{{Name: "db", Command: "npx", Args: []string{"@mcp/sqlite"}}},
			},
			wantErr: false,
		},
		{
			name: "mcp server missing ref and name",
			profile: Profile{
				Name:       "test",
				MCPServers: []MCPServerEntry{{}},
			},
			wantErr: true,
		},
		{
			name: "mcp server ref and command conflict",
			profile: Profile{
				Name:       "test",
				MCPServers: []MCPServerEntry{{Ref: "github", Command: "npx"}},
			},
			wantErr: true,
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			err := Validate(&tt.profile)
			if (err != nil) != tt.wantErr {
				t.Errorf("Validate() error = %v, wantErr %v", err, tt.wantErr)
			}
		})
	}
}

func TestYAMLRoundTrip(t *testing.T) {
	original := Profile{
		Name:               "code-review",
		Description:        "Code review mode",
		Extends:            "base",
		Model:              "opus",
		Effort:             "high",
		PermissionMode:     "plan",
		AppendSystemPrompt: "Focus on security.\n",
		AllowedTools:       []string{"Bash(git:*)", "Read"},
		DisallowedTools:    []string{"Bash(rm:*)"},
		MCPServers: []MCPServerEntry{
			{Ref: "github"},
			{Name: "custom-db", Command: "npx", Args: []string{"@mcp/sqlite"}, Env: map[string]string{"DB_PATH": "./data.db"}},
		},
		Skills:   []PathEntry{{Path: "~/.claude/skills/code-review"}},
		Agents:   []PathEntry{{Path: "~/.claude/agents/reviewer.md"}},
		Commands: []PathEntry{{Path: "./commands/deploy.md"}},
	}

	data, err := yaml.Marshal(&original)
	if err != nil {
		t.Fatalf("Marshal: %v", err)
	}

	var parsed Profile
	if err := yaml.Unmarshal(data, &parsed); err != nil {
		t.Fatalf("Unmarshal: %v", err)
	}

	if parsed.Name != original.Name {
		t.Errorf("Name: got %q, want %q", parsed.Name, original.Name)
	}
	if parsed.Model != original.Model {
		t.Errorf("Model: got %q, want %q", parsed.Model, original.Model)
	}
	if len(parsed.MCPServers) != len(original.MCPServers) {
		t.Errorf("MCPServers: got %d, want %d", len(parsed.MCPServers), len(original.MCPServers))
	}
	if len(parsed.AllowedTools) != len(original.AllowedTools) {
		t.Errorf("AllowedTools: got %d, want %d", len(parsed.AllowedTools), len(original.AllowedTools))
	}
}

func TestMergeProfiles(t *testing.T) {
	parent := &Profile{
		Name:           "base",
		Model:          "sonnet",
		Effort:         "medium",
		PermissionMode: "default",
		AllowedTools:   []string{"Read", "Grep"},
		Settings:       map[string]any{"key1": "val1", "key2": "val2"},
	}

	child := &Profile{
		Name:         "child",
		Extends:      "base",
		Model:        "opus",
		AllowedTools: []string{"Read", "Grep", "Bash(git:*)"},
		Settings:     map[string]any{"key2": "override", "key3": "val3"},
	}

	merged := mergeProfiles(parent, child)

	if merged.Name != "child" {
		t.Errorf("Name: got %q, want %q", merged.Name, "child")
	}
	if merged.Model != "opus" {
		t.Errorf("Model: got %q, want %q", merged.Model, "opus")
	}
	if merged.Effort != "medium" {
		t.Errorf("Effort: got %q, want %q (inherited)", merged.Effort, "medium")
	}
	if merged.Extends != "" {
		t.Errorf("Extends: got %q, want empty (resolved)", merged.Extends)
	}
	if len(merged.AllowedTools) != 3 {
		t.Errorf("AllowedTools: got %d, want 3 (child replaces)", len(merged.AllowedTools))
	}
	if v, ok := merged.Settings["key1"]; !ok || v != "val1" {
		t.Errorf("Settings[key1]: got %v, want val1 (inherited)", v)
	}
	if v, ok := merged.Settings["key2"]; !ok || v != "override" {
		t.Errorf("Settings[key2]: got %v, want override (child wins)", v)
	}
	if v, ok := merged.Settings["key3"]; !ok || v != "val3" {
		t.Errorf("Settings[key3]: got %v, want val3 (child adds)", v)
	}
}

func TestLoadAndSaveProfile(t *testing.T) {
	dir := t.TempDir()
	profilesDir := filepath.Join(dir, ".jig", "profiles")

	p := &Profile{
		Name:        "test-save",
		Description: "Test save profile",
		Model:       "opus",
	}

	// Save
	if err := EnsureDir(profilesDir); err != nil {
		t.Fatal(err)
	}
	path := ProfilePath(profilesDir, p.Name)
	if err := atomicWriteYAML(path, p); err != nil {
		t.Fatalf("atomicWriteYAML: %v", err)
	}

	// Verify file exists
	if _, err := os.Stat(path); err != nil {
		t.Fatalf("file not found: %v", err)
	}

	// Load
	loaded, err := loadFromFile(path, SourceProject)
	if err != nil {
		t.Fatalf("loadFromFile: %v", err)
	}
	if loaded.Name != "test-save" {
		t.Errorf("Name: got %q, want %q", loaded.Name, "test-save")
	}
	if loaded.Model != "opus" {
		t.Errorf("Model: got %q, want %q", loaded.Model, "opus")
	}
}

func TestDefaults(t *testing.T) {
	d := DefaultProfile()
	if d.Effort != "high" {
		t.Errorf("default Effort: got %q, want %q", d.Effort, "high")
	}
	if d.PermissionMode != "default" {
		t.Errorf("default PermissionMode: got %q, want %q", d.PermissionMode, "default")
	}
}

func TestPaths(t *testing.T) {
	cwd := "/tmp/testproject"

	projDir := ProjectDir(cwd)
	if projDir != "/tmp/testproject/.jig" {
		t.Errorf("ProjectDir: got %q", projDir)
	}

	projProfiles := ProjectProfilesDir(cwd)
	if projProfiles != "/tmp/testproject/.jig/profiles" {
		t.Errorf("ProjectProfilesDir: got %q", projProfiles)
	}

	shortcut := ShortcutPath(cwd)
	if shortcut != "/tmp/testproject/.jig.yaml" {
		t.Errorf("ShortcutPath: got %q", shortcut)
	}

	profilePath := ProfilePath("/some/dir", "my-profile")
	if profilePath != "/some/dir/my-profile.yaml" {
		t.Errorf("ProfilePath: got %q", profilePath)
	}
}

// TestValidateComponentName_AcceptsLegitimate tests the validateComponentName helper
func TestValidateComponentName_AcceptsLegitimate(t *testing.T) {
	tests := []string{
		"my-agent",
		"skill-v2",
		"component-a",
		"name-with-dashes",
	}
	for _, name := range tests {
		err := validateComponentName(name)
		if err != nil {
			t.Errorf("validateComponentName(%q) should accept, got error: %v", name, err)
		}
	}
}

func TestValidateComponentName_RejectsEmpty(t *testing.T) {
	err := validateComponentName("")
	if err == nil {
		t.Error("validateComponentName(\"\") should reject empty string")
	}
}

func TestValidateComponentName_RejectsDot(t *testing.T) {
	err := validateComponentName(".")
	if err == nil {
		t.Error("validateComponentName(\".\") should reject dot")
	}
}

func TestValidateComponentName_RejectsDotDot(t *testing.T) {
	err := validateComponentName("..")
	if err == nil {
		t.Error("validateComponentName(\"..\") should reject dotdot")
	}
}

func TestValidateComponentName_RejectsAbsolutePath(t *testing.T) {
	tests := []string{
		"/etc/passwd",
		"/absolute/path",
	}
	for _, name := range tests {
		err := validateComponentName(name)
		if err == nil {
			t.Errorf("validateComponentName(%q) should reject absolute path", name)
		}
	}
}

func TestValidateComponentName_RejectsPathSeparators(t *testing.T) {
	tests := []string{
		"../../../etc/passwd",
		"subdir/agent",
		"agent/../../../etc/item",
		"./relative/path",
	}
	for _, name := range tests {
		err := validateComponentName(name)
		if err == nil {
			t.Errorf("validateComponentName(%q) should reject path separators", name)
		}
	}
}

func TestValidate_RejectsAbsoluteHookScriptDest(t *testing.T) {
	p := &Profile{
		Name: "test",
		HookScripts: []HookScript{
			{Dest: "/etc/passwd", Path: "somescript.sh"},
		},
	}
	err := Validate(p)
	if err == nil {
		t.Error("Validate should reject absolute hook_scripts destination")
	}
}

func TestValidate_RejectsTraversalHookScriptDest(t *testing.T) {
	p := &Profile{
		Name: "test",
		HookScripts: []HookScript{
			{Dest: "../../outside/directory/file", Path: "somescript.sh"},
		},
	}
	err := Validate(p)
	if err == nil {
		t.Error("Validate should reject traversal in hook_scripts destination")
	}
}

func TestValidate_AcceptsLegitimateHookScriptDest(t *testing.T) {
	p := &Profile{
		Name: "test",
		HookScripts: []HookScript{
			{Dest: "hooks/validate.sh", Path: "scripts/validate.sh"},
			{Dest: "script.sh", Path: "scripts/other.sh"},
		},
	}
	err := Validate(p)
	if err != nil && strings.Contains(err.Error(), "hook_scripts") {
		t.Errorf("Validate should accept legitimate hook_scripts destination: %v", err)
	}
}

func TestValidate_RejectsInvalidPluginComponentName(t *testing.T) {
	p := &Profile{
		Name: "test",
		PluginComponents: map[string]PluginComponentSelection{
			"plugin1": {
				Agents: []string{"../../../etc/passwd"},
			},
		},
	}
	err := Validate(p)
	if err == nil {
		t.Error("Validate should reject invalid plugin component name in agents")
	}
}

func TestValidate_AcceptsValidPluginComponentName(t *testing.T) {
	p := &Profile{
		Name: "test",
		PluginComponents: map[string]PluginComponentSelection{
			"plugin1": {
				Agents:   []string{"reviewer-agent"},
				Skills:   []string{"code-analyzer"},
				Commands: []string{"validate"},
			},
		},
	}
	err := Validate(p)
	if err != nil && strings.Contains(err.Error(), "plugin_components") {
		t.Errorf("Validate should accept valid plugin component names: %v", err)
	}
}
