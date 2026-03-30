package tui

import (
	"fmt"

	tea "github.com/charmbracelet/bubbletea"
	"github.com/jforsythe/jig/internal/config"
	"github.com/jforsythe/jig/internal/plugin"
	"github.com/jforsythe/jig/internal/scanner"
	"github.com/jforsythe/jig/internal/tui/screens"
	"github.com/jforsythe/jig/internal/tui/shared"
)

// App is the root BubbleTea model with screen routing.
type App struct {
	profiles []config.Profile
	cwd      string
	theme    Theme
	screen   shared.Screen
	home     screens.HomeModel
	editor   screens.EditorModel
	preview  screens.PreviewModel
	picker   screens.PickerModel
	result   *shared.Result
	width    int
	height   int
	err      error
}

// New creates a new App starting on the home screen.
func New(profiles []config.Profile, cwd string) *App {
	theme := NewTheme()
	return &App{
		profiles: profiles,
		cwd:      cwd,
		theme:    theme,
		screen:   shared.ScreenHome,
		home:     screens.NewHome(profiles, theme.ProfileName, theme.ProfileDesc, theme.ProfileSource, theme.Selected, theme.Dimmed, theme.StatusBar, theme.StatusKey, theme.Title),
	}
}

// NewPickerApp creates a new App starting on the picker screen.
func NewPickerApp(disc *scanner.Discovery, cwd string) *App {
	theme := NewTheme()
	return &App{
		cwd:    cwd,
		theme:  theme,
		screen: shared.ScreenPicker,
		picker: screens.NewPicker(disc, theme.Title, theme.Accent, theme.Dimmed, theme.Success, theme.StatusBar, theme.StatusKey),
	}
}

// Run starts the TUI and returns the result.
func (a *App) Run() (*shared.Result, error) {
	p := tea.NewProgram(a, tea.WithAltScreen())
	m, err := p.Run()
	if err != nil {
		return nil, err
	}

	app := m.(*App)
	return app.result, nil
}

func (a *App) Init() tea.Cmd {
	return tea.WindowSize()
}

func (a *App) Update(msg tea.Msg) (tea.Model, tea.Cmd) {
	switch msg := msg.(type) {
	case tea.WindowSizeMsg:
		a.width = msg.Width
		a.height = msg.Height
		a.home = a.home.SetSize(msg.Width, msg.Height)
		if a.screen == shared.ScreenPreview {
			a.preview = a.preview.SetSize(msg.Width, msg.Height)
		}
		if a.screen == shared.ScreenEditor {
			a.editor = a.editor.SetSize(msg.Width, msg.Height)
		}
		if a.screen == shared.ScreenPicker {
			a.picker = a.picker.SetSize(msg.Width, msg.Height)
		}
		return a, nil

	case tea.KeyMsg:
		if msg.String() == shared.KeyCtrlC {
			return a, tea.Quit
		}
		// When an error is displayed, any key press dismisses it.
		if a.err != nil {
			a.err = nil
			return a, nil
		}

	case shared.DeleteProfileMsg:
		_ = config.DeleteProfile(msg.Name, a.cwd, msg.Global)
		return a.switchScreen(shared.SwitchScreenMsg{Screen: shared.ScreenHome})

	case shared.SwitchScreenMsg:
		return a.switchScreen(msg)

	case shared.LaunchProfileMsg:
		a.result = &shared.Result{
			ProfileName: msg.ProfileName,
			Profile:     msg.Profile,
		}
		return a, tea.Quit

	case shared.ErrorMsg:
		a.err = msg.Err
		return a, nil
	}

	// Route to current screen
	switch a.screen {
	case shared.ScreenHome:
		home, cmd := a.home.Update(msg)
		a.home = home.(screens.HomeModel)
		return a, cmd
	case shared.ScreenEditor:
		editor, cmd := a.editor.Update(msg)
		a.editor = editor.(screens.EditorModel)
		return a, cmd
	case shared.ScreenPreview:
		preview, cmd := a.preview.Update(msg)
		a.preview = preview.(screens.PreviewModel)
		return a, cmd
	case shared.ScreenPicker:
		picker, cmd := a.picker.Update(msg)
		a.picker = picker.(screens.PickerModel)
		return a, cmd
	}

	return a, nil
}

func (a *App) View() string {
	if a.err != nil {
		return fmt.Sprintf("\n  %s\n\n  Press any key to continue.\n", a.theme.Error.Render(a.err.Error()))
	}

	switch a.screen {
	case shared.ScreenHome:
		return a.home.View()
	case shared.ScreenEditor:
		return a.editor.View()
	case shared.ScreenPreview:
		return a.preview.View()
	case shared.ScreenPicker:
		return a.picker.View()
	default:
		return "Unknown screen"
	}
}

func (a *App) switchScreen(msg shared.SwitchScreenMsg) (*App, tea.Cmd) {
	a.screen = msg.Screen

	switch msg.Screen {
	case shared.ScreenHome:
		// Refresh profiles
		profiles, _ := config.ListProfiles(a.cwd)
		a.profiles = profiles
		a.home = screens.NewHome(profiles, a.theme.ProfileName, a.theme.ProfileDesc, a.theme.ProfileSource, a.theme.Selected, a.theme.Dimmed, a.theme.StatusBar, a.theme.StatusKey, a.theme.Title)
		a.home = a.home.SetSize(a.width, a.height)

	case shared.ScreenEditor:
		p := msg.Profile
		if p == nil {
			p = &config.Profile{}
		}
		plugins, _ := plugin.Resolve() // non-fatal
		a.editor = screens.NewEditor(p, a.cwd, plugins, a.theme.Title, a.theme.ActiveTab, a.theme.Tab, a.theme.Normal, a.theme.Dimmed, a.theme.StatusBar, a.theme.StatusKey, a.theme.Accent)
		a.editor = a.editor.SetSize(a.width, a.height)

	case shared.ScreenPreview:
		p := msg.Profile
		a.preview = screens.NewPreview(p, a.cwd, a.theme.Title, a.theme.Preview, a.theme.StatusBar, a.theme.StatusKey, a.theme.Accent, a.theme.Dimmed)
		a.preview = a.preview.SetSize(a.width, a.height)

	case shared.ScreenPicker:
		// Picker is initialized externally via NewPickerApp; switching back is not supported
	}

	return a, nil
}
