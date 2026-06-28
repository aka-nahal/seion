//! A minimal single-line text input with a movable cursor.
//!
//! Char-indexed throughout, so it behaves correctly with multi-byte input (a
//! search for `夜に駆ける` edits cleanly). It holds no styling — the search view
//! decides how to draw it.

/// Editable text plus a cursor position (measured in characters).
#[derive(Default, Clone, Debug)]
pub struct Input {
    text: String,
    /// Cursor position as a character index in `0..=len`.
    cursor: usize,
}

impl Input {
    /// The current text.
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Is the field empty?
    pub fn is_empty(&self) -> bool {
        self.text.is_empty()
    }

    /// Cursor position, in characters from the start.
    pub fn cursor(&self) -> usize {
        self.cursor
    }

    /// Number of characters.
    pub fn len(&self) -> usize {
        self.text.chars().count()
    }

    /// Insert a character at the cursor and step the cursor forward.
    pub fn insert(&mut self, c: char) {
        let byte = self.byte_at(self.cursor);
        self.text.insert(byte, c);
        self.cursor += 1;
    }

    /// Delete the character before the cursor (backspace).
    pub fn backspace(&mut self) {
        if self.cursor == 0 {
            return;
        }
        let start = self.byte_at(self.cursor - 1);
        let end = self.byte_at(self.cursor);
        self.text.replace_range(start..end, "");
        self.cursor -= 1;
    }

    /// Move the cursor one character left.
    pub fn left(&mut self) {
        self.cursor = self.cursor.saturating_sub(1);
    }

    /// Move the cursor one character right.
    pub fn right(&mut self) {
        if self.cursor < self.len() {
            self.cursor += 1;
        }
    }

    /// Jump to the start.
    pub fn home(&mut self) {
        self.cursor = 0;
    }

    /// Jump to the end.
    pub fn end(&mut self) {
        self.cursor = self.len();
    }

    /// Byte offset of character index `char_idx` (clamped to the string length).
    fn byte_at(&self, char_idx: usize) -> usize {
        self.text
            .char_indices()
            .nth(char_idx)
            .map(|(b, _)| b)
            .unwrap_or(self.text.len())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn types_and_deletes() {
        let mut input = Input::default();
        for c in "lofi".chars() {
            input.insert(c);
        }
        assert_eq!(input.text(), "lofi");
        assert_eq!(input.cursor(), 4);
        input.backspace();
        assert_eq!(input.text(), "lof");
    }

    #[test]
    fn cursor_moves_and_inserts_in_the_middle() {
        let mut input = Input::default();
        for c in "loi".chars() {
            input.insert(c);
        }
        input.left(); // between 'o' and 'i'
        input.insert('f');
        assert_eq!(input.text(), "lofi");
    }

    #[test]
    fn handles_multibyte_characters() {
        let mut input = Input::default();
        for c in "夜に".chars() {
            input.insert(c);
        }
        input.backspace();
        assert_eq!(input.text(), "夜");
        assert_eq!(input.len(), 1);
    }
}
