//! A box over the rest of the screen, made of parts the caller chooses -
//! text, lines, a field, buttons - and, like JavaScript's, `alert`, `confirm`
//! and `prompt`, built from them. A dialog draws only its box; it runs above a
//! [`Page`], which draws itself beneath it.

mod alert;
mod confirm;
mod draw;
mod prompt;

use std::ops::ControlFlow;
use std::time::Duration;

use anyhow::Result;
use ratatui::Frame;
use ratatui::crossterm::event::{Event, MouseButton, MouseEventKind};
use ratatui::layout::{Position, Rect};
use ratatui::style::Style;

use crate::commands::app::components::form::FieldKind;
use crate::commands::app::components::status_bar::HotKeys;
use crate::commands::app::window::{RunnableWindow, Window};

pub use alert::alert;
pub use confirm::confirm;
pub use draw::draw;
pub use prompt::prompt;

/// A window drawn over a page: the page shows the dialog's keys in its status
/// bar.
pub trait Dialog: Window {
    fn hot_keys(&self) -> HotKeys;

    /// Shows it above `page` until it's done (see [`RunnableWindow::run`]).
    fn popup(self, page: &impl Page) -> Result<Self::Output>
    where
        Self: Sized,
    {
        Above { page, dialog: self }.run()
    }
}

/// What a dialog is shown above.
pub trait Page {
    /// Draws it with `hot_keys` in its status bar - the dialog's.
    fn draw_page(&self, frame: &mut Frame, hot_keys: &HotKeys);
}

/// A dialog, and the page beneath it.
struct Above<'a, P, D> {
    page: &'a P,
    dialog: D,
}

impl<P: Page, D: Dialog> Window for Above<'_, P, D> {
    type Output = D::Output;

    fn draw(&mut self, frame: &mut Frame) {
        self.page.draw_page(frame, &self.dialog.hot_keys());
        self.dialog.draw(frame);
    }

    fn event(&mut self, event: Event) -> Result<ControlFlow<D::Output>> {
        self.dialog.event(event)
    }

    fn interval(&self) -> Option<Duration> {
        self.dialog.interval()
    }

    fn finished(&mut self) -> Option<D::Output> {
        self.dialog.finished()
    }

    fn opened(&mut self) -> Result<()> {
        self.dialog.opened()
    }
}

/// What a dialog box shows: under its title, its parts, top to bottom. It
/// sizes itself to them, in the middle of the screen.
pub struct DialogBox<'a> {
    pub title: String,
    /// The border's and the title's.
    pub color: Style,
    pub parts: Vec<Part<'a>>,
}

pub enum Part<'a> {
    /// Text, broken between words: at the box's width - or a narrower one
    /// for `prose`, which is read rather than glanced at.
    Text {
        text: String,
        style: Style,
        prose: bool,
    },
    /// Lines, each broken like text: the last of them, if they don't all fit.
    Lines { lines: &'a [String], style: Style },
    /// An empty line.
    Gap,
    Field {
        field: &'a mut dyn FieldKind,
        focused: bool,
    },
    /// Right-aligned: `highlighted` on yellow if they have the `focus`, else
    /// on cyan - what Enter does from elsewhere.
    Buttons {
        labels: Vec<String>,
        highlighted: usize,
        focus: bool,
    },
}

impl Part<'_> {
    /// `text`, in `style`.
    pub fn text(text: impl Into<String>, style: Style) -> Self {
        Part::Text {
            text: text.into(),
            style,
            prose: false,
        }
    }
}

/// Where a box's field and buttons were drawn, for clicks.
#[derive(Default)]
pub struct Drawn {
    pub field: Option<Rect>,
    pub buttons: Vec<Rect>,
}

/// The button a left click is on, among the ones drawn at `buttons`.
fn clicked(buttons: &[Rect], event: &Event) -> Option<usize> {
    let Event::Mouse(mouse) = event else {
        return None;
    };
    if mouse.kind != MouseEventKind::Down(MouseButton::Left) {
        return None;
    }
    let at = Position::new(mouse.column, mouse.row);
    buttons.iter().position(|button| button.contains(at))
}
