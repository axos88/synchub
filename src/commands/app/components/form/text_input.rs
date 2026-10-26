//! Text being typed, with a cursor in it.

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

#[derive(Default)]
pub struct TextInput {
    value: String,
    /// Where typing goes, in characters.
    cursor: usize,
}

impl TextInput {
    /// `value`, the cursor at its end.
    pub fn new(value: &str) -> Self {
        TextInput {
            value: value.to_string(),
            cursor: value.chars().count(),
        }
    }

    pub fn value(&self) -> &str {
        &self.value
    }

    /// Typing and moving in it: Left, Right, Home and End (Ctrl-A, Ctrl-E),
    /// Backspace, Delete, Ctrl-U to clear; whether it took `key`.
    pub fn key(&mut self, key: KeyEvent) -> bool {
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        let len = self.value.chars().count();
        match key.code {
            KeyCode::Char('a') if ctrl => self.cursor = 0,
            KeyCode::Char('e') if ctrl => self.cursor = len,
            KeyCode::Char('u') if ctrl => {
                self.value.clear();
                self.cursor = 0;
            }
            KeyCode::Char(c) if !ctrl => {
                self.value.insert(self.byte(self.cursor), c);
                self.cursor += 1;
            }
            KeyCode::Backspace if self.cursor > 0 => {
                self.cursor -= 1;
                self.value.remove(self.byte(self.cursor));
            }
            KeyCode::Delete if self.cursor < len => {
                self.value.remove(self.byte(self.cursor));
            }
            KeyCode::Left => self.cursor = self.cursor.saturating_sub(1),
            KeyCode::Right => self.cursor = (self.cursor + 1).min(len),
            KeyCode::Home => self.cursor = 0,
            KeyCode::End => self.cursor = len,
            _ => return false,
        }
        true
    }

    /// What of it fits in `width` columns with the cursor in view, and the
    /// cursor's column in that.
    pub fn shown(&self, width: usize) -> (String, usize) {
        let start = (self.cursor + 1).saturating_sub(width.max(1));
        let shown = self.value.chars().skip(start).take(width).collect();
        (shown, self.cursor - start)
    }

    /// The byte offset of the character `at`.
    fn byte(&self, at: usize) -> usize {
        self.value
            .char_indices()
            .nth(at)
            .map_or(self.value.len(), |(i, _)| i)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn press(input: &mut TextInput, keys: &[KeyCode]) {
        keys.iter()
            .for_each(|&code| assert!(input.key(KeyEvent::from(code)), "{code:?}"));
    }

    #[test]
    fn typing_goes_where_the_cursor_is() {
        let mut input = TextInput::new("néz");
        press(&mut input, &[KeyCode::Left, KeyCode::Char('x')]);
        assert_eq!(input.value(), "néxz");
        press(
            &mut input,
            &[
                KeyCode::Home,
                KeyCode::Char('<'),
                KeyCode::End,
                KeyCode::Char('>'),
            ],
        );
        assert_eq!(input.value(), "<néxz>");
        press(
            &mut input,
            &[
                KeyCode::Left,
                KeyCode::Backspace,
                KeyCode::Home,
                KeyCode::Delete,
            ],
        );
        assert_eq!(input.value(), "néx>");
    }

    #[test]
    fn it_doesnt_take_what_it_cant_do() {
        let mut input = TextInput::new("ab");
        assert!(!input.key(KeyEvent::from(KeyCode::Delete)));
        assert!(!input.key(KeyEvent::from(KeyCode::Up)));
        press(&mut input, &[KeyCode::Home]);
        assert!(!input.key(KeyEvent::from(KeyCode::Backspace)));
    }

    #[test]
    fn long_text_scrolls_to_keep_the_cursor_in_view() {
        let mut input = TextInput::new("abcdefgh");
        assert_eq!(input.shown(4), ("fgh".to_string(), 3));
        press(&mut input, &[KeyCode::Home]);
        assert_eq!(input.shown(4), ("abcd".to_string(), 0));
    }
}
