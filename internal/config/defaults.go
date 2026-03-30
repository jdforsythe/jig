package config

// DefaultProfile returns a profile with sensible defaults.
func DefaultProfile() Profile {
	return Profile{
		Effort:         "high",
		PermissionMode: "default",
	}
}

// ValidEfforts is the set of valid effort levels.
var ValidEfforts = []string{"low", "medium", "high", "max"}

// ValidPermissionModes is the set of valid permission modes.
var ValidPermissionModes = []string{"default", "plan", "autoaccept", "bypassPermissions"}

// ValidModels is the set of valid model names.
var ValidModels = []string{"opus", "sonnet", "haiku"}
