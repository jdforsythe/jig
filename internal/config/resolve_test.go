package config

import (
	"testing"
)

func TestResolve_EndToEnd(t *testing.T) {
	homeDir := t.TempDir()
	cwd := t.TempDir()
	t.Setenv("HOME", homeDir)

	projDir := ProjectProfilesDir(cwd)
	writeProfile(t, projDir, "e2e-test", Profile{
		Name:        "e2e-test",
		Description: "end to end test",
		Model:       "sonnet",
	})

	resolved, err := Resolve("e2e-test", cwd, nil)
	if err != nil {
		t.Fatalf("Resolve() error = %v", err)
	}

	if resolved.Model != "sonnet" {
		t.Errorf("Model = %q, want sonnet", resolved.Model)
	}
	// Defaults applied
	if resolved.Effort != "high" {
		t.Errorf("Effort = %q, want high (default)", resolved.Effort)
	}
	if resolved.PermissionMode != "default" {
		t.Errorf("PermissionMode = %q, want default (default)", resolved.PermissionMode)
	}
}

func TestResolve_WithOverrides(t *testing.T) {
	homeDir := t.TempDir()
	cwd := t.TempDir()
	t.Setenv("HOME", homeDir)

	projDir := ProjectProfilesDir(cwd)
	writeProfile(t, projDir, "override-test", Profile{
		Name:  "override-test",
		Model: "sonnet",
	})

	overrides := &Profile{Model: "opus", Effort: "max"}
	resolved, err := Resolve("override-test", cwd, overrides)
	if err != nil {
		t.Fatalf("Resolve() error = %v", err)
	}

	if resolved.Model != "opus" {
		t.Errorf("Model = %q, want opus (override applied)", resolved.Model)
	}
	if resolved.Effort != "max" {
		t.Errorf("Effort = %q, want max (override applied)", resolved.Effort)
	}
}

func TestResolve_NilOverrides(t *testing.T) {
	homeDir := t.TempDir()
	cwd := t.TempDir()
	t.Setenv("HOME", homeDir)

	projDir := ProjectProfilesDir(cwd)
	writeProfile(t, projDir, "nil-override", Profile{
		Name:  "nil-override",
		Model: "haiku",
	})

	resolved, err := Resolve("nil-override", cwd, nil)
	if err != nil {
		t.Fatalf("Resolve() error = %v", err)
	}
	if resolved.Model != "haiku" {
		t.Errorf("Model = %q, want haiku (nil overrides should be no-op)", resolved.Model)
	}
}

func TestResolve_NotFound(t *testing.T) {
	homeDir := t.TempDir()
	cwd := t.TempDir()
	t.Setenv("HOME", homeDir)

	_, err := Resolve("does-not-exist", cwd, nil)
	if err == nil {
		t.Error("Resolve() expected error for missing profile, got nil")
	}
}

func TestResolve_WithInheritance(t *testing.T) {
	homeDir := t.TempDir()
	cwd := t.TempDir()
	t.Setenv("HOME", homeDir)

	projDir := ProjectProfilesDir(cwd)
	writeProfile(t, projDir, "parent", Profile{
		Name:   "parent",
		Effort: "medium",
		Model:  "sonnet",
	})
	writeProfile(t, projDir, "child", Profile{
		Name:    "child",
		Extends: "parent",
		Model:   "opus",
	})

	resolved, err := Resolve("child", cwd, nil)
	if err != nil {
		t.Fatalf("Resolve() error = %v", err)
	}

	if resolved.Model != "opus" {
		t.Errorf("Model = %q, want opus (child wins)", resolved.Model)
	}
	if resolved.Effort != "medium" {
		t.Errorf("Effort = %q, want medium (inherited from parent)", resolved.Effort)
	}
	if resolved.Extends != "" {
		t.Errorf("Extends = %q, want empty (resolved away)", resolved.Extends)
	}
}

