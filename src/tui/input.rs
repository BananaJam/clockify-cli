/// A minimal single-line text input; cursor is a char index.
pub struct Input {
    value: String,
    cursor: usize,
}

impl Input {
    pub fn new(initial: &str) -> Input {
        Input { value: initial.to_string(), cursor: initial.chars().count() }
    }

    pub fn value(&self) -> &str {
        &self.value
    }

    fn byte_idx(&self, char_idx: usize) -> usize {
        self.value
            .char_indices()
            .nth(char_idx)
            .map(|(i, _)| i)
            .unwrap_or(self.value.len())
    }

    pub fn insert(&mut self, c: char) {
        let i = self.byte_idx(self.cursor);
        self.value.insert(i, c);
        self.cursor += 1;
    }

    pub fn backspace(&mut self) {
        if self.cursor > 0 {
            self.cursor -= 1;
            let i = self.byte_idx(self.cursor);
            self.value.remove(i);
        }
    }

    pub fn delete(&mut self) {
        if self.cursor < self.value.chars().count() {
            let i = self.byte_idx(self.cursor);
            self.value.remove(i);
        }
    }

    pub fn left(&mut self) {
        self.cursor = self.cursor.saturating_sub(1);
    }

    pub fn right(&mut self) {
        self.cursor = (self.cursor + 1).min(self.value.chars().count());
    }

    pub fn home(&mut self) {
        self.cursor = 0;
    }

    pub fn end(&mut self) {
        self.cursor = self.value.chars().count();
    }

    /// (before cursor, char under cursor, after cursor) for rendering.
    pub fn split_at_cursor(&self) -> (String, String, String) {
        let chars: Vec<char> = self.value.chars().collect();
        let before: String = chars[..self.cursor].iter().collect();
        let under: String = chars.get(self.cursor).map(|c| c.to_string()).unwrap_or_else(|| " ".to_string());
        let after: String = if self.cursor < chars.len() {
            chars[self.cursor + 1..].iter().collect()
        } else {
            String::new()
        };
        (before, under, after)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn editing_works() {
        let mut i = Input::new("ab");
        i.insert('c'); // abc
        assert_eq!(i.value(), "abc");
        i.left();
        i.left();
        i.insert('x'); // axbc
        assert_eq!(i.value(), "axbc");
        i.backspace(); // abc
        assert_eq!(i.value(), "abc");
        i.end();
        i.backspace();
        assert_eq!(i.value(), "ab");
        i.home();
        i.delete();
        assert_eq!(i.value(), "b");
    }

    #[test]
    fn unicode_safe() {
        let mut i = Input::new("héllo");
        i.end();
        i.backspace();
        assert_eq!(i.value(), "héll");
        i.home();
        i.right();
        i.delete();
        assert_eq!(i.value(), "hll");
    }
}
