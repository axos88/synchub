//! A list that remembers where it was drawn, so it can tell which item a
//! click is on: ratatui keeps nothing of a widget between frames.

use std::cell::Cell;

use ratatui::Frame;
use ratatui::crossterm::event::{MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::{Position, Rect};
use ratatui::widgets::{Block, List, ListState};

use crate::commands::app::gesture::{Gesture, GestureDetector};

/// A list of one-line items, with its selection, scroll position and where
/// it was last drawn. Kept in cells: drawing records where it went, and a
/// page is drawn from a shared reference - beneath a dialog too.
#[derive(Default)]
pub struct ClickableList {
    state: Cell<ListState>,
    /// Where the items were last drawn: inside the border.
    content_area: Cell<Rect>,
    /// How many there were.
    len: Cell<usize>,
    gestures: GestureDetector,
}

impl ClickableList {
    pub fn selected(&self) -> Option<usize> {
        self.state.get().selected()
    }

    pub fn select(&self, index: Option<usize>) {
        self.update(|state| state.select(index));
    }

    pub fn select_next(&self) {
        self.update(ListState::select_next);
    }

    pub fn select_previous(&self) {
        self.update(ListState::select_previous);
    }

    /// Draws `list` in `area`, inside `block`.
    pub fn draw(&self, frame: &mut Frame, area: Rect, block: Block, list: List) {
        self.content_area.set(block.inner(area));
        self.len.set(list.len());
        self.update(|state| frame.render_stateful_widget(list.block(block), area, state));
    }

    /// The gesture `mouse`'s left click makes on an item, as the list was last
    /// drawn.
    pub fn clicked(&mut self, mouse: &MouseEvent) -> Option<Gesture<usize>> {
        if mouse.kind != MouseEventKind::Down(MouseButton::Left) {
            return None;
        }
        let gesture = self.gestures.add(Position::new(mouse.column, mouse.row))?;
        gesture.map(|position| self.item_at(position)).transpose()
    }

    /// The item at `position`, as the list was last drawn.
    fn item_at(&self, position: Position) -> Option<usize> {
        let content_area = self.content_area.get();
        if !content_area.contains(position) {
            return None;
        }
        let index = self.state.get().offset() + usize::from(position.y - content_area.y);
        (index < self.len.get()).then_some(index)
    }

    fn update(&self, change: impl FnOnce(&mut ListState)) {
        let mut state = self.state.get();
        change(&mut state);
        self.state.set(state);
    }
}

#[cfg(test)]
mod tests {
    use ratatui::crossterm::event::KeyModifiers;

    use super::*;

    /// A bordered list of `len` items in (10, 5, 20, 6): the items on rows 6..=9.
    fn drawn(len: usize, offset: usize) -> ClickableList {
        let area = Rect::new(10, 5, 20, 6);
        ClickableList {
            state: Cell::new(ListState::default().with_offset(offset)),
            content_area: Cell::new(Block::bordered().title(" t ").inner(area)),
            len: Cell::new(len),
            gestures: GestureDetector::default(),
        }
    }

    fn click(column: u16, row: u16) -> MouseEvent {
        MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column,
            row,
            modifiers: KeyModifiers::NONE,
        }
    }

    fn index(list: &mut ClickableList, column: u16, row: u16) -> Option<usize> {
        list.clicked(&click(column, row)).map(Gesture::at)
    }

    #[test]
    fn clicks_map_to_the_items_inside_the_border() {
        let mut list = drawn(3, 0);
        assert_eq!(index(&mut list, 12, 6), Some(0));
        assert_eq!(index(&mut list, 12, 8), Some(2));
        assert_eq!(index(&mut list, 12, 9), None); // past the last item
        assert_eq!(index(&mut list, 10, 6), None); // on the border
        assert_eq!(index(&mut drawn(9, 4), 12, 7), Some(5)); // scrolled
    }

    #[test]
    fn only_left_clicks_count() {
        let mut list = drawn(3, 0);
        let right = MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Right),
            ..click(12, 6)
        };
        assert!(list.clicked(&right).is_none());
    }

    #[test]
    fn a_second_click_on_the_same_item_is_a_double_click() {
        let mut list = drawn(3, 0);
        let mut clicked = |row| list.clicked(&click(12, row));
        assert_eq!(clicked(7), Some(Gesture::Click(1)));
        assert_eq!(clicked(7), Some(Gesture::DoubleClick(1)));
        // A third starts over.
        assert_eq!(clicked(7), Some(Gesture::Click(1)));
        assert_eq!(clicked(8), Some(Gesture::Click(2)));
    }
}
