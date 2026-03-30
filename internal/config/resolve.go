package config

// Resolve performs the full resolution pipeline:
// 1. Load profile by name
// 2. Resolve inheritance chain
// 3. Apply CLI overrides
// 4. Apply defaults for missing values
// 5. Validate
func Resolve(name, cwd string, overrides *Profile) (*Profile, error) {
	p, err := LoadProfile(name, cwd)
	if err != nil {
		return nil, err
	}

	p, err = ResolveInheritance(p, cwd)
	if err != nil {
		return nil, err
	}

	if overrides != nil {
		p = applyOverrides(p, overrides)
	}

	applyDefaults(p)

	if err := Validate(p); err != nil {
		return nil, err
	}

	return p, nil
}

// applyOverrides merges CLI overrides onto the profile.
// Only non-zero values in overrides take effect.
func applyOverrides(p, overrides *Profile) *Profile {
	result := *p
	if overrides.Model != "" {
		result.Model = overrides.Model
	}
	if overrides.Effort != "" {
		result.Effort = overrides.Effort
	}
	if overrides.PermissionMode != "" {
		result.PermissionMode = overrides.PermissionMode
	}
	if overrides.SystemPrompt != "" {
		result.SystemPrompt = overrides.SystemPrompt
	}
	if overrides.AppendSystemPrompt != "" {
		result.AppendSystemPrompt = overrides.AppendSystemPrompt
	}
	if overrides.SessionAgent != "" {
		result.SessionAgent = overrides.SessionAgent
	}
	if overrides.AllowedTools != nil {
		result.AllowedTools = overrides.AllowedTools
	}
	if overrides.DisallowedTools != nil {
		result.DisallowedTools = overrides.DisallowedTools
	}
	if overrides.ExtraFlags != nil {
		result.ExtraFlags = overrides.ExtraFlags
	}
	return &result
}

// applyDefaults fills in default values for any unset fields.
func applyDefaults(p *Profile) {
	defaults := DefaultProfile()
	if p.Effort == "" {
		p.Effort = defaults.Effort
	}
	if p.PermissionMode == "" {
		p.PermissionMode = defaults.PermissionMode
	}
}