func TestResolve_FailsValidation(t *testing.T) {
	homeDir := t.TempDir()
	cwd := t.TempDir()
	t.Setenv("HOME", homeDir)

	projDir := ProjectProfilesDir(cwd)
	// Profile with invalid model will fail validation after loading
	writeProfile(t, projDir, "invalid-model", Profile{
		Name:  "invalid-model",
		Model: "gpt-4",
	})

	_, err := Resolve("invalid-model", cwd, nil)
	if err == nil {
		t.Error("Resolve() expected error for invalid model, got nil")
	}
}

func TestApplyOverrides_AllFields(t *testing.T) {
	base := &Profile{
		Model:              "sonnet",
		Effort:             "low",
		PermissionMode:     "default",
		SystemPrompt:       "original",
		AppendSystemPrompt: "original-append",
		SessionAgent:       "original-agent",
		AllowedTools:       []string{"Read"},
		DisallowedTools:    []string{"Bash"},
		ExtraFlags:         []string{"--old"},
	}

	overrides := &Profile{
		Model:              "opus",
		Effort:             "max",
		PermissionMode:     "plan",
		SystemPrompt:       "new-prompt",
		AppendSystemPrompt: "new-append",
		SessionAgent:       "new-agent",
		AllowedTools:       []string{"Read", "Write"},
		DisallowedTools:    []string{"Edit"},
		ExtraFlags:         []string{"--new"},
	}

	result := applyOverrides(base, overrides)

	checks := []struct {
		field string
		got   string
		want  string
	}{
		{"Model", result.Model, "opus"},
		{"Effort", result.Effort, "max"},
		{"PermissionMode", result.PermissionMode, "plan"},
		{"SystemPrompt", result.SystemPrompt, "new-prompt"},
		{"AppendSystemPrompt", result.AppendSystemPrompt, "new-append"},
		{"SessionAgent", result.SessionAgent, "new-agent"},
	}

	for _, c := range checks {
		if c.got != c.want {
			t.Errorf("%s = %q, want %q", c.field, c.got, c.want)
		}
	}

	if len(result.AllowedTools) != 2 {
		t.Errorf("AllowedTools len = %d, want 2", len(result.AllowedTools))
	}
	if len(result.DisallowedTools) != 1 || result.DisallowedTools[0] != "Edit" {
		t.Errorf("DisallowedTools = %v, want [Edit]", result.DisallowedTools)
	}
	if len(result.ExtraFlags) != 1 || result.ExtraFlags[0] != "--new" {
		t.Errorf("ExtraFlags = %v, want [--new]", result.ExtraFlags)
	}
}

func TestApplyOverrides_ZeroValuesNoOp(t *testing.T) {
	base := &Profile{
		Model:  "opus",
		Effort: "high",
	}
	overrides := &Profile{} // all zero

	result := applyOverrides(base, overrides)

	if result.Model != "opus" {
		t.Errorf("Model = %q, zero override should not change value", result.Model)
	}
	if result.Effort != "high" {
		t.Errorf("Effort = %q, zero override should not change value", result.Effort)
	}
}

func TestApplyDefaults(t *testing.T) {
	// Empty profile gets both defaults
	p := &Profile{Name: "test"}
	applyDefaults(p)
	if p.Effort != "high" {
		t.Errorf("Effort = %q, want high", p.Effort)
	}
	if p.PermissionMode != "default" {
		t.Errorf("PermissionMode = %q, want default", p.PermissionMode)
	}
}

func TestApplyDefaults_DoesNotOverrideSet(t *testing.T) {
	p := &Profile{Name: "test", Effort: "low", PermissionMode: "plan"}
	applyDefaults(p)
	if p.Effort != "low" {
		t.Errorf("Effort = %q, should not be overridden by default", p.Effort)
	}
	if p.PermissionMode != "plan" {
		t.Errorf("PermissionMode = %q, should not be overridden by default", p.PermissionMode)
	}
}

