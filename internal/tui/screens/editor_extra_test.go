package screens

import (
	"strings"
	"testing"

	tea "github.com/charmbracelet/bubbletea"
	"github.com/jdforsythe/jig/internal/config"
	"github.com/jdforsythe/jig/internal/scanner"
	"github.com/jdforsythe/jig/internal/tui/shared"
)

// testEditorPicker creates a picker-mode EditorModel for tests.
func testEditorPicker(disc *scanner.Discovery) EditorModel {
	title, active, tab, normal, dim, status, key, accent := zeroStyles()
	return NewEditorPicker("/tmp/test-picker", disc, nil, title, active, tab, normal, dim, status, key, accent)
}

// ── mcpServerRefContains ──────────────────────────────────────────────────────

func TestMCPServerRefContains_ByRef(t *testing.T) {
	entries := []config.MCPServerEntry{{Ref: "srv"}}
	if !mcpServerRefContains(entries, "srv") {
		t.Error("should match entry by Ref field")
	}
}

func TestMCPServerRefContains_ByName(t *testing.T) {
	entries := []config.MCPServerEntry{{Name: "srv"}}
	if !mcpServerRefContains(entries, "srv") {
		t.Error("should match entry by Name field when Ref is empty")
	}
}

func TestMCPServerRefContains_NotFound(t *testing.T) {
	entries := []config.MCPServerEntry{{Ref: "other"}}
	if mcpServerRefContains(entries, "srv") {
		t.Error("should return false when no entry matches")
	}
}

func TestMCPServerRefContains_NilSlice(t *testing.T) {
	if mcpServerRefContains(nil, "srv") {
		t.Error("nil slice should return false")
	}
}

// ── buildDiscItems — MCP servers ──────────────────────────────────────────────

func TestBuildDiscItems_IncludesMCPServers(t *testing.T) {
	disc := &scanner.Discovery{
		MCPServers: []scanner.ServerInfo{{Name: "my-srv", Source: "user"}},
	}
	m := testEditor(&config.Profile{Name: "test"}, disc)
	if got := countCategory(m.discItems, "MCP Server"); got != 1 {
		t.Errorf("MCP Server count = %d, want 1", got)
	}
	if m.discItems[0].Category != "MCP Server" {
		t.Errorf("item category = %q, want MCP Server", m.discItems[0].Category)
	}
}

func TestBuildDiscItems_MCPPluginSourcesExcluded(t *testing.T) {
	disc := &scanner.Discovery{
		MCPServers: []scanner.ServerInfo{
			{Name: "regular", Source: "user"},
			{Name: "plugin-srv", Source: "forge@marketplace"},
		},
	}
	m := testEditor(&config.Profile{Name: "test"}, disc)
	if got := countCategory(m.discItems, "MCP Server"); got != 1 {
		t.Errorf("MCP Server count = %d, want 1 (plugin source excluded)", got)
	}
	if m.discItems[0].Name != "regular" {
		t.Errorf("discItems[0].Name = %q, want regular", m.discItems[0].Name)
	}
}

func TestBuildDiscItems_MCPPreSelected(t *testing.T) {
	disc := &scanner.Discovery{
		MCPServers: []scanner.ServerInfo{{Name: "my-srv", Source: "user"}},
	}
	p := &config.Profile{
		Name:       "test",
		MCPServers: []config.MCPServerEntry{{Ref: "my-srv"}},
	}
	m := testEditor(p, disc)

	selMap := make(map[string]bool)
	for _, item := range m.discItems {
		if item.Category == "MCP Server" {
			selMap[item.Name] = item.Selected
		}
	}
	if !selMap["my-srv"] {
		t.Error("my-srv should be pre-selected (Ref in profile.MCPServers)")
	}
}

func TestBuildDiscItems_MCPNotPreSelected(t *testing.T) {
	disc := &scanner.Discovery{
		MCPServers: []scanner.ServerInfo{{Name: "other-srv", Source: "user"}},
	}
	m := testEditor(&config.Profile{Name: "test"}, disc)
	for _, item := range m.discItems {
		if item.Category == "MCP Server" && item.Selected {
			t.Errorf("other-srv should NOT be pre-selected (not in profile)")
		}
	}
}

// ── applyFields — MCP server merging ─────────────────────────────────────────

func TestApplyFields_SelectedMCPWrittenToProfile(t *testing.T) {
	disc := &scanner.Discovery{
		MCPServers: []scanner.ServerInfo{{Name: "db-srv", Source: "user"}},
	}
	m := testEditor(&config.Profile{Name: "test"}, disc)
	for i := range m.discItems {
		if m.discItems[i].Category == "MCP Server" {
			m.discItems[i].Selected = true
		}
	}
	m.applyFields()

	if len(m.profile.MCPServers) != 1 || m.profile.MCPServers[0].Ref != "db-srv" {
		t.Errorf("MCPServers = %v, want [{Ref: db-srv}]", m.profile.MCPServers)
	}
}

