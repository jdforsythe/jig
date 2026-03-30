package config

import (
	"os"
	"path/filepath"
)

const (
	globalDirName   = ".jig"
	profilesDirName = "profiles"
	shortcutFile    = ".jig.yaml"
)

// GlobalDir returns ~/.jig/.
func GlobalDir() (string, error) {
	home, err := os.UserHomeDir()
	if err != nil {
		return "", err
	}
	return filepath.Join(home, globalDirName), nil
}

// GlobalProfilesDir returns ~/.jig/profiles/.
func GlobalProfilesDir() (string, error) {
	gd, err := GlobalDir()
	if err != nil {
		return "", err
	}
	return filepath.Join(gd, profilesDirName), nil
}

// ProjectDir returns <cwd>/.jig/.
func ProjectDir(cwd string) string {
	return filepath.Join(cwd, globalDirName)
}

// ProjectProfilesDir returns <cwd>/.jig/profiles/.
func ProjectProfilesDir(cwd string) string {
	return filepath.Join(cwd, globalDirName, profilesDirName)
}

// ShortcutPath returns <cwd>/.jig.yaml.
func ShortcutPath(cwd string) string {
	return filepath.Join(cwd, shortcutFile)
}

// EnsureDir creates a directory and all parents if they don't exist.
func EnsureDir(path string) error {
	return os.MkdirAll(path, 0755)
}

// ProfilePath returns the full path for a profile file given a directory and name.
func ProfilePath(dir, name string) string {
	return filepath.Join(dir, name+".yaml")
}
