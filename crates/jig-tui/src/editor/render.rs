use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Wrap},
};

use super::{EditorSection, EditorState, SectionInputMode};
use crate::app::AppMode;

pub fn render_editor(frame: &mut Frame, state: &mut EditorState, area: Rect) {
    // Layout: left side (sections + content) + right preview pane
    let has_preview = area.width >= 100;

    let (left_area, preview_area) = if has_preview {
        let chunks = Layout::horizontal([Constraint::Fill(1), Constraint::Percentage(38)]).split(area);
        (chunks[0], Some(chunks[1]))
    } else {
        (area, None)
    };

    // Split left into: section nav (22 wide) + content
    let left_chunks =
        Layout::horizontal([Constraint::Length(22), Constraint::Fill(1)]).split(left_area);

    let nav_area = left_chunks[0];
    let content_area = left_chunks[1];

    // Split content into: main content + status bar (1 line)
    let content_chunks =
        Layout::vertical([Constraint::Fill(1), Constraint::Length(1)]).split(content_area);

    render_section_nav(frame, state, nav_area);
    render_section_content(frame, state, content_chunks[0]);
    render_status_bar(frame, state, content_chunks[1]);

    if let Some(prev_area) = preview_area {
        render_preview_pane(frame, state, prev_area);
    }
}

pub fn render_editor_standalone(frame: &mut Frame, state: &mut EditorState, _mode: &AppMode) {
    render_editor(frame, state, frame.area());
}

fn render_section_nav(frame: &mut Frame, state: &EditorState, area: Rect) {
    let items: Vec<ListItem> = EditorSection::ALL
        .iter()
        .map(|s| {
            let style = if *s == state.section {
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };
            ListItem::new(Line::from(Span::styled(s.title(), style)))
        })
        .collect();

    let list = List::new(items).block(Block::default().borders(Borders::ALL).title(" Sections "));

    let mut list_state = ListState::default();
    list_state.select(Some(state.section.index()));

    frame.render_stateful_widget(list, area, &mut list_state);
}

fn render_section_content(frame: &mut Frame, state: &EditorState, area: Rect) {
    let title = format!(" {} ", state.section.title());
    let block = Block::default().borders(Borders::ALL).title(title);

    if state.section.is_single_line() {
        let value = if state.input_mode == SectionInputMode::EditLine {
            format!("{}_", state.input_buffer) // show cursor
        } else {
            state.section_value()
        };

        let hint = if state.input_mode == SectionInputMode::Navigate {
            " [i/Enter to edit]"
        } else {
            " [Enter to confirm, Esc to cancel]"
        };

        let text = format!("{value}{hint}");
        let para = Paragraph::new(text)
            .block(block)
            .wrap(Wrap { trim: false });
        frame.render_widget(para, area);
    } else {
        // List section
        let section_items = state.section_items();
        let mut items_to_show: Vec<ListItem> = section_items
            .iter()
            .enumerate()
            .map(|(i, item)| {
                let style = if i == state.section_cursor {
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default()
                };
                ListItem::new(Span::styled(item.clone(), style))
            })
            .collect();

        // Show input buffer when in insert/edit mode
        if matches!(
            state.input_mode,
            SectionInputMode::InsertItem | SectionInputMode::EditItem
        ) {
            let input_line = format!("> {}_", state.input_buffer);
            items_to_show.push(ListItem::new(Span::styled(
                input_line,
                Style::default().fg(Color::Green),
            )));
        }

        let hint = if items_to_show.is_empty() {
            " [a to add]".to_owned()
        } else if state.input_mode == SectionInputMode::Navigate {
            " [a:add d:del i:edit]".to_owned()
        } else {
            " [Enter:confirm Esc:cancel]".to_owned()
        };

        let list = List::new(items_to_show).block(block.title_bottom(hint));

        let mut list_state = ListState::default();
        list_state.select(if section_items.is_empty() {
            None
        } else {
            Some(state.section_cursor)
        });

        frame.render_stateful_widget(list, area, &mut list_state);
    }
}

fn render_status_bar(frame: &mut Frame, state: &EditorState, area: Rect) {
    let dirty_indicator = if state.dirty { "● " } else { "" };
    let mode_str = match state.input_mode {
        SectionInputMode::Navigate => "NAVIGATE",
        SectionInputMode::EditLine => "EDIT",
        SectionInputMode::InsertItem => "INSERT",
        SectionInputMode::EditItem => "EDIT",
    };
    let msg = state.status_message.as_deref().unwrap_or("");
    let text = format!(
        "{dirty_indicator}{mode_str}  Ctrl-S/:w=save  Ctrl-Z=undo  ?=help  q=exit  {msg}"
    );

    let para = Paragraph::new(text).style(Style::default().fg(Color::DarkGray));
    frame.render_widget(para, area);
}

fn render_preview_pane(frame: &mut Frame, state: &EditorState, area: Rect) {
    let token_info = format!("~{} tokens", state.preview_token_count);
    let block = Block::default()
        .borders(Borders::ALL)
        .title(format!(" Preview ({token_info}) "));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    // Render scrolled preview lines
    let start = state.preview_scroll as usize;
    let lines: Vec<Line> = state
        .preview_lines
        .iter()
        .skip(start)
        .take(inner.height as usize)
        .cloned()
        .collect();

    let para = Paragraph::new(lines);
    frame.render_widget(para, inner);
}
