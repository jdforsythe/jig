use std::time::{Duration, Instant};

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use jig_core::editor::{EditorDraft, SaveScope, resolve_draft_preview};
use ratatui::text::Line;

use crate::app::AppMode;

pub mod render;
pub mod save_popup;
pub mod sections;
pub mod undo;
pub mod which_key;

use undo::UndoStack;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditorSection {
    AllowedTools,
    DisallowedTools,
    Model,
    Persona,
    PersonaRules,
    McpServers,
    ContextFragments,
    Hooks,
    PassthroughFlags,
}

impl EditorSection {
    pub const ALL: &'static [Self] = &[
        Self::AllowedTools,
        Self::DisallowedTools,
        Self::Model,
        Self::Persona,
        Self::PersonaRules,
        Self::McpServers,
        Self::ContextFragments,
        Self::Hooks,
        Self::PassthroughFlags,
    ];

    pub fn title(self) -> &'static str {
        match self {
            Self::AllowedTools => "Allowed Tools",
            Self::DisallowedTools => "Disallowed Tools",
            Self::Model => "Model",
            Self::Persona => "Persona Name",
            Self::PersonaRules => "Persona Rules",
            Self::McpServers => "MCP Servers",
            Self::ContextFragments => "Context Fragments",
            Self::Hooks => "Hooks",
            Self::PassthroughFlags => "Passthrough Flags",
        }
    }

    pub fn next(self) -> Self {
        let idx = Self::ALL.iter().position(|s| *s == self).unwrap_or(0);
        Self::ALL[(idx + 1) % Self::ALL.len()]
    }

    pub fn prev(self) -> Self {
        let idx = Self::ALL.iter().position(|s| *s == self).unwrap_or(0);
        Self::ALL[(idx + Self::ALL.len() - 1) % Self::ALL.len()]
    }

    pub fn index(self) -> usize {
        Self::ALL.iter().position(|s| *s == self).unwrap_or(0)
    }

    pub fn is_list_section(self) -> bool {
        matches!(
            self,
            Self::AllowedTools
                | Self::DisallowedTools
                | Self::PersonaRules
                | Self::ContextFragments
                | Self::Hooks
                | Self::PassthroughFlags
                | Self::McpServers
        )
    }

    pub fn is_single_line(self) -> bool {
        matches!(self, Self::Model | Self::Persona)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SectionInputMode {
    Navigate,
    EditLine,   // editing a single-line field
    InsertItem, // inserting a new list item
    EditItem,   // editing an existing list item
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditorEntryPoint {
    CustomAdHoc,
    EditTemplate,
    EditPersona,
    NewTemplate,
}

pub struct EditorState {
    pub draft: EditorDraft,
    pub section: EditorSection,
    pub section_cursor: usize,
    pub input_mode: SectionInputMode,
    pub input_buffer: String,
    pub undo: UndoStack<EditorDraft>,
    pub preview_lines: Vec<Line<'static>>,
    pub preview_token_count: usize,
    pub preview_scroll: u16,
    pub save_scope: SaveScope,
    pub entry_point: EditorEntryPoint,
    pub dirty: bool,
    pub status_message: Option<String>,
    pub pending_g: bool,
    pub pending_colon: bool,
    pub colon_buffer: String,
    last_preview_update: Instant,
}

impl EditorState {
    pub fn new(draft: EditorDraft, entry_point: EditorEntryPoint) -> Self {
        let mut state = Self {
            draft,
            section: EditorSection::ALL[0],
            section_cursor: 0,
            input_mode: SectionInputMode::Navigate,
            input_buffer: String::new(),
            undo: UndoStack::new(50),
            preview_lines: Vec::new(),
            preview_token_count: 0,
            preview_scroll: 0,
            save_scope: SaveScope::Project,
            entry_point,
            dirty: false,
            status_message: None,
            pending_g: false,
            pending_colon: false,
            colon_buffer: String::new(),
            last_preview_update: Instant::now() - Duration::from_secs(1),
        };
        state.refresh_preview();
        state
    }

    pub fn new_custom_adhoc(persona: String) -> Self {
        let persona_name = if persona == "None (no persona)" {
            None
        } else {
            Some(persona)
        };
        let draft = EditorDraft { persona_name, ..EditorDraft::default() };
        Self::new(draft, EditorEntryPoint::CustomAdHoc)
    }

    pub fn new_blank() -> Self {
        Self::new(EditorDraft::default(), EditorEntryPoint::NewTemplate)
    }

    pub fn new_from_template(name: &str, entry: EditorEntryPoint) -> Self {
        let draft = jig_core::editor::load_draft_for_template(name);
        Self::new(draft, entry)
    }

    pub fn should_update_preview(&self) -> bool {
        self.last_preview_update.elapsed() > Duration::from_millis(100)
    }

    pub fn refresh_preview(&mut self) {
        use crate::widgets::markdown_viewer::markdown_to_lines;

        let preview = resolve_draft_preview(&self.draft);
        self.preview_token_count = preview.token_count;

        // Build preview text
        let mut text = String::new();
        if let Some(name) = &preview.template_name {
            if !name.is_empty() {
                text.push_str(&format!("# Template: {name}\n\n"));
            }
        }
        if !preview.permissions_summary.is_empty() {
            text.push_str(&format!(
                "**Permissions:** {}\n\n",
                preview.permissions_summary
            ));
        }
        if !preview.system_prompt_lines.is_empty() {
            text.push_str("**System Prompt:**\n");
            for line in &preview.system_prompt_lines {
                text.push_str(&format!("{line}\n"));
            }
        }
        if preview.token_count > 0 {
            text.push_str(&format!("\n---\n~{} tokens", preview.token_count));
        }

        self.preview_lines = markdown_to_lines(&text);
        self.last_preview_update = Instant::now();
    }

    /// Returns the list items for the current section.
    pub fn section_items(&self) -> Vec<String> {
        match self.section {
            EditorSection::AllowedTools => self.draft.allowed_tools.clone(),
            EditorSection::DisallowedTools => self.draft.disallowed_tools.clone(),
            EditorSection::PersonaRules => self.draft.persona_rules.clone(),
            EditorSection::PassthroughFlags => self.draft.claude_flags.clone(),
            EditorSection::ContextFragments => self
                .draft
                .context_fragments
                .iter()
                .map(|f| {
                    format!(
                        "{} (p:{})",
                        f.path.display(),
                        f.priority.unwrap_or(0)
                    )
                })
                .collect(),
            EditorSection::McpServers => self.draft.mcp_servers.keys().cloned().collect(),
            EditorSection::Hooks => {
                let mut items: Vec<String> = self
                    .draft
                    .pre_launch_hooks
                    .iter()
                    .map(|h| format!("[pre] {}", hook_display(h)))
                    .collect();
                items.extend(
                    self.draft
                        .post_exit_hooks
                        .iter()
                        .map(|h| format!("[post] {}", hook_display(h))),
                );
                items
            }
            EditorSection::Model => vec![],  // single-line, not a list
            EditorSection::Persona => vec![], // single-line
        }
    }

    /// Returns the single-line value for single-line sections.
    pub fn section_value(&self) -> String {
        match self.section {
            EditorSection::Model => self.draft.model.clone().unwrap_or_default(),
            EditorSection::Persona => self.draft.persona_name.clone().unwrap_or_default(),
            _ => String::new(),
        }
    }

    pub fn snapshot(&mut self) {
        let draft = self.draft.clone();
        self.undo.push(&draft);
        self.dirty = true;
    }

    /// Handle a key event. Mode is passed to allow mode transitions.
    pub fn handle_key(&mut self, key: KeyEvent, mode: &mut AppMode) {
        match self.input_mode {
            SectionInputMode::Navigate => self.handle_navigate_key(key, mode),
            SectionInputMode::EditLine
            | SectionInputMode::InsertItem
            | SectionInputMode::EditItem => {
                self.handle_input_key(key);
            }
        }
        if self.should_update_preview() {
            self.refresh_preview();
        }
    }

    fn handle_navigate_key(&mut self, key: KeyEvent, mode: &mut AppMode) {
        // Handle Ctrl combos first
        if key.modifiers.contains(KeyModifiers::CONTROL) {
            match key.code {
                KeyCode::Char('z') => {
                    if let Some(prev) = self.undo.pop() {
                        self.draft = prev;
                        self.dirty = true;
                        self.refresh_preview();
                    }
                    return;
                }
                KeyCode::Char('s') => {
                    *mode = AppMode::EditorSave;
                    return;
                }
                _ => {}
            }
        }

        // Handle colon command mode (:w)
        if self.pending_colon {
            match key.code {
                KeyCode::Char(c) => {
                    self.colon_buffer.push(c);
                }
                KeyCode::Esc => {
                    self.pending_colon = false;
                    self.colon_buffer.clear();
                    return;
                }
                KeyCode::Enter => {
                    if self.colon_buffer == "w" {
                        *mode = AppMode::EditorSave;
                    }
                    self.pending_colon = false;
                    self.colon_buffer.clear();
                    return;
                }
                _ => {
                    self.pending_colon = false;
                    self.colon_buffer.clear();
                    return;
                }
            }
            return;
        }

        match key.code {
            KeyCode::Char(':') => {
                self.pending_colon = true;
                self.colon_buffer.clear();
                self.pending_g = false;
            }
            KeyCode::Char('g') => {
                if self.pending_g {
                    // gg → first section
                    self.section = EditorSection::ALL[0];
                    self.section_cursor = 0;
                    self.pending_g = false;
                } else {
                    self.pending_g = true;
                }
            }
            KeyCode::Char('G') => {
                self.pending_g = false;
                self.section = *EditorSection::ALL.last().expect("ALL is non-empty");
                self.section_cursor = 0;
            }
            KeyCode::Char('J') | KeyCode::Tab => {
                self.pending_g = false;
                self.section = self.section.next();
                self.section_cursor = 0;
            }
            KeyCode::Char('K') => {
                self.pending_g = false;
                self.section = self.section.prev();
                self.section_cursor = 0;
            }
            KeyCode::BackTab => {
                self.pending_g = false;
                self.section = self.section.prev();
                self.section_cursor = 0;
            }
            KeyCode::Char('j') | KeyCode::Down => {
                self.pending_g = false;
                let items = self.section_items();
                if !items.is_empty() {
                    self.section_cursor =
                        (self.section_cursor + 1).min(items.len().saturating_sub(1));
                }
            }
            KeyCode::Char('k') | KeyCode::Up => {
                self.pending_g = false;
                self.section_cursor = self.section_cursor.saturating_sub(1);
            }
            KeyCode::Char('a') => {
                self.pending_g = false;
                if self.section.is_list_section() {
                    self.input_buffer.clear();
                    self.input_mode = SectionInputMode::InsertItem;
                } else if self.section.is_single_line() {
                    self.input_buffer = self.section_value();
                    self.input_mode = SectionInputMode::EditLine;
                }
            }
            KeyCode::Char('i') | KeyCode::Enter
                if !self.section.is_list_section()
                    || self.section_items().is_empty() =>
            {
                self.pending_g = false;
                if self.section.is_single_line() {
                    self.input_buffer = self.section_value();
                    self.input_mode = SectionInputMode::EditLine;
                } else {
                    // list section but empty — start insert
                    self.input_buffer.clear();
                    self.input_mode = SectionInputMode::InsertItem;
                }
            }
            KeyCode::Char('i') => {
                self.pending_g = false;
                // Edit item at cursor (list section with items)
                let items = self.section_items();
                if self.section_cursor < items.len() {
                    self.input_buffer = items[self.section_cursor].clone();
                    self.input_mode = SectionInputMode::EditItem;
                }
            }
            KeyCode::Char('d') => {
                self.pending_g = false;
                self.snapshot();
                let cursor = self.section_cursor;
                match self.section {
                    EditorSection::AllowedTools => {
                        if cursor < self.draft.allowed_tools.len() {
                            self.draft.allowed_tools.remove(cursor);
                        }
                    }
                    EditorSection::DisallowedTools => {
                        if cursor < self.draft.disallowed_tools.len() {
                            self.draft.disallowed_tools.remove(cursor);
                        }
                    }
                    EditorSection::PersonaRules => {
                        if cursor < self.draft.persona_rules.len() {
                            self.draft.persona_rules.remove(cursor);
                        }
                    }
                    EditorSection::PassthroughFlags => {
                        if cursor < self.draft.claude_flags.len() {
                            self.draft.claude_flags.remove(cursor);
                        }
                    }
                    _ => {}
                }
                let new_len = self.section_items().len();
                if self.section_cursor >= new_len && new_len > 0 {
                    self.section_cursor = new_len - 1;
                }
            }
            KeyCode::Char('?') => {
                self.pending_g = false;
                *mode = AppMode::WhichKey;
            }
            KeyCode::Esc | KeyCode::Char('q') => {
                self.pending_g = false;
                *mode = AppMode::Normal;
            }
            _ => {
                self.pending_g = false;
            }
        }
    }

    fn handle_input_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc => {
                self.input_buffer.clear();
                self.input_mode = SectionInputMode::Navigate;
            }
            KeyCode::Enter => {
                let value = self.input_buffer.trim().to_owned();
                self.input_buffer.clear();
                let mode = self.input_mode;
                self.input_mode = SectionInputMode::Navigate;

                if !value.is_empty() {
                    self.snapshot();
                    match mode {
                        SectionInputMode::InsertItem => self.append_to_section(value),
                        SectionInputMode::EditItem => self.replace_at_cursor(value),
                        SectionInputMode::EditLine => self.set_single_line_value(value),
                        SectionInputMode::Navigate => {}
                    }
                }
            }
            KeyCode::Backspace => {
                self.input_buffer.pop();
            }
            KeyCode::Char(c) => {
                self.input_buffer.push(c);
            }
            _ => {}
        }
    }

    fn append_to_section(&mut self, value: String) {
        match self.section {
            EditorSection::AllowedTools => self.draft.allowed_tools.push(value),
            EditorSection::DisallowedTools => self.draft.disallowed_tools.push(value),
            EditorSection::PersonaRules => self.draft.persona_rules.push(value),
            EditorSection::PassthroughFlags => self.draft.claude_flags.push(value),
            _ => {}
        }
        let items = self.section_items();
        self.section_cursor = items.len().saturating_sub(1);
        self.refresh_preview();
    }

    fn replace_at_cursor(&mut self, value: String) {
        let cursor = self.section_cursor;
        match self.section {
            EditorSection::AllowedTools => {
                if cursor < self.draft.allowed_tools.len() {
                    self.draft.allowed_tools[cursor] = value;
                }
            }
            EditorSection::DisallowedTools => {
                if cursor < self.draft.disallowed_tools.len() {
                    self.draft.disallowed_tools[cursor] = value;
                }
            }
            EditorSection::PersonaRules => {
                if cursor < self.draft.persona_rules.len() {
                    self.draft.persona_rules[cursor] = value;
                }
            }
            EditorSection::PassthroughFlags => {
                if cursor < self.draft.claude_flags.len() {
                    self.draft.claude_flags[cursor] = value;
                }
            }
            _ => {}
        }
        self.refresh_preview();
    }

    fn set_single_line_value(&mut self, value: String) {
        match self.section {
            EditorSection::Model => {
                self.draft.model = if value.is_empty() { None } else { Some(value) };
            }
            EditorSection::Persona => {
                self.draft.persona_name = if value.is_empty() { None } else { Some(value) };
            }
            _ => {}
        }
        self.refresh_preview();
    }
}

