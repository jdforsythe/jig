package screens

import (
	"fmt"
	"strings"

	tea "github.com/charmbracelet/bubbletea"
	"github.com/charmbracelet/lipgloss"
	"github.com/jforsythe/jig/internal/claude"
	"github.com/jforsythe/jig/internal/config"
	"github.com/jforsythe/jig/internal/tui/shared"
	"gopkg.in/yaml.v3"
)

// PreviewModel shows the resolved profile and generated plugin dir.
type PreviewModel struct {
	profile      *config.Profile
	cwd          string
	content      string
	scroll       int
	width        int
	height       int
	titleStyle   lipgloss.Style
	previewStyle lipgloss.Style
	statusStyle  lipgloss.Style
	statusKey    lipgloss.Style
	accentStyle  lipgloss.Style
	dimStyle     lipgloss.Style
}

// NewPreview creates the preview screen.
func NewPreview(p *config.Profile, cwd string, titleStyle, previewStyle, statusStyle, statusKey, accentStyle, dimStyle lipgloss.Style) PreviewModel {
	m := PreviewModel{
		profile:      p,
		cwd:          cwd,
		titleStyle:   titleStyle,
		previewStyle: previewStyle,
		statusStyle:  statusStyle,
		statusKey:    statusKey,
		accentStyle:  accentStyle,
		dimStyle:     dimStyle,
	}
	m.buildContent()
	return m
}

func (m *PreviewModel) buildContent() {
	var b strings.Builder

	// Resolved profile YAML
	b.WriteString(m.accentStyle.Render("Resolved Profile:") + "\n")
	data, err := yaml.Marshal(m.profile)
	if err != nil {
		b.WriteString(fmt.Sprintf("Error: %v\n", err))
	} else {
		b.WriteString(string(data))
	}

	b.WriteString("\n")

	// CLI args that would be generated
	b.WriteString(m.accentStyle.Render("CLI Arguments:") + "\n")
	args := claude.BuildCLIArgs(m.profile, "<plugin-dir>", nil)
	b.WriteString("claude")
	for _, a := range args {
		if strings.HasPrefix(a, "--") {
			b.WriteString(" \\\n  " + a)
		} else {
			b.WriteString(" " + a)
		}
	}
	b.WriteString("\n")

	m.content = b.String()
}

func (m PreviewModel) SetSize(w, h int) PreviewModel {
	m.width = w
	m.height = h
	return m
}

func (m PreviewModel) Init() tea.Cmd { return nil }

func (m PreviewModel) Update(msg tea.Msg) (tea.Model, tea.Cmd) {
	switch msg := msg.(type) {
	case tea.KeyMsg:
		switch msg.String() {
		case shared.KeyUp, "k":
			if m.scroll > 0 {
				m.scroll--
			}
		case shared.KeyDown, "j":
			m.scroll++
		case shared.KeyEnter:
			name := m.profile.Name
			return m, func() tea.Msg {
				return shared.LaunchProfileMsg{ProfileName: name}
			}
		case shared.KeyEsc, shared.KeyQ:
			return m, func() tea.Msg {
				return shared.SwitchScreenMsg{Screen: shared.ScreenHome}
			}
		}
	}
	return m, nil
}

func (m PreviewModel) View() string {
	var b strings.Builder

	b.WriteString("\n  " + m.titleStyle.Render("Preview: "+m.profile.Name) + "\n\n")

	// Scrollable content
	lines := strings.Split(m.content, "\n")
	viewHeight := m.height - 6
	if viewHeight < 5 {
		viewHeight = 20
	}

	// Clamp scroll
	maxScroll := len(lines) - viewHeight
	if maxScroll < 0 {
		maxScroll = 0
	}
	if m.scroll > maxScroll {
		m.scroll = maxScroll
	}

	end := m.scroll + viewHeight
	if end > len(lines) {
		end = len(lines)
	}

	visible := lines[m.scroll:end]
	for _, line := range visible {
		b.WriteString("  " + line + "\n")
	}

	// Status bar
	keys := []string{
		m.statusKey.Render("enter") + " launch",
		m.statusKey.Render("j/k") + " scroll",
		m.statusKey.Render("esc") + " back",
	}
	if m.width > 0 {
		b.WriteString(m.statusStyle.Width(m.width).Render(strings.Join(keys, "  ")))
	} else {
		b.WriteString(m.statusStyle.Render(strings.Join(keys, "  ")))
	}

	return b.String()
}
