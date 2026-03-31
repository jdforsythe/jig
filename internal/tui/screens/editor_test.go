package screens

import (
	"strings"
	"testing"

	tea "github.com/charmbracelet/bubbletea"
	"github.com/charmbracelet/lipgloss"
	"github.com/jforsythe/jig/internal/config"
	"github.com/jforsythe/jig/internal/scanner"
	"github.com/jforsythe/jig/internal/tui/shared"
)

// ── helpers ──────────────────────────────────────────────────────────────────

func zeroStyles() (lipgloss.Style, lipgloss.Style, lipgloss.Style, lipgloss.Style, lipgloss.Style, lipgloss.Style, lipgloss.Style, lipgloss.Style) {
	s := lipgloss.NewStyle()
	return s, s, s, s, s, s, s, s
}

func testEditor(p *config.Profile, disc *scanner.Discovery) EditorModel {
	title, active, tab, normal, dim, status, key, accent := zeroStyles()
	return NewEditor(p, "/tmp/test-editor", disc, nil, title, active, tab, normal, dim, status, key, accent)
}

func pressSpecial(k tea.KeyType) tea.KeyMsg { return tea.KeyMsg{Type: k} }
func pressRune(r rune) tea.KeyMsg           { return tea.KeyMsg{Type: tea.KeyRunes, Runes: []rune{r}} }

func countCategory(items []PickerItem, cat string) int {
	n := 0
	for _, item := range items {
		if item.Category == cat {
			n++
		}
	}
	return n
}

// ── tab structure ─────────────────────────────────────────────────────────────

func TestTabOrder_PluginsBeforeAdvanced(t *testing.T) {
	if TabPlugins >= TabAdvanced {
		t.Errorf("TabPlugins (%d) must be < TabAdvanced (%d)", TabPlugins, TabAdvanced)
	}
}

func TestTabOrder_ComponentsBeforePlugins(t *testing.T) {
	if TabComponents >= TabPlugins {
		t.Errorf("TabComponents (%d) must be < TabPlugins (%d)", TabComponents, TabPlugins)
	}
}

func TestTabNames(t *testing.T) {
	cases := []struct {
		tab  Tab
		want string
	}{
		{TabGeneral, "General"},
		{TabTools, "Tools"},
		{TabMCP, "MCP Servers"},
		{TabHooks, "Hooks"},
		{TabComponents, "Components"},
		{TabPlugins, "Plugins"},
		{TabAdvanced, "Advanced"},
	}
	for _, tc := range cases {
		if int(tc.tab) >= len(tabNames) {
			t.Errorf("tab %d out of range (len=%d)", tc.tab, len(tabNames))
			continue
		}
		if got := tabNames[tc.tab]; got != tc.want {
			t.Errorf("tabNames[%d] = %q, want %q", tc.tab, got, tc.want)
		}
	}
}

func TestTabNamesCount(t *testing.T) {
	const want = 7 // General, Tools, MCP, Hooks, Components, Plugins, Advanced
	if got := len(tabNames); got != want {
		t.Errorf("len(tabNames) = %d, want %d", got, want)
	}
}

// ── pathEntryContains ─────────────────────────────────────────────────────────

func TestPathEntryContains(t *testing.T) {
	cases := []struct {
		name    string
		entries []config.PathEntry
		path    string
		want    bool
	}{
		{"nil slice", nil, "/a", false},
		{"empty slice", []config.PathEntry{}, "/a", false},
		{"found first", []config.PathEntry{{Path: "/a"}, {Path: "/b"}}, "/a", true},
		{"found last", []config.PathEntry{{Path: "/a"}, {Path: "/b"}}, "/b", true},
		{"not found", []config.PathEntry{{Path: "/a"}, {Path: "/b"}}, "/c", false},
		{"prefix not match", []config.PathEntry{{Path: "/a/b/c"}}, "/a/b", false},
	}
	for _, tc := range cases {
		t.Run(tc.name, func(t *testing.T) {
			if got := pathEntryContains(tc.entries, tc.path); got != tc.want {
				t.Errorf("pathEntryContains() = %v, want %v", got, tc.want)
			}
		})
	}
}

// ── buildDiscItems ────────────────────────────────────────────────────────────

func TestBuildDiscItems_NilDisc(t *testing.T) {
	m := testEditor(&config.Profile{Name: "test"}, nil)
	if len(m.discItems) != 0 {
		t.Errorf("discItems len = %d, want 0 for nil disc", len(m.discItems))
	}
	if len(m.discFiltered) != 0 {
		t.Errorf("discFiltered len = %d, want 0 for nil disc", len(m.discFiltered))
	}
}

