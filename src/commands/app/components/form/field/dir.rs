//! A directory field: chosen in the directory browser rather than typed.

use std::path::PathBuf;

use anyhow::Result;
use ratatui::Frame;
use ratatui::crossterm::event::{KeyCode, KeyEvent, MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::Rect;
use ratatui::widgets::{Block, Paragraph};

use super::{EventResponse, FieldKind};
use crate::commands::app::components::dir_browser;
use crate::commands::app::window::RunnableWindow;
use crate::paths;

#[derive(Default)]
pub struct DirField {
    label: String,
    value: String,
    /// Directories its browser marks (path, name).
    marked: Vec<(PathBuf, String)>,
    required: bool,
}

impl DirField {
    /// An empty field, not required, marking nothing, its label to be set.
    pub fn new() -> Self {
        Self::default()
    }

    pub fn label(mut self, label: &str) -> Self {
        self.label = label.into();
        self
    }

    /// The directory it starts with.
    pub fn value(mut self, value: &str) -> Self {
        self.value = value.into();
        self
    }

    /// The form isn't done while it's empty.
    pub fn required(mut self) -> Self {
        self.required = true;
        self
    }

    /// Its browser marks the directories `marked` (path, name).
    pub fn marking(mut self, marked: Vec<(PathBuf, String)>) -> Self {
        self.marked = marked;
        self
    }

    /// The directory chosen.
    pub fn path(&self) -> &str {
        &self.value
    }

    /// Lets the user choose the directory in the directory browser, starting
    /// at the one it has - or the current directory; kept if cancelled.
    fn choose(&mut self) -> Result<EventResponse> {
        let start = if self.value.is_empty() {
            std::env::current_dir()?
        } else {
            paths::expand_tilde(&self.value)
        };
        let browser = dir_browser::choose_dir(&self.label, &start, &self.marked);
        if let Some(dir) = browser.run()? {
            self.value = paths::relative_to_home(&dir);
        }
        Ok(EventResponse::Handled)
    }
}

impl FieldKind for DirField {
    fn title(&self) -> String {
        if self.required {
            format!("{} (required; Space or click: choose)", self.label)
        } else {
            format!("{} (Space or click: choose)", self.label)
        }
    }

    fn missing(&self) -> bool {
        self.required && self.value.is_empty()
    }

    fn draw(&mut self, frame: &mut Frame, area: Rect, block: Block, _: bool) {
        frame.render_widget(Paragraph::new(self.value.as_str()).block(block), area);
    }

    /// Empty, it can only be filled in its browser: that opens.
    fn opened(&mut self) -> Result<()> {
        if self.value.is_empty() {
            self.choose()?;
        }
        Ok(())
    }

    fn key(&mut self, key: KeyEvent) -> Result<EventResponse> {
        match key.code {
            KeyCode::Char(' ') => self.choose(),
            _ => Ok(EventResponse::Ignored),
        }
    }

    fn mouse(&mut self, mouse: &MouseEvent, _area: Rect) -> Result<EventResponse> {
        if mouse.kind == MouseEventKind::Down(MouseButton::Left) {
            self.choose()
        } else {
            Ok(EventResponse::Ignored)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_leaves_other_keys_to_the_form() {
        let mut field = DirField::new().value("~");
        let mut press = |code| field.key(KeyEvent::from(code)).unwrap();
        assert!(matches!(press(KeyCode::Char('x')), EventResponse::Ignored));
        assert!(matches!(press(KeyCode::Down), EventResponse::Ignored));
    }

    #[test]
    fn empty_its_missing_if_required() {
        assert!(!DirField::new().missing());
        assert!(DirField::new().required().missing());
        assert!(!DirField::new().value("~").required().missing());
    }
}
