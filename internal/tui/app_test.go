package tui

import (
	"errors"
	"os"
	"path/filepath"
	"strings"
	"testing"

	tea "github.com/charmbracelet/bubbletea"
	"github.com/jdforsythe/jig/internal/config"
	"github.com/jdforsythe/jig/internal/scanner"
	"github.com/jdforsythe/jig/internal/tui/shared"
	"gopkg.in/yaml.v3"
)

// ── helpers ───────────────────────────────────────────────────────────────────

// newTestApp builds an App with an isolated filesystem (no real profiles).
func newTestApp(t *testing.T) (*App, string) {
	t.Helper()
	tmpDir := t.TempDir()
	t.Setenv("HOME", tmpDir)
	app := New([]config.Profile{}, tmpDir)
	return app, tmpDir
}

// sendMsg sends a single message through App.Update and returns the updated app.
func sendMsg(a *App, msg tea.Msg) (*App, tea.Cmd) {
	m, cmd := a.Update(msg)
	return m.(*App), cmd
}

// isCmdQuit returns true if the returned tea.Cmd is tea.Quit.
// BubbleTea does not export a way to compare commands directly, so we check
// the function pointer via reflection-free comparison: run the command and
// check the resulting message type.
func isCmdQuit(cmd tea.Cmd) bool {
	if cmd == nil {
		return false
	}
	msg := cmd()
	_, ok := msg.(tea.QuitMsg)
	return ok
}

// writeProfileYAML writes a minimal valid profile YAML into a project profiles dir.
func writeProfileYAML(t *testing.T, profilesDir, name string) {
	t.Helper()
	if err := os.MkdirAll(profilesDir, 0755); err != nil {
		t.Fatalf("MkdirAll: %v", err)
	}
	p := config.Profile{Name: name}
	data, err := yaml.Marshal(&p)
	if err != nil {
		t.Fatalf("yaml.Marshal: %v", err)
	}
	path := filepath.Join(profilesDir, name+".yaml")
	if err := os.WriteFile(path, data, 0644); err != nil {
		t.Fatalf("WriteFile: %v", err)
	}
}

// ── constructor tests ─────────────────────────────────────────────────────────

func TestNew_StartsOnHome(t *testing.T) {
	app, _ := newTestApp(t)
	if app.screen != shared.ScreenHome {
		t.Errorf("New() screen = %v, want ScreenHome", app.screen)
	}
}

func TestNew_StoresProfiles(t *testing.T) {
	tmpDir := t.TempDir()
	t.Setenv("HOME", tmpDir)
	profiles := []config.Profile{{Name: "test-profile"}}
	app := New(profiles, tmpDir)
	if len(app.profiles) != 1 {
		t.Errorf("New() profiles len = %d, want 1", len(app.profiles))
	}
}

func TestNewPickerApp_StartsOnEditor(t *testing.T) {
	tmpDir := t.TempDir()
	t.Setenv("HOME", tmpDir)
	app := NewPickerApp(&scanner.Discovery{}, tmpDir)
	if app.screen != shared.ScreenEditor {
		t.Errorf("NewPickerApp() screen = %v, want ScreenEditor", app.screen)
	}
}

// ── Update: no-filesystem tests ───────────────────────────────────────────────

func TestApp_Update_CtrlC_Quits(t *testing.T) {
	app, _ := newTestApp(t)
	_, cmd := sendMsg(app, tea.KeyMsg{Type: tea.KeyCtrlC})
	if !isCmdQuit(cmd) {
		t.Error("ctrl+c should return tea.Quit cmd")
	}
}

func TestApp_Update_ErrorMsg_SetsErr(t *testing.T) {
	app, _ := newTestApp(t)
	app, _ = sendMsg(app, shared.ErrorMsg{Err: errors.New("something broke")})
	if app.err == nil {
		t.Error("ErrorMsg should set app.err")
	}
	if app.err.Error() != "something broke" {
		t.Errorf("app.err = %v, want 'something broke'", app.err)
	}
}

func TestApp_Update_KeyWhileErr_ClearsErr(t *testing.T) {
	app, _ := newTestApp(t)
	app, _ = sendMsg(app, shared.ErrorMsg{Err: errors.New("oops")})
	if app.err == nil {
		t.Fatal("expected error to be set")
	}
	// Any key press should clear the error
	app, _ = sendMsg(app, tea.KeyMsg{Type: tea.KeyRunes, Runes: []rune("x")})
	if app.err != nil {
		t.Errorf("key press should clear app.err, got %v", app.err)
	}
}

func TestApp_Update_LaunchProfile_SetsResult(t *testing.T) {
	app, _ := newTestApp(t)
	app, cmd := sendMsg(app, shared.LaunchProfileMsg{ProfileName: "myprofile"})

	if app.result == nil {
		t.Fatal("LaunchProfileMsg should set result")
	}
	if app.result.ProfileName != "myprofile" {
		t.Errorf("result.ProfileName = %q, want %q", app.result.ProfileName, "myprofile")
	}
	if !isCmdQuit(cmd) {
		t.Error("LaunchProfileMsg should return tea.Quit cmd")
	}
}

