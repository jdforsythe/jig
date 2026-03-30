package scanner

import (
	"os"
	"path/filepath"
)

// SourceLocations returns all directories that should be scanned for resources.
func SourceLocations(cwd string) []string {
	var locs []string

	home, err := os.UserHomeDir()
	if err == nil {
		claudeDir := filepath.Join(home, ".claude")
		locs = append(locs,
			filepath.Join(claudeDir, "skills"),
			filepath.Join(claudeDir, "agents"),
			filepath.Join(claudeDir, "commands"),
		)
	}

	projectClaude := filepath.Join(cwd, ".claude")
	locs = append(locs,
		filepath.Join(projectClaude, "skills"),
		filepath.Join(projectClaude, "agents"),
		filepath.Join(projectClaude, "commands"),
	)

	return locs
}
