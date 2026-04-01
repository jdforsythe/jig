package screens

import (
	"fmt"
	"strings"

	tea "github.com/charmbracelet/bubbletea"
	"github.com/charmbracelet/lipgloss"
	"github.com/jdforsythe/jig/internal/config"
	"github.com/jdforsythe/jig/internal/plugin"
	"github.com/jdforsythe/jig/internal/scanner"
	"github.com/jdforsythe/jig/internal/tui/shared"
)

// Tab identifiers for the editor.
type Tab int

const (
	TabGeneral Tab = iota
	TabTools
	TabMCP
	TabHooks
	TabComponents
	TabPlugins
	TabAdvanced
)

var tabNames = []string{"General", "Tools", "MCP Servers", "Hooks", "Components", "Plugins", "Advanced"}

// Field represents an editable field.
type Field struct {
	Label    string
	Value    string
	Options  []string // if non-nil, this is a select field
	Disabled bool     // if true, field is read-only
}

// pluginCompItem is a selectable component within a plugin.
type pluginCompItem struct {
	category string // "Agents", "Skills", "Commands", "MCP Servers"
	name     string
}

// EditorModel is the tabbed profile editor.
type EditorModel struct {
	profile        *config.Profile
	cwd            string
	isNew          bool
	pickerMode     bool   // true when used as ad-hoc picker (--pick), no save to disk
	origName       string // original profile name before editing
	activeTab      Tab
	fieldCursor    int
	editing        bool
	editBuffer     string
	width          int
	height         int
	titleStyle     lipgloss.Style
	activeTabStyle lipgloss.Style
	tabStyle       lipgloss.Style
	normalStyle    lipgloss.Style
	dimStyle       lipgloss.Style
	statusStyle    lipgloss.Style
	statusKey      lipgloss.Style
	accentStyle    lipgloss.Style
	fields         [][]Field // fields per tab

	// Plugins tab state
	plugins          []*plugin.PluginInfo
	pluginCursor     int
	expandedPlugin   string // plugin key being viewed, or ""
	compItems        []pluginCompItem
	compCursor       int
	compScrollOffset int

	// Components tab state
	disc          *scanner.Discovery
	discItems     []PickerItem
	discFiltered  []int
	discCursor    int
	discFilter    string
	discFiltering bool
}

// NewEditor creates the editor screen.
func NewEditor(p *config.Profile, cwd string, disc *scanner.Discovery, plugins []*plugin.PluginInfo, titleStyle, activeTabStyle, tabStyle, normalStyle, dimStyle, statusStyle, statusKey, accentStyle lipgloss.Style) EditorModel {
	isNew := p.Name == ""
	if isNew {
		p = &config.Profile{Name: "new-profile"}
	}

	m := EditorModel{
		profile:        p,
		cwd:            cwd,
		isNew:          isNew,
		origName:       p.Name,
		disc:           disc,
		plugins:        plugins,
		titleStyle:     titleStyle,
		activeTabStyle: activeTabStyle,
		tabStyle:       tabStyle,
		normalStyle:    normalStyle,
		dimStyle:       dimStyle,
		statusStyle:    statusStyle,
		statusKey:      statusKey,
		accentStyle:    accentStyle,
	}
	m.buildDiscItems()
	m.buildFields()
	return m
}

// NewEditorPicker creates a picker-mode editor for jig run --pick.
// Name and Description are locked; pressing s launches the ad-hoc profile instead of saving.
func NewEditorPicker(cwd string, disc *scanner.Discovery, plugins []*plugin.PluginInfo, titleStyle, activeTabStyle, tabStyle, normalStyle, dimStyle, statusStyle, statusKey, accentStyle lipgloss.Style) EditorModel {
	p := &config.Profile{Name: "ad-hoc", Effort: "high", PermissionMode: "default"}
	m := EditorModel{
		profile:        p,
		cwd:            cwd,
		isNew:          true,
		pickerMode:     true,
		origName:       "ad-hoc",
		disc:           disc,
		plugins:        plugins,
		titleStyle:     titleStyle,
		activeTabStyle: activeTabStyle,
		tabStyle:       tabStyle,
		normalStyle:    normalStyle,
		dimStyle:       dimStyle,
		statusStyle:    statusStyle,
		statusKey:      statusKey,
		accentStyle:    accentStyle,
	}
	m.buildDiscItems()
	m.buildFields()
	return m
}

