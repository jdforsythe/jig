package claude

import (
	"fmt"
	"os/exec"
	"strings"
)

// DetectResult holds information about the found claude binary.
type DetectResult struct {
	Path    string
	Version string
}

// Detect finds the claude binary and checks its version.
func Detect() (*DetectResult, error) {
	path, err := exec.LookPath("claude")
	if err != nil {
		return nil, fmt.Errorf("claude not found in PATH: %w\n\nInstall Claude Code: https://docs.anthropic.com/en/docs/claude-code", err)
	}

	version, err := getVersion(path)
	if err != nil {
		// Non-fatal: we found the binary, just can't get version
		return &DetectResult{Path: path, Version: "unknown"}, nil
	}

	return &DetectResult{Path: path, Version: version}, nil
}

func getVersion(path string) (string, error) {
	out, err := exec.Command(path, "--version").Output()
	if err != nil {
		return "", err
	}
	return strings.TrimSpace(string(out)), nil
}