func TestApplyFields_UnselectedMCPRemovedFromProfile(t *testing.T) {
	disc := &scanner.Discovery{
		MCPServers: []scanner.ServerInfo{{Name: "db-srv", Source: "user"}},
	}
	p := &config.Profile{
		Name:       "test",
		MCPServers: []config.MCPServerEntry{{Ref: "db-srv"}},
	}
	m := testEditor(p, disc)
	for i := range m.discItems {
		if m.discItems[i].Category == "MCP Server" {
			m.discItems[i].Selected = false
		}
	}
	m.applyFields()

	if len(m.profile.MCPServers) != 0 {
		t.Errorf("MCPServers = %v, want empty (deselected)", m.profile.MCPServers)
	}
}

func TestApplyFields_NonDiscoveredMCPPreserved(t *testing.T) {
	// Manually-added server not in any disc file should survive applyFields.
	disc := &scanner.Discovery{
		MCPServers: []scanner.ServerInfo{{Name: "discovered-srv", Source: "user"}},
	}
	p := &config.Profile{
		Name:       "test",
		MCPServers: []config.MCPServerEntry{{Ref: "manual-srv"}},
	}
	m := testEditor(p, disc)
	// leave discovered-srv unselected; manual-srv is not in discItems at all
	m.applyFields()

	found := false
	for _, e := range m.profile.MCPServers {
		if e.Ref == "manual-srv" {
			found = true
		}
	}
	if !found {
		t.Errorf("MCPServers = %v, manual-srv should be preserved (not in disc)", m.profile.MCPServers)
	}
}

func TestApplyFields_MCPDeduplicatesOnReapply(t *testing.T) {
	disc := &scanner.Discovery{
		MCPServers: []scanner.ServerInfo{{Name: "db-srv", Source: "user"}},
	}
	m := testEditor(&config.Profile{Name: "test"}, disc)
	for i := range m.discItems {
		if m.discItems[i].Category == "MCP Server" {
			m.discItems[i].Selected = true
		}
	}
	m.applyFields()
	m.applyFields() // second call must not add duplicates

	count := 0
	for _, e := range m.profile.MCPServers {
		if e.Ref == "db-srv" {
			count++
		}
	}
	if count != 1 {
		t.Errorf("db-srv appears %d times after double applyFields, want 1", count)
	}
}

func TestApplyFields_NilDiscMCPPreserved(t *testing.T) {
	p := &config.Profile{
		Name:       "test",
		MCPServers: []config.MCPServerEntry{{Ref: "manual-srv"}},
	}
	m := testEditor(p, nil)
	m.applyFields()

	if len(m.profile.MCPServers) != 1 || m.profile.MCPServers[0].Ref != "manual-srv" {
		t.Errorf("MCPServers = %v, want [{Ref: manual-srv}] (nil disc should preserve existing entries)", m.profile.MCPServers)
	}
}

// ── Field.Disabled + edit gating ─────────────────────────────────────────────

func TestUpdate_EnterOnDisabledFieldNoOp(t *testing.T) {
	m := testEditorPicker(nil)
	// In picker mode, Name (index 0) is disabled.
	m.fieldCursor = 0

	model, _ := m.Update(pressSpecial(tea.KeyEnter))
	updated := model.(EditorModel)
	if updated.editing {
		t.Error("enter on disabled field should not enter edit mode")
	}
}

func TestUpdate_EnterOnEnabledFieldOpensEdit(t *testing.T) {
	m := testEditor(&config.Profile{Name: "test"}, nil)
	// "Extends" field (index 2) is enabled and has no Options — enter opens edit mode.
	m.fieldCursor = 2

	model, _ := m.Update(pressSpecial(tea.KeyEnter))
	updated := model.(EditorModel)
	if !updated.editing {
		t.Error("enter on enabled field should enter edit mode")
	}
}

// ── NewEditorPicker ───────────────────────────────────────────────────────────

func TestNewEditorPicker_PickerModeSet(t *testing.T) {
	m := testEditorPicker(nil)
	if !m.pickerMode {
		t.Error("pickerMode should be true")
	}
}

func TestNewEditorPicker_NameDisabled(t *testing.T) {
	m := testEditorPicker(nil)
	if !m.fields[TabGeneral][0].Disabled {
		t.Error("Name field (index 0) should be disabled in picker mode")
	}
}

func TestNewEditorPicker_DescriptionDisabled(t *testing.T) {
	m := testEditorPicker(nil)
	if !m.fields[TabGeneral][1].Disabled {
		t.Error("Description field (index 1) should be disabled in picker mode")
	}
}

func TestNewEditorPicker_ProfileName(t *testing.T) {
	m := testEditorPicker(nil)
	if m.profile.Name != "ad-hoc" {
		t.Errorf("profile.Name = %q, want ad-hoc", m.profile.Name)
	}
}

