package claude

import "github.com/jdforsythe/jig/internal/config"

// BuildCLIArgs constructs the claude CLI argument slice from a resolved profile.
// settingsPath points to the jig-generated settings file (always non-empty).
func BuildCLIArgs(p *config.Profile, pluginDir, settingsPath string, passthrough []string) []string {
	var args []string

	// Settings file for plugin isolation (always present)
	args = append(args, "--settings", settingsPath)

	// Plugin dir
	args = append(args, "--plugin-dir", pluginDir)

	// Model
	if p.Model != "" {
		args = append(args, "--model", p.Model)
	}

	// Effort
	if p.Effort != "" {
		args = append(args, "--effort", p.Effort)
	}

	// Permission mode
	if p.PermissionMode != "" {
		args = append(args, "--permission-mode", p.PermissionMode)
	}

	// System prompt
	if p.SystemPrompt != "" {
		args = append(args, "--system-prompt", p.SystemPrompt)
	}

	// Append system prompt
	if p.AppendSystemPrompt != "" {
		args = append(args, "--append-system-prompt", p.AppendSystemPrompt)
	}

	// Allowed tools
	if len(p.AllowedTools) > 0 {
		args = append(args, "--allowedTools")
		args = append(args, p.AllowedTools...)
	}

	// Disallowed tools
	if len(p.DisallowedTools) > 0 {
		args = append(args, "--disallowedTools")
		args = append(args, p.DisallowedTools...)
	}

	// Session agent
	if p.SessionAgent != "" {
		args = append(args, "--agent", p.SessionAgent)
	}

	// Extra flags from profile
	args = append(args, p.ExtraFlags...)

	// Passthrough flags (everything after --)
	args = append(args, passthrough...)

	return args
}
