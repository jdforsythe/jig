package shared

import (
	tea "github.com/charmbracelet/bubbletea"
	"github.com/jdforsythe/jig/internal/config"
)

// Screen identifiers.
type Screen int

const (
	ScreenHome Screen = iota
	ScreenEditor
	ScreenPreview
	ScreenPicker
)

// Navigation messages.

// SwitchScreenMsg requests a screen transition.
type SwitchScreenMsg struct {
	Screen  Screen
	Profile *config.Profile
}

// LaunchProfileMsg requests launching a profile.
type LaunchProfileMsg struct {
	ProfileName string
	Profile     *config.Profile // set for ad-hoc launches (picker mode)
}

// ProfilesUpdatedMsg signals that the profile list changed.
type ProfilesUpdatedMsg struct {
	Profiles []config.Profile
}

// ErrorMsg carries an error to display.
type ErrorMsg struct {
	Err error
}

// DeleteProfileMsg requests deleting a profile.
type DeleteProfileMsg struct {
	Name   string
	Global bool
}

// Result is returned from the TUI to the caller.
type Result struct {
	ProfileName string
	Profile     *config.Profile // set for ad-hoc launches (picker mode)
}

// Key constants for the TUI.
const (
	KeyEnter    = "enter"
	KeyEsc      = "esc"
	KeyTab      = "tab"
	KeyShiftTab = "shift+tab"
	KeyUp       = "up"
	KeyDown     = "down"
	KeyLeft     = "left"
	KeyRight    = "right"
	KeySpace    = " "
	KeySlash    = "/"
	KeyQ        = "q"
	KeyN        = "n"
	KeyE        = "e"
	KeyD        = "d"
	KeyP        = "p"
	KeyCtrlC    = "ctrl+c"
)

// IsQuit returns true if the key message is a quit action.
func IsQuit(msg tea.KeyMsg) bool {
	switch msg.String() {
	case KeyQ, KeyCtrlC:
		return true
	}
	return false
}