func (m *EditorModel) buildFields() {
	m.fields = [][]Field{
		// General
		{
			{Label: "Name", Value: m.profile.Name, Disabled: m.pickerMode},
			{Label: "Description", Value: m.profile.Description, Disabled: m.pickerMode},
			{Label: "Extends", Value: m.profile.Extends},
			{Label: "Model", Value: m.profile.Model, Options: append([]string{""}, config.ValidModels...)},
			{Label: "Effort", Value: m.profile.Effort, Options: append([]string{""}, config.ValidEfforts...)},
			{Label: "Permission Mode", Value: m.profile.PermissionMode, Options: append([]string{""}, config.ValidPermissionModes...)},
			{Label: "Session Agent", Value: m.profile.SessionAgent},
		},
		// Tools
		{
			{Label: "Allowed Tools", Value: strings.Join(m.profile.AllowedTools, ", ")},
			{Label: "Disallowed Tools", Value: strings.Join(m.profile.DisallowedTools, ", ")},
		},
		// MCP Servers
		{
			{Label: "MCP Servers", Value: formatMCPServers(m.profile.MCPServers)},
		},
		// Hooks
		{
			{Label: "System Prompt", Value: m.profile.SystemPrompt},
			{Label: "Append System Prompt", Value: m.profile.AppendSystemPrompt},
		},
		// Components — no fields; handled by custom rendering
		{},
		// Plugins — no fields; handled by custom rendering
		{},
		// Advanced
		{
			{Label: "Extra Flags", Value: strings.Join(m.profile.ExtraFlags, " ")},
		},
	}
}

func (m *EditorModel) buildDiscItems() {
	if m.disc == nil {
		m.applyDiscFilter()
		return
	}
	for _, s := range m.disc.MCPServers {
		if isPluginSource(s.Source) {
			continue
		}
		m.discItems = append(m.discItems, PickerItem{
			Name:     s.Name,
			Category: "MCP Server",
			Source:   s.Source,
			Selected: mcpServerRefContains(m.profile.MCPServers, s.Name),
		})
	}
	for _, s := range m.disc.Skills {
		if isPluginSource(s.Source) {
			continue
		}
		m.discItems = append(m.discItems, PickerItem{
			Name:     s.Name,
			Category: "Skill",
			Source:   s.Source,
			Path:     s.Path,
			Selected: pathEntryContains(m.profile.Skills, s.Path),
		})
	}
	for _, a := range m.disc.Agents {
		if isPluginSource(a.Source) {
			continue
		}
		m.discItems = append(m.discItems, PickerItem{
			Name:     a.Name,
			Category: "Agent",
			Source:   a.Source,
			Path:     a.Path,
			Selected: pathEntryContains(m.profile.Agents, a.Path),
		})
	}
	for _, c := range m.disc.Commands {
		if isPluginSource(c.Source) {
			continue
		}
		m.discItems = append(m.discItems, PickerItem{
			Name:     c.Name,
			Category: "Command",
			Source:   c.Source,
			Path:     c.Path,
			Selected: pathEntryContains(m.profile.Commands, c.Path),
		})
	}
	m.applyDiscFilter()
}

func mcpServerRefContains(entries []config.MCPServerEntry, name string) bool {
	for _, e := range entries {
		if e.Ref == name || e.Name == name {
			return true
		}
	}
	return false
}

func pathEntryContains(entries []config.PathEntry, path string) bool {
	for _, e := range entries {
		if e.Path == path {
			return true
		}
	}
	return false
}

