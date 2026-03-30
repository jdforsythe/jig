package config

import "fmt"

const maxInheritanceDepth = 10

// ResolveInheritance walks the extends chain and merges profiles bottom-up.
func ResolveInheritance(p *Profile, cwd string) (*Profile, error) {
	return resolveChain(p, cwd, make(map[string]bool), 0)
}

func resolveChain(p *Profile, cwd string, visited map[string]bool, depth int) (*Profile, error) {
	if depth > maxInheritanceDepth {
		return nil, fmt.Errorf("inheritance chain exceeds max depth of %d", maxInheritanceDepth)
	}

	if p.Extends == "" {
		return p, nil
	}

	if visited[p.Name] {
		return nil, fmt.Errorf("inheritance cycle detected: profile %q extends itself", p.Name)
	}
	visited[p.Name] = true

	parent, err := LoadProfile(p.Extends, cwd)
	if err != nil {
		return nil, fmt.Errorf("loading parent profile %q: %w", p.Extends, err)
	}

	resolvedParent, err := resolveChain(parent, cwd, visited, depth+1)
	if err != nil {
		return nil, err
	}

	merged := mergeProfiles(resolvedParent, p)
	return merged, nil
}

// mergeProfiles merges child over parent following the merge rules:
// - Scalars: child replaces parent
// - Lists: child replaces parent entirely
// - Maps: deep merge (child keys override parent keys)
func mergeProfiles(parent, child *Profile) *Profile {
	result := *parent

	// Scalars: child wins if non-zero
	if child.Name != "" {
		result.Name = child.Name
	}
	if child.Description != "" {
		result.Description = child.Description
	}
	if child.Model != "" {
		result.Model = child.Model
	}
	if child.Effort != "" {
		result.Effort = child.Effort
	}
	if child.PermissionMode != "" {
		result.PermissionMode = child.PermissionMode
	}
	if child.SystemPrompt != "" {
		result.SystemPrompt = child.SystemPrompt
	}
	if child.AppendSystemPrompt != "" {
		result.AppendSystemPrompt = child.AppendSystemPrompt
	}
	if child.SessionAgent != "" {
		result.SessionAgent = child.SessionAgent
	}

	// Clear extends — it's been resolved
	result.Extends = ""

	// Lists: child replaces entirely
	if child.AllowedTools != nil {
		result.AllowedTools = child.AllowedTools
	}
	if child.DisallowedTools != nil {
		result.DisallowedTools = child.DisallowedTools
	}
	if child.MCPServers != nil {
		result.MCPServers = child.MCPServers
	}
	if child.Skills != nil {
		result.Skills = child.Skills
	}
	if child.Agents != nil {
		result.Agents = child.Agents
	}
	if child.Commands != nil {
		result.Commands = child.Commands
	}
	if child.HookScripts != nil {
		result.HookScripts = child.HookScripts
	}
	if child.ExtraFlags != nil {
		result.ExtraFlags = child.ExtraFlags
	}

	// Maps: deep merge
	result.Hooks = deepMergeHooks(parent.Hooks, child.Hooks)
	result.Settings = deepMergeMap(parent.Settings, child.Settings)
	result.EnabledPlugins = mergeEnabledPlugins(parent.EnabledPlugins, child.EnabledPlugins)
	result.PluginComponents = mergePluginComponents(parent.PluginComponents, child.PluginComponents)

	// Source tracking from child
	result.source = child.source
	result.filePath = child.filePath

	return &result
}

func deepMergeHooks(parent, child map[string][]HookMatcher) map[string][]HookMatcher {
	if child == nil {
		return parent
	}
	if parent == nil {
		return child
	}
	result := make(map[string][]HookMatcher, len(parent)+len(child))
	for k, v := range parent {
		result[k] = v
	}
	for k, v := range child {
		result[k] = v
	}
	return result
}

func mergeEnabledPlugins(parent, child map[string]bool) map[string]bool {
	if child == nil {
		return parent
	}
	if parent == nil {
		return child
	}
	result := make(map[string]bool, len(parent)+len(child))
	for k, v := range parent {
		result[k] = v
	}
	for k, v := range child {
		result[k] = v
	}
	return result
}

func mergePluginComponents(parent, child map[string]PluginComponentSelection) map[string]PluginComponentSelection {
	if child == nil {
		return parent
	}
	if parent == nil {
		return child
	}
	result := make(map[string]PluginComponentSelection, len(parent)+len(child))
	for k, v := range parent {
		result[k] = v
	}
	for k, v := range child {
		result[k] = v // child's selection replaces parent's for that plugin key
	}
	return result
}

func deepMergeMap(parent, child map[string]any) map[string]any {
	if child == nil {
		return parent
	}
	if parent == nil {
		return child
	}
	result := make(map[string]any, len(parent)+len(child))
	for k, v := range parent {
		result[k] = v
	}
	for k, v := range child {
		if childMap, ok := v.(map[string]any); ok {
			if parentMap, ok := result[k].(map[string]any); ok {
				result[k] = deepMergeMap(parentMap, childMap)
				continue
			}
		}
		result[k] = v
	}
	return result
}