fn hook_display(hook: &jig_core::config::schema::HookEntry) -> String {
    match hook {
        jig_core::config::schema::HookEntry::Exec { exec } => exec.join(" "),
        jig_core::config::schema::HookEntry::Shell { command, .. } => command.clone(),
    }
}

/// Run a standalone editor TUI session (for `jig template new` / `jig template edit`).
pub fn run_editor_tui(
    entry_point: EditorEntryPoint,
    initial_draft: Option<EditorDraft>,
    project_dir: &std::path::Path,
) -> std::io::Result<Option<EditorDraft>> {
    use crossterm::{
        event::{DisableMouseCapture, EnableMouseCapture},
        execute,
        terminal::{
            EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
        },
    };
    use ratatui::{Terminal, backend::CrosstermBackend};

    let draft = initial_draft.unwrap_or_default();
    let mut state = EditorState::new(draft, entry_point);
    let mut app_mode = AppMode::Editor;

    enable_raw_mode()?;
    let mut stdout = std::io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let cleanup = || {
        let _ = disable_raw_mode();
        let _ = execute!(
            std::io::stdout(),
            LeaveAlternateScreen,
            DisableMouseCapture
        );
    };

    let result =
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(
            || -> std::io::Result<Option<EditorDraft>> {
                loop {
                    terminal.draw(|f| {
                        render::render_editor_standalone(f, &mut state, &app_mode);
                    })?;

                    if crossterm::event::poll(Duration::from_millis(50))? {
                        if let crossterm::event::Event::Key(key) = crossterm::event::read()? {
                            if key.kind == crossterm::event::KeyEventKind::Press {
                                match app_mode {
                                    AppMode::Editor | AppMode::EditorSave => {
                                        state.handle_key(key, &mut app_mode);
                                    }
                                    AppMode::Normal => {
                                        return Ok(if state.dirty {
                                            Some(state.draft)
                                        } else {
                                            None
                                        });
                                    }
                                    _ => {}
                                }
                            }
                        }
                    }
                }
            },
        ));

    cleanup();

    // Suppress unused variable warning — project_dir is available for future use
    let _ = project_dir;

    match result {
        Ok(r) => r,
        Err(_) => Ok(None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use jig_core::editor::EditorDraft;

    #[test]
    fn test_section_navigation_wraps_forward() {
        let last = *EditorSection::ALL.last().unwrap();
        assert_eq!(
            last.next(),
            EditorSection::ALL[0],
            "last section must wrap to first"
        );
    }

    #[test]
    fn test_section_navigation_wraps_backward() {
        let first = EditorSection::ALL[0];
        assert_eq!(
            first.prev(),
            *EditorSection::ALL.last().unwrap(),
            "first section must wrap to last"
        );
    }

    #[test]
    fn test_pending_g_state_machine_gg_jumps_to_first() {
        let draft = EditorDraft::default();
        let mut state = EditorState::new(draft, EditorEntryPoint::CustomAdHoc);
        state.section = *EditorSection::ALL.last().unwrap(); // Start at last

        let mut mode = AppMode::Editor;
        let g_key = KeyEvent::new(KeyCode::Char('g'), KeyModifiers::NONE);

        state.handle_key(g_key, &mut mode);
        assert!(state.pending_g, "After first g, pending_g must be true");

        state.handle_key(g_key, &mut mode);
        assert!(!state.pending_g, "After second g, pending_g must be false");
        assert_eq!(
            state.section,
            EditorSection::ALL[0],
            "gg must jump to first section"
        );
    }

    #[test]
    fn test_pending_g_clears_on_other_key() {
        let draft = EditorDraft::default();
        let mut state = EditorState::new(draft, EditorEntryPoint::CustomAdHoc);

        let mut mode = AppMode::Editor;
        let g_key = KeyEvent::new(KeyCode::Char('g'), KeyModifiers::NONE);
        let j_key = KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE);

        state.handle_key(g_key, &mut mode);
        assert!(state.pending_g);

        state.handle_key(j_key, &mut mode);
        assert!(!state.pending_g, "pending_g must clear on non-g key");
    }

    #[test]
    fn test_ctrl_z_restores_prior_draft() {
        let draft = EditorDraft::default();
        let mut state = EditorState::new(draft, EditorEntryPoint::CustomAdHoc);

        // Snapshot + mutate
        state.snapshot();
        state.draft.allowed_tools.push("Read".to_owned());

        let mut mode = AppMode::Editor;
        let ctrl_z = KeyEvent::new(KeyCode::Char('z'), KeyModifiers::CONTROL);
        state.handle_key(ctrl_z, &mut mode);

        assert!(
            state.draft.allowed_tools.is_empty(),
            "Ctrl-Z must restore prior draft"
        );
    }

    #[test]
    fn test_section_j_moves_cursor_down() {
        let mut draft = EditorDraft::default();
        draft.allowed_tools = vec!["Read".to_owned(), "Bash".to_owned()];
        let mut state = EditorState::new(draft, EditorEntryPoint::CustomAdHoc);
        state.section = EditorSection::AllowedTools;
        state.section_cursor = 0;

        let mut mode = AppMode::Editor;
        let j_key = KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE);
        state.handle_key(j_key, &mut mode);

        assert_eq!(state.section_cursor, 1);
    }

    #[test]
    fn test_section_k_moves_cursor_up() {
        let mut draft = EditorDraft::default();
        draft.allowed_tools = vec!["Read".to_owned(), "Bash".to_owned()];
        let mut state = EditorState::new(draft, EditorEntryPoint::CustomAdHoc);
        state.section = EditorSection::AllowedTools;
        state.section_cursor = 1;

        let mut mode = AppMode::Editor;
        let k_key = KeyEvent::new(KeyCode::Char('k'), KeyModifiers::NONE);
        state.handle_key(k_key, &mut mode);

        assert_eq!(state.section_cursor, 0);
    }

    #[test]
    fn test_d_deletes_item_at_cursor() {
        let mut draft = EditorDraft::default();
        draft.allowed_tools = vec!["Read".to_owned(), "Bash".to_owned()];
        let mut state = EditorState::new(draft, EditorEntryPoint::CustomAdHoc);
        state.section = EditorSection::AllowedTools;
        state.section_cursor = 0;

        let mut mode = AppMode::Editor;
        let d_key = KeyEvent::new(KeyCode::Char('d'), KeyModifiers::NONE);
        state.handle_key(d_key, &mut mode);

        assert_eq!(state.draft.allowed_tools, vec!["Bash".to_owned()]);
    }

    #[test]
    fn test_tab_advances_section() {
        let draft = EditorDraft::default();
        let mut state = EditorState::new(draft, EditorEntryPoint::CustomAdHoc);
        state.section = EditorSection::ALL[0];

        let mut mode = AppMode::Editor;
        let tab_key = KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE);
        state.handle_key(tab_key, &mut mode);

        assert_eq!(state.section, EditorSection::ALL[1]);
    }

    // ── P2: Editor preview contract tests ───────────────────────

    #[test]
    fn test_editor_preview_populated_on_init() {
        // EditorState::new() calls refresh_preview() in its constructor
        let mut draft = EditorDraft::default();
        draft.persona_name = Some("test".to_owned());
        draft.persona_rules = vec!["Rule one".to_owned()];
        let state = EditorState::new(draft, EditorEntryPoint::CustomAdHoc);
        assert!(
            !state.preview_lines.is_empty(),
            "EditorState constructor must populate preview_lines via refresh_preview"
        );
        assert!(
            state.preview_token_count > 0,
            "EditorState constructor must populate token count for draft with persona rules"
        );
    }

    #[test]
    fn test_editor_preview_updates_after_adding_rule() {
        let draft = EditorDraft::default();
        let mut state = EditorState::new(draft, EditorEntryPoint::CustomAdHoc);
        let initial_count = state.preview_token_count;

        // Directly add a persona rule and refresh (simulating the effect of 'a' key + input)
        state.draft.persona_rules.push("A new test rule for preview".to_owned());
        state.refresh_preview();

        assert!(
            state.preview_token_count > initial_count,
            "preview_token_count must increase after adding a persona rule"
        );
    }

    #[test]
    fn test_editor_debounce_100ms() {
        let draft = EditorDraft::default();
        let state = EditorState::new(draft, EditorEntryPoint::CustomAdHoc);
        // Constructor just called refresh_preview, so last_preview_update is fresh
        assert!(
            !state.should_update_preview(),
            "should_update_preview must be false immediately after init"
        );
        std::thread::sleep(Duration::from_millis(110));
        assert!(
            state.should_update_preview(),
            "should_update_preview must be true after 110ms"
        );
    }
}
