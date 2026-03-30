package config

import (
	"os"
	"path/filepath"
	"testing"
)

func TestSaveProfile_ProjectScope(t *testing.T) {
	homeDir := t.TempDir()
	cwd := t.TempDir()
	t.Setenv("HOME", homeDir)

	p := &Profile{Name: "save-test", Model: "opus", Description: "saved profile"}

	if err := SaveProfile(p, cwd, false); err != nil {
		t.Fatalf("SaveProfile() error = %v", err)
	}

	expected := ProfilePath(ProjectProfilesDir(cwd), "save-test")
	if _, err := os.Stat(expected); err != nil {
		t.Fatalf("expected file %q to exist: %v", expected, err)
	}

	// Round-trip: reload and verify
	loaded, err := loadFromFile(expected, SourceProject)
	if err != nil {
		t.Fatalf("loadFromFile() error = %v", err)
	}
	if loaded.Name != "save-test" || loaded.Model != "opus" {
		t.Errorf("loaded profile mismatch: %+v", loaded)
	}
}

func TestSaveProfile_GlobalScope(t *testing.T) {
	homeDir := t.TempDir()
	cwd := t.TempDir()
	t.Setenv("HOME", homeDir)

	p := &Profile{Name: "global-save", Model: "haiku"}

	if err := SaveProfile(p, cwd, true); err != nil {
		t.Fatalf("SaveProfile(global) error = %v", err)
	}

	globalDir, _ := GlobalProfilesDir()
	expected := ProfilePath(globalDir, "global-save")
	if _, err := os.Stat(expected); err != nil {
		t.Fatalf("expected file %q to exist: %v", expected, err)
	}
}

func TestSaveProfile_CreatesDirectories(t *testing.T) {
	homeDir := t.TempDir()
	cwd := t.TempDir()
	t.Setenv("HOME", homeDir)

	// Project profiles dir does NOT exist yet
	projDir := ProjectProfilesDir(cwd)
	if _, err := os.Stat(projDir); !os.IsNotExist(err) {
		t.Skip("profiles dir already exists, skipping mkdir test")
	}

	p := &Profile{Name: "mkdirtest", Model: "opus"}
	if err := SaveProfile(p, cwd, false); err != nil {
		t.Fatalf("SaveProfile() error = %v", err)
	}

	if _, err := os.Stat(projDir); err != nil {
		t.Error("SaveProfile() should have created the profiles directory")
	}
}

func TestSaveProfile_AtomicWrite(t *testing.T) {
	homeDir := t.TempDir()
	cwd := t.TempDir()
	t.Setenv("HOME", homeDir)

	// Write once, then overwrite — both should succeed and leave no temp files
	p := &Profile{Name: "atomic", Model: "sonnet"}
	if err := SaveProfile(p, cwd, false); err != nil {
		t.Fatal(err)
	}

	p2 := &Profile{Name: "atomic", Model: "opus"}
	if err := SaveProfile(p2, cwd, false); err != nil {
		t.Fatal(err)
	}

	// Only the final version should be on disk; no stray temp files
	dir := ProjectProfilesDir(cwd)
	entries, _ := os.ReadDir(dir)
	for _, e := range entries {
		if filepath.Ext(e.Name()) != ".yaml" {
			t.Errorf("unexpected file %q in profiles dir after atomic writes", e.Name())
		}
	}

	loaded, _ := loadFromFile(ProfilePath(dir, "atomic"), SourceProject)
	if loaded.Model != "opus" {
		t.Errorf("Model = %q, want opus (should be overwritten)", loaded.Model)
	}
}

func TestDeleteProfile(t *testing.T) {
	homeDir := t.TempDir()
	cwd := t.TempDir()
	t.Setenv("HOME", homeDir)

	p := &Profile{Name: "to-delete", Model: "opus"}
	if err := SaveProfile(p, cwd, false); err != nil {
		t.Fatal(err)
	}

	if err := DeleteProfile("to-delete", cwd, false); err != nil {
		t.Fatalf("DeleteProfile() error = %v", err)
	}

	path := ProfilePath(ProjectProfilesDir(cwd), "to-delete")
	if _, err := os.Stat(path); !os.IsNotExist(err) {
		t.Error("file should not exist after delete")
	}
}

func TestDeleteProfile_NotFound(t *testing.T) {
	homeDir := t.TempDir()
	cwd := t.TempDir()
	t.Setenv("HOME", homeDir)

	err := DeleteProfile("nonexistent", cwd, false)
	if err == nil {
		t.Error("DeleteProfile() expected error for nonexistent profile, got nil")
	}
}
