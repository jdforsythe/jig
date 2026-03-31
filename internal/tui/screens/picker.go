package screens

import (
	"fmt"
	"strings"

	tea "github.com/charmbracelet/bubbletea"
	"github.com/charmbracelet/lipgloss"
	"github.com/jdforsythe/jig/internal/config"
	"github.com/jdforsythe/jig/internal/scanner"
	"github.com/jdforsythe/jig/internal/tui/shared"
)

// isPluginSource returns true if source is a plugin key (not "user" or "project").
func isPluginSource(source string) bool {
	return source != "" && source != "user" && source != "project"
}

// PickerItem is a selectable item in the picker.
type PickerItem struct {
	Name     string
	Category string
	Source   string // "user", "project", or plugin key
	Path     string // filesystem path for skills/agents/commands
	Selected bool
}

// PickerModel is the ad-hoc multi-column checklist selector.
type PickerModel struct {
	items        []PickerItem
	filtered     []int // indices into items
	cursor       int
	filter       string
	filtering    bool
	width        int
	height       int
	titleStyle   lipgloss.Style
	accentStyle  lipgloss.Style
	dimStyle     lipgloss.Style
	successStyle lipgloss.Style
	statusStyle  lipgloss.Style
	statusKey    lipgloss.Style
}

// NewPicker creates the ad-hoc picker from scanned resources.
func NewPicker(disc *scanner.Discovery, titleStyle, accentStyle, dimStyle, successStyle, statusStyle, statusKey lipgloss.Style) PickerModel {
	var items []PickerItem

	for _, s := range disc.MCPServers {
		items = append(items, PickerItem{Name: s.Name, Category: "MCP Server", Source: s.Source})
	}
	for _, s := range disc.Skills {
		items = append(items, PickerItem{Name: s.Name, Category: "Skill", Source: s.Source, Path: s.Path})
	}
	for _, a := range disc.Agents {
		items = append(items, PickerItem{Name: a.Name, Category: "Agent", Source: a.Source, Path: a.Path})
	}
	for _, c := range disc.Commands {
		items = append(items, PickerItem{Name: c.Name, Category: "Command", Source: c.Source, Path: c.Path})
	}

	// Add components from installed plugins.
	for _, pi := range disc.Plugins {
		for _, name := range pi.Components.MCPServers {
			items = append(items, PickerItem{Name: name, Category: "MCP Server", Source: pi.Key})
		}
		for _, name := range pi.Components.Skills {
			items = append(items, PickerItem{Name: name, Category: "Skill", Source: pi.Key})
		}
		for _, name := range pi.Components.Agents {
			items = append(items, PickerItem{Name: name, Category: "Agent", Source: pi.Key})
		}
		for _, name := range pi.Components.Commands {
			items = append(items, PickerItem{Name: name, Category: "Command", Source: pi.Key})
		}
	}

	m := PickerModel{
		items:        items,
		titleStyle:   titleStyle,
		accentStyle:  accentStyle,
		dimStyle:     dimStyle,
		successStyle: successStyle,
		statusStyle:  statusStyle,
		statusKey:    statusKey,
	}
	m.applyFilter()
	return m
}

func (m *PickerModel) applyFilter() {
	m.filtered = nil
	for i, item := range m.items {
		if m.filter == "" || strings.Contains(strings.ToLower(item.Name), strings.ToLower(m.filter)) || strings.Contains(strings.ToLower(item.Category), strings.ToLower(m.filter)) {
			m.filtered = append(m.filtered, i)
		}
	}
	if m.cursor >= len(m.filtered) {
		m.cursor = len(m.filtered) - 1
	}
	if m.cursor < 0 {
		m.cursor = 0
	}
}

func (m PickerModel) SetSize(w, h int) PickerModel {
	m.width = w
	m.height = h
	return m
}

func (m PickerModel) Init() tea.Cmd { return nil }

func (m PickerModel) Update(msg tea.Msg) (tea.Model, tea.Cmd) {
	switch msg := msg.(type) {
	case tea.KeyMsg:
		if m.filtering {
			return m.updateFilter(msg)
		}

		switch msg.String() {
		case shared.KeyUp, "k":
			if m.cursor > 0 {
				m.cursor--
			}
		case shared.KeyDown, "j":
			if m.cursor < len(m.filtered)-1 {
				m.cursor++
			}
		case shared.KeySpace:
			if len(m.filtered) > 0 {
				idx := m.filtered[m.cursor]
				m.items[idx].Selected = !m.items[idx].Selected
			}
		case shared.KeySlash:
			m.filtering = true
			m.filter = ""
		case shared.KeyEnter:
			p := m.buildProfile()
			return m, func() tea.Msg {
				return shared.LaunchProfileMsg{ProfileName: "ad-hoc", Profile: p}
			}
		case shared.KeyEsc, shared.KeyQ:
			return m, tea.Quit
		}
	}
	return m, nil
}

