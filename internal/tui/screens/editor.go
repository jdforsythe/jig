package screens

import (
	"fmt"
	"strings"

	tea "github.com/charmbracelet/bubbletea"
	"github.com/charmbracelet/lipgloss"
	"github.com/jforsythe/jig/internal/config"
	"github.com/jforsythe/jig/internal/tui/shared"
)

// Tab identifiers for the editor.
type Tab int

const (
	TabGeneral Tab = iota
	TabTools
	TabMCP
	TabHooks
	TabAdvanced
)

var tabNames = []string{"General", "Tools", "MCP Servers", "Hooks", "Advanced"}

// Field represents an editable field.
type Field struct {
	Label   string
	Value   string
	Options []string // if non-nil, this is a select field
}

// EditorModel is the tabbed profile editor.
type EditorModel struct {
	profile        *config.Profile
	cwd            string
	isNew          bool
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
}

// NewEditor creates the editor screen.
func NewEditor(p *config.Profile, cwd string, titleStyle, activeTabStyle, tabStyle, normalStyle, dimStyle, statusStyle, statusKey, accentStyle lipgloss.Style) EditorModel {
	isNew := p.Name == ""
	if isNew {
		p = &config.Profile{Name: "new-profile"}
	}

	m := EditorModel{
		profile:        p,
		cwd:            cwd,
		isNew:          isNew,
		titleStyle:     titleStyle,
		activeTabStyle: activeTabStyle,
		tabStyle:       tabStyle,
		normalStyle:    normalStyle,
		dimStyle:       dimStyle,
		statusStyle:    statusStyle,
		statusKey:      statusKey,
		accentStyle:    accentStyle,
	}
	m.buildFields()
	return m
}

func (m *EditorModel) buildFields() {
	m.fields = [][]Field{
		// General
		{
			{Label: "Name", Value: m.profile.Name},
			{Label: "Description", Value: m.profile.Description},
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
		// Advanced
		{
			{Label: "Extra Flags", Value: strings.Join(m.profile.ExtraFlags, " ")},
		},
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
			m.editing = true
			field := m.fields[m.activeTab][m.fieldCursor]
			if field.Options != nil {
				// Cycle through options
				m.cycleOption()
				m.editing = false
			} else {
				m.editBuffer = field.Value
			}
		case "s":
			m.applyFields()
			if err := config.SaveProfile(m.profile, m.cwd, m.profile.Source() == config.SourceGlobal); err != nil {
				return m, func() tea.Msg { return shared.ErrorMsg{Err: err} }
			}
			return m, func() tea.Msg {
				return shared.SwitchScreenMsg{Screen: shared.ScreenHome}
			}
		case shared.KeyEsc:
			return m, func() tea.Msg {
				return shared.SwitchScreenMsg{Screen: shared.ScreenHome}
			}
		}
	}
	return m, nil
}

func (m *EditorModel) updateEditing(msg tea.KeyMsg) (tea.Model, tea.Cmd) {
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

	// Title
	action := "Edit"
	if m.isNew {
		action = "New"
	}
	b.WriteString("\n  " + m.titleStyle.Render(fmt.Sprintf("%s Profile: %s", action, m.profile.Name)) + "\n\n")

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

	// Fields
	fields := m.fields[m.activeTab]
	for i, f := range fields {
		cursor := "  "
		if i == m.fieldCursor {
			cursor = m.accentStyle.Render("> ")
		}

		label := m.dimStyle.Render(fmt.Sprintf("%-20s", f.Label))
		value := f.Value
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

		b.WriteString(fmt.Sprintf("%s%s  %s\n", cursor, label, value))
	}

	// Status bar
	b.WriteString("\n")
	var keys []string
	if m.editing {
		keys = []string{
			m.statusKey.Render("enter") + " confirm",
			m.statusKey.Render("esc") + " cancel",
		}
	} else {
		keys = []string{
			m.statusKey.Render("enter") + " edit",
			m.statusKey.Render("tab") + " next tab",
			m.statusKey.Render("s") + " save",
			m.statusKey.Render("esc") + " back",
		}
	}
	if m.width > 0 {
		b.WriteString(m.statusStyle.Width(m.width).Render(strings.Join(keys, "  ")))
	} else {
		b.WriteString(m.statusStyle.Render(strings.Join(keys, "  ")))
	}

	return b.String()
}