func (m *EditorModel) applyDiscFilter() {
	m.discFiltered = nil
	for i, item := range m.discItems {
		if m.discFilter == "" ||
			strings.Contains(strings.ToLower(item.Name), strings.ToLower(m.discFilter)) ||
			strings.Contains(strings.ToLower(item.Category), strings.ToLower(m.discFilter)) {
			m.discFiltered = append(m.discFiltered, i)
		}
	}
	if m.discCursor >= len(m.discFiltered) {
		m.discCursor = len(m.discFiltered) - 1
	}
	if m.discCursor < 0 {
		m.discCursor = 0
	}
}

func formatMCPServers(servers []config.MCPServerEntry) string {
	var parts []string
	for _, s := range servers {
		if s.Ref != "" {
			parts = append(parts, "ref:"+s.Ref)
		} else {
			parts = append(parts, s.Name)
		}
	}
	return strings.Join(parts, ", ")
}

func (m EditorModel) SetSize(w, h int) EditorModel {
	m.width = w
	m.height = h
	return m
}

func (m EditorModel) Init() tea.Cmd { return nil }

func (m EditorModel) Update(msg tea.Msg) (tea.Model, tea.Cmd) {
	switch msg := msg.(type) {
	case tea.KeyMsg:
		if m.editing {
			return m.updateEditing(msg)
		}

		// Plugins and Components tabs have custom key handling
		if m.activeTab == TabPlugins {
			return m.updatePluginsTab(msg)
		}
		if m.activeTab == TabComponents {
			return m.updateComponentsTab(msg)
		}

		switch msg.String() {
		case shared.KeyTab:
			m.activeTab = Tab((int(m.activeTab) + 1) % len(tabNames))
			m.fieldCursor = 0
		case shared.KeyShiftTab:
			m.activeTab = Tab((int(m.activeTab) - 1 + len(tabNames)) % len(tabNames))
			m.fieldCursor = 0
		case shared.KeyUp, "k":
			fields := m.fields[m.activeTab]
			if m.fieldCursor > 0 {
				m.fieldCursor--
			} else {
				m.fieldCursor = len(fields) - 1
			}
		case shared.KeyDown, "j":
			fields := m.fields[m.activeTab]
			if m.fieldCursor < len(fields)-1 {
				m.fieldCursor++
			} else {
				m.fieldCursor = 0
			}
		case shared.KeyEnter:
			field := m.fields[m.activeTab][m.fieldCursor]
			if field.Disabled {
				break
			}
			m.editing = true
			if field.Options != nil {
				m.cycleOption()
				m.editing = false
			} else {
				m.editBuffer = field.Value
			}
		case "s":
			return m.handleSave()
		case shared.KeyEsc:
			return m.handleBack()
		}
	}
	return m, nil
}

// updatePluginsTab handles key input on the Plugins tab.
func (m EditorModel) updatePluginsTab(msg tea.KeyMsg) (tea.Model, tea.Cmd) {
	switch msg.String() {
	case shared.KeyTab:
		m.activeTab = Tab((int(m.activeTab) + 1) % len(tabNames))
		m.pluginCursor = 0
		m.expandedPlugin = ""
		m.compItems = nil
		m.compCursor = 0
	case shared.KeyShiftTab:
		m.activeTab = Tab((int(m.activeTab) - 1 + len(tabNames)) % len(tabNames))
		m.pluginCursor = 0
		m.expandedPlugin = ""
		m.compItems = nil
		m.compCursor = 0

	case shared.KeyUp, "k":
		if m.expandedPlugin != "" {
			if m.compCursor > 0 {
				m.compCursor--
				m.clampCompScroll()
			}
		} else {
			if m.pluginCursor > 0 {
				m.pluginCursor--
			}
		}
	case shared.KeyDown, "j":
		if m.expandedPlugin != "" {
			if m.compCursor < len(m.compItems)-1 {
				m.compCursor++
				m.clampCompScroll()
			}
		} else {
			if m.pluginCursor < len(m.plugins)-1 {
				m.pluginCursor++
			}
		}

	case shared.KeyEnter, shared.KeyRight:
		if m.expandedPlugin == "" && len(m.plugins) > 0 {
			// Expand selected plugin
			pi := m.plugins[m.pluginCursor]
			m.expandedPlugin = pi.Key
			m.compItems = buildCompItems(pi)
			m.compCursor = 0
			m.compScrollOffset = 0
		}

	case shared.KeyLeft, shared.KeyEsc:
		if m.expandedPlugin != "" {
			// Collapse back to plugin list
			m.expandedPlugin = ""
			m.compItems = nil
			m.compCursor = 0
			m.compScrollOffset = 0
		} else {
			return m.handleBack()
		}

	case "f":
		// Toggle full enable for the selected plugin
		if m.expandedPlugin == "" && len(m.plugins) > 0 {
			pi := m.plugins[m.pluginCursor]
			m.togglePluginEnabled(pi.Key)
		}

	case shared.KeySpace:
		// Toggle individual component selection (only in component view)
		if m.expandedPlugin != "" && len(m.compItems) > 0 {
			item := m.compItems[m.compCursor]
			m.toggleComponent(item.category, item.name)
		}

	case "s":
		return m.handleSave()
	}

	return m, nil
}

