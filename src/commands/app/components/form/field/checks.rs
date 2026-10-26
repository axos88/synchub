//! A checkbox list: each item shown by its label, answered by its key.

use anyhow::Result;
use ratatui::Frame;
use ratatui::crossterm::event::{KeyCode, KeyEvent, MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, List, ListItem};

use super::{EventResponse, FieldKind};
use crate::commands::app::components::clickable_list::ClickableList;

pub struct ChecksField<K> {
    label: String,
    items: Vec<Check<K>>,
    cursor: usize,
    list: ClickableList,
}

/// An item of a checkbox list: shown by its `label`, answered by its `key`.
pub struct Check<K> {
    pub key: K,
    pub label: String,
    /// To start with.
    pub ticked: bool,
}

impl<K> ChecksField<K> {
    /// An empty list, its label and items to be set.
    pub fn new() -> Self {
        ChecksField {
            label: String::new(),
            items: Vec::new(),
            cursor: 0,
            list: ClickableList::default(),
        }
    }

    pub fn label(mut self, label: &str) -> Self {
        self.label = label.into();
        self
    }

    pub fn items(mut self, items: Vec<Check<K>>) -> Self {
        self.items = items;
        self
    }

    /// The keys of the items ticked.
    pub fn ticked(&self) -> impl Iterator<Item = &K> {
        self.items
            .iter()
            .filter(|item| item.ticked)
            .map(|item| &item.key)
    }

    fn toggle(&mut self, item: usize) {
        if let Some(item) = self.items.get_mut(item) {
            item.ticked = !item.ticked;
        }
    }
}

impl<K> FieldKind for ChecksField<K> {
    fn title(&self) -> String {
        self.label.clone()
    }

    fn height(&self) -> u16 {
        self.items.len().clamp(1, 12) as u16 + 2
    }

    fn draw(&mut self, frame: &mut Frame, area: Rect, block: Block, focused: bool) {
        let rows: Vec<ListItem> = self
            .items
            .iter()
            .map(|item| {
                ListItem::new(Line::from(vec![
                    Span::raw(if item.ticked { "[x] " } else { "[ ] " }),
                    Span::raw(item.label.as_str()),
                ]))
            })
            .collect();
        let highlight = if focused {
            Style::new().add_modifier(Modifier::REVERSED)
        } else {
            Style::new()
        };
        self.list.select(Some(self.cursor));
        self.list.draw(
            frame,
            area,
            block,
            List::new(rows).highlight_style(highlight),
        );
    }

    fn key(&mut self, key: KeyEvent) -> Result<EventResponse> {
        match key.code {
            KeyCode::Up if self.cursor > 0 => self.cursor -= 1,
            KeyCode::Down if self.cursor + 1 < self.items.len() => self.cursor += 1,
            KeyCode::Char(' ') => self.toggle(self.cursor),
            _ => return Ok(EventResponse::Ignored),
        }
        Ok(EventResponse::Handled)
    }

    fn mouse(&mut self, mouse: &MouseEvent) -> Result<EventResponse> {
        match mouse.kind {
            MouseEventKind::ScrollUp => self.cursor = self.cursor.saturating_sub(1),
            MouseEventKind::ScrollDown => {
                self.cursor = (self.cursor + 1).min(self.items.len().saturating_sub(1))
            }
            MouseEventKind::Down(MouseButton::Left) => {
                if let Some(gesture) = self.list.clicked(mouse) {
                    self.cursor = gesture.at();
                    self.toggle(gesture.at());
                }
            }
            _ => return Ok(EventResponse::Ignored),
        }
        Ok(EventResponse::Handled)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_leaves_moving_past_its_ends_to_the_form() {
        let check = |key: &'static str, ticked: bool| Check {
            key,
            label: key.to_uppercase(),
            ticked,
        };
        let mut field = ChecksField::new().items(vec![check("a", true), check("b", false)]);
        let mut press = |code| field.key(KeyEvent::from(code)).unwrap();
        assert!(matches!(press(KeyCode::Up), EventResponse::Ignored));
        assert!(matches!(press(KeyCode::Down), EventResponse::Handled));
        assert!(matches!(press(KeyCode::Down), EventResponse::Ignored));
        assert!(matches!(press(KeyCode::Char(' ')), EventResponse::Handled));
        assert_eq!(field.ticked().collect::<Vec<_>>(), [&"a", &"b"]);
    }
}