func TestBuildDiscItems_SkillsAgentsCommands(t *testing.T) {
	disc := &scanner.Discovery{
		Skills:   []scanner.ResourceInfo{{Name: "s1", Path: "/s1", Source: "user"}},
		Agents:   []scanner.ResourceInfo{{Name: "a1", Path: "/a1.md", Source: "user"}},
		Commands: []scanner.ResourceInfo{{Name: "c1", Path: "/c1.md", Source: "project"}},
	}
	m := testEditor(&config.Profile{Name: "test"}, disc)

	if got := countCategory(m.discItems, "Skill"); got != 1 {
		t.Errorf("Skill count = %d, want 1", got)
	}
	if got := countCategory(m.discItems, "Agent"); got != 1 {
		t.Errorf("Agent count = %d, want 1", got)
	}
	if got := countCategory(m.discItems, "Command"); got != 1 {
		t.Errorf("Command count = %d, want 1", got)
	}
	if len(m.discItems) != 3 {
		t.Errorf("discItems len = %d, want 3", len(m.discItems))
	}
}

func TestBuildDiscItems_ExcludesPluginSources(t *testing.T) {
	disc := &scanner.Discovery{
		Skills: []scanner.ResourceInfo{
			{Name: "user-skill", Path: "/user/s", Source: "user"},
			{Name: "plugin-skill", Path: "/plugin/s", Source: "forge@marketplace"},
		},
		Agents: []scanner.ResourceInfo{
			{Name: "plugin-agent", Path: "/plugin/a.md", Source: "ss-eng@market"},
		},
	}
	m := testEditor(&config.Profile{Name: "test"}, disc)

	if len(m.discItems) != 1 {
		t.Errorf("discItems len = %d, want 1 (plugin sources excluded)", len(m.discItems))
	}
	if m.discItems[0].Name != "user-skill" {
		t.Errorf("discItems[0].Name = %q, want user-skill", m.discItems[0].Name)
	}
}

func TestBuildDiscItems_PreSelectsExistingSkills(t *testing.T) {
	disc := &scanner.Discovery{
		Skills: []scanner.ResourceInfo{
			{Name: "alpha", Path: "/skills/alpha", Source: "user"},
			{Name: "beta", Path: "/skills/beta", Source: "user"},
		},
	}
	p := &config.Profile{
		Name:   "test",
		Skills: []config.PathEntry{{Path: "/skills/alpha"}},
	}
	m := testEditor(p, disc)

	selMap := make(map[string]bool)
	for _, item := range m.discItems {
		selMap[item.Name] = item.Selected
	}
	if !selMap["alpha"] {
		t.Error("alpha should be pre-selected (in profile.Skills)")
	}
	if selMap["beta"] {
		t.Error("beta should NOT be pre-selected")
	}
}

func TestBuildDiscItems_PreSelectsExistingAgentsAndCommands(t *testing.T) {
	disc := &scanner.Discovery{
		Agents:   []scanner.ResourceInfo{{Name: "ag", Path: "/agents/ag.md", Source: "user"}},
		Commands: []scanner.ResourceInfo{{Name: "cmd", Path: "/cmds/cmd.md", Source: "project"}},
	}
	p := &config.Profile{
		Name:     "test",
		Agents:   []config.PathEntry{{Path: "/agents/ag.md"}},
		Commands: []config.PathEntry{}, // cmd NOT in profile
	}
	m := testEditor(p, disc)

	selMap := make(map[string]bool)
	for _, item := range m.discItems {
		selMap[item.Name] = item.Selected
	}
	if !selMap["ag"] {
		t.Error("ag should be pre-selected (in profile.Agents)")
	}
	if selMap["cmd"] {
		t.Error("cmd should NOT be pre-selected (not in profile.Commands)")
	}
}

func TestBuildDiscItems_FilteredMatchesTotal(t *testing.T) {
	// With empty filter, discFiltered should index all discItems.
	disc := &scanner.Discovery{
		Skills: []scanner.ResourceInfo{
			{Name: "x", Path: "/x", Source: "user"},
			{Name: "y", Path: "/y", Source: "user"},
		},
	}
	m := testEditor(&config.Profile{Name: "test"}, disc)
	if len(m.discFiltered) != len(m.discItems) {
		t.Errorf("discFiltered len = %d, want %d (equals discItems with no filter)", len(m.discFiltered), len(m.discItems))
	}
}

// ── applyDiscFilter ───────────────────────────────────────────────────────────

func TestApplyDiscFilter_EmptyFilter_ShowsAll(t *testing.T) {
	disc := &scanner.Discovery{
		Skills: []scanner.ResourceInfo{
			{Name: "alpha", Path: "/a", Source: "user"},
			{Name: "beta", Path: "/b", Source: "user"},
		},
	}
	m := testEditor(&config.Profile{Name: "test"}, disc)
	// Default: no filter
	if len(m.discFiltered) != 2 {
		t.Errorf("discFiltered len = %d, want 2 (no filter)", len(m.discFiltered))
	}
}

