//! Forms: text, directory and checkbox fields, filled in on a full screen.

mod draw;
mod events;
mod field;
mod text_input;

pub use field::{Check, ChecksField, DirField, Field, FieldKind, FormField, TextField};

use std::marker::PhantomData;
use std::ops::{ControlFlow, Index};

use anyhow::Result;
use ratatui::Frame;
use ratatui::crossterm::event::Event;
use ratatui::layout::Rect;

use crate::commands::app::terminal::pressed;
use crate::commands::app::window::Window;

/// A form: fields filled in on a full screen, until Enter with every required
/// one filled in (`Some`, the answers) or Esc (`None`). Enter on an
/// incomplete form moves to the next required field that's still empty.
pub struct Form {
    title: String,
    fields: Vec<Field>,
    focus: usize,
    /// Each field's area, from the last draw.
    layout: Vec<Rect>,
}

/// A field of a form, by its kind: reads its answer from the form's
/// [`Answers`] - of that form only.
pub struct Handle<F> {
    index: usize,
    kind: PhantomData<fn() -> F>,
}

impl<F> Clone for Handle<F> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<F> Copy for Handle<F> {}

/// A submitted form's fields, read by their handles: `answers[label].value()`.
pub struct Answers(Vec<Field>);

impl<F: FormField> Index<Handle<F>> for Answers {
    type Output = F;

    fn index(&self, handle: Handle<F>) -> &F {
        F::of(&self.0[handle.index]).expect("a handle finds the kind of field it was made for")
    }
}

impl Form {
    pub fn new(title: impl Into<String>) -> Self {
        Form {
            title: title.into(),
            fields: Vec::new(),
            focus: 0,
            layout: Vec::new(),
        }
    }

    /// Adds `field`, below the others; its handle reads its answer.
    pub fn add<F: FormField>(&mut self, field: F) -> Handle<F> {
        self.fields.push(field.into());
        Handle {
            index: self.fields.len() - 1,
            kind: PhantomData,
        }
    }
}

impl Window for Form {
    type Output = Option<Answers>;

    fn draw(&mut self, frame: &mut Frame) {
        self.layout = draw::draw(frame, &self.title, &mut self.fields, self.focus);
    }

    /// The focused field's turn, as the form opens: an empty directory field
    /// starts in its browser.
    fn opened(&mut self) -> Result<()> {
        self.fields
            .get_mut(self.focus)
            .into_iter()
            .try_for_each(Field::opened)
    }

    fn event(&mut self, event: Event) -> Result<ControlFlow<Option<Answers>>> {
        if let Event::Mouse(mouse) = event {
            self.mouse(&mouse)?;
            return Ok(ControlFlow::Continue(()));
        }
        match pressed(&event) {
            Some(key) => self.key(key),
            None => Ok(ControlFlow::Continue(())),
        }
    }
}