func TestMergeProfiles_EnabledPlugins(t *testing.T) {
	parent := &Profile{
		Name: "parent",
		EnabledPlugins: map[string]bool{
			"forge@market": true,
		},
	}
	child := &Profile{
		Name: "child",
		EnabledPlugins: map[string]bool{
			"ss-engineering@market": true,
			"forge@market":          false, // child overrides
		},
	}

	merged := mergeProfiles(parent, child)

	if merged.EnabledPlugins["forge@market"] != false {
		t.Error("forge@market should be false (child overrides parent)")
	}
	if merged.EnabledPlugins["ss-engineering@market"] != true {
		t.Error("ss-engineering@market should be true (child adds)")
	}
}

func TestMergeProfiles_EnabledPlugins_NilParent(t *testing.T) {
	parent := &Profile{Name: "parent"}
	child := &Profile{
		Name:           "child",
		EnabledPlugins: map[string]bool{"forge@market": true},
	}

	merged := mergeProfiles(parent, child)
	if !merged.EnabledPlugins["forge@market"] {
		t.Error("child's enabled plugin should be present when parent has nil map")
	}
}

func TestMergeProfiles_EnabledPlugins_NilChild(t *testing.T) {
	parent := &Profile{
		Name:           "parent",
		EnabledPlugins: map[string]bool{"forge@market": true},
	}
	child := &Profile{Name: "child"}

	merged := mergeProfiles(parent, child)
	if !merged.EnabledPlugins["forge@market"] {
		t.Error("parent's enabled plugin should be inherited when child has nil map")
	}
}

func TestMergeProfiles_PluginComponents(t *testing.T) {
	parent := &Profile{
		Name: "parent",
		PluginComponents: map[string]PluginComponentSelection{
			"forge@market": {Agents: []string{"librarian"}},
		},
	}
	child := &Profile{
		Name: "child",
		PluginComponents: map[string]PluginComponentSelection{
			"ss-engineering@market": {Agents: []string{"repo-research-analyst"}},
			"forge@market":          {Agents: []string{"mission-planner"}}, // replaces parent for this key
		},
	}

	merged := mergeProfiles(parent, child)

	forge := merged.PluginComponents["forge@market"]
	if len(forge.Agents) != 1 || forge.Agents[0] != "mission-planner" {
		t.Errorf("forge agents = %v, want [mission-planner] (child replaces)", forge.Agents)
	}

	ss := merged.PluginComponents["ss-engineering@market"]
	if len(ss.Agents) != 1 || ss.Agents[0] != "repo-research-analyst" {
		t.Errorf("ss agents = %v, want [repo-research-analyst]", ss.Agents)
	}
}

func TestMergeProfiles_PluginComponents_BothNil(t *testing.T) {
	parent := &Profile{Name: "parent"}
	child := &Profile{Name: "child"}

	merged := mergeProfiles(parent, child)
	if merged.PluginComponents != nil {
		t.Error("PluginComponents should be nil when both parent and child have nil")
	}
}

func TestValidConstants(t *testing.T) {
	// Ensure valid values match what Claude Code actually accepts
	wantEfforts := map[string]bool{"low": true, "medium": true, "high": true, "max": true}
	for _, e := range ValidEfforts {
		if !wantEfforts[e] {
			t.Errorf("unexpected effort value %q", e)
		}
	}
	if len(ValidEfforts) != 4 {
		t.Errorf("ValidEfforts has %d values, want 4", len(ValidEfforts))
	}

	wantModels := map[string]bool{"opus": true, "sonnet": true, "haiku": true}
	for _, m := range ValidModels {
		if !wantModels[m] {
			t.Errorf("unexpected model value %q", m)
		}
	}
	if len(ValidModels) != 3 {
		t.Errorf("ValidModels has %d values, want 3", len(ValidModels))
	}

	wantPerms := map[string]bool{"default": true, "plan": true, "autoaccept": true, "bypassPermissions": true}
	for _, p := range ValidPermissionModes {
		if !wantPerms[p] {
			t.Errorf("unexpected permission mode %q", p)
		}
	}
	if len(ValidPermissionModes) != 4 {
		t.Errorf("ValidPermissionModes has %d values, want 4", len(ValidPermissionModes))
	}
}
