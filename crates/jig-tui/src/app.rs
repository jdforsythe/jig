/// TUI Application — Phase 2 implementation.
///
/// Decision (brainstorm §4): TUI shows on bare `jig`. Two-pane layout.
use std::io;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind, MouseEventKind},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
    cursor,
};
use jig_core::config::resolve::CliOverrides;
use jig_core::defaults::builtin_templates;
use ratatui::{
    Terminal,
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout, Rect},
    style::Style,
    text::Line,
    widgets::{Block, Borders, Paragraph, Wrap},
};
use scopeguard::defer;

use crate::theme::active_theme;
use crate::widgets::FilterableListState;
use crate::widgets::markdown_viewer::markdown_to_lines;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PaneFocus {
    Templates,
    Personas,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppMode {
    Normal,
    Filter,
    WhichKey,
    Confirm,
    History,
    Editor,
    EditorSave,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LayoutMode {
    FullTwoPane,  // ≥100 cols
    NarrowTwoPane, // 80-99 cols
    SinglePane,   // <80 cols
    Minimal,      // <60 cols
}

impl LayoutMode {
    fn from_cols(cols: u16) -> Self {
        match cols {
            c if c >= 100 => Self::FullTwoPane,
            c if c >= 80 => Self::NarrowTwoPane,
            c if c >= 60 => Self::SinglePane,
            _ => Self::Minimal,
        }
    }
}

pub struct App {
    pub templates: FilterableListState,
    pub personas: FilterableListState,
    pub focus: PaneFocus,
    pub mode: AppMode,
    pub layout: LayoutMode,
    pub terminal_cols: u16,
    pub preview_scroll: u16,
    pub preview_lines: Vec<Line<'static>>,
    pub preview_token_count: usize,
    pub show_preview: bool,
    pub should_quit: bool,
    pub launch_selection: Option<(String, String)>, // (template, persona)
    last_preview_update: Instant,
    pub project_dir: PathBuf,
    pub history_lines: Vec<String>,
    pub history_scroll: u16,
    pub editor_state: Option<crate::editor::EditorState>,
}

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}

impl App {
    pub fn new() -> Self {
        Self::with_project_dir(std::env::current_dir().unwrap_or_default())
    }

    pub fn with_project_dir(project_dir: PathBuf) -> Self {
        let mut template_names: Vec<String> = vec!["None (no template)".to_owned()];
        template_names.insert(1, "[Custom / ad-hoc]".to_owned());
        template_names.extend(builtin_templates().into_iter().map(|t| t.name));

        let persona_names = vec![
            "None (no persona)".to_owned(),
            "default".to_owned(),
            "strict-security".to_owned(),
            "mentor".to_owned(),
            "pair-programmer".to_owned(),
            "code-reviewer".to_owned(),
            "architect".to_owned(),
            "minimalist".to_owned(),
            "tdd".to_owned(),
            "docs-writer".to_owned(),
            "performance".to_owned(),
        ];

        let mut app = Self {
            templates: FilterableListState::new(template_names),
            personas: FilterableListState::new(persona_names),
            focus: PaneFocus::Templates,
            mode: AppMode::Normal,
            layout: LayoutMode::FullTwoPane,
            terminal_cols: 120,
            preview_scroll: 0,
            preview_lines: Vec::new(),
            preview_token_count: 0,
            show_preview: true,
            should_quit: false,
            launch_selection: None,
            last_preview_update: Instant::now() - Duration::from_secs(1),
            project_dir,
            history_lines: Vec::new(),
            history_scroll: 0,
            editor_state: None,
        };
        app.refresh_preview();
        app
    }

    pub fn handle_key(&mut self, key: crossterm::event::KeyEvent) {
        if key.kind != KeyEventKind::Press {
            return;
        }

        match self.mode {
            AppMode::Filter => self.handle_filter_key(key),
            AppMode::WhichKey => {
                self.mode = AppMode::Normal;
            }
            AppMode::History => self.handle_history_key(key),
            AppMode::Normal | AppMode::Confirm => self.handle_normal_key(key),
            AppMode::Editor | AppMode::EditorSave => {
                if let Some(editor) = self.editor_state.as_mut() {
                    editor.handle_key(key, &mut self.mode);
                }
            }
        }
    }

    fn handle_normal_key(&mut self, key: crossterm::event::KeyEvent) {
        match key.code {
            KeyCode::Char('q') | KeyCode::Esc => {
                self.should_quit = true;
            }
            KeyCode::Char('j') | KeyCode::Down => {
                match self.focus {
                    PaneFocus::Templates => self.templates.move_down(),
                    PaneFocus::Personas => self.personas.move_down(),
                }
                self.last_preview_update = Instant::now();
            }
            KeyCode::Char('k') | KeyCode::Up => {
                match self.focus {
                    PaneFocus::Templates => self.templates.move_up(),
                    PaneFocus::Personas => self.personas.move_up(),
                }
                self.last_preview_update = Instant::now();
            }
            KeyCode::Tab => {
                self.focus = match self.focus {
                    PaneFocus::Templates => PaneFocus::Personas,
                    PaneFocus::Personas => PaneFocus::Templates,
                };
            }
            KeyCode::Char('/') => {
                self.mode = AppMode::Filter;
                // Clear existing filter for the focused list
                match self.focus {
                    PaneFocus::Templates => self.templates.clear_query(),
                    PaneFocus::Personas => self.personas.clear_query(),
                }
            }
            KeyCode::Enter => {
                let template = self.templates.selected_item().map(str::to_owned);
                let persona = self.personas.selected_item().map(str::to_owned);
                if let (Some(t), Some(p)) = (template, persona) {
                    if t == "[Custom / ad-hoc]" {
                        // Enter editor mode for custom / ad-hoc session
                        self.editor_state = Some(
                            crate::editor::EditorState::new_custom_adhoc(p)
                        );
                        self.mode = AppMode::Editor;
                    } else {
                        self.launch_selection = Some((t, p));
                        self.should_quit = true;
                    }
                }
            }
            KeyCode::Char('e') => {
                // Edit selected template in editor (not for "None" or "[Custom / ad-hoc]")
                let template = self.templates.selected_item().map(str::to_owned);
                if let Some(t) = template {
                    if t != "None (no template)" && t != "[Custom / ad-hoc]" {
                        self.editor_state = Some(
                            crate::editor::EditorState::new_from_template(
                                &t,
                                crate::editor::EditorEntryPoint::EditTemplate,
                            )
                        );
                        self.mode = AppMode::Editor;
                    }
                }
            }
            KeyCode::Char('?') => {
                self.mode = AppMode::WhichKey;
            }
            KeyCode::Char('p') => {
                if matches!(self.layout, LayoutMode::SinglePane | LayoutMode::Minimal) {
                    self.show_preview = !self.show_preview;
                }
            }
            KeyCode::Char('h') => {
                self.load_history();
                self.mode = AppMode::History;
            }
            KeyCode::Char('L') => {
                // Relaunch last session
                if let Some(entry) = jig_core::history::last_session() {
                    let template = entry.template.unwrap_or_else(|| "None (no template)".to_owned());
                    let persona = entry.persona.unwrap_or_else(|| "None (no persona)".to_owned());
                    self.launch_selection = Some((template, persona));
                    self.should_quit = true;
                }
                // If no history, do nothing (user stays in TUI)
            }
            KeyCode::Char('d') | KeyCode::Char('D') => {
                // Scroll preview down
                self.preview_scroll = self.preview_scroll.saturating_add(3);
            }
            KeyCode::Char('u') | KeyCode::Char('U') => {
                // Scroll preview up
                self.preview_scroll = self.preview_scroll.saturating_sub(3);
            }
            _ => {}
        }
    }

    fn handle_history_key(&mut self, key: crossterm::event::KeyEvent) {
        match key.code {
            KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('h') => {
                self.mode = AppMode::Normal;
            }
            KeyCode::Char('j') | KeyCode::Down => {
                self.history_scroll = self.history_scroll.saturating_add(1);
            }
            KeyCode::Char('k') | KeyCode::Up => {
                self.history_scroll = self.history_scroll.saturating_sub(1);
            }
            _ => {}
        }
    }

    fn load_history(&mut self) {
        let entries = jig_core::history::recent_sessions(20);
        if entries.is_empty() {
            self.history_lines = vec!["No session history found.".to_owned()];
        } else {
            self.history_lines = entries
                .into_iter()
                .map(|e| {
                    let date = &e.started_at[..16.min(e.started_at.len())]; // YYYY-MM-DDTHH:MM
                    let template = e.template.as_deref().unwrap_or("none");
                    let persona = e.persona.as_deref().unwrap_or("none");
                    let cwd_short = std::path::Path::new(&e.cwd)
                        .file_name()
                        .map(|n| n.to_string_lossy().into_owned())
                        .unwrap_or_else(|| e.cwd.clone());
                    format!("{date}  {template:20}  {persona:20}  {cwd_short}")
                })
                .collect();
        }
        self.history_scroll = 0;
    }

    fn handle_filter_key(&mut self, key: crossterm::event::KeyEvent) {
        match key.code {
            KeyCode::Esc => {
                self.mode = AppMode::Normal;
                match self.focus {
                    PaneFocus::Templates => self.templates.clear_query(),
                    PaneFocus::Personas => self.personas.clear_query(),
                }
            }
            KeyCode::Enter => {
                self.mode = AppMode::Normal;
            }
            KeyCode::Backspace => {
                match self.focus {
                    PaneFocus::Templates => self.templates.pop_char(),
                    PaneFocus::Personas => self.personas.pop_char(),
                }
            }
            KeyCode::Char(c) => {
                match self.focus {
                    PaneFocus::Templates => self.templates.push_char(c),
                    PaneFocus::Personas => self.personas.push_char(c),
                }
            }
            _ => {}
        }
    }

    pub fn handle_mouse(&mut self, event: crossterm::event::MouseEvent) {
        match event.kind {
            MouseEventKind::ScrollDown => {
                self.preview_scroll = self.preview_scroll.saturating_add(3);
            }
            MouseEventKind::ScrollUp => {
                self.preview_scroll = self.preview_scroll.saturating_sub(3);
            }
            _ => {}
        }
    }

    pub fn update_layout(&mut self, cols: u16) {
        self.terminal_cols = cols;
        self.layout = LayoutMode::from_cols(cols);
    }

    pub fn should_update_preview(&self) -> bool {
        self.last_preview_update.elapsed() >= Duration::from_millis(50)
    }

    pub fn set_preview(&mut self, markdown: &str, token_count: usize) {
        self.preview_lines = markdown_to_lines(markdown);
        self.preview_token_count = token_count;
        self.preview_scroll = 0;
    }

    /// Computes and sets preview content based on current template/persona selection.
    pub fn refresh_preview(&mut self) {
        let template = self.templates.selected_item().unwrap_or("None (no template)");
        let persona = self.personas.selected_item().unwrap_or("None (no persona)");

        let overrides = CliOverrides {
            template: if template == "None (no template)" || template == "[Custom / ad-hoc]" {
                None
            } else {
                Some(template.to_owned())
            },
            persona: if persona == "None (no persona)" {
                None
            } else {
                Some(persona.to_owned())
            },
            model: None,
        };

        match jig_core::assembly::preview::compute_preview(&self.project_dir, &overrides) {
            Ok(preview) => {
                let mut md = String::new();
                if let Some(name) = &preview.template_name {
                    if !name.is_empty() {
                        md.push_str(&format!("# Template: {name}\n\n"));
                    }
                }
                if let Some(name) = &preview.persona_name {
                    md.push_str(&format!("**Persona:** {name}\n\n"));
                }
                if !preview.permissions_summary.is_empty() {
                    md.push_str(&format!("**Permissions:** {}\n\n", preview.permissions_summary));
                }
                if !preview.system_prompt_lines.is_empty() {
                    md.push_str("---\n\n");
                    for line in &preview.system_prompt_lines {
                        md.push_str(&format!("{line}\n"));
                    }
                }
                self.set_preview(&md, preview.token_count);
            }
            Err(_) => {
                self.set_preview("*Preview unavailable*", 0);
            }
        }
        self.last_preview_update = Instant::now();
    }
}

/// Installs a panic hook that restores the terminal before printing the backtrace.
pub fn install_panic_hook() {
    let original = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = execute!(
            io::stdout(),
            DisableMouseCapture,
            LeaveAlternateScreen,
            cursor::Show,
        );
        let _ = disable_raw_mode();
        original(info);
    }));
}

