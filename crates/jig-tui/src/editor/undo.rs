/// Capped undo stack. Capacity defaults to 50 snapshots.
pub struct UndoStack<T: Clone> {
    stack: Vec<T>,
    capacity: usize,
}

impl<T: Clone> UndoStack<T> {
    pub fn new(capacity: usize) -> Self {
        Self {
            stack: Vec::new(),
            capacity,
        }
    }

    /// Snapshot the current state before a mutation.
    pub fn push(&mut self, state: &T) {
        if self.stack.len() >= self.capacity {
            self.stack.remove(0);
        }
        self.stack.push(state.clone());
    }

    /// Restore the most recent snapshot.
    pub fn pop(&mut self) -> Option<T> {
        self.stack.pop()
    }

    pub fn can_undo(&self) -> bool {
        !self.stack.is_empty()
    }

    pub fn len(&self) -> usize {
        self.stack.len()
    }

    pub fn is_empty(&self) -> bool {
        self.stack.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_push_pop_roundtrip() {
        let mut stack: UndoStack<String> = UndoStack::new(50);
        stack.push(&"hello".to_owned());
        assert_eq!(stack.pop(), Some("hello".to_owned()));
    }

    #[test]
    fn test_can_undo_false_on_empty() {
        let stack: UndoStack<i32> = UndoStack::new(50);
        assert!(!stack.can_undo());
    }

    #[test]
    fn test_undo_at_capacity_evicts_oldest() {
        let mut stack: UndoStack<i32> = UndoStack::new(3);
        stack.push(&1);
        stack.push(&2);
        stack.push(&3);
        stack.push(&4); // evicts 1
        assert_eq!(stack.len(), 3);
        assert_eq!(stack.pop(), Some(4));
        assert_eq!(stack.pop(), Some(3));
        assert_eq!(stack.pop(), Some(2));
        assert_eq!(stack.pop(), None); // 1 was evicted
    }

    #[test]
    fn test_multiple_pushes_and_pops() {
        let mut stack: UndoStack<Vec<String>> = UndoStack::new(50);
        let v1 = vec!["a".to_owned()];
        let v2 = vec!["a".to_owned(), "b".to_owned()];
        stack.push(&v1);
        stack.push(&v2);
        assert_eq!(stack.pop(), Some(v2));
        assert_eq!(stack.pop(), Some(v1));
        assert!(!stack.can_undo());
    }
}
