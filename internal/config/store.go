package config

import (
	"fmt"
	"os"
	"path/filepath"

	"gopkg.in/yaml.v3"
)

// SaveProfile writes a profile to disk using atomic write.
// If global is true, saves to ~/.jig/profiles/; otherwise to <cwd>/.jig/profiles/.
func SaveProfile(p *Profile, cwd string, global bool) error {
	var dir string
	if global {
		gd, err := GlobalProfilesDir()
		if err != nil {
			return err
		}
		dir = gd
	} else {
		dir = ProjectProfilesDir(cwd)
	}

	if err := EnsureDir(dir); err != nil {
		return fmt.Errorf("creating profiles dir: %w", err)
	}

	dest := ProfilePath(dir, p.Name)
	return atomicWriteYAML(dest, p)
}

// DeleteProfile removes a profile file.
func DeleteProfile(name, cwd string, global bool) error {
	var dir string
	if global {
		gd, err := GlobalProfilesDir()
		if err != nil {
			return err
		}
		dir = gd
	} else {
		dir = ProjectProfilesDir(cwd)
	}

	path := ProfilePath(dir, name)
	if err := os.Remove(path); err != nil {
		return fmt.Errorf("deleting profile %q: %w", name, err)
	}
	return nil
}

// atomicWriteYAML marshals v to YAML and writes it atomically to path.
func atomicWriteYAML(path string, v any) error {
	data, err := yaml.Marshal(v)
	if err != nil {
		return fmt.Errorf("marshaling YAML: %w", err)
	}

	dir := filepath.Dir(path)
	tmp, err := os.CreateTemp(dir, ".jig-tmp-*.yaml")
	if err != nil {
		return fmt.Errorf("creating temp file: %w", err)
	}
	tmpName := tmp.Name()

	if _, err := tmp.Write(data); err != nil {
		tmp.Close()
		os.Remove(tmpName)
		return fmt.Errorf("writing temp file: %w", err)
	}
	if err := tmp.Close(); err != nil {
		os.Remove(tmpName)
		return err
	}

	if err := os.Rename(tmpName, path); err != nil {
		os.Remove(tmpName)
		return fmt.Errorf("renaming temp file: %w", err)
	}
	return nil
}
