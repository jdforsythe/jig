use nucleo_matcher::{
    Config, Matcher, Utf32Str,
    pattern::{Atom, AtomKind, CaseMatching, Normalization},
};
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Modifier, Style},
    text::Line,
    widgets::{Block, Borders, List, ListItem, ListState, StatefulWidget},
};

use crate::theme::active_theme;

/// State for a fuzzy-filterable list widget.
/// `Matcher` is `!Send + !Sync` — must stay on the TUI thread.
pub struct FilterableListState {
    pub items: Vec<String>,
    pub filtered: Vec<(u16, usize)>, // (score, original_index)
    pub query: String,
    pub list_state: ListState,
    matcher: Matcher,
}

impl FilterableListState {
    pub fn new(items: Vec<String>) -> Self {
        let mut state = Self {
            items,
            filtered: Vec::new(),
            query: String::new(),
            list_state: ListState::default(),
            matcher: Matcher::new(Config::DEFAULT),
        };
        // Pre-populate at init (not on first keypress)
        state.update_filter();
        if !state.filtered.is_empty() {
            state.list_state.select(Some(0));
        }
        state
    }

    pub fn update_filter(&mut self) {
        if self.query.is_empty() {
            // Empty query shows all items
            self.filtered = self
                .items
                .iter()
                .enumerate()
                .map(|(i, _)| (0u16, i))
                .collect();
            return;
        }

        let atom = Atom::new(&self.query, CaseMatching::Smart, Normalization::Smart, AtomKind::Fuzzy, false);
        self.filtered = self
            .items
            .iter()
            .enumerate()
            .filter_map(|(i, item)| {
                let mut indices = Vec::new(); // scratch buffer — must outlive Utf32Str
                let haystack = Utf32Str::new(item, &mut indices);
                atom.score(haystack, &mut self.matcher)
                    .map(|score| (score, i))
            })
            .collect();

        // High score first
        self.filtered.sort_unstable_by(|a, b| b.0.cmp(&a.0));
    }

    pub fn push_char(&mut self, c: char) {
        self.query.push(c);
        self.update_filter();
        // Reset selection to top
        if self.filtered.is_empty() {
            self.list_state.select(None);
        } else {
            self.list_state.select(Some(0));
        }
    }

    pub fn pop_char(&mut self) {
        self.query.pop();
        self.update_filter();
        if self.filtered.is_empty() {
            self.list_state.select(None);
        } else {
            self.list_state.select(Some(0));
        }
    }

    pub fn clear_query(&mut self) {
        self.query.clear();
        self.update_filter();
        self.list_state.select(Some(0));
    }

    pub fn selected_item(&self) -> Option<&str> {
        let selected = self.list_state.selected()?;
        let (_, orig_idx) = self.filtered.get(selected)?;
        self.items.get(*orig_idx).map(String::as_str)
    }

    pub fn move_down(&mut self) {
        let len = self.filtered.len();
        if len == 0 {
            return;
        }
        let next = self.list_state.selected().map_or(0, |i| (i + 1).min(len - 1));
        self.list_state.select(Some(next));
    }

    pub fn move_up(&mut self) {
        if self.filtered.is_empty() {
            return;
        }
        let prev = self.list_state.selected().map_or(0, |i| i.saturating_sub(1));
        self.list_state.select(Some(prev));
    }
}

/// Widget wrapper for rendering a FilterableListState.
pub struct FilterableListWidget<'a> {
    pub title: &'a str,
    pub focused: bool,
}

impl<'a> StatefulWidget for FilterableListWidget<'a> {
    type State = FilterableListState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        let theme = active_theme();
        let border_style = if self.focused {
            Style::default().fg(theme.border_focused)
        } else {
            Style::default().fg(theme.border_unfocused)
        };

        let items: Vec<ListItem> = if state.filtered.is_empty() {
            vec![ListItem::new(Line::from("No results"))]
        } else {
            state
                .filtered
                .iter()
                .map(|(_, orig_idx)| {
                    let name = state.items.get(*orig_idx).map(String::as_str).unwrap_or("");
                    ListItem::new(Line::from(name))
                })
                .collect()
        };

        let title = format!(" {} ", self.title);
        let filter_suffix = if !state.query.is_empty() {
            format!(" /{}/", state.query)
        } else {
            String::new()
        };
        let block = Block::default()
            .title(format!("{title}{filter_suffix}"))
            .borders(Borders::ALL)
            .border_style(border_style);

        let list = List::new(items)
            .block(block)
            .highlight_style(Style::default().add_modifier(Modifier::BOLD));

        StatefulWidget::render(list, area, buf, &mut state.list_state);
    }
}