func TestApplyDiscFilter_ByName(t *testing.T) {
	disc := &scanner.Discovery{
		Skills: []scanner.ResourceInfo{
			{Name: "alpha", Path: "/a", Source: "user"},
			{Name: "beta", Path: "/b", Source: "user"},
			{Name: "alphabet", Path: "/c", Source: "user"},
		},
	}
	m := testEditor(&config.Profile{Name: "test"}, disc)
	m.discFilter = "alpha"
	m.applyDiscFilter()

	if len(m.discFiltered) != 2 {
		t.Errorf("discFiltered len = %d, want 2 (alpha + alphabet)", len(m.discFiltered))
	}
}

func TestApplyDiscFilter_ByCategory(t *testing.T) {
	disc := &scanner.Discovery{
		Skills: []scanner.ResourceInfo{{Name: "s", Path: "/s", Source: "user"}},
		Agents: []scanner.ResourceInfo{{Name: "a", Path: "/a", Source: "user"}},
	}
	m := testEditor(&config.Profile{Name: "test"}, disc)
	m.discFilter = "skill"
	m.applyDiscFilter()

	if len(m.discFiltered) != 1 {
		t.Errorf("discFiltered len = %d, want 1 (skill only)", len(m.discFiltered))
	}
	if m.discItems[m.discFiltered[0]].Category != "Skill" {
		t.Errorf("filtered item category = %q, want Skill", m.discItems[m.discFiltered[0]].Category)
	}
}

func TestApplyDiscFilter_CaseInsensitive(t *testing.T) {
	disc := &scanner.Discovery{
		Skills: []scanner.ResourceInfo{{Name: "MySkill", Path: "/s", Source: "user"}},
	}
	m := testEditor(&config.Profile{Name: "test"}, disc)
	m.discFilter = "myskill"
	m.applyDiscFilter()

	if len(m.discFiltered) != 1 {
		t.Errorf("discFiltered len = %d, want 1 (case-insensitive match)", len(m.discFiltered))
	}
}

func TestApplyDiscFilter_NoMatch_ClampsCursor(t *testing.T) {
	disc := &scanner.Discovery{
		Skills: []scanner.ResourceInfo{
			{Name: "alpha", Path: "/a", Source: "user"},
			{Name: "beta", Path: "/b", Source: "user"},
		},
	}
	m := testEditor(&config.Profile{Name: "test"}, disc)
	m.discCursor = 1
	m.discFilter = "zzznomatch"
	m.applyDiscFilter()

	if len(m.discFiltered) != 0 {
		t.Errorf("discFiltered len = %d, want 0 (no match)", len(m.discFiltered))
	}
	if m.discCursor != 0 {
		t.Errorf("discCursor = %d after no-match filter, want 0 (clamped)", m.discCursor)
	}
}

// ── applyFields (component slice handling) ────────────────────────────────────

func TestApplyFields_SelectedComponentsWrittenToProfile(t *testing.T) {
	disc := &scanner.Discovery{
		Skills:   []scanner.ResourceInfo{{Name: "s", Path: "/skills/s", Source: "user"}},
		Agents:   []scanner.ResourceInfo{{Name: "a", Path: "/agents/a.md", Source: "user"}},
		Commands: []scanner.ResourceInfo{{Name: "c", Path: "/cmds/c.md", Source: "project"}},
	}
	m := testEditor(&config.Profile{Name: "test"}, disc)
	for i := range m.discItems {
		m.discItems[i].Selected = true
	}
	m.applyFields()

	if len(m.profile.Skills) != 1 || m.profile.Skills[0].Path != "/skills/s" {
		t.Errorf("Skills = %v, want [{/skills/s}]", m.profile.Skills)
	}
	if len(m.profile.Agents) != 1 || m.profile.Agents[0].Path != "/agents/a.md" {
		t.Errorf("Agents = %v, want [{/agents/a.md}]", m.profile.Agents)
	}
	if len(m.profile.Commands) != 1 || m.profile.Commands[0].Path != "/cmds/c.md" {
		t.Errorf("Commands = %v, want [{/cmds/c.md}]", m.profile.Commands)
	}
}

func TestApplyFields_UnselectedNotWritten(t *testing.T) {
	disc := &scanner.Discovery{
		Skills: []scanner.ResourceInfo{
			{Name: "selected", Path: "/a", Source: "user"},
			{Name: "skipped", Path: "/b", Source: "user"},
		},
	}
	m := testEditor(&config.Profile{Name: "test"}, disc)
	m.discItems[0].Selected = true
	// m.discItems[1] remains unselected
	m.applyFields()

	if len(m.profile.Skills) != 1 || m.profile.Skills[0].Path != "/a" {
		t.Errorf("Skills = %v, want only [{/a}]", m.profile.Skills)
	}
}

func TestApplyFields_NilDiscProducesNilSlices(t *testing.T) {
	m := testEditor(&config.Profile{Name: "test"}, nil)
	m.applyFields()

	if m.profile.Skills != nil {
		t.Errorf("Skills should be nil, got %v", m.profile.Skills)
	}
	if m.profile.Agents != nil {
		t.Errorf("Agents should be nil, got %v", m.profile.Agents)
	}
	if m.profile.Commands != nil {
		t.Errorf("Commands should be nil, got %v", m.profile.Commands)
	}
}