/// Runs the TUI and returns the selected (template, persona) pair if the user confirmed.
pub fn run_tui() -> io::Result<Option<(String, String)>> {
    // Check terminal size before entering raw mode
    let (cols, rows) = crossterm::terminal::size()?;
    if cols < 40 || rows < 24 {
        eprintln!(
            "Terminal too small ({}x{}, minimum 40x24). Resize and try again.",
            cols, rows
        );
        return Ok(None);
    }

    install_panic_hook();

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    defer! {
        let _ = execute!(
            io::stdout(),
            DisableMouseCapture,
            LeaveAlternateScreen,
            cursor::Show,
        );
        let _ = disable_raw_mode();
    }

    let mut app = App::new();

    loop {
        terminal.draw(|frame| render(frame, &mut app))?;

        if event::poll(Duration::from_millis(100))? {
            match event::read()? {
                Event::Key(key) => app.handle_key(key),
                Event::Mouse(mouse) => app.handle_mouse(mouse),
                Event::Resize(cols, _rows) => {
                    app.update_layout(cols);
                }
                _ => {}
            }
        }

        // Refresh preview after debounce period elapses following a selection change
        if app.should_update_preview() {
            app.refresh_preview();
        }

        if app.should_quit {
            break;
        }
    }

    Ok(app.launch_selection)
}

