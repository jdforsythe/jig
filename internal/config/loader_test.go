package config

import (
	"os"
	"path/filepath"
	"testing"

	"gopkg.in/yaml.v3"
)

// writeProfile writes a profile YAML to dir/<name>.yaml.
func writeProfile(t *testing.T, dir, name string, p Profile) {
	t.Helper()
	if err := os.MkdirAll(dir, 0755); err != nil {
		t.Fatal(err)
	}
	data, err := yaml.Marshal(p)
	if err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(filepath.Join(dir, name+".yaml"), data, 0644); err != nil {
		t.Fatal(err)
	}
}

func TestLoadProfile_ProjectFirst(t *testing.T) {
	homeDir := t.TempDir()
	cwd := t.TempDir()
	t.Setenv("HOME", homeDir)

	projDir := ProjectProfilesDir(cwd)
	globalDir, _ := GlobalProfilesDir()

	writeProfile(t, projDir, "shared", Profile{Name: "shared", Model: "opus"})
	writeProfile(t, globalDir, "shared", Profile{Name: "shared", Model: "sonnet"})

	p, err := LoadProfile("shared", cwd)
	if err != nil {
		t.Fatalf("LoadProfile() error = %v", err)
	}

	if p.Model != "opus" {
		t.Errorf("Model = %q, want opus (project wins over global)", p.Model)
	}
	if p.Source() != SourceProject {
		t.Errorf("Source = %v, want SourceProject", p.Source())
	}
}

func TestLoadProfile_FallsBackToGlobal(t *testing.T) {
	homeDir := t.TempDir()
	cwd := t.TempDir()
	t.Setenv("HOME", homeDir)

	globalDir, _ := GlobalProfilesDir()
	writeProfile(t, globalDir, "global-only", Profile{Name: "global-only", Model: "haiku"})

	p, err := LoadProfile("global-only", cwd)
	if err != nil {
		t.Fatalf("LoadProfile() error = %v", err)
	}
	if p.Model != "haiku" {
		t.Errorf("Model = %q, want haiku", p.Model)
	}
	if p.Source() != SourceGlobal {
		t.Errorf("Source = %v, want SourceGlobal", p.Source())
	}
}

func TestLoadProfile_Shortcut(t *testing.T) {
	homeDir := t.TempDir()
	cwd := t.TempDir()
	t.Setenv("HOME", homeDir)

	// Write shortcut file
	shortcutProfile := Profile{Name: "shortcut-name", Model: "sonnet"}
	data, _ := yaml.Marshal(shortcutProfile)
	if err := os.WriteFile(ShortcutPath(cwd), data, 0644); err != nil {
		t.Fatal(err)
	}

	p, err := LoadProfile("shortcut-name", cwd)
	if err != nil {
		t.Fatalf("LoadProfile() error = %v", err)
	}
	if p.Source() != SourceShortcut {
		t.Errorf("Source = %v, want SourceShortcut", p.Source())
	}
}

func TestLoadProfile_NotFound(t *testing.T) {
	homeDir := t.TempDir()
	cwd := t.TempDir()
	t.Setenv("HOME", homeDir)

	_, err := LoadProfile("nonexistent", cwd)
	if err == nil {
		t.Error("LoadProfile() expected error, got nil")
	}
}

func TestLoadProfile_FilePath(t *testing.T) {
	homeDir := t.TempDir()
	cwd := t.TempDir()
	t.Setenv("HOME", homeDir)

	projDir := ProjectProfilesDir(cwd)
	writeProfile(t, projDir, "my-prof", Profile{Name: "my-prof"})

	p, err := LoadProfile("my-prof", cwd)
	if err != nil {
		t.Fatal(err)
	}
	if p.FilePath() == "" {
		t.Error("FilePath() is empty, expected non-empty path")
	}
	if filepath.Base(p.FilePath()) != "my-prof.yaml" {
		t.Errorf("FilePath() = %q, expected to end in my-prof.yaml", p.FilePath())
	}
}