func TestApp_Update_LaunchAdHoc_SetsProfile(t *testing.T) {
	app, _ := newTestApp(t)
	adhoc := &config.Profile{Name: "adhoc"}
	app, cmd := sendMsg(app, shared.LaunchProfileMsg{Profile: adhoc})

	if app.result == nil {
		t.Fatal("LaunchProfileMsg should set result")
	}
	if app.result.Profile == nil {
		t.Error("result.Profile should be set for ad-hoc launch")
	}
	if app.result.Profile.Name != "adhoc" {
		t.Errorf("result.Profile.Name = %q, want %q", app.result.Profile.Name, "adhoc")
	}
	if !isCmdQuit(cmd) {
		t.Error("LaunchProfileMsg should return tea.Quit cmd")
	}
}

func TestApp_Update_WindowSize_UpdatesDimensions(t *testing.T) {
	app, _ := newTestApp(t)
	app, _ = sendMsg(app, tea.WindowSizeMsg{Width: 120, Height: 40})
	if app.width != 120 {
		t.Errorf("width = %d, want 120", app.width)
	}
	if app.height != 40 {
		t.Errorf("height = %d, want 40", app.height)
	}
}

// ── Update: screen-switch tests (filesystem needed) ───────────────────────────

func TestApp_Update_SwitchToHome(t *testing.T) {
	app, _ := newTestApp(t)
	// Start by switching away from home first
	app, _ = sendMsg(app, shared.SwitchScreenMsg{
		Screen:  shared.ScreenPreview,
		Profile: &config.Profile{},
	})
	// Now switch back to home
	app, _ = sendMsg(app, shared.SwitchScreenMsg{Screen: shared.ScreenHome})
	if app.screen != shared.ScreenHome {
		t.Errorf("screen = %v, want ScreenHome", app.screen)
	}
}

func TestApp_Update_SwitchToEditor(t *testing.T) {
	app, _ := newTestApp(t)
	app, _ = sendMsg(app, shared.SwitchScreenMsg{
		Screen:  shared.ScreenEditor,
		Profile: &config.Profile{},
	})
	if app.screen != shared.ScreenEditor {
		t.Errorf("screen = %v, want ScreenEditor", app.screen)
	}
}

func TestApp_Update_SwitchToPreview(t *testing.T) {
	app, _ := newTestApp(t)
	p := &config.Profile{Name: "preview-test"}
	app, _ = sendMsg(app, shared.SwitchScreenMsg{
		Screen:  shared.ScreenPreview,
		Profile: p,
	})
	if app.screen != shared.ScreenPreview {
		t.Errorf("screen = %v, want ScreenPreview", app.screen)
	}
}

func TestApp_Update_DeleteProfile_SwitchesToHome(t *testing.T) {
	tmpDir := t.TempDir()
	t.Setenv("HOME", tmpDir)

	cwd := t.TempDir()
	profilesDir := filepath.Join(cwd, ".jig", "profiles")
	writeProfileYAML(t, profilesDir, "to-delete")

	app := New([]config.Profile{}, cwd)
	// Switch to a different screen first so we can verify the switch back
	app, _ = sendMsg(app, shared.SwitchScreenMsg{
		Screen:  shared.ScreenPreview,
		Profile: &config.Profile{},
	})
	app, _ = sendMsg(app, shared.DeleteProfileMsg{Name: "to-delete", Global: false})

	if app.screen != shared.ScreenHome {
		t.Errorf("after DeleteProfileMsg screen = %v, want ScreenHome", app.screen)
	}

	// Profile file should be gone
	path := filepath.Join(profilesDir, "to-delete.yaml")
	if _, err := os.Stat(path); !os.IsNotExist(err) {
		t.Errorf("profile file should have been deleted: %v", err)
	}
}

// ── View tests ────────────────────────────────────────────────────────────────

func TestApp_View_ErrorState(t *testing.T) {
	app, _ := newTestApp(t)
	app.err = errors.New("test error message")

	view := app.View()

	if !strings.Contains(view, "Press any key to continue") {
		t.Errorf("error view should contain 'Press any key to continue'\ngot: %s", view)
	}
	if !strings.Contains(view, "test error message") {
		t.Errorf("error view should contain the error message\ngot: %s", view)
	}
}

func TestApp_View_UnknownScreen(t *testing.T) {
	app, _ := newTestApp(t)
	app.screen = shared.Screen(99)

	view := app.View()

	if !strings.Contains(view, "Unknown screen") {
		t.Errorf("unknown screen view should contain 'Unknown screen'\ngot: %s", view)
	}
}

func TestApp_View_HomeScreen(t *testing.T) {
	app, _ := newTestApp(t)
	// Should not panic and should return non-empty string
	view := app.View()
	if view == "" {
		t.Error("home screen View() returned empty string")
	}
}

// ── Theme tests ───────────────────────────────────────────────────────────────

func TestNewTheme_NoColor(t *testing.T) {
	t.Setenv("NO_COLOR", "1")
	theme := NewTheme()
	// With NO_COLOR, styles should still be usable (no panic)
	_ = theme.Title.Render("test")
	_ = theme.Error.Render("err")
	_ = theme.ActiveTab.Render("tab")
}

func TestNewTheme_WithColor(t *testing.T) {
	t.Setenv("NO_COLOR", "")
	theme := NewTheme()
	_ = theme.Title.Render("test")
	_ = theme.StatusBar.Render("status")
}