// updateComponentsTab handles key input on the Components tab.
func (m EditorModel) updateComponentsTab(msg tea.KeyMsg) (tea.Model, tea.Cmd) {
	if m.discFiltering {
		return m.updateDiscFilter(msg)
	}
	switch msg.String() {
	case shared.KeyTab:
		m.activeTab = Tab((int(m.activeTab) + 1) % len(tabNames))
		m.discCursor = 0
	case shared.KeyShiftTab:
		m.activeTab = Tab((int(m.activeTab) - 1 + len(tabNames)) % len(tabNames))
		m.discCursor = 0
	case shared.KeyUp, "k":
		if m.discCursor > 0 {
			m.discCursor--
		}
	case shared.KeyDown, "j":
		if m.discCursor < len(m.discFiltered)-1 {
			m.discCursor++
		}
	case shared.KeySpace:
		if len(m.discFiltered) > 0 {
			idx := m.discFiltered[m.discCursor]
			m.discItems[idx].Selected = !m.discItems[idx].Selected
		}
	case shared.KeySlash:
		m.discFiltering = true
		m.discFilter = ""
	case "s":
		return m.handleSave()
	case shared.KeyEsc:
		return m.handleBack()
	}
	return m, nil
}

func (m EditorModel) updateDiscFilter(msg tea.KeyMsg) (tea.Model, tea.Cmd) {
	switch msg.String() {
	case shared.KeyEnter, shared.KeyEsc:
		m.discFiltering = false
	case "backspace":
		if len(m.discFilter) > 0 {
			m.discFilter = m.discFilter[:len(m.discFilter)-1]
			m.applyDiscFilter()
		}
	default:
		if len(msg.String()) == 1 {
			m.discFilter += msg.String()
			m.applyDiscFilter()
		}
	}
	return m, nil
}

func (m EditorModel) updateEditing(msg tea.KeyMsg) (tea.Model, tea.Cmd) {
	switch msg.String() {
	case shared.KeyEnter:
		m.fields[m.activeTab][m.fieldCursor].Value = m.editBuffer
		m.editing = false
	case shared.KeyEsc:
		m.editing = false
	case "backspace":
		if len(m.editBuffer) > 0 {
			m.editBuffer = m.editBuffer[:len(m.editBuffer)-1]
		}
	default:
		if len(msg.String()) == 1 {
			m.editBuffer += msg.String()
		}
	}
	return m, nil
}

func (m *EditorModel) cycleOption() {
	field := &m.fields[m.activeTab][m.fieldCursor]
	if field.Options == nil {
		return
	}
	current := field.Value
	for i, opt := range field.Options {
		if opt == current {
			field.Value = field.Options[(i+1)%len(field.Options)]
			return
		}
	}
	field.Value = field.Options[0]
}

