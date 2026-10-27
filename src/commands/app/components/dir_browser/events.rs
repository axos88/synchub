//! What the keys and the mouse do in the browser.

use std::path::PathBuf;

use ratatui::crossterm::event::{
    KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};
use ratatui::layout::Position;

use super::{Browser, Entry, dir_shown};
use crate::commands::app::gesture::Gesture;
use crate::commands::app::terminal::ctrl_c;
use crate::paths;

impl Browser<'_> {
    /// A click highlights an entry, a double click opens it, and a click on
    /// the path edits it; the wheel moves the highlight.
    pub(super) fn mouse(&mut self, mouse: &MouseEvent) {
        if let Some(target) = &mut self.target
            && mouse.kind == MouseEventKind::Down(MouseButton::Left)
        {
            target.editing = self
                .path_area
                .contains(Position::new(mouse.column, mouse.row));
        }
        match mouse.kind {
            MouseEventKind::ScrollUp => self.step(-1),
            MouseEventKind::ScrollDown => self.step(1),
            _ => {
                if let Some(gesture) = self.list.clicked(mouse) {
                    self.list.select(Some(gesture.at()));
                    self.message.clear();
                    self.retarget();
                    if let Gesture::DoubleClick(_) = gesture {
                        self.open_selected();
                    }
                }
            }
        }
    }

    /// Handles a key; `Some` when done: chosen, or cancelled (`None` inside).
    pub(super) fn key(&mut self, key: KeyEvent) -> Option<Option<PathBuf>> {
        self.message.clear();
        if key.code == KeyCode::Esc || ctrl_c(&key) {
            return Some(None);
        }
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        if ctrl && key.code == KeyCode::Char('h') {
            self.open(paths::home(), None);
            return None;
        }
        match &self.target {
            None => self.pick_key(key),
            Some(target) if target.editing => self.edit_key(key, ctrl),
            Some(_) => self.list_key(key, ctrl),
        }
    }

    /// A key while picking a directory.
    fn pick_key(&mut self, key: KeyEvent) -> Option<Option<PathBuf>> {
        match key.code {
            KeyCode::Char('q') => return Some(None),
            KeyCode::Up | KeyCode::Char('k') => self.list.select_previous(),
            KeyCode::Down | KeyCode::Char('j') => self.list.select_next(),
            KeyCode::Enter if matches!(self.selected(), Some(Entry::Here)) => {
                return Some(Some(self.dir.clone()));
            }
            KeyCode::Enter | KeyCode::Right | KeyCode::Char('l') => self.open_selected(),
            KeyCode::Left | KeyCode::Backspace | KeyCode::Char('h' | '-') => self.up(),
            KeyCode::Char('~') => self.open(paths::home(), None),
            KeyCode::Char('.') => {
                self.hidden = !self.hidden;
                let keep = match self.selected() {
                    Some(Entry::Dir(dir)) => Some(dir.clone()),
                    _ => None,
                };
                self.load(keep.as_deref());
            }
            KeyCode::Char(' ' | 's' | 'c') => return Some(Some(self.highlighted_dir())),
            _ => {}
        }
        None
    }

    /// A key on the list while choosing a target - no letter keys: typing
    /// goes to the path.
    fn list_key(&mut self, key: KeyEvent, ctrl: bool) -> Option<Option<PathBuf>> {
        match key.code {
            KeyCode::Up if self.list.selected().unwrap_or(0) == 0 => {
                self.target.as_mut()?.editing = true
            }
            KeyCode::Up => self.step(-1),
            KeyCode::Down => self.step(1),
            KeyCode::Enter => return self.enter(),
            KeyCode::Right => self.open_selected(),
            KeyCode::Left => self.up(),
            KeyCode::Char(_) | KeyCode::Backspace if !ctrl => {
                let dir = dir_shown(&self.dir);
                let target = self.target.as_mut()?;
                if key.code != KeyCode::Backspace {
                    // In place of the highlighted entry: a name in the directory shown.
                    target.path = dir;
                }
                target.editing = true;
                return self.edit_key(key, ctrl);
            }
            _ => {}
        }
        None
    }

    /// A key while the path is being edited.
    fn edit_key(&mut self, key: KeyEvent, ctrl: bool) -> Option<Option<PathBuf>> {
        let target = self.target.as_mut()?;
        if !matches!(key.code, KeyCode::Tab | KeyCode::BackTab) {
            target.stop_completing();
        }
        match key.code {
            KeyCode::Enter => return self.enter(),
            KeyCode::Down => target.editing = false,
            KeyCode::Tab => target.complete(true),
            KeyCode::BackTab => target.complete(false),
            KeyCode::Char('w') if ctrl => target.delete_last_part(),
            KeyCode::Char('u') if ctrl => target.path.clear(),
            KeyCode::Char(c) if !ctrl => target.path.push(c),
            KeyCode::Backspace => {
                target.path.pop();
            }
            _ => {}
        }
        self.follow();
        None
    }
}