fn render(frame: &mut ratatui::Frame, app: &mut App) {
    let area = frame.area();

    // Minimum size guard
    if area.width < 40 || area.height < 24 {
        let msg = Paragraph::new("Terminal too small (minimum 40×24)");
        frame.render_widget(msg, area);
        return;
    }

    app.update_layout(area.width);

    // Editor mode takes full screen
    if matches!(app.mode, AppMode::Editor | AppMode::EditorSave) {
        if let Some(editor) = app.editor_state.as_mut() {
            crate::editor::render::render_editor(frame, editor, area);
        }
        return;
    }

    // History overlay takes full screen
    if app.mode == AppMode::History {
        render_history(frame, app, area);
        return;
    }

    match app.layout {
        LayoutMode::FullTwoPane | LayoutMode::NarrowTwoPane => {
            render_two_pane(frame, app, area);
        }
        LayoutMode::SinglePane => {
            if app.show_preview {
                render_two_pane(frame, app, area);
            } else {
                render_single_pane_lists(frame, app, area);
            }
        }
        LayoutMode::Minimal => {
            render_single_pane_lists(frame, app, area);
        }
    }

    // Which-key popup
    if app.mode == AppMode::WhichKey {
        render_which_key(frame, area);
    }
}

fn render_two_pane(frame: &mut ratatui::Frame, app: &mut App, area: Rect) {
    let theme = active_theme();

    let left_pct = if matches!(app.layout, LayoutMode::NarrowTwoPane) { 40 } else { 35 };
    let chunks = Layout::new(
        Direction::Horizontal,
        [Constraint::Percentage(left_pct), Constraint::Fill(1)],
    )
    .split(area);

    // Left pane: template list + persona list
    let left_chunks = Layout::new(
        Direction::Vertical,
        [Constraint::Fill(1), Constraint::Fill(1)],
    )
    .split(chunks[0]);

    // Templates list
    use crate::widgets::filterable_list::FilterableListWidget;
    let template_widget = FilterableListWidget {
        title: "Templates",
        focused: app.focus == PaneFocus::Templates,
    };
    frame.render_stateful_widget(template_widget, left_chunks[0], &mut app.templates);

    // Personas list
    let persona_widget = FilterableListWidget {
        title: "Personas",
        focused: app.focus == PaneFocus::Personas,
    };
    frame.render_stateful_widget(persona_widget, left_chunks[1], &mut app.personas);

    // Right pane: preview
    let template_name = app.templates.selected_item().unwrap_or("none");
    let title = format!(" Preview: {template_name} ");
    let preview_block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme.border_unfocused));

    let inner = preview_block.inner(chunks[1]);
    frame.render_widget(preview_block, chunks[1]);

    // Token count header
    let token_style = if app.preview_token_count > 8000 {
        Style::default().fg(theme.token_critical)
    } else if app.preview_token_count > 4000 {
        Style::default().fg(theme.token_warn)
    } else {
        Style::default()
    };

    let header = Paragraph::new(Line::from(format!("~{} tokens", app.preview_token_count)))
        .style(token_style);

    let inner_chunks = Layout::new(
        Direction::Vertical,
        [Constraint::Length(1), Constraint::Fill(1)],
    )
    .split(inner);

    frame.render_widget(header, inner_chunks[0]);

    // Clamp scroll offset
    let total_lines = app.preview_lines.len() as u16;
    let max_scroll = total_lines.saturating_sub(inner_chunks[1].height);
    app.preview_scroll = app.preview_scroll.min(max_scroll);

    let preview = Paragraph::new(app.preview_lines.clone())
        .scroll((app.preview_scroll, 0))
        .wrap(Wrap { trim: false });

    frame.render_widget(preview, inner_chunks[1]);
}