func TestNewEditorPicker_DefaultEffortAndPermission(t *testing.T) {
	m := testEditorPicker(nil)
	if m.profile.Effort != "high" {
		t.Errorf("profile.Effort = %q, want high", m.profile.Effort)
	}
	if m.profile.PermissionMode != "default" {
		t.Errorf("profile.PermissionMode = %q, want default", m.profile.PermissionMode)
	}
}

func TestNewEditorPicker_MCPServersInDiscItems(t *testing.T) {
	disc := &scanner.Discovery{
		MCPServers: []scanner.ServerInfo{{Name: "test-srv", Source: "user"}},
	}
	m := testEditorPicker(disc)
	if got := countCategory(m.discItems, "MCP Server"); got != 1 {
		t.Errorf("MCP Server count = %d, want 1", got)
	}
}

// ── handleSave / handleBack ───────────────────────────────────────────────────

func TestHandleSave_PickerMode_EmitsLaunchMsg(t *testing.T) {
	m := testEditorPicker(nil)
	_, cmd := m.handleSave()
	if cmd == nil {
		t.Fatal("handleSave in picker mode should return a command")
	}
	msg := cmd()
	launchMsg, ok := msg.(shared.LaunchProfileMsg)
	if !ok {
		t.Fatalf("handleSave returned %T, want LaunchProfileMsg", msg)
	}
	if launchMsg.ProfileName != "ad-hoc" {
		t.Errorf("ProfileName = %q, want ad-hoc", launchMsg.ProfileName)
	}
	if launchMsg.Profile == nil {
		t.Error("Profile should be non-nil in LaunchProfileMsg")
	}
}

func TestHandleSave_EditorMode_EmitsSwitchToHome(t *testing.T) {
	cwd := t.TempDir()
	title, active, tab, normal, dim, status, key, accent := zeroStyles()
	p := &config.Profile{Name: "test-profile"}
	m := NewEditor(p, cwd, nil, nil, title, active, tab, normal, dim, status, key, accent)

	_, cmd := m.handleSave()
	if cmd == nil {
		t.Fatal("handleSave in editor mode should return a command")
	}
	msg := cmd()
	switchMsg, ok := msg.(shared.SwitchScreenMsg)
	if !ok {
		t.Fatalf("handleSave returned %T, want SwitchScreenMsg", msg)
	}
	if switchMsg.Screen != shared.ScreenHome {
		t.Errorf("Screen = %d, want ScreenHome (%d)", switchMsg.Screen, shared.ScreenHome)
	}
}

func TestHandleBack_PickerMode_Quits(t *testing.T) {
	m := testEditorPicker(nil)
	_, cmd := m.handleBack()
	if cmd == nil {
		t.Fatal("handleBack in picker mode should return a command")
	}
	msg := cmd()
	if _, ok := msg.(tea.QuitMsg); !ok {
		t.Fatalf("handleBack in picker mode returned %T, want tea.QuitMsg", msg)
	}
}

func TestHandleBack_EditorMode_SwitchesToHome(t *testing.T) {
	m := testEditor(&config.Profile{Name: "test"}, nil)
	_, cmd := m.handleBack()
	if cmd == nil {
		t.Fatal("handleBack in editor mode should return a command")
	}
	msg := cmd()
	switchMsg, ok := msg.(shared.SwitchScreenMsg)
	if !ok {
		t.Fatalf("handleBack in editor mode returned %T, want SwitchScreenMsg", msg)
	}
	if switchMsg.Screen != shared.ScreenHome {
		t.Errorf("Screen = %d, want ScreenHome (%d)", switchMsg.Screen, shared.ScreenHome)
	}
}

// ── View — picker mode rendering ─────────────────────────────────────────────

func TestView_PickerMode_Header(t *testing.T) {
	m := testEditorPicker(nil)
	view := m.View()
	if !strings.Contains(view, "Ad-hoc Picker") {
		t.Errorf("picker mode View should contain 'Ad-hoc Picker', got:\n%s", view)
	}
	if strings.Contains(view, "Profile:") {
		t.Errorf("picker mode View should NOT contain 'Profile:', got:\n%s", view)
	}
}

func TestView_PickerMode_StatusLaunch(t *testing.T) {
	m := testEditorPicker(nil)
	view := m.View()
	if !strings.Contains(view, "launch") {
		t.Errorf("picker mode status bar should contain 'launch', got:\n%s", view)
	}
	if !strings.Contains(view, "quit") {
		t.Errorf("picker mode status bar should contain 'quit', got:\n%s", view)
	}
}

func TestView_EditorMode_StatusSave(t *testing.T) {
	m := testEditor(&config.Profile{Name: "test"}, nil)
	view := m.View()
	if !strings.Contains(view, "save") {
		t.Errorf("editor mode status bar should contain 'save', got:\n%s", view)
	}
	if !strings.Contains(view, "back") {
		t.Errorf("editor mode status bar should contain 'back', got:\n%s", view)
	}
}