func TestApplyFields_ClearsComponentsWhenNoneSelected(t *testing.T) {
	// Start with a profile that has skills, then deselect all → profile.Skills should be nil.
	disc := &scanner.Discovery{
		Skills: []scanner.ResourceInfo{{Name: "s", Path: "/s", Source: "user"}},
	}
	existing := &config.Profile{
		Name:   "test",
		Skills: []config.PathEntry{{Path: "/s"}},
	}
	m := testEditor(existing, disc)
	// Deselect
	for i := range m.discItems {
		m.discItems[i].Selected = false
	}
	m.applyFields()

	if len(m.profile.Skills) != 0 {
		t.Errorf("Skills = %v, want empty (all deselected)", m.profile.Skills)
	}
}

// ── updateComponentsTab ───────────────────────────────────────────────────────

func TestUpdateComponentsTab_SpaceTogglesOn(t *testing.T) {
	disc := &scanner.Discovery{
		Skills: []scanner.ResourceInfo{{Name: "s", Path: "/s", Source: "user"}},
	}
	m := testEditor(&config.Profile{Name: "test"}, disc)
	m.activeTab = TabComponents

	model, _ := m.updateComponentsTab(pressSpecial(tea.KeySpace))
	updated := model.(EditorModel)
	if !updated.discItems[0].Selected {
		t.Error("space should select unselected item")
	}
}

func TestUpdateComponentsTab_SpaceTogglesOff(t *testing.T) {
	disc := &scanner.Discovery{
		Skills: []scanner.ResourceInfo{{Name: "s", Path: "/s", Source: "user"}},
	}
	m := testEditor(&config.Profile{Name: "test"}, disc)
	m.activeTab = TabComponents
	m.discItems[0].Selected = true

	model, _ := m.updateComponentsTab(pressSpecial(tea.KeySpace))
	updated := model.(EditorModel)
	if updated.discItems[0].Selected {
		t.Error("space should deselect selected item")
	}
}

func TestUpdateComponentsTab_SpaceOnEmptyList(t *testing.T) {
	// Should not panic when filtered list is empty.
	m := testEditor(&config.Profile{Name: "test"}, nil)
	m.activeTab = TabComponents

	// Must not panic
	_, _ = m.updateComponentsTab(pressSpecial(tea.KeySpace))
}

func TestUpdateComponentsTab_DownMovesForward(t *testing.T) {
	disc := &scanner.Discovery{
		Skills: []scanner.ResourceInfo{
			{Name: "s1", Path: "/s1", Source: "user"},
			{Name: "s2", Path: "/s2", Source: "user"},
		},
	}
	m := testEditor(&config.Profile{Name: "test"}, disc)
	m.activeTab = TabComponents

	model, _ := m.updateComponentsTab(pressSpecial(tea.KeyDown))
	updated := model.(EditorModel)
	if updated.discCursor != 1 {
		t.Errorf("discCursor = %d after down, want 1", updated.discCursor)
	}
}

func TestUpdateComponentsTab_DownClampsAtEnd(t *testing.T) {
	disc := &scanner.Discovery{
		Skills: []scanner.ResourceInfo{
			{Name: "s1", Path: "/s1", Source: "user"},
			{Name: "s2", Path: "/s2", Source: "user"},
		},
	}
	m := testEditor(&config.Profile{Name: "test"}, disc)
	m.activeTab = TabComponents
	m.discCursor = 1 // at last item

	model, _ := m.updateComponentsTab(pressSpecial(tea.KeyDown))
	updated := model.(EditorModel)
	if updated.discCursor != 1 {
		t.Errorf("discCursor = %d, should stay at 1 (last item)", updated.discCursor)
	}
}

func TestUpdateComponentsTab_UpMovesBack(t *testing.T) {
	disc := &scanner.Discovery{
		Skills: []scanner.ResourceInfo{
			{Name: "s1", Path: "/s1", Source: "user"},
			{Name: "s2", Path: "/s2", Source: "user"},
		},
	}
	m := testEditor(&config.Profile{Name: "test"}, disc)
	m.activeTab = TabComponents
	m.discCursor = 1

	model, _ := m.updateComponentsTab(pressSpecial(tea.KeyUp))
	updated := model.(EditorModel)
	if updated.discCursor != 0 {
		t.Errorf("discCursor = %d after up, want 0", updated.discCursor)
	}
}

func TestUpdateComponentsTab_UpClampsAtStart(t *testing.T) {
	m := testEditor(&config.Profile{Name: "test"}, nil)
	m.activeTab = TabComponents

	model, _ := m.updateComponentsTab(pressSpecial(tea.KeyUp))
	updated := model.(EditorModel)
	if updated.discCursor != 0 {
		t.Errorf("discCursor = %d, should stay at 0 (already at start)", updated.discCursor)
	}
}