func (m *EditorModel) applyFields() {
	general := m.fields[TabGeneral]
	m.profile.Name = general[0].Value
	m.profile.Description = general[1].Value
	m.profile.Extends = general[2].Value
	m.profile.Model = general[3].Value
	m.profile.Effort = general[4].Value
	m.profile.PermissionMode = general[5].Value
	m.profile.SessionAgent = general[6].Value

	tools := m.fields[TabTools]
	m.profile.AllowedTools = splitCSV(tools[0].Value)
	m.profile.DisallowedTools = splitCSV(tools[1].Value)

	hooks := m.fields[TabHooks]
	m.profile.SystemPrompt = hooks[0].Value
	m.profile.AppendSystemPrompt = hooks[1].Value

	advanced := m.fields[TabAdvanced]
	m.profile.ExtraFlags = splitCSV(advanced[0].Value)
	// Plugins tab changes are applied directly to m.profile as they happen
	// Apply component selections from the Components tab
	// Build sets of discovered MCP server names for safe merging
	discoveredMCPs := make(map[string]bool)
	selectedMCPs := make(map[string]bool)
	var skills, agents, commands []config.PathEntry
	for _, item := range m.discItems {
		switch item.Category {
		case "MCP Server":
			discoveredMCPs[item.Name] = true
			if item.Selected {
				selectedMCPs[item.Name] = true
			}
		case "Skill":
			if item.Selected {
				skills = append(skills, config.PathEntry{Path: item.Path})
			}
		case "Agent":
			if item.Selected {
				agents = append(agents, config.PathEntry{Path: item.Path})
			}
		case "Command":
			if item.Selected {
				commands = append(commands, config.PathEntry{Path: item.Path})
			}
		}
	}
	// Rebuild MCPServers: preserve manually-added (non-discovered) entries,
	// use checklist for discovered entries.
	var mcpServers []config.MCPServerEntry
	for _, e := range m.profile.MCPServers {
		ref := e.Ref
		if ref == "" {
			ref = e.Name
		}
		if !discoveredMCPs[ref] {
			mcpServers = append(mcpServers, e) // keep non-discovered entries
		} else if selectedMCPs[ref] {
			mcpServers = append(mcpServers, e) // keep selected discovered entries
		}
	}
	// Add newly selected discovered entries not already in profile
	for _, item := range m.discItems {
		if item.Category == "MCP Server" && item.Selected && !mcpServerRefContains(m.profile.MCPServers, item.Name) {
			mcpServers = append(mcpServers, config.MCPServerEntry{Ref: item.Name})
		}
	}
	m.profile.MCPServers = mcpServers
	m.profile.Skills = skills
	m.profile.Agents = agents
	m.profile.Commands = commands
}

// handleSave saves the profile (editor mode) or launches it (picker mode).
func (m EditorModel) handleSave() (tea.Model, tea.Cmd) {
	m.applyFields()
	if m.pickerMode {
		p := m.profile
		return m, func() tea.Msg {
			return shared.LaunchProfileMsg{ProfileName: "ad-hoc", Profile: p}
		}
	}
	global := m.profile.Source() == config.SourceGlobal
	if err := config.SaveProfile(m.profile, m.cwd, global); err != nil {
		return m, func() tea.Msg { return shared.ErrorMsg{Err: err} }
	}
	if !m.isNew && m.origName != "" && m.origName != m.profile.Name {
		_ = config.DeleteProfile(m.origName, m.cwd, global)
	}
	return m, func() tea.Msg {
		return shared.SwitchScreenMsg{Screen: shared.ScreenHome}
	}
}

// handleBack returns to home (editor mode) or quits (picker mode).
func (m EditorModel) handleBack() (tea.Model, tea.Cmd) {
	if m.pickerMode {
		return m, tea.Quit
	}
	return m, func() tea.Msg {
		return shared.SwitchScreenMsg{Screen: shared.ScreenHome}
	}
}