func (m PickerModel) updateFilter(msg tea.KeyMsg) (tea.Model, tea.Cmd) {
	switch msg.String() {
	case shared.KeyEnter, shared.KeyEsc:
		m.filtering = false
	case "backspace":
		if len(m.filter) > 0 {
			m.filter = m.filter[:len(m.filter)-1]
			m.applyFilter()
		}
	default:
		if len(msg.String()) == 1 {
			m.filter += msg.String()
			m.applyFilter()
		}
	}
	return m, nil
}

// buildProfile constructs an ad-hoc profile from the selected items.
func (m *PickerModel) buildProfile() *config.Profile {
	p := &config.Profile{Name: "ad-hoc"}
	for _, item := range m.items {
		if !item.Selected {
			continue
		}
		plugin := isPluginSource(item.Source)
		switch item.Category {
		case "MCP Server":
			if plugin {
				if p.PluginComponents == nil {
					p.PluginComponents = make(map[string]config.PluginComponentSelection)
				}
				sel := p.PluginComponents[item.Source]
				sel.MCPServers = append(sel.MCPServers, item.Name)
				p.PluginComponents[item.Source] = sel
			} else {
				p.MCPServers = append(p.MCPServers, config.MCPServerEntry{Ref: item.Name})
			}
		case "Skill":
			if plugin {
				if p.PluginComponents == nil {
					p.PluginComponents = make(map[string]config.PluginComponentSelection)
				}
				sel := p.PluginComponents[item.Source]
				sel.Skills = append(sel.Skills, item.Name)
				p.PluginComponents[item.Source] = sel
			} else {
				p.Skills = append(p.Skills, config.PathEntry{Path: item.Path})
			}
		case "Agent":
			if plugin {
				if p.PluginComponents == nil {
					p.PluginComponents = make(map[string]config.PluginComponentSelection)
				}
				sel := p.PluginComponents[item.Source]
				sel.Agents = append(sel.Agents, item.Name)
				p.PluginComponents[item.Source] = sel
			} else {
				p.Agents = append(p.Agents, config.PathEntry{Path: item.Path})
			}
		case "Command":
			if plugin {
				if p.PluginComponents == nil {
					p.PluginComponents = make(map[string]config.PluginComponentSelection)
				}
				sel := p.PluginComponents[item.Source]
				sel.Commands = append(sel.Commands, item.Name)
				p.PluginComponents[item.Source] = sel
			} else {
				p.Commands = append(p.Commands, config.PathEntry{Path: item.Path})
			}
		}
	}
	return p
}

// sourceLabel returns a human-readable label for a resource source.
func sourceLabel(source string) string {
	switch source {
	case "user":
		return "User"
	case "project":
		return "Project"
	default:
		if source != "" {
			return source
		}
		return "Unknown"
	}
}

// SelectedItems returns all selected items.
func (m PickerModel) SelectedItems() []PickerItem {
	var selected []PickerItem
	for _, item := range m.items {
		if item.Selected {
			selected = append(selected, item)
		}
	}
	return selected
}

func (m PickerModel) View() string {
	var b strings.Builder

	b.WriteString(shared.RenderHeader(m.titleStyle, m.dimStyle, "Ad-hoc Picker"))

	if m.filtering {
		b.WriteString("  " + m.accentStyle.Render("/") + m.filter + "█\n\n")
	}

	// Group items by category, then by source within each category
	lastCat := ""
	lastSrc := ""
	for i, idx := range m.filtered {
		item := m.items[idx]

		if item.Category != lastCat {
			if lastCat != "" {
				b.WriteString("\n")
			}
			b.WriteString("  " + m.accentStyle.Render(item.Category) + "\n")
			lastCat = item.Category
			lastSrc = ""
		}

		if item.Source != "" && item.Source != lastSrc {
			srcLabel := sourceLabel(item.Source)
			b.WriteString("  " + m.dimStyle.Render("── "+srcLabel+" ──") + "\n")
			lastSrc = item.Source
		}

		cursor := "  "
		if i == m.cursor {
			cursor = m.accentStyle.Render("> ")
		}

		check := m.dimStyle.Render("[ ]")
		if item.Selected {
			check = m.successStyle.Render("[x]")
		}

		b.WriteString(fmt.Sprintf("  %s%s %s\n", cursor, check, item.Name))
	}

	if len(m.filtered) == 0 {
		b.WriteString("  " + m.dimStyle.Render("No items found.") + "\n")
	}

	// Status bar
	b.WriteString("\n")
	selected := len(m.SelectedItems())
	keys := []string{
		m.statusKey.Render("space") + " toggle",
		m.statusKey.Render("/") + " filter",
		m.statusKey.Render("enter") + " launch",
		m.statusKey.Render("esc") + " back",
		m.dimStyle.Render(fmt.Sprintf("(%d selected)", selected)),
	}
	if m.width > 0 {
		b.WriteString(m.statusStyle.Width(m.width).Render(strings.Join(keys, "  ")))
	} else {
		b.WriteString(m.statusStyle.Render(strings.Join(keys, "  ")))
	}

	return b.String()
}