func TestUpdateComponentsTab_JDownKUp(t *testing.T) {
	disc := &scanner.Discovery{
		Skills: []scanner.ResourceInfo{
			{Name: "s1", Path: "/s1", Source: "user"},
			{Name: "s2", Path: "/s2", Source: "user"},
		},
	}
	m := testEditor(&config.Profile{Name: "test"}, disc)
	m.activeTab = TabComponents

	model, _ := m.updateComponentsTab(pressRune('j'))
	m = model.(EditorModel)
	if m.discCursor != 1 {
		t.Errorf("j: discCursor = %d, want 1", m.discCursor)
	}

	model, _ = m.updateComponentsTab(pressRune('k'))
	m = model.(EditorModel)
	if m.discCursor != 0 {
		t.Errorf("k: discCursor = %d, want 0", m.discCursor)
	}
}

func TestUpdateComponentsTab_TabAdvancesTab(t *testing.T) {
	m := testEditor(&config.Profile{Name: "test"}, nil)
	m.activeTab = TabComponents

	model, _ := m.updateComponentsTab(pressSpecial(tea.KeyTab))
	updated := model.(EditorModel)
	if updated.activeTab == TabComponents {
		t.Error("tab should advance to next tab")
	}
	if updated.activeTab != TabPlugins {
		t.Errorf("activeTab = %d after tab from Components, want TabPlugins (%d)", updated.activeTab, TabPlugins)
	}
}

func TestUpdateComponentsTab_ShiftTabToPrevious(t *testing.T) {
	m := testEditor(&config.Profile{Name: "test"}, nil)
	m.activeTab = TabComponents

	model, _ := m.updateComponentsTab(pressSpecial(tea.KeyShiftTab))
	updated := model.(EditorModel)
	if updated.activeTab != TabHooks {
		t.Errorf("activeTab = %d after shift+tab from Components, want TabHooks (%d)", updated.activeTab, TabHooks)
	}
}

func TestUpdateComponentsTab_SlashEntersFilterMode(t *testing.T) {
	m := testEditor(&config.Profile{Name: "test"}, nil)
	m.activeTab = TabComponents

	model, _ := m.updateComponentsTab(pressRune('/'))
	updated := model.(EditorModel)
	if !updated.discFiltering {
		t.Error("/ should enter filter mode")
	}
	if updated.discFilter != "" {
		t.Errorf("discFilter should be reset on enter, got %q", updated.discFilter)
	}
}

func TestUpdateComponentsTab_EscSendsHomeMsg(t *testing.T) {
	m := testEditor(&config.Profile{Name: "test"}, nil)
	m.activeTab = TabComponents

	_, cmd := m.updateComponentsTab(pressSpecial(tea.KeyEsc))
	if cmd == nil {
		t.Fatal("esc should return a command")
	}
	msg := cmd()
	switchMsg, ok := msg.(shared.SwitchScreenMsg)
	if !ok {
		t.Fatalf("esc command returned %T, want SwitchScreenMsg", msg)
	}
	if switchMsg.Screen != shared.ScreenHome {
		t.Errorf("screen = %d, want ScreenHome", switchMsg.Screen)
	}
}

// ── updateDiscFilter ──────────────────────────────────────────────────────────

func TestUpdateDiscFilter_AppendCharacter(t *testing.T) {
	m := testEditor(&config.Profile{Name: "test"}, nil)
	m.discFiltering = true

	model, _ := m.updateDiscFilter(pressRune('a'))
	updated := model.(EditorModel)
	if updated.discFilter != "a" {
		t.Errorf("discFilter = %q, want \"a\"", updated.discFilter)
	}

	model, _ = updated.updateDiscFilter(pressRune('b'))
	updated = model.(EditorModel)
	if updated.discFilter != "ab" {
		t.Errorf("discFilter = %q, want \"ab\"", updated.discFilter)
	}
}

func TestUpdateDiscFilter_BackspaceRemovesChar(t *testing.T) {
	m := testEditor(&config.Profile{Name: "test"}, nil)
	m.discFiltering = true
	m.discFilter = "abc"

	model, _ := m.updateDiscFilter(pressSpecial(tea.KeyBackspace))
	updated := model.(EditorModel)
	if updated.discFilter != "ab" {
		t.Errorf("discFilter = %q, want \"ab\"", updated.discFilter)
	}
}

func TestUpdateDiscFilter_BackspaceOnEmpty(t *testing.T) {
	// Should not panic when filter is already empty.
	m := testEditor(&config.Profile{Name: "test"}, nil)
	m.discFiltering = true
	m.discFilter = ""

	_, _ = m.updateDiscFilter(pressSpecial(tea.KeyBackspace))
}

