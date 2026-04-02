package shared

import (
	"strings"
	"testing"

	"github.com/charmbracelet/lipgloss"
)

func TestRenderHeader_WithSubtitle(t *testing.T) {
	style := lipgloss.NewStyle()
	subtitle := "my subtitle text"

	out := RenderHeader(style, style, subtitle)

	for _, want := range []string{
		"Intelligent Context Utilization",
		"Claude Code Session Configurator",
		subtitle,
	} {
		if !strings.Contains(out, want) {
			t.Errorf("RenderHeader output missing %q\ngot:\n%s", want, out)
		}
	}
}

func TestRenderHeader_WithoutSubtitle(t *testing.T) {
	style := lipgloss.NewStyle()

	out := RenderHeader(style, style, "")

	if !strings.Contains(out, "Intelligent Context Utilization") {
		t.Errorf("RenderHeader missing tagline\ngot:\n%s", out)
	}
	if !strings.Contains(out, "Claude Code Session Configurator") {
		t.Errorf("RenderHeader missing configurator text\ngot:\n%s", out)
	}
	// Empty subtitle should not add an extra line at the end
	if strings.HasSuffix(strings.TrimRight(out, "\n"), "\n\n") {
		t.Errorf("RenderHeader with empty subtitle has unexpected trailing newlines")
	}
}

func TestRenderHeader_ContainsLogo(t *testing.T) {
	style := lipgloss.NewStyle()
	out := RenderHeader(style, style, "")

	// The logo art contains these rune sequences
	if !strings.Contains(out, "╻") || !strings.Contains(out, "┗━┛") {
		t.Errorf("RenderHeader output missing logo art runes\ngot:\n%s", out)
	}
}
