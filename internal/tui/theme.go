package tui

import (
	"os"

	"github.com/charmbracelet/lipgloss"
	"github.com/muesli/termenv"
)

// Theme holds all styles used in the TUI.
type Theme struct {
	Title          lipgloss.Style
	Subtitle       lipgloss.Style
	Selected       lipgloss.Style
	Normal         lipgloss.Style
	Dimmed         lipgloss.Style
	Accent         lipgloss.Style
	Error          lipgloss.Style
	Success        lipgloss.Style
	StatusBar      lipgloss.Style
	StatusKey      lipgloss.Style
	Tab            lipgloss.Style
	ActiveTab      lipgloss.Style
	Border         lipgloss.Style
	Preview        lipgloss.Style
	ProfileName    lipgloss.Style
	ProfileDesc    lipgloss.Style
	ProfileSource  lipgloss.Style
	Cursor         lipgloss.Style
	CheckboxOn     lipgloss.Style
	CheckboxOff    lipgloss.Style
}

// NewTheme creates a theme with adaptive colors.
func NewTheme() Theme {
	noColor := os.Getenv("NO_COLOR") != ""

	accent := lipgloss.Color("63")    // purple
	highlight := lipgloss.Color("39") // blue
	success := lipgloss.Color("42")   // green
	danger := lipgloss.Color("196")   // red
	dim := lipgloss.Color("240")      // gray

	if noColor {
		// Use ANSI profile so bold/underline escape codes are still rendered.
		// The default NO_COLOR behavior sets profile to Ascii which strips everything.
		r := lipgloss.NewRenderer(os.Stdout, termenv.WithProfile(termenv.ANSI))
		return Theme{
			Title:          r.NewStyle().Bold(true),
			Subtitle:       r.NewStyle(),
			Selected:       r.NewStyle().Bold(true),
			Normal:         r.NewStyle(),
			Dimmed:         r.NewStyle(),
			Accent:         r.NewStyle().Bold(true),
			Error:          r.NewStyle().Bold(true),
			Success:        r.NewStyle().Bold(true),
			StatusBar:      r.NewStyle(),
			StatusKey:      r.NewStyle().Bold(true),
			Tab:            r.NewStyle(),
			ActiveTab:      r.NewStyle().Bold(true).Underline(true),
			Border:         r.NewStyle(),
			Preview:        r.NewStyle(),
			ProfileName:    r.NewStyle().Bold(true),
			ProfileDesc:    r.NewStyle(),
			ProfileSource:  r.NewStyle(),
			Cursor:         r.NewStyle().Bold(true),
			CheckboxOn:     r.NewStyle().Bold(true),
			CheckboxOff:    r.NewStyle(),
		}
	}

	return Theme{
		Title:         lipgloss.NewStyle().Bold(true).Foreground(accent),
		Subtitle:      lipgloss.NewStyle().Foreground(dim),
		Selected:      lipgloss.NewStyle().Bold(true).Foreground(highlight),
		Normal:        lipgloss.NewStyle(),
		Dimmed:        lipgloss.NewStyle().Foreground(dim),
		Accent:        lipgloss.NewStyle().Foreground(accent),
		Error:         lipgloss.NewStyle().Foreground(danger).Bold(true),
		Success:       lipgloss.NewStyle().Foreground(success),
		StatusBar:     lipgloss.NewStyle().Background(lipgloss.Color("236")).Padding(0, 1),
		StatusKey:     lipgloss.NewStyle().Bold(true).Foreground(accent),
		Tab:           lipgloss.NewStyle().Padding(0, 2).Foreground(dim),
		ActiveTab:     lipgloss.NewStyle().Padding(0, 2).Bold(true).Foreground(highlight).Underline(true),
		Border:        lipgloss.NewStyle().Border(lipgloss.RoundedBorder()).BorderForeground(dim),
		Preview:       lipgloss.NewStyle().Padding(1, 2),
		ProfileName:   lipgloss.NewStyle().Bold(true).Foreground(highlight),
		ProfileDesc:   lipgloss.NewStyle().Foreground(dim),
		ProfileSource: lipgloss.NewStyle().Foreground(dim).Italic(true),
		Cursor:        lipgloss.NewStyle().Bold(true).Foreground(accent),
		CheckboxOn:    lipgloss.NewStyle().Foreground(success),
		CheckboxOff:   lipgloss.NewStyle().Foreground(dim),
	}
}