// togglePluginEnabled toggles whether a plugin is fully enabled.
func (m *EditorModel) togglePluginEnabled(key string) {
	if m.profile.EnabledPlugins == nil {
		m.profile.EnabledPlugins = make(map[string]bool)
	}
	m.profile.EnabledPlugins[key] = !m.profile.EnabledPlugins[key]
}

// toggleComponent toggles a named component in plugin_components.
func (m *EditorModel) toggleComponent(category, name string) {
	if m.profile.PluginComponents == nil {
		m.profile.PluginComponents = make(map[string]config.PluginComponentSelection)
	}
	sel := m.profile.PluginComponents[m.expandedPlugin]
	switch category {
	case "Agents":
		sel.Agents = toggleStringSlice(sel.Agents, name)
	case "Skills":
		sel.Skills = toggleStringSlice(sel.Skills, name)
	case "Commands":
		sel.Commands = toggleStringSlice(sel.Commands, name)
	case "MCP Servers":
		sel.MCPServers = toggleStringSlice(sel.MCPServers, name)
	}
	m.profile.PluginComponents[m.expandedPlugin] = sel
}

// isComponentSelected reports whether a component is in plugin_components.
func (m *EditorModel) isComponentSelected(category, name string) bool {
	if m.profile.PluginComponents == nil {
		return false
	}
	sel, ok := m.profile.PluginComponents[m.expandedPlugin]
	if !ok {
		return false
	}
	switch category {
	case "Agents":
		return sliceContains(sel.Agents, name)
	case "Skills":
		return sliceContains(sel.Skills, name)
	case "Commands":
		return sliceContains(sel.Commands, name)
	case "MCP Servers":
		return sliceContains(sel.MCPServers, name)
	}
	return false
}

// clampCompScroll adjusts compScrollOffset to keep compCursor visible.
func (m *EditorModel) clampCompScroll() {
	// Approximate visible rows: height minus header/footer/tabs overhead (~10 lines).
	visible := m.height - 10
	if visible < 4 {
		visible = 4
	}
	if m.compCursor < m.compScrollOffset {
		m.compScrollOffset = m.compCursor
	}
	if m.compCursor >= m.compScrollOffset+visible {
		m.compScrollOffset = m.compCursor - visible + 1
	}
}

func buildCompItems(pi *plugin.PluginInfo) []pluginCompItem {
	var items []pluginCompItem
	for _, a := range pi.Components.Agents {
		items = append(items, pluginCompItem{category: "Agents", name: a})
	}
	for _, s := range pi.Components.Skills {
		items = append(items, pluginCompItem{category: "Skills", name: s})
	}
	for _, c := range pi.Components.Commands {
		items = append(items, pluginCompItem{category: "Commands", name: c})
	}
	for _, srv := range pi.Components.MCPServers {
		items = append(items, pluginCompItem{category: "MCP Servers", name: srv})
	}
	return items
}

func toggleStringSlice(slice []string, s string) []string {
	for i, v := range slice {
		if v == s {
			result := make([]string, 0, len(slice)-1)
			result = append(result, slice[:i]...)
			result = append(result, slice[i+1:]...)
			return result
		}
	}
	return append(slice, s)
}

func sliceContains(slice []string, s string) bool {
	for _, v := range slice {
		if v == s {
			return true
		}
	}
	return false
}

func splitCSV(s string) []string {
	if s == "" {
		return nil
	}
	parts := strings.Split(s, ",")
	var result []string
	for _, p := range parts {
		p = strings.TrimSpace(p)
		if p != "" {
			result = append(result, p)
		}
	}
	return result
}

