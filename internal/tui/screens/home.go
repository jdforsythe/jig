package screens

import (
	"fmt"
	"strings"

	tea "github.com/charmbracelet/bubbletea"
	"github.com/charmbracelet/lipgloss"
	"github.com/jforsythe/jig/internal/config"
	"github.com/jforsythe/jig/internal/tui/shared"
)

// HomeModel is the main profile list screen.
type HomeModel struct {
	profiles        []config.Profile
	cursor          int
	width           int
	height          int
	nameStyle       lipgloss.Style
	descStyle       lipgloss.Style
	sourceStyle     lipgloss.Style
	selectedStyle   lipgloss.Style
	dimStyle        lipgloss.Style
	statusStyle     lipgloss.Style
	statusKey       lipgloss.Style
	titleStyle      lipgloss.Style
	confirmingDelete bool
	deleteIdx        int
}

// NewHome creates the home screen model.
func NewHome(profiles []config.Profile, nameStyle, descStyle, sourceStyle, selectedStyle, dimStyle, statusStyle, statusKey, titleStyle lipgloss.Style) HomeModel {
	return HomeModel{
		profiles:      profiles,
		nameStyle:     nameStyle,
		descStyle:     descStyle,
		sourceStyle:   sourceStyle,
		selectedStyle: selectedStyle,
		dimStyle:      dimStyle,
		statusStyle:   statusStyle,
		statusKey:     statusKey,
		titleStyle:    titleStyle,
	}
}

func (m HomeModel) SetSize(w, h int) HomeModel {
	m.width = w
	m.height = h
	return m
}

func (m HomeModel) Init() tea.Cmd { return nil }

func (m HomeModel) Update(msg tea.Msg) (tea.Model, tea.Cmd) {
	switch msg := msg.(type) {
	case tea.KeyMsg:
		if m.confirmingDelete {
			switch msg.String() {
			case "y", "Y":
				p := m.profiles[m.deleteIdx]
				m.confirmingDelete = false
				return m, func() tea.Msg {
					return shared.DeleteProfileMsg{Name: p.Name, Global: p.Source() == config.SourceGlobal}
				}
			default:
				m.confirmingDelete = false
				return m, nil
			}
		}
		switch msg.String() {
		case shared.KeyUp, "k":
			if m.cursor > 0 {
				m.cursor--
			}
		case shared.KeyDown, "j":
			if m.cursor < len(m.profiles)-1 {
				m.cursor++
			}
		case shared.KeyEnter:
			if len(m.profiles) > 0 {
				name := m.profiles[m.cursor].Name
				return m, func() tea.Msg {
					return shared.LaunchProfileMsg{ProfileName: name}
				}
			}
		case shared.KeyN:
			return m, func() tea.Msg {
				return shared.SwitchScreenMsg{Screen: shared.ScreenEditor, Profile: nil}
			}
		case shared.KeyE:
			if len(m.profiles) > 0 {
				p := m.profiles[m.cursor]
				return m, func() tea.Msg {
					return shared.SwitchScreenMsg{Screen: shared.ScreenEditor, Profile: &p}
				}
			}
		case shared.KeyD:
			if len(m.profiles) > 0 {
				m.confirmingDelete = true
				m.deleteIdx = m.cursor
				return m, nil
			}
		case "v":
			if len(m.profiles) > 0 {
				p := m.profiles[m.cursor]
				return m, func() tea.Msg {
					return shared.SwitchScreenMsg{Screen: shared.ScreenPreview, Profile: &p}
				}
			}
		case shared.KeyQ:
			return m, tea.Quit
		}
	}
	return m, nil
}

func (m HomeModel) View() string {
	var b strings.Builder

	b.WriteString(shared.RenderHeader(m.titleStyle, m.dimStyle, ""))

	if len(m.profiles) == 0 {
		b.WriteString("  No profiles found.\n")
		b.WriteString(m.dimStyle.Render("  Press n to create one, or run: jig profiles create <name>") + "\n")
	} else {
		for i, p := range m.profiles {
			cursor := "  "
			name := m.nameStyle.Render(p.Name)
			if i == m.cursor {
				cursor = m.selectedStyle.Render("> ")
				name = m.selectedStyle.Render(p.Name)
			}

			source := ""
			switch p.Source() {
			case config.SourceGlobal:
				source = m.sourceStyle.Render(" (global)")
			case config.SourceProject:
				source = m.sourceStyle.Render(" (project)")
			case config.SourceShortcut:
				source = m.sourceStyle.Render(" (shortcut)")
			}

			desc := ""
			if p.Description != "" {
				desc = m.descStyle.Render(" - " + p.Description)
			}

			b.WriteString(fmt.Sprintf("%s%s%s%s\n", cursor, name, source, desc))
		}
	}

	// Confirm-delete overlay
	if m.confirmingDelete && m.deleteIdx < len(m.profiles) {
		name := m.profiles[m.deleteIdx].Name
		b.WriteString("\n  " + m.dimStyle.Render(fmt.Sprintf("Delete %q? ", name)) + m.statusKey.Render("[y]") + m.dimStyle.Render("es / ") + m.statusKey.Render("[n]") + m.dimStyle.Render("o") + "\n")
		return b.String()
	}

	// Status bar
	b.WriteString("\n")
	keys := []string{
		m.statusKey.Render("enter") + " launch",
		m.statusKey.Render("n") + " new",
		m.statusKey.Render("e") + " edit",
		m.statusKey.Render("d") + " delete",
		m.statusKey.Render("v") + " preview",
		m.statusKey.Render("q") + " quit",
	}
	if m.width > 0 {
		b.WriteString(m.statusStyle.Width(m.width).Render(strings.Join(keys, "  ")))
	} else {
		b.WriteString(m.statusStyle.Render(strings.Join(keys, "  ")))
	}

	return b.String()
}
