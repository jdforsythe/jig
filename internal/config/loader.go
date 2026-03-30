package config

import (
	"fmt"
	"os"
	"path/filepath"
	"strings"

	"gopkg.in/yaml.v3"
)

// LoadProfile loads a profile by name, searching project scope first, then global.
func LoadProfile(name, cwd string) (*Profile, error) {
	// 1. Check project profiles dir
	projPath := ProfilePath(ProjectProfilesDir(cwd), name)
	if p, err := loadFromFile(projPath, SourceProject); err == nil {
		return p, nil
	}

	// 2. Check shortcut file (if name matches or is empty)
	shortcut := ShortcutPath(cwd)
	if _, err := os.Stat(shortcut); err == nil {
		p, err := loadFromFile(shortcut, SourceShortcut)
		if err == nil && (name == "" || p.Name == name) {
			return p, nil
		}
	}

	// 3. Check global profiles dir
	globalDir, err := GlobalProfilesDir()
	if err != nil {
		return nil, fmt.Errorf("resolving global profiles dir: %w", err)
	}
	globalPath := ProfilePath(globalDir, name)
	if p, err := loadFromFile(globalPath, SourceGlobal); err == nil {
		return p, nil
	}

	return nil, fmt.Errorf("profile %q not found", name)
}

// LoadProfileFromFile loads a profile from a specific file path.
func LoadProfileFromFile(path string) (*Profile, error) {
	return loadFromFile(path, SourceProject)
}

func loadFromFile(path string, source ProfileSource) (*Profile, error) {
	data, err := os.ReadFile(path)
	if err != nil {
		return nil, err
	}

	var p Profile
	if err := yaml.Unmarshal(data, &p); err != nil {
		return nil, fmt.Errorf("parsing %s: %w", path, err)
	}

	p.source = source
	p.filePath = path
	return &p, nil
}

// ListProfiles returns all profiles found in project and global scopes.
func ListProfiles(cwd string) ([]Profile, error) {
	seen := make(map[string]bool)
	var profiles []Profile

	// Project profiles take precedence
	projDir := ProjectProfilesDir(cwd)
	if ps, err := loadProfilesFromDir(projDir, SourceProject); err == nil {
		for _, p := range ps {
			seen[p.Name] = true
			profiles = append(profiles, p)
		}
	}

	// Shortcut file
	shortcut := ShortcutPath(cwd)
	if p, err := loadFromFile(shortcut, SourceShortcut); err == nil && !seen[p.Name] {
		seen[p.Name] = true
		profiles = append(profiles, *p)
	}

	// Global profiles
	globalDir, err := GlobalProfilesDir()
	if err == nil {
		if ps, err := loadProfilesFromDir(globalDir, SourceGlobal); err == nil {
			for _, p := range ps {
				if !seen[p.Name] {
					profiles = append(profiles, p)
				}
			}
		}
	}

	return profiles, nil
}

func loadProfilesFromDir(dir string, source ProfileSource) ([]Profile, error) {
	entries, err := os.ReadDir(dir)
	if err != nil {
		return nil, err
	}

	var profiles []Profile
	for _, e := range entries {
		if e.IsDir() || strings.HasPrefix(e.Name(), ".") || filepath.Ext(e.Name()) != ".yaml" {
			continue
		}
		p, err := loadFromFile(filepath.Join(dir, e.Name()), source)
		if err != nil {
			continue // skip malformed profiles
		}
		profiles = append(profiles, *p)
	}
	return profiles, nil
}