func (m EditorModel) View() string {
	var b strings.Builder

	var header string
	if m.pickerMode {
		header = "Ad-hoc Picker"
	} else {
		action := "Edit"
		if m.isNew {
			action = "New"
		}
		header = fmt.Sprintf("%s Profile: %s", action, m.profile.Name)
	}
	b.WriteString(shared.RenderHeader(m.titleStyle, m.dimStyle, header))

	// Tabs
	var tabs []string
	for i, name := range tabNames {
		if Tab(i) == m.activeTab {
			tabs = append(tabs, m.activeTabStyle.Render(name))
		} else {
			tabs = append(tabs, m.tabStyle.Render(name))
		}
	}
	b.WriteString("  " + strings.Join(tabs, " ") + "\n")
	minWidth := 40
	lineWidth := m.width - 4
	if lineWidth < minWidth {
		lineWidth = minWidth
	}
	b.WriteString("  " + strings.Repeat("─", lineWidth) + "\n\n")

	// Render tab content
	if m.activeTab == TabPlugins {
		b.WriteString(m.viewPluginsTab())
	} else if m.activeTab == TabComponents {
		b.WriteString(m.viewComponentsTab())
	} else {
		fields := m.fields[m.activeTab]
		for i, f := range fields {
			cursor := "  "
			if i == m.fieldCursor && !f.Disabled {
				cursor = m.accentStyle.Render("> ")
			}

			label := m.dimStyle.Render(fmt.Sprintf("%-20s", f.Label))
			var value string
			if f.Disabled {
				if f.Value == "" {
					value = m.dimStyle.Render("—")
				} else {
					value = m.dimStyle.Render(f.Value)
				}
			} else {
				value = f.Value
				if value == "" {
					value = m.dimStyle.Render("(empty)")
				}

				if m.editing && i == m.fieldCursor {
					value = m.accentStyle.Render(m.editBuffer + "█")
				}

				if f.Options != nil && !m.editing {
					if value == "" {
						value = m.dimStyle.Render("(none)")
					} else {
						value = m.accentStyle.Render(value)
					}
				}
			}

			b.WriteString(fmt.Sprintf("%s%s  %s\n", cursor, label, value))
		}
	}

	// Status bar
	b.WriteString("\n")
	saveLabel := "save"
	backLabel := "back"
	if m.pickerMode {
		saveLabel = "launch"
		backLabel = "quit"
	}
	var keys []string
	if m.activeTab == TabPlugins {
		if m.expandedPlugin != "" {
			keys = []string{
				m.statusKey.Render("space") + " toggle",
				m.statusKey.Render("←/esc") + " back",
				m.statusKey.Render("s") + " " + saveLabel,
			}
		} else {
			keys = []string{
				m.statusKey.Render("f") + " toggle full",
				m.statusKey.Render("enter") + " components",
				m.statusKey.Render("s") + " " + saveLabel,
				m.statusKey.Render("esc") + " " + backLabel,
			}
		}
	} else if m.activeTab == TabComponents {
		keys = []string{
			m.statusKey.Render("space") + " toggle",
			m.statusKey.Render("/") + " filter",
			m.statusKey.Render("s") + " " + saveLabel,
			m.statusKey.Render("esc") + " " + backLabel,
		}
	} else if m.editing {
		keys = []string{
			m.statusKey.Render("enter") + " confirm",
			m.statusKey.Render("esc") + " cancel",
		}
	} else {
		keys = []string{
			m.statusKey.Render("enter") + " edit",
			m.statusKey.Render("tab") + " next tab",
			m.statusKey.Render("s") + " " + saveLabel,
			m.statusKey.Render("esc") + " " + backLabel,
		}
	}
	if m.width > 0 {
		b.WriteString(m.statusStyle.Width(m.width).Render(strings.Join(keys, "  ")))
	} else {
		b.WriteString(m.statusStyle.Render(strings.Join(keys, "  ")))
	}

	return b.String()
}

