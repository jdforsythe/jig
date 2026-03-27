use crossterm::event::{KeyCode, KeyEvent};
use jig_core::editor::SaveScope;
use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
};

pub struct SavePopupState {
    pub scope: SaveScope,
    pub name_input: String,
    pub focus: SaveFocus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SaveFocus {
    NameField,
    ScopeSelector,
    SaveButton,
    CancelButton,
}

impl SavePopupState {
    pub fn new(current_name: &str) -> Self {
        Self {
            scope: SaveScope::Project,
            name_input: current_name.to_owned(),
            focus: SaveFocus::NameField,
        }
    }

    /// Returns Some(scope, name) when user confirms, None when cancelled.
    pub fn handle_key(&mut self, key: KeyEvent) -> SavePopupResult {
        match key.code {
            KeyCode::Tab => {
                self.focus = match self.focus {
                    SaveFocus::NameField => SaveFocus::ScopeSelector,
                    SaveFocus::ScopeSelector => SaveFocus::SaveButton,
                    SaveFocus::SaveButton => SaveFocus::CancelButton,
                    SaveFocus::CancelButton => SaveFocus::NameField,
                };
                SavePopupResult::Continue
            }
            KeyCode::Esc => SavePopupResult::Cancel,
            KeyCode::Enter => match self.focus {
                SaveFocus::SaveButton => SavePopupResult::Save {
                    scope: self.scope,
                    name: self.name_input.trim().to_owned(),
                },
                SaveFocus::CancelButton => SavePopupResult::Cancel,
                SaveFocus::NameField => {
                    self.focus = SaveFocus::ScopeSelector;
                    SavePopupResult::Continue
                }
                SaveFocus::ScopeSelector => {
                    self.focus = SaveFocus::SaveButton;
                    SavePopupResult::Continue
                }
            },
            KeyCode::Char('h') | KeyCode::Left if self.focus == SaveFocus::ScopeSelector => {
                self.scope = match self.scope {
                    SaveScope::Local => SaveScope::Project,
                    SaveScope::Project => SaveScope::Global,
                    SaveScope::Global => SaveScope::Global,
                };
                SavePopupResult::Continue
            }
            KeyCode::Char('l') | KeyCode::Right if self.focus == SaveFocus::ScopeSelector => {
                self.scope = match self.scope {
                    SaveScope::Global => SaveScope::Project,
                    SaveScope::Project => SaveScope::Local,
                    SaveScope::Local => SaveScope::Local,
                };
                SavePopupResult::Continue
            }
            KeyCode::Backspace if self.focus == SaveFocus::NameField => {
                self.name_input.pop();
                SavePopupResult::Continue
            }
            KeyCode::Char(c) if self.focus == SaveFocus::NameField => {
                self.name_input.push(c);
                SavePopupResult::Continue
            }
            _ => SavePopupResult::Continue,
        }
    }
}

#[derive(Debug)]
pub enum SavePopupResult {
    Continue,
    Cancel,
    Save { scope: SaveScope, name: String },
}

pub fn render_save_popup(frame: &mut Frame, state: &SavePopupState, area: Rect) {
    let popup_width = 50.min(area.width.saturating_sub(4));
    let popup_height = 8.min(area.height.saturating_sub(4));

    let x = area.x + (area.width.saturating_sub(popup_width)) / 2;
    let y = area.y + (area.height.saturating_sub(popup_height)) / 2;
    let popup_area = Rect::new(x, y, popup_width, popup_height);

    frame.render_widget(Clear, popup_area);

    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Save as Template ");

    let inner = block.inner(popup_area);
    frame.render_widget(block, popup_area);

    let rows = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
    ])
    .split(inner);

    // Name field
    let name_style = if state.focus == SaveFocus::NameField {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default()
    };
    let cursor = if state.focus == SaveFocus::NameField {
        "_"
    } else {
        ""
    };
    frame.render_widget(
        Paragraph::new(format!("Name: {}{}", state.name_input, cursor)).style(name_style),
        rows[0],
    );

    // Scope selector
    let scopes = [
        (SaveScope::Global, "Global"),
        (SaveScope::Project, "Project"),
        (SaveScope::Local, "Local"),
    ];
    let scope_spans: Vec<Span> = scopes
        .iter()
        .map(|(s, label)| {
            let style = if *s == state.scope {
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD)
            } else if state.focus == SaveFocus::ScopeSelector {
                Style::default().fg(Color::DarkGray)
            } else {
                Style::default()
            };
            Span::styled(format!("[{label}] "), style)
        })
        .collect();
    let scope_label = Span::raw("Scope: ");
    let mut spans = vec![scope_label];
    spans.extend(scope_spans);
    frame.render_widget(Paragraph::new(Line::from(spans)), rows[1]);

    // Buttons
    let save_style = if state.focus == SaveFocus::SaveButton {
        Style::default()
            .fg(Color::Green)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default()
    };
    let cancel_style = if state.focus == SaveFocus::CancelButton {
        Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)
    } else {
        Style::default()
    };

    let buttons = Line::from(vec![
        Span::styled("[Save]", save_style),
        Span::raw("  "),
        Span::styled("[Cancel]", cancel_style),
    ]);
    frame.render_widget(
        Paragraph::new(buttons).alignment(Alignment::Center),
        rows[3],
    );

    frame.render_widget(
        Paragraph::new("Tab:focus  h/l:scope  Enter:confirm  Esc:cancel")
            .style(Style::default().fg(Color::DarkGray)),
        rows[4],
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scope_cycling_right() {
        let mut state = SavePopupState::new("test");
        state.focus = SaveFocus::ScopeSelector;
        assert_eq!(state.scope, SaveScope::Project);

        state.handle_key(KeyEvent::new(
            KeyCode::Char('l'),
            crossterm::event::KeyModifiers::NONE,
        ));
        assert_eq!(state.scope, SaveScope::Local);
    }

    #[test]
    fn test_scope_cycling_left() {
        let mut state = SavePopupState::new("test");
        state.focus = SaveFocus::ScopeSelector;

        state.handle_key(KeyEvent::new(
            KeyCode::Char('h'),
            crossterm::event::KeyModifiers::NONE,
        ));
        assert_eq!(state.scope, SaveScope::Global);
    }

    #[test]
    fn test_name_input() {
        let mut state = SavePopupState::new("");
        state.focus = SaveFocus::NameField;

        state.handle_key(KeyEvent::new(
            KeyCode::Char('a'),
            crossterm::event::KeyModifiers::NONE,
        ));
        state.handle_key(KeyEvent::new(
            KeyCode::Char('b'),
            crossterm::event::KeyModifiers::NONE,
        ));
        assert_eq!(state.name_input, "ab");
    }

    #[test]
    fn test_cancel_returns_cancel() {
        let mut state = SavePopupState::new("test");
        let result = state.handle_key(KeyEvent::new(
            KeyCode::Esc,
            crossterm::event::KeyModifiers::NONE,
        ));
        assert!(matches!(result, SavePopupResult::Cancel));
    }

    #[test]
    fn test_save_button_returns_save() {
        let mut state = SavePopupState::new("my-template");
        state.focus = SaveFocus::SaveButton;
        let result = state.handle_key(KeyEvent::new(
            KeyCode::Enter,
            crossterm::event::KeyModifiers::NONE,
        ));
        assert!(matches!(result, SavePopupResult::Save { .. }));
        if let SavePopupResult::Save { name, scope } = result {
            assert_eq!(name, "my-template");
            assert_eq!(scope, SaveScope::Project);
        }
    }
}
