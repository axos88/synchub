//! A form's fields: text, a directory, or checkboxes - each its own type,
//! drawing itself and handling its own input; `Field` says which, and
//! forwards to it.

mod checks;
mod dir;
mod text;

use std::any::Any;

use anyhow::Result;
use ratatui::Frame;
use ratatui::crossterm::event::{KeyEvent, MouseEvent};
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::widgets::Block;

pub use checks::{Check, ChecksField};
pub use dir::DirField;
pub use text::TextField;

/// A field the form holds, by kind.
pub enum Field {
    Text(TextField),
    /// A directory, chosen in the directory browser rather than typed.
    Dir(DirField),
    /// Its key type hidden: a form holds every kind in one list.
    Checks(Box<dyn ChecksKind>),
}

/// What a field made of an event.
pub enum EventResponse {
    Handled,
    /// Not its business: the form's (e.g. Up at the top of a list moves the focus).
    Ignored,
}

/// What a form - or a dialog - asks of every kind of field.
pub trait FieldKind {
    /// Its border's title.
    fn title(&self) -> String;

    /// Its height, border included.
    fn height(&self) -> u16 {
        3
    }

    /// The width it would like, border included.
    fn width(&self) -> usize {
        40
    }

    /// Whether the form can't be done while it's like this.
    fn missing(&self) -> bool {
        false
    }

    /// Draws it in `area`, inside `block`.
    fn draw(&mut self, frame: &mut Frame, area: Rect, block: Block, focused: bool);

    /// As a form opens with the focus on it.
    fn opened(&mut self) -> Result<()> {
        Ok(())
    }

    /// A key while it has the focus.
    fn key(&mut self, key: KeyEvent) -> Result<EventResponse>;

    /// A mouse event on it, drawn at `area`: a click focuses it, and does
    /// what Space does - in a text field, puts the cursor where it lands.
    fn mouse(&mut self, mouse: &MouseEvent, area: Rect) -> Result<EventResponse>;

    /// Draws it in `area`, in its titled border - yellow while it has the
    /// focus.
    fn show(&mut self, frame: &mut Frame, area: Rect, focused: bool) {
        let style = if focused {
            Style::new().yellow()
        } else {
            Style::new()
        };
        let block = Block::bordered().title(self.title()).border_style(style);
        self.draw(frame, area, block, focused);
    }
}

/// A checkbox field, its key type hidden.
pub trait ChecksKind: FieldKind + Any {}

impl<K: 'static> ChecksKind for ChecksField<K> {}

/// A kind of field a form's handle finds again among its answers.
pub trait FormField: FieldKind + Into<Field> {
    fn of(field: &Field) -> Option<&Self>;
}

impl FormField for TextField {
    fn of(field: &Field) -> Option<&Self> {
        match field {
            Field::Text(text) => Some(text),
            _ => None,
        }
    }
}

impl FormField for DirField {
    fn of(field: &Field) -> Option<&Self> {
        match field {
            Field::Dir(dir) => Some(dir),
            _ => None,
        }
    }
}

impl<K: 'static> FormField for ChecksField<K> {
    fn of(field: &Field) -> Option<&Self> {
        match field {
            Field::Checks(checks) => (checks.as_ref() as &dyn Any).downcast_ref(),
            _ => None,
        }
    }
}

impl From<TextField> for Field {
    fn from(text: TextField) -> Self {
        Field::Text(text)
    }
}

impl From<DirField> for Field {
    fn from(dir: DirField) -> Self {
        Field::Dir(dir)
    }
}

impl<K: 'static> From<ChecksField<K>> for Field {
    fn from(checks: ChecksField<K>) -> Self {
        Field::Checks(Box::new(checks))
    }
}

impl Field {
    pub(super) fn missing(&self) -> bool {
        self.kind().missing()
    }

    pub(super) fn height(&self) -> u16 {
        self.kind().height()
    }

    pub(super) fn draw(&mut self, frame: &mut Frame, area: Rect, focused: bool) {
        self.kind_mut().show(frame, area, focused);
    }

    /// As a form opens with the focus on it: an empty directory field, which
    /// only its browser can fill, opens that - cancelled, it stays empty.
    pub(super) fn opened(&mut self) -> Result<()> {
        self.kind_mut().opened()
    }

    pub(super) fn key(&mut self, key: KeyEvent) -> Result<EventResponse> {
        self.kind_mut().key(key)
    }

    pub(super) fn mouse(&mut self, mouse: &MouseEvent, area: Rect) -> Result<EventResponse> {
        self.kind_mut().mouse(mouse, area)
    }

    fn kind(&self) -> &dyn FieldKind {
        match self {
            Field::Text(text) => text,
            Field::Dir(dir) => dir,
            Field::Checks(checks) => checks.as_ref(),
        }
    }

    fn kind_mut(&mut self) -> &mut dyn FieldKind {
        match self {
            Field::Text(text) => text,
            Field::Dir(dir) => dir,
            Field::Checks(checks) => checks.as_mut(),
        }
    }
}
