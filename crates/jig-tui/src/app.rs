/// TUI Application — Phase 2 implementation.
///
/// Decision (brainstorm §4): TUI shows on bare `jig`. Two-pane layout.
use std::io;
use std::time::{Duration, Instant};

use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind, MouseEventKind},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
    cursor,
};
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
}

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}

impl App {
    pub fn new() -> Self {
        let template_names: Vec<String> = builtin_templates()
            .into_iter()
            .map(|t| t.name)
            .collect();

        let persona_names = vec![
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

        Self {
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
            last_preview_update: Instant::now(),
        }
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
            AppMode::Normal | AppMode::Confirm => self.handle_normal_key(key),
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
                    self.launch_selection = Some((t, p));
                    self.should_quit = true;
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

fn render_which_key(frame: &mut ratatui::Frame, area: Rect) {
    use ratatui::widgets::Clear;

    let popup_width = 50u16.min(area.width.saturating_sub(4));
    let popup_height = 16u16.min(area.height.saturating_sub(4));
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
        Line::from(" d      Dry-run"),
        Line::from(" p      Toggle preview"),
        Line::from(" C-d    Scroll preview ↓"),
        Line::from(" C-u    Scroll preview ↑"),
        Line::from(" ?      This help"),
        Line::from(" q/Esc  Quit"),
    ];

    let popup = Paragraph::new(keybindings)
        .block(Block::default().borders(Borders::ALL).title(" Help "));
    frame.render_widget(popup, popup_area);
}