func TestListProfiles_ProjectWinsDedup(t *testing.T) {
	homeDir := t.TempDir()
	cwd := t.TempDir()
	t.Setenv("HOME", homeDir)

	projDir := ProjectProfilesDir(cwd)
	globalDir, _ := GlobalProfilesDir()

	// Same name in both scopes
	writeProfile(t, projDir, "shared", Profile{Name: "shared", Model: "opus"})
	writeProfile(t, globalDir, "shared", Profile{Name: "shared", Model: "sonnet"})
	// Also a global-only profile
	writeProfile(t, globalDir, "global-only", Profile{Name: "global-only", Model: "haiku"})

	profiles, err := ListProfiles(cwd)
	if err != nil {
		t.Fatalf("ListProfiles() error = %v", err)
	}

	byName := make(map[string]Profile)
	for _, p := range profiles {
		byName[p.Name] = p
	}

	shared, ok := byName["shared"]
	if !ok {
		t.Fatal("shared profile not found")
	}
	if shared.Model != "opus" {
		t.Errorf("shared.Model = %q, want opus (project wins)", shared.Model)
	}

	if _, ok := byName["global-only"]; !ok {
		t.Error("global-only profile should appear when not shadowed by project")
	}
}

func TestListProfiles_SkipsMalformed(t *testing.T) {
	homeDir := t.TempDir()
	cwd := t.TempDir()
	t.Setenv("HOME", homeDir)

	projDir := ProjectProfilesDir(cwd)
	if err := os.MkdirAll(projDir, 0755); err != nil {
		t.Fatal(err)
	}

	// Write a valid profile
	writeProfile(t, projDir, "valid", Profile{Name: "valid", Model: "opus"})

	// Write malformed YAML
	if err := os.WriteFile(filepath.Join(projDir, "bad.yaml"), []byte("this: {is: bad: yaml: :"), 0644); err != nil {
		t.Fatal(err)
	}

	// Write a directory (should be skipped)
	if err := os.MkdirAll(filepath.Join(projDir, "subdir.yaml"), 0755); err != nil {
		t.Fatal(err)
	}

	// Write a hidden file (should be skipped)
	if err := os.WriteFile(filepath.Join(projDir, ".gitkeep"), []byte(""), 0644); err != nil {
		t.Fatal(err)
	}

	profiles, err := ListProfiles(cwd)
	if err != nil {
		t.Fatalf("ListProfiles() error = %v", err)
	}

	if len(profiles) != 1 || profiles[0].Name != "valid" {
		t.Errorf("ListProfiles() = %v, want only [valid]", profiles)
	}
}

func TestListProfiles_ShortcutFile(t *testing.T) {
	homeDir := t.TempDir()
	cwd := t.TempDir()
	t.Setenv("HOME", homeDir)

	sc := Profile{Name: "shortcut-profile", Model: "sonnet"}
	data, _ := yaml.Marshal(sc)
	if err := os.WriteFile(ShortcutPath(cwd), data, 0644); err != nil {
		t.Fatal(err)
	}

	profiles, err := ListProfiles(cwd)
	if err != nil {
		t.Fatalf("ListProfiles() error = %v", err)
	}

	found := false
	for _, p := range profiles {
		if p.Name == "shortcut-profile" && p.Source() == SourceShortcut {
			found = true
		}
	}
	if !found {
		t.Error("shortcut profile not found in ListProfiles()")
	}
}

func TestListProfiles_Empty(t *testing.T) {
	homeDir := t.TempDir()
	cwd := t.TempDir()
	t.Setenv("HOME", homeDir)

	profiles, err := ListProfiles(cwd)
	if err != nil {
		t.Fatalf("ListProfiles() error = %v", err)
	}
	if len(profiles) != 0 {
		t.Errorf("ListProfiles() = %v, want empty list", profiles)
	}
}

func TestLoadProfilesFromDir_NonYAMLSkipped(t *testing.T) {
	dir := t.TempDir()

	writeProfile(t, dir, "good", Profile{Name: "good"})
	if err := os.WriteFile(filepath.Join(dir, "readme.txt"), []byte("not yaml"), 0644); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(filepath.Join(dir, "script.sh"), []byte("#!/bin/bash"), 0644); err != nil {
		t.Fatal(err)
	}

	profiles, err := loadProfilesFromDir(dir, SourceProject)
	if err != nil {
		t.Fatalf("loadProfilesFromDir() error = %v", err)
	}
	if len(profiles) != 1 || profiles[0].Name != "good" {
		t.Errorf("got %v, want only [good]", profiles)
	}
}
