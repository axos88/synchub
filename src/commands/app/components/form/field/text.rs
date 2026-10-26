//! A text field.

use anyhow::Result;
use ratatui::Frame;
use ratatui::crossterm::event::{KeyEvent, MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::{Margin, Position, Rect};
use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::widgets::{Block, Paragraph};

use super::super::text_input::TextInput;
use super::{EventResponse, FieldKind};

#[derive(Default)]
pub struct TextField {
    label: String,
    input: TextInput,
    /// Shown, dimmed, while the value is empty.
    placeholder: String,
    required: bool,
}

impl TextField {
    /// An empty field, not required, its label and the rest to be set.
    pub fn new() -> Self {
        Self::default()
    }

    pub fn label(mut self, label: &str) -> Self {
        self.label = label.into();
        self
    }

    /// The text it starts with, the cursor after it.
    pub fn value(mut self, value: &str) -> Self {
        self.input = TextInput::new(value);
        self
    }

    /// Shown, dimmed, while it's empty.
    pub fn placeholder(mut self, placeholder: &str) -> Self {
        self.placeholder = placeholder.into();
        self
    }

    /// The form isn't done while it's empty.
    pub fn required(mut self) -> Self {
        self.required = true;
        self
    }

    /// What's typed in it.
    pub fn text(&self) -> &str {
        self.input.value()
    }
}

impl FieldKind for TextField {
    fn title(&self) -> String {
        if self.required {
            format!("{} (required)", self.label)
        } else {
            self.label.clone()
        }
    }

    fn width(&self) -> usize {
        (self.text().chars().count() + 2).max(40)
    }

    fn missing(&self) -> bool {
        self.required && self.text().trim().is_empty()
    }

    fn draw(&mut self, frame: &mut Frame, area: Rect, block: Block, focused: bool) {
        let (shown, cursor) = self.input.shown(usize::from(area.width.saturating_sub(2)));
        let text = if self.text().is_empty() {
            Line::from(self.placeholder.as_str().dim())
        } else {
            Line::from(shown)
        };
        frame.render_widget(Paragraph::new(text).block(block), area);
        if focused {
            frame.set_cursor_position((area.x + 1 + cursor as u16, area.y + 1));
        }
    }

    fn key(&mut self, key: KeyEvent) -> Result<EventResponse> {
        Ok(if self.input.key(key) {
            EventResponse::Handled
        } else {
            EventResponse::Ignored
        })
    }

    fn mouse(&mut self, mouse: &MouseEvent, area: Rect) -> Result<EventResponse> {
        if mouse.kind != MouseEventKind::Down(MouseButton::Left) {
            return Ok(EventResponse::Ignored);
        }
        // On the border, it only focuses the field.
        let text = area.inner(Margin::new(1, 1));
        if text.contains(Position::new(mouse.column, mouse.row)) {
            let column = usize::from(mouse.column - text.x);
            self.input.place_cursor(column, usize::from(text.width));
        }
        Ok(EventResponse::Handled)
    }
}

#[cfg(test)]
mod tests {
    use ratatui::crossterm::event::{KeyCode, KeyModifiers};

    use super::*;

    #[test]
    fn it_takes_typing() {
        let mut field = TextField::new().value("a");
        let mut press = |code| field.key(KeyEvent::from(code)).unwrap();
        press(KeyCode::Char('b'));
        press(KeyCode::Backspace);
        press(KeyCode::Char('c'));
        assert!(matches!(press(KeyCode::Up), EventResponse::Ignored));
        assert_eq!(field.text(), "ac");
    }

    fn click(column: u16, row: u16) -> MouseEvent {
        MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column,
            row,
            modifiers: KeyModifiers::NONE,
        }
    }

    #[test]
    fn a_click_puts_the_cursor_where_it_lands() {
        // Drawn at (10, 5), 20 wide: the text starts at column 11, on row 6.
        let area = Rect::new(10, 5, 20, 3);
        let mut field = TextField::new().value("hello");
        assert!(matches!(
            field.mouse(&click(13, 6), area).unwrap(),
            EventResponse::Handled
        ));
        field.key(KeyEvent::from(KeyCode::Char('X'))).unwrap();
        assert_eq!(field.text(), "heXllo");
        // On the border it only focuses: the cursor stays.
        field.mouse(&click(10, 6), area).unwrap();
        field.key(KeyEvent::from(KeyCode::Char('Y'))).unwrap();
        assert_eq!(field.text(), "heXYllo");
    }

    #[test]
    fn its_empty_if_required_and_blank() {
        assert!(!TextField::new().value(" ").missing());
        assert!(TextField::new().value(" ").required().missing());
    }
}