func (m EditorModel) viewComponentsTab() string {
	var b strings.Builder

	if m.discFiltering {
		b.WriteString("  " + m.accentStyle.Render("/") + m.discFilter + "█\n\n")
	}

	if len(m.discItems) == 0 {
		b.WriteString("  " + m.dimStyle.Render("No resources discovered.") + "\n")
		return b.String()
	}

	lastCat := ""
	lastSrc := ""
	for i, idx := range m.discFiltered {
		item := m.discItems[idx]

		if item.Category != lastCat {
			if lastCat != "" {
				b.WriteString("\n")
			}
			b.WriteString("  " + m.accentStyle.Render(item.Category) + "\n")
			lastCat = item.Category
			lastSrc = ""
		}

		if item.Source != "" && item.Source != lastSrc {
			b.WriteString("  " + m.dimStyle.Render("── "+sourceLabel(item.Source)+" ──") + "\n")
			lastSrc = item.Source
		}

		cursor := "  "
		if i == m.discCursor {
			cursor = m.accentStyle.Render("> ")
		}

		check := m.dimStyle.Render("[ ]")
		if item.Selected {
			check = m.accentStyle.Render("[✓]")
		}

		b.WriteString(fmt.Sprintf("  %s%s %s\n", cursor, check, item.Name))
	}

	if len(m.discFiltered) == 0 {
		b.WriteString("  " + m.dimStyle.Render("No items match filter.") + "\n")
	}

	return b.String()
}

func (m EditorModel) viewPluginsTab() string {
	var b strings.Builder

	if len(m.plugins) == 0 {
		b.WriteString("  " + m.dimStyle.Render("No plugins installed.") + "\n")
		b.WriteString("  " + m.dimStyle.Render("Install plugins with: /plugin install <name>") + "\n")
		return b.String()
	}

	if m.expandedPlugin != "" {
		// Component view
		b.WriteString("  " + m.accentStyle.Render("← ") + m.dimStyle.Render(m.expandedPlugin) + "\n\n")

		if len(m.compItems) == 0 {
			b.WriteString("  " + m.dimStyle.Render("No components found.") + "\n")
			return b.String()
		}

		visible := m.height - 10
		if visible < 4 {
			visible = 4
		}

		lastCat := ""
		for i, item := range m.compItems {
			if i < m.compScrollOffset {
				// Still track category headers for items above the scroll window.
				lastCat = item.category
				continue
			}
			if i >= m.compScrollOffset+visible {
				break
			}

			if item.category != lastCat {
				if lastCat != "" {
					b.WriteString("\n")
				}
				b.WriteString("  " + m.dimStyle.Render("── "+item.category+" ──") + "\n")
				lastCat = item.category
			}

			cursor := "    "
			if i == m.compCursor {
				cursor = m.accentStyle.Render("  > ")
			}

			check := m.dimStyle.Render("[ ]")
			if m.isComponentSelected(item.category, item.name) {
				check = m.accentStyle.Render("[✓]")
			}

			b.WriteString(fmt.Sprintf("%s%s %s\n", cursor, check, item.name))
		}
		total := len(m.compItems)
		showing := m.compScrollOffset + visible
		if showing > total {
			showing = total
		}
		if total > visible {
			b.WriteString(m.dimStyle.Render(fmt.Sprintf("  (%d-%d of %d)", m.compScrollOffset+1, showing, total)) + "\n")
		}
	} else {
		// Plugin list view
		for i, pi := range m.plugins {
			cursor := "  "
			if i == m.pluginCursor {
				cursor = m.accentStyle.Render("> ")
			}

			name := pi.Name
			if pi.Marketplace != "" {
				name = fmt.Sprintf("%s @ %s", pi.Name, pi.Marketplace)
			}

			enabled := m.dimStyle.Render("[disabled]")
			if m.profile.EnabledPlugins != nil && m.profile.EnabledPlugins[pi.Key] {
				enabled = m.accentStyle.Render("[enabled]")
			}

			compCount := 0
			if m.profile.PluginComponents != nil {
				sel := m.profile.PluginComponents[pi.Key]
				compCount = len(sel.Agents) + len(sel.Skills) + len(sel.Commands) + len(sel.MCPServers)
			}

			compInfo := ""
			if compCount > 0 {
				compInfo = m.dimStyle.Render(fmt.Sprintf("  %d components selected", compCount))
			}

			b.WriteString(fmt.Sprintf("%s%-50s %s%s\n", cursor, name, enabled, compInfo))
		}
	}

	return b.String()
}