func TestUpdateDiscFilter_EnterExitsFilter(t *testing.T) {
	m := testEditor(&config.Profile{Name: "test"}, nil)
	m.discFiltering = true
	m.discFilter = "foo"

	model, _ := m.updateDiscFilter(pressSpecial(tea.KeyEnter))
	updated := model.(EditorModel)
	if updated.discFiltering {
		t.Error("enter should exit filter mode")
	}
	if updated.discFilter != "foo" {
		t.Error("filter text should be preserved after exit")
	}
}

func TestUpdateDiscFilter_EscExitsFilter(t *testing.T) {
	m := testEditor(&config.Profile{Name: "test"}, nil)
	m.discFiltering = true

	model, _ := m.updateDiscFilter(pressSpecial(tea.KeyEsc))
	updated := model.(EditorModel)
	if updated.discFiltering {
		t.Error("esc should exit filter mode")
	}
}

func TestUpdateDiscFilter_AppliesFilterToItems(t *testing.T) {
	disc := &scanner.Discovery{
		Skills: []scanner.ResourceInfo{
			{Name: "alpha", Path: "/a", Source: "user"},
			{Name: "beta", Path: "/b", Source: "user"},
		},
	}
	m := testEditor(&config.Profile{Name: "test"}, disc)
	m.discFiltering = true

	model, _ := m.updateDiscFilter(pressRune('b'))
	updated := model.(EditorModel)
	if len(updated.discFiltered) != 1 {
		t.Errorf("discFiltered len = %d, want 1 (only beta matches 'b')", len(updated.discFiltered))
	}
}

// ── viewComponentsTab ─────────────────────────────────────────────────────────

func TestViewComponentsTab_NoDiscShowsEmpty(t *testing.T) {
	m := testEditor(&config.Profile{Name: "test"}, nil)
	view := m.viewComponentsTab()
	if !strings.Contains(view, "No resources discovered") {
		t.Errorf("view should show 'No resources discovered', got:\n%s", view)
	}
}

func TestViewComponentsTab_ShowsItemNames(t *testing.T) {
	disc := &scanner.Discovery{
		Skills: []scanner.ResourceInfo{{Name: "my-skill", Path: "/s", Source: "user"}},
		Agents: []scanner.ResourceInfo{{Name: "my-agent", Path: "/a", Source: "user"}},
	}
	m := testEditor(&config.Profile{Name: "test"}, disc)
	view := m.viewComponentsTab()

	if !strings.Contains(view, "my-skill") {
		t.Errorf("view should contain my-skill, got:\n%s", view)
	}
	if !strings.Contains(view, "my-agent") {
		t.Errorf("view should contain my-agent, got:\n%s", view)
	}
}

func TestViewComponentsTab_ShowsCategoryHeaders(t *testing.T) {
	disc := &scanner.Discovery{
		Skills:   []scanner.ResourceInfo{{Name: "s", Path: "/s", Source: "user"}},
		Agents:   []scanner.ResourceInfo{{Name: "a", Path: "/a", Source: "user"}},
		Commands: []scanner.ResourceInfo{{Name: "c", Path: "/c", Source: "project"}},
	}
	m := testEditor(&config.Profile{Name: "test"}, disc)
	view := m.viewComponentsTab()

	for _, cat := range []string{"Skill", "Agent", "Command"} {
		if !strings.Contains(view, cat) {
			t.Errorf("view should show category %q, got:\n%s", cat, view)
		}
	}
}

func TestViewComponentsTab_ShowsCheckmarkForSelected(t *testing.T) {
	disc := &scanner.Discovery{
		Skills: []scanner.ResourceInfo{{Name: "s", Path: "/s", Source: "user"}},
	}
	m := testEditor(&config.Profile{Name: "test"}, disc)
	m.discItems[0].Selected = true
	view := m.viewComponentsTab()

	if !strings.Contains(view, "[✓]") {
		t.Errorf("view should show [✓] for selected item, got:\n%s", view)
	}
}

func TestViewComponentsTab_ShowsEmptyCheckForUnselected(t *testing.T) {
	disc := &scanner.Discovery{
		Skills: []scanner.ResourceInfo{{Name: "s", Path: "/s", Source: "user"}},
	}
	m := testEditor(&config.Profile{Name: "test"}, disc)
	// discItems[0].Selected = false (default)
	view := m.viewComponentsTab()

	if !strings.Contains(view, "[ ]") {
		t.Errorf("view should show [ ] for unselected item, got:\n%s", view)
	}
}

func TestViewComponentsTab_ShowsFilterWhenFiltering(t *testing.T) {
	m := testEditor(&config.Profile{Name: "test"}, nil)
	m.discFiltering = true
	m.discFilter = "foobar"
	view := m.viewComponentsTab()

	if !strings.Contains(view, "foobar") {
		t.Errorf("view should show filter text 'foobar', got:\n%s", view)
	}
}

