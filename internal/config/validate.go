package config

import (
	"fmt"
	"regexp"
	"strings"
)

var namePattern = regexp.MustCompile(`^[a-z][a-z0-9-]*$`)

// Validate checks a profile for errors.
func Validate(p *Profile) error {
	var errs []string

	if p.Name == "" {
		errs = append(errs, "name is required")
	} else if !namePattern.MatchString(p.Name) {
		errs = append(errs, "name must be kebab-case (lowercase alphanumeric and hyphens, starting with a letter)")
	}

	if p.Model != "" && !contains(ValidModels, p.Model) {
		errs = append(errs, fmt.Sprintf("invalid model %q (valid: %s)", p.Model, strings.Join(ValidModels, ", ")))
	}

	if p.Effort != "" && !contains(ValidEfforts, p.Effort) {
		errs = append(errs, fmt.Sprintf("invalid effort %q (valid: %s)", p.Effort, strings.Join(ValidEfforts, ", ")))
	}

	if p.PermissionMode != "" && !contains(ValidPermissionModes, p.PermissionMode) {
		errs = append(errs, fmt.Sprintf("invalid permission_mode %q (valid: %s)", p.PermissionMode, strings.Join(ValidPermissionModes, ", ")))
	}

	for i, mcp := range p.MCPServers {
		if mcp.Ref == "" && mcp.Name == "" {
			errs = append(errs, fmt.Sprintf("mcp_servers[%d]: must have either ref or name", i))
		}
		if mcp.Ref != "" && mcp.Command != "" {
			errs = append(errs, fmt.Sprintf("mcp_servers[%d]: ref and command are mutually exclusive", i))
		}
		if mcp.Ref == "" && mcp.Command == "" && mcp.Name != "" {
			errs = append(errs, fmt.Sprintf("mcp_servers[%d] %q: inline server requires command", i, mcp.Name))
		}
	}

	for i, s := range p.Skills {
		if s.Path == "" {
			errs = append(errs, fmt.Sprintf("skills[%d]: path is required", i))
		}
	}
	for i, a := range p.Agents {
		if a.Path == "" {
			errs = append(errs, fmt.Sprintf("agents[%d]: path is required", i))
		}
	}
	for i, c := range p.Commands {
		if c.Path == "" {
			errs = append(errs, fmt.Sprintf("commands[%d]: path is required", i))
		}
	}

	for i, hs := range p.HookScripts {
		if hs.Path == "" {
			errs = append(errs, fmt.Sprintf("hook_scripts[%d]: path is required", i))
		}
		if hs.Dest == "" {
			errs = append(errs, fmt.Sprintf("hook_scripts[%d]: dest is required", i))
		}
	}

	if len(errs) > 0 {
		return &ValidationError{Errors: errs}
	}
	return nil
}

// ValidationError holds multiple validation errors.
type ValidationError struct {
	Errors []string
}

func (e *ValidationError) Error() string {
	return fmt.Sprintf("validation failed:\n  - %s", strings.Join(e.Errors, "\n  - "))
}

func contains(slice []string, s string) bool {
	for _, v := range slice {
		if v == s {
			return true
		}
	}
	return false
}
