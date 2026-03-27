// Per-section state helpers
// Currently a placeholder — section state is managed in EditorState directly
// This module exists for future expansion of per-section complex state

pub struct ListSectionState {
    pub items: Vec<String>,
    pub cursor: usize,
}

impl ListSectionState {
    pub fn new(items: Vec<String>) -> Self {
        Self { items, cursor: 0 }
    }

    pub fn move_up(&mut self) {
        self.cursor = self.cursor.saturating_sub(1);
    }

    pub fn move_down(&mut self) {
        if !self.items.is_empty() {
            self.cursor = (self.cursor + 1).min(self.items.len() - 1);
        }
    }

    pub fn insert(&mut self, value: String) {
        self.items.push(value);
        self.cursor = self.items.len() - 1;
    }

    pub fn delete_at_cursor(&mut self) {
        if self.cursor < self.items.len() {
            self.items.remove(self.cursor);
            if self.cursor >= self.items.len() && !self.items.is_empty() {
                self.cursor = self.items.len() - 1;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_insert_and_delete_roundtrip() {
        let mut state = ListSectionState::new(vec![]);
        state.insert("Read".to_owned());
        state.insert("Bash".to_owned());
        assert_eq!(state.items.len(), 2);

        state.cursor = 0;
        state.delete_at_cursor();
        assert_eq!(state.items, vec!["Bash"]);
    }

    #[test]
    fn test_cursor_stays_in_bounds_after_delete() {
        let mut state = ListSectionState::new(vec!["a".to_owned(), "b".to_owned()]);
        state.cursor = 1;
        state.delete_at_cursor(); // deletes "b"
        assert_eq!(state.cursor, 0, "cursor must stay in bounds");
    }

    #[test]
    fn test_move_up_at_zero_stays() {
        let mut state = ListSectionState::new(vec!["a".to_owned()]);
        state.cursor = 0;
        state.move_up();
        assert_eq!(state.cursor, 0);
    }
}