func TestViewComponentsTab_NoMatchFilter(t *testing.T) {
	disc := &scanner.Discovery{
		Skills: []scanner.ResourceInfo{{Name: "alpha", Path: "/a", Source: "user"}},
	}
	m := testEditor(&config.Profile{Name: "test"}, disc)
	m.discFilter = "zzznomatch"
	m.applyDiscFilter()
	view := m.viewComponentsTab()

	if !strings.Contains(view, "No items match filter") {
		t.Errorf("view should show 'No items match filter', got:\n%s", view)
	}
}

func TestViewComponentsTab_SourceLabels(t *testing.T) {
	disc := &scanner.Discovery{
		Skills: []scanner.ResourceInfo{
			{Name: "user-skill", Path: "/u", Source: "user"},
			{Name: "proj-skill", Path: "/p", Source: "project"},
		},
	}
	m := testEditor(&config.Profile{Name: "test"}, disc)
	view := m.viewComponentsTab()

	if !strings.Contains(view, "User") {
		t.Errorf("view should contain 'User' source label, got:\n%s", view)
	}
	if !strings.Contains(view, "Project") {
		t.Errorf("view should contain 'Project' source label, got:\n%s", view)
	}
}

// ── View() integration ────────────────────────────────────────────────────────

func TestView_ContainsComponentsTab(t *testing.T) {
	m := testEditor(&config.Profile{Name: "test"}, nil)
	view := m.View()
	if !strings.Contains(view, "Components") {
		t.Errorf("main View() should show 'Components' tab name, got:\n%s", view)
	}
}

func TestView_PluginsBeforeAdvanced(t *testing.T) {
	m := testEditor(&config.Profile{Name: "test"}, nil)
	view := m.View()

	pluginsIdx := strings.Index(view, "Plugins")
	advancedIdx := strings.Index(view, "Advanced")
	if pluginsIdx == -1 {
		t.Fatal("View() should contain 'Plugins'")
	}
	if advancedIdx == -1 {
		t.Fatal("View() should contain 'Advanced'")
	}
	if pluginsIdx > advancedIdx {
		t.Errorf("'Plugins' (at %d) should appear before 'Advanced' (at %d) in tab bar", pluginsIdx, advancedIdx)
	}
}

func TestView_ComponentsBeforePlugins(t *testing.T) {
	m := testEditor(&config.Profile{Name: "test"}, nil)
	view := m.View()

	componentsIdx := strings.Index(view, "Components")
	pluginsIdx := strings.Index(view, "Plugins")
	if componentsIdx == -1 {
		t.Fatal("View() should contain 'Components'")
	}
	if pluginsIdx == -1 {
		t.Fatal("View() should contain 'Plugins'")
	}
	if componentsIdx > pluginsIdx {
		t.Errorf("'Components' (at %d) should appear before 'Plugins' (at %d) in tab bar", componentsIdx, pluginsIdx)
	}
}

func TestView_ComponentsStatusBar(t *testing.T) {
	m := testEditor(&config.Profile{Name: "test"}, nil)
	m.activeTab = TabComponents
	view := m.View()

	for _, hint := range []string{"space", "filter", "save", "back"} {
		if !strings.Contains(view, hint) {
			t.Errorf("Components status bar should contain %q hint, got:\n%s", hint, view)
		}
	}
}

func TestView_ComponentsTabContent(t *testing.T) {
	disc := &scanner.Discovery{
		Agents: []scanner.ResourceInfo{{Name: "coder", Path: "/a", Source: "user"}},
	}
	m := testEditor(&config.Profile{Name: "test"}, disc)
	m.activeTab = TabComponents
	view := m.View()

	if !strings.Contains(view, "coder") {
		t.Errorf("View() on Components tab should show discovered items, got:\n%s", view)
	}
}

// ── NewEditor initialisation ──────────────────────────────────────────────────

func TestNewEditor_EmptyNameBecomesNewProfile(t *testing.T) {
	m := testEditor(&config.Profile{}, nil)
	if m.profile.Name != "new-profile" {
		t.Errorf("profile.Name = %q, want new-profile", m.profile.Name)
	}
	if !m.isNew {
		t.Error("isNew should be true for empty-name profile")
	}
}

func TestNewEditor_ExistingProfilePreserved(t *testing.T) {
	p := &config.Profile{Name: "my-profile", Model: "opus"}
	m := testEditor(p, nil)
	if m.isNew {
		t.Error("isNew should be false for named profile")
	}
	if m.origName != "my-profile" {
		t.Errorf("origName = %q, want my-profile", m.origName)
	}
	if m.fields[TabGeneral][3].Value != "opus" {
		t.Errorf("General[Model] field = %q, want opus", m.fields[TabGeneral][3].Value)
	}
}

func TestNewEditor_NilDiscIsHandled(t *testing.T) {
	// Should not panic.
	m := testEditor(&config.Profile{Name: "safe"}, nil)
	_ = m.viewComponentsTab()
}

// ── E2E: tab navigation via Update ───────────────────────────────────────────

