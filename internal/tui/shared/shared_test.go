package shared

import (
	"testing"

	tea "github.com/charmbracelet/bubbletea"
)

func TestIsQuit(t *testing.T) {
	tests := []struct {
		key  string
		want bool
	}{
		{"q", true},
		{"ctrl+c", true},
		{"enter", false},
		{"esc", false},
		{"n", false},
		{"e", false},
		{" ", false},
		{"tab", false},
		{"up", false},
		{"down", false},
	}

	for _, tt := range tests {
		t.Run(tt.key, func(t *testing.T) {
			msg := tea.KeyMsg{Type: tea.KeyRunes, Runes: []rune(tt.key)}
			if tt.key == "ctrl+c" {
				msg = tea.KeyMsg{Type: tea.KeyCtrlC}
			}
			got := IsQuit(msg)
			if got != tt.want {
				t.Errorf("IsQuit(%q) = %v, want %v", tt.key, got, tt.want)
			}
		})
	}
}

func TestScreenConstants(t *testing.T) {
	if ScreenHome != 0 {
		t.Errorf("ScreenHome = %d, want 0", ScreenHome)
	}
	if ScreenEditor != 1 {
		t.Errorf("ScreenEditor = %d, want 1", ScreenEditor)
	}
	if ScreenPreview != 2 {
		t.Errorf("ScreenPreview = %d, want 2", ScreenPreview)
	}
	if ScreenPicker != 3 {
		t.Errorf("ScreenPicker = %d, want 3", ScreenPicker)
	}
}

func TestKeyConstants(t *testing.T) {
	tests := []struct {
		name string
		got  string
		want string
	}{
		{"KeyEnter", KeyEnter, "enter"},
		{"KeyEsc", KeyEsc, "esc"},
		{"KeyTab", KeyTab, "tab"},
		{"KeyShiftTab", KeyShiftTab, "shift+tab"},
		{"KeyUp", KeyUp, "up"},
		{"KeyDown", KeyDown, "down"},
		{"KeyLeft", KeyLeft, "left"},
		{"KeyRight", KeyRight, "right"},
		{"KeySpace", KeySpace, " "},
		{"KeyQ", KeyQ, "q"},
		{"KeyCtrlC", KeyCtrlC, "ctrl+c"},
		{"KeyN", KeyN, "n"},
		{"KeyE", KeyE, "e"},
		{"KeyD", KeyD, "d"},
		{"KeyP", KeyP, "p"},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			if tt.got != tt.want {
				t.Errorf("%s = %q, want %q", tt.name, tt.got, tt.want)
			}
		})
	}
}
