package shared

import "github.com/charmbracelet/lipgloss"

const logoArt = "   ╻╻┏━╸\n   ┃┃┃╺┓\n  ╺┛╹┗━┛"

// RenderHeader renders the shared app header with the jig logo, title, and
// an optional screen-specific subtitle.
func RenderHeader(titleStyle, dimStyle lipgloss.Style, subtitle string) string {
	logo := titleStyle.Render(logoArt)

	whiteBold := lipgloss.NewStyle().Bold(true).Foreground(lipgloss.Color("15"))
	tagline := whiteBold.Render("Intelligent Context Utilization")
	configurator := dimStyle.Render("Claude Code Session Configurator")

	var text string
	if subtitle != "" {
		text = "\n" + tagline + "\n" + configurator + "\n" + dimStyle.Render(subtitle)
	} else {
		text = "\n" + tagline + "\n" + configurator
	}

	return "\n" + lipgloss.JoinHorizontal(lipgloss.Top, logo, "  ", text) + "\n\n"
}