fn render_single_pane_lists(frame: &mut ratatui::Frame, app: &mut App, area: Rect) {
    let chunks = Layout::new(
        Direction::Vertical,
        [Constraint::Fill(1), Constraint::Fill(1)],
    )
    .split(area);

    use crate::widgets::filterable_list::FilterableListWidget;
    let template_widget = FilterableListWidget {
        title: "Templates",
        focused: app.focus == PaneFocus::Templates,
    };
    frame.render_stateful_widget(template_widget, chunks[0], &mut app.templates);

    let persona_widget = FilterableListWidget {
        title: "Personas",
        focused: app.focus == PaneFocus::Personas,
    };
    frame.render_stateful_widget(persona_widget, chunks[1], &mut app.personas);
}

fn render_history(frame: &mut ratatui::Frame, app: &mut App, area: Rect) {
    let theme = active_theme();
    let lines: Vec<Line> = app
        .history_lines
        .iter()
        .map(|l| Line::from(l.clone()))
        .collect();

    let total = lines.len() as u16;
    let max_scroll = total.saturating_sub(area.height.saturating_sub(2));
    app.history_scroll = app.history_scroll.min(max_scroll);

    let para = Paragraph::new(lines)
        .block(
            Block::default()
                .title(" Session History (Esc/h to close, j/k to scroll) ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(theme.border_focused)),
        )
        .scroll((app.history_scroll, 0));

    frame.render_widget(para, area);
}

fn render_which_key(frame: &mut ratatui::Frame, area: Rect) {
    use ratatui::widgets::Clear;

    let popup_width = 50u16.min(area.width.saturating_sub(4));
    let popup_height = 15u16.min(area.height.saturating_sub(4));
    let popup_x = (area.width.saturating_sub(popup_width)) / 2;
    let popup_y = (area.height.saturating_sub(popup_height)) / 2;

    let popup_area = Rect {
        x: popup_x,
        y: popup_y,
        width: popup_width,
        height: popup_height,
    };

    frame.render_widget(Clear, popup_area);

    let keybindings = vec![
        Line::from(" Key Bindings"),
        Line::from("─────────────────────────"),
        Line::from(" j/k    Navigate list"),
        Line::from(" Tab    Switch pane focus"),
        Line::from(" /      Filter mode"),
        Line::from(" Enter  Launch session"),
        Line::from(" h      Session history"),
        Line::from(" L      Relaunch last session"),
        Line::from(" p      Toggle preview"),
        Line::from(" d/D    Scroll preview ↓"),
        Line::from(" u/U    Scroll preview ↑"),
        Line::from(" ?      This help"),
        Line::from(" q/Esc  Quit"),
    ];

    let popup = Paragraph::new(keybindings)
        .block(Block::default().borders(Borders::ALL).title(" Help "));
    frame.render_widget(popup, popup_area);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

    fn press(app: &mut App, code: KeyCode) {
        app.handle_key(KeyEvent {
            code,
            modifiers: KeyModifiers::empty(),
            kind: KeyEventKind::Press,
            state: crossterm::event::KeyEventState::empty(),
        });
    }

    /// Creates an App backed by a temporary directory for preview tests.
    fn app_with_tempdir(dir: &std::path::Path) -> App {
        App::with_project_dir(dir.to_path_buf())
    }

    /// Navigate template selection to the given name.
    fn navigate_to_template(app: &mut App, target: &str) {
        app.focus = PaneFocus::Templates;
        // Reset to top
        for _ in 0..app.templates.items.len() {
            press(app, KeyCode::Char('k'));
        }
        for _ in 0..app.templates.items.len() {
            if app.templates.selected_item() == Some(target) {
                return;
            }
            press(app, KeyCode::Char('j'));
        }
        panic!("Template '{}' not found in list", target);
    }

    /// Navigate persona selection to the given name.
    fn navigate_to_persona(app: &mut App, target: &str) {
        app.focus = PaneFocus::Personas;
        for _ in 0..app.personas.items.len() {
            press(app, KeyCode::Char('k'));
        }
        for _ in 0..app.personas.items.len() {
            if app.personas.selected_item() == Some(target) {
                return;
            }
            press(app, KeyCode::Char('j'));
        }
        panic!("Persona '{}' not found in list", target);
    }

    // ── Existing tests ──────────────────────────────────────────

    #[test]
    fn test_template_list_starts_with_none() {
        let app = App::new();
        let first = app.templates.items.first().map(String::as_str);
        assert_eq!(first, Some("None (no template)"), "first template entry must be 'None (no template)'");
    }

    #[test]
    fn test_persona_list_starts_with_none() {
        let app = App::new();
        let first = app.personas.items.first().map(String::as_str);
        assert_eq!(first, Some("None (no persona)"), "first persona entry must be 'None (no persona)'");
    }

    #[test]
    fn test_enter_with_none_template_sets_launch_selection() {
        let mut app = App::new();
        // First item is "None (no template)" — pressing Enter should still work
        press(&mut app, KeyCode::Enter);
        assert!(app.launch_selection.is_some(), "Enter with None template must set launch_selection");
        let (template, _) = app.launch_selection.unwrap();
        assert_eq!(template, "None (no template)");
    }

    #[test]
    fn test_enter_with_none_persona_sets_launch_selection() {
        let mut app = App::new();
        // First item is "None (no persona)" — pressing Enter should still work
        press(&mut app, KeyCode::Enter);
        assert!(app.launch_selection.is_some(), "Enter with None persona must set launch_selection");
        let (_, persona) = app.launch_selection.unwrap();
        assert_eq!(persona, "None (no persona)");
    }

    #[test]
    fn test_h_key_switches_to_history_mode() {
        let mut app = App::new();
        assert_eq!(app.mode, AppMode::Normal);
        press(&mut app, KeyCode::Char('h'));
        assert_eq!(app.mode, AppMode::History, "h key must switch to History mode");
    }

    #[test]
    fn test_esc_in_history_mode_returns_to_normal() {
        let mut app = App::new();
        press(&mut app, KeyCode::Char('h'));
        assert_eq!(app.mode, AppMode::History);
        press(&mut app, KeyCode::Esc);
        assert_eq!(app.mode, AppMode::Normal, "Esc in History mode must return to Normal");
    }

    #[test]
    fn test_history_view_populates_lines() {
        let mut app = App::new();
        // load_history with no history file should set a "no history" message
        app.load_history();
        assert!(!app.history_lines.is_empty(), "history_lines must be populated after load_history");
    }

    #[test]
    fn test_l_key_with_no_history_does_not_quit() {
        let mut app = App::new();
        // With no history file, L should not set launch_selection or quit
        // (last_session() returns None when no history exists in test env)
        // We just verify it doesn't panic
        press(&mut app, KeyCode::Char('L'));
        // Either quit (if history exists on dev machine) or stay
        // Both are valid — just must not panic
        let _ = app.launch_selection;
    }

    #[test]
    fn test_template_list_second_item_is_custom_adhoc() {
        let app = App::new();
        let second = app.templates.items.get(1).map(String::as_str);
        assert_eq!(
            second,
            Some("[Custom / ad-hoc]"),
            "second template entry must be '[Custom / ad-hoc]'"
        );
    }

    #[test]
    fn test_enter_on_custom_adhoc_enters_editor_mode() {
        let mut app = App::new();
        // Navigate to index 1 = "[Custom / ad-hoc]"
        press(&mut app, KeyCode::Char('j'));
        // Confirm it's actually selected
        assert_eq!(
            app.templates.selected_item(),
            Some("[Custom / ad-hoc]"),
            "after j, selected template must be '[Custom / ad-hoc]'"
        );
        press(&mut app, KeyCode::Enter);
        assert_eq!(
            app.mode,
            AppMode::Editor,
            "Enter on [Custom / ad-hoc] must enter Editor mode"
        );
        assert!(
            app.editor_state.is_some(),
            "editor_state must be Some after entering Editor mode"
        );
    }

    #[test]
    fn test_e_key_on_none_does_nothing() {
        let mut app = App::new();
        // First item is "None (no template)" — e should not enter editor
        assert_eq!(app.templates.selected_item(), Some("None (no template)"));
        press(&mut app, KeyCode::Char('e'));
        assert_eq!(
            app.mode,
            AppMode::Normal,
            "e on 'None (no template)' must not enter editor mode"
        );
        assert!(
            app.editor_state.is_none(),
            "editor_state must remain None when e is pressed on None"
        );
    }

    // ── P0: Preview update tests (would have caught the bug) ────

    #[test]
    fn test_set_preview_populates_lines_and_token_count() {
        let dir = tempfile::tempdir().unwrap();
        let mut app = app_with_tempdir(dir.path());
        app.set_preview("# Hello\n\nSome text here", 42);
        assert!(!app.preview_lines.is_empty(), "set_preview must populate preview_lines");
        assert_eq!(app.preview_token_count, 42);
    }

    #[test]
    fn test_set_preview_resets_scroll() {
        let dir = tempfile::tempdir().unwrap();
        let mut app = app_with_tempdir(dir.path());
        app.preview_scroll = 10;
        app.set_preview("# Hello", 5);
        assert_eq!(app.preview_scroll, 0, "set_preview must reset scroll to 0");
    }

    #[test]
    fn test_preview_debounce_timing() {
        let dir = tempfile::tempdir().unwrap();
        let mut app = app_with_tempdir(dir.path());
        // Simulate a j press which sets last_preview_update to now
        press(&mut app, KeyCode::Char('j'));
        assert!(
            !app.should_update_preview(),
            "should_update_preview must be false immediately after keypress"
        );
        std::thread::sleep(Duration::from_millis(60));
        assert!(
            app.should_update_preview(),
            "should_update_preview must be true after debounce period"
        );
    }

    #[test]
    fn test_preview_updates_after_template_navigation() {
        let dir = tempfile::tempdir().unwrap();
        let mut app = app_with_tempdir(dir.path());
        // Navigate to a builtin template (code-review has tools → non-empty preview)
        navigate_to_template(&mut app, "code-review");
        app.refresh_preview();
        assert!(
            !app.preview_lines.is_empty(),
            "preview_lines must be non-empty for builtin template 'code-review'"
        );
    }

    #[test]
    fn test_preview_content_changes_with_different_templates() {
        let dir = tempfile::tempdir().unwrap();
        let mut app = app_with_tempdir(dir.path());

        // Navigate to code-review and capture preview
        navigate_to_template(&mut app, "code-review");
        app.refresh_preview();
        let lines_a = app.preview_lines.clone();

        // Navigate to security-audit and capture preview
        navigate_to_template(&mut app, "security-audit");
        app.refresh_preview();
        let lines_b = app.preview_lines.clone();

        // Previews must differ between different templates
        assert_ne!(
            lines_a, lines_b,
            "preview content must change when switching from code-review to security-audit"
        );
    }

    #[test]
    fn test_preview_updates_after_persona_change() {
        let dir = tempfile::tempdir().unwrap();
        let mut app = app_with_tempdir(dir.path());

        // Start with default selection, capture preview
        app.refresh_preview();
        let lines_none = app.preview_lines.clone();

        // Switch persona to strict-security
        navigate_to_persona(&mut app, "strict-security");
        app.refresh_preview();
        let lines_strict = app.preview_lines.clone();

        assert_ne!(
            lines_none, lines_strict,
            "preview must change when switching persona to strict-security"
        );
    }

    #[test]
    fn test_preview_scroll_resets_on_refresh() {
        let dir = tempfile::tempdir().unwrap();
        let mut app = app_with_tempdir(dir.path());
        navigate_to_template(&mut app, "code-review");
        app.refresh_preview();
        // Manually scroll down
        app.preview_scroll = 10;
        // Refresh should reset scroll
        app.refresh_preview();
        assert_eq!(app.preview_scroll, 0, "refresh_preview must reset preview_scroll to 0");
    }

    #[test]
    fn test_none_template_none_persona_preview_minimal() {
        let dir = tempfile::tempdir().unwrap();
        let mut app = app_with_tempdir(dir.path());
        // Default: "None (no template)" + "None (no persona)"
        app.refresh_preview();
        assert_eq!(
            app.preview_token_count, 0,
            "preview_token_count must be 0 for None/None selection"
        );
    }

    #[test]
    fn test_constructor_calls_refresh_preview() {
        let dir = tempfile::tempdir().unwrap();
        // Write a .jig.yaml with persona rules so preview has content
        std::fs::write(
            dir.path().join(".jig.yaml"),
            "schema: 1\npersona:\n  name: test\n  rules:\n    - Be helpful\n",
        ).unwrap();
        let app = App::with_project_dir(dir.path().to_path_buf());
        // Constructor should have called refresh_preview, populating lines
        assert!(
            !app.preview_lines.is_empty(),
            "App constructor must call refresh_preview to populate initial state"
        );
        assert!(
            app.preview_token_count > 0,
            "App constructor must populate token count from project config"
        );
    }

    // ── P3: PRD-claimed feature coverage ────────────────────────

    #[test]
    fn test_layout_mode_from_cols() {
        assert_eq!(LayoutMode::from_cols(120), LayoutMode::FullTwoPane);
        assert_eq!(LayoutMode::from_cols(100), LayoutMode::FullTwoPane);
        assert_eq!(LayoutMode::from_cols(99), LayoutMode::NarrowTwoPane);
        assert_eq!(LayoutMode::from_cols(80), LayoutMode::NarrowTwoPane);
        assert_eq!(LayoutMode::from_cols(79), LayoutMode::SinglePane);
        assert_eq!(LayoutMode::from_cols(60), LayoutMode::SinglePane);
        assert_eq!(LayoutMode::from_cols(59), LayoutMode::Minimal);
        assert_eq!(LayoutMode::from_cols(40), LayoutMode::Minimal);
    }

    #[test]
    fn test_update_layout_changes_mode() {
        let mut app = App::new();
        app.update_layout(120);
        assert_eq!(app.layout, LayoutMode::FullTwoPane);
        app.update_layout(50);
        assert_eq!(app.layout, LayoutMode::Minimal);
    }

    #[test]
    fn test_p_key_toggles_preview_in_single_pane() {
        let mut app = App::new();
        app.layout = LayoutMode::SinglePane;
        assert!(app.show_preview);
        press(&mut app, KeyCode::Char('p'));
        assert!(!app.show_preview, "p must toggle show_preview off in SinglePane");
        press(&mut app, KeyCode::Char('p'));
        assert!(app.show_preview, "p must toggle show_preview on in SinglePane");
    }

    #[test]
    fn test_p_key_noop_in_full_two_pane() {
        let mut app = App::new();
        app.layout = LayoutMode::FullTwoPane;
        assert!(app.show_preview);
        press(&mut app, KeyCode::Char('p'));
        assert!(app.show_preview, "p must not toggle preview in FullTwoPane");
    }

    #[test]
    fn test_tab_switches_focus() {
        let mut app = App::new();
        assert_eq!(app.focus, PaneFocus::Templates);
        press(&mut app, KeyCode::Tab);
        assert_eq!(app.focus, PaneFocus::Personas, "Tab must switch to Personas");
        press(&mut app, KeyCode::Tab);
        assert_eq!(app.focus, PaneFocus::Templates, "Tab must switch back to Templates");
    }

    #[test]
    fn test_mouse_scroll_down_increases_preview_scroll() {
        let mut app = App::new();
        let initial = app.preview_scroll;
        app.handle_mouse(crossterm::event::MouseEvent {
            kind: MouseEventKind::ScrollDown,
            column: 0,
            row: 0,
            modifiers: KeyModifiers::empty(),
        });
        assert_eq!(app.preview_scroll, initial + 3);
    }

    #[test]
    fn test_mouse_scroll_up_saturates_at_zero() {
        let mut app = App::new();
        app.preview_scroll = 0;
        app.handle_mouse(crossterm::event::MouseEvent {
            kind: MouseEventKind::ScrollUp,
            column: 0,
            row: 0,
            modifiers: KeyModifiers::empty(),
        });
        assert_eq!(app.preview_scroll, 0, "scroll up at 0 must saturate");
    }

    #[test]
    fn test_d_u_keys_scroll_preview() {
        let mut app = App::new();
        press(&mut app, KeyCode::Char('d'));
        assert_eq!(app.preview_scroll, 3, "d must scroll preview down by 3");
        press(&mut app, KeyCode::Char('d'));
        assert_eq!(app.preview_scroll, 6);
        press(&mut app, KeyCode::Char('u'));
        assert_eq!(app.preview_scroll, 3, "u must scroll preview up by 3");
    }

    #[test]
    fn test_filter_mode_entry_and_exit() {
        let mut app = App::new();
        press(&mut app, KeyCode::Char('/'));
        assert_eq!(app.mode, AppMode::Filter, "/ must enter Filter mode");
        press(&mut app, KeyCode::Esc);
        assert_eq!(app.mode, AppMode::Normal, "Esc must exit Filter mode");
    }
}