func TestE2E_TabToComponents(t *testing.T) {
	m := testEditor(&config.Profile{Name: "test"}, nil)

	// Start at TabGeneral; press Tab until we reach TabComponents.
	tabsNeeded := int(TabComponents) - int(TabGeneral)
	for i := 0; i < tabsNeeded; i++ {
		model, _ := m.Update(pressSpecial(tea.KeyTab))
		m = model.(EditorModel)
	}

	if m.activeTab != TabComponents {
		t.Errorf("activeTab = %d, want TabComponents (%d)", m.activeTab, TabComponents)
	}
	view := m.View()
	if !strings.Contains(view, "space") {
		t.Errorf("Components tab status bar should appear in view, got:\n%s", view)
	}
}

func TestE2E_TabCyclesWrap(t *testing.T) {
	m := testEditor(&config.Profile{Name: "test"}, nil)
	m.activeTab = TabAdvanced // last tab

	model, _ := m.Update(pressSpecial(tea.KeyTab))
	updated := model.(EditorModel)
	if updated.activeTab != TabGeneral {
		t.Errorf("tab from last wraps to TabGeneral, got %d", updated.activeTab)
	}
}

func TestE2E_ShiftTabCyclesWrap(t *testing.T) {
	m := testEditor(&config.Profile{Name: "test"}, nil)
	// Start at TabGeneral; shift+tab wraps to last tab.
	model, _ := m.Update(pressSpecial(tea.KeyShiftTab))
	updated := model.(EditorModel)
	if updated.activeTab != TabAdvanced {
		t.Errorf("shift+tab from first wraps to TabAdvanced, got %d", updated.activeTab)
	}
}

func TestE2E_SelectAndDeselect(t *testing.T) {
	disc := &scanner.Discovery{
		Skills: []scanner.ResourceInfo{{Name: "s", Path: "/s", Source: "user"}},
	}
	m := testEditor(&config.Profile{Name: "test"}, disc)
	m.activeTab = TabComponents

	// Navigate to Components tab and toggle via full Update path.
	model, _ := m.Update(pressSpecial(tea.KeySpace))
	m = model.(EditorModel)
	if !m.discItems[0].Selected {
		t.Error("space via Update should select item")
	}

	model, _ = m.Update(pressSpecial(tea.KeySpace))
	m = model.(EditorModel)
	if m.discItems[0].Selected {
		t.Error("second space via Update should deselect item")
	}
}

func TestE2E_ComponentsRoundTrip(t *testing.T) {
	disc := &scanner.Discovery{
		Skills:   []scanner.ResourceInfo{{Name: "linter", Path: "/linter", Source: "user"}},
		Agents:   []scanner.ResourceInfo{{Name: "reviewer", Path: "/reviewer.md", Source: "user"}},
		Commands: []scanner.ResourceInfo{{Name: "deploy", Path: "/deploy.md", Source: "project"}},
	}
	m := testEditor(&config.Profile{Name: "test"}, disc)
	m.activeTab = TabComponents

	// Select skill and agent; leave command unselected.
	for i := range m.discItems {
		if m.discItems[i].Category != "Command" {
			m.discItems[i].Selected = true
		}
	}
	m.applyFields()

	if len(m.profile.Skills) != 1 || m.profile.Skills[0].Path != "/linter" {
		t.Errorf("Skills = %v, want [{/linter}]", m.profile.Skills)
	}
	if len(m.profile.Agents) != 1 || m.profile.Agents[0].Path != "/reviewer.md" {
		t.Errorf("Agents = %v, want [{/reviewer.md}]", m.profile.Agents)
	}
	if len(m.profile.Commands) != 0 {
		t.Errorf("Commands = %v, want empty (not selected)", m.profile.Commands)
	}
}

func TestE2E_PreselectionPreservedOnLoad(t *testing.T) {
	disc := &scanner.Discovery{
		Skills: []scanner.ResourceInfo{
			{Name: "a", Path: "/a", Source: "user"},
			{Name: "b", Path: "/b", Source: "user"},
		},
		Agents: []scanner.ResourceInfo{
			{Name: "agent", Path: "/agent.md", Source: "user"},
		},
	}
	existing := &config.Profile{
		Name:   "loaded",
		Skills: []config.PathEntry{{Path: "/a"}},
	}
	m := testEditor(existing, disc)

	selectedCount := 0
	for _, item := range m.discItems {
		if item.Selected {
			selectedCount++
		}
	}
	if selectedCount != 1 {
		t.Errorf("pre-selected count = %d, want 1 (only /a)", selectedCount)
	}

	for _, item := range m.discItems {
		switch item.Path {
		case "/a":
			if !item.Selected {
				t.Error("/a should be pre-selected")
			}
		case "/b":
			if item.Selected {
				t.Error("/b should NOT be pre-selected")
			}
		case "/agent.md":
			if item.Selected {
				t.Error("/agent.md should NOT be pre-selected (not in profile.Agents)")
			}
		}
	}
}
