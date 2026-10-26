//! What the keys and the mouse do on the main page, and the status bar
//! saying so.

use std::ops::ControlFlow;

use anyhow::Result;
use ratatui::crossterm::event::{KeyCode, KeyEvent, MouseEvent};

use super::loaded;
use crate::commands::app::App;
use crate::commands::app::components::status_bar::{HotKey, HotKeys};
use crate::commands::app::components::tree::{Key, TreeResponse};
use crate::commands::app::state::State;
use crate::commands::app::terminal;

/// Whether `key` quits the app: q, Esc or Ctrl-C.
pub fn quits(key: &KeyEvent) -> bool {
    matches!(key.code, KeyCode::Char('q') | KeyCode::Esc) || terminal::ctrl_c(key)
}

impl App {
    /// The mouse is the tree's; a row it activates is the page's to act on.
    pub(super) fn mouse(&mut self, mouse: MouseEvent) -> Result<()> {
        if let TreeResponse::Activated(row) = self.tree.mouse(loaded(&self.state)?, &mouse) {
            self.activate(row)?;
        }
        Ok(())
    }

    /// Offers `key` to the tree, then does what it does for the selection;
    /// `Break` when it quits the app.
    pub(super) fn key(&mut self, key: KeyEvent) -> Result<ControlFlow<()>> {
        if quits(&key) {
            return Ok(ControlFlow::Break(()));
        }
        match self.tree.key(loaded(&self.state)?, &key) {
            TreeResponse::Activated(row) => self.activate(row)?,
            TreeResponse::Handled => {}
            TreeResponse::Ignored => self.action(key)?,
        }
        Ok(ControlFlow::Continue(()))
    }

    /// What a key the tree doesn't take does for the selection.
    fn action(&mut self, key: KeyEvent) -> Result<()> {
        let selected = self.tree.selected().cloned();
        let expanded = self.tree.expanded().cloned();
        match (key.code, selected, expanded) {
            (KeyCode::Char(' '), Some(Key::Device(folder, device)), _) => {
                self.toggle(&folder, &device)?
            }
            (KeyCode::Char('s'), Some(Key::Device(folder, device)), _) => {
                self.share(&folder, &device)?
            }
            (KeyCode::Char('u'), Some(Key::Device(folder, device)), _) => {
                self.unshare(&folder, &device)?
            }
            (KeyCode::Char('n'), ..) => self.new_share()?,
            (KeyCode::Char('m'), _, Some(folder)) => self.move_folder(&folder)?,
            (KeyCode::Char('d'), Some(Key::Folder(folder)), _) => self.remove(folder)?,
            (KeyCode::Char('g'), _, Some(folder)) => self.gitignore(&folder)?,
            (KeyCode::Char('a'), Some(Key::Elsewhere(folder)), _) => self.fetch_folder(&folder)?,
            (KeyCode::Char('R'), ..) => self.refresh()?,
            (KeyCode::Char('r'), Some(Key::Device(_, device)), _) => self.rename_device(&device)?,
            (KeyCode::Char('r'), Some(Key::Folder(folder) | Key::Elsewhere(folder)), _) => {
                self.rename_folder(&folder)?
            }
            _ => {}
        }
        Ok(())
    }

    /// What an activated row is for - moving a folder, sharing it with a
    /// device or not, getting a hub share here.
    fn activate(&mut self, row: Key) -> Result<()> {
        match row {
            Key::Folder(folder) => self.move_folder(&folder),
            Key::Device(folder, device) => self.toggle(&folder, &device),
            Key::Elsewhere(folder) => self.fetch_folder(&folder),
        }
    }

    /// The keys that do something for the selection, and what - for the
    /// status bar. Keys doing the same share an entry.
    pub fn hot_keys(&self, state: &State) -> HotKeys {
        let hub = state.hub_ready();
        let mut hot_keys = Vec::new();
        match self.tree.selected() {
            Some(Key::Folder(id)) => {
                let label = state.label(id);
                hot_keys.push(HotKey::new(
                    &["Enter", "m"],
                    format!("move {label} to another directory"),
                ));
                if hub {
                    hot_keys.push(HotKey::new(&["r"], format!("rename {label}")));
                    hot_keys.push(HotKey::new(&["d"], format!("remove {label}")));
                }
            }
            Some(Key::Device(folder, device)) => {
                let name = state.name(device);
                if hub && Some(device) != state.hub_id() {
                    hot_keys.push(if state.shared(folder, device) {
                        HotKey::new(&["Enter", "Space", "u"], format!("unshare from {name}"))
                    } else {
                        HotKey::new(&["Enter", "Space", "s"], format!("share with {name}"))
                    });
                }
                if hub {
                    hot_keys.push(HotKey::new(&["r"], format!("rename {name}")));
                }
                hot_keys.push(HotKey::new(&["m"], format!("move {}", state.label(folder))));
            }
            Some(Key::Elsewhere(id)) => {
                let name = state.elsewhere(id).map(|f| f.name()).unwrap_or_default();
                hot_keys.push(HotKey::new(
                    &["Enter", "a"],
                    format!("get {name} on this device"),
                ));
                if hub {
                    hot_keys.push(HotKey::new(&["r"], format!("rename {}", state.label(id))));
                }
            }
            None => {}
        }
        if let Some(folder) = self.tree.expanded() {
            let label = state.label(folder);
            hot_keys.push(HotKey::new(&["g"], format!("gitignore {label}")));
        }
        if hub {
            hot_keys.push(HotKey::new(&["n"], "new share"));
        }
        hot_keys.push(HotKey::new(&["R"], "refresh"));
        hot_keys.push(HotKey::new(&["q"], "quit"));

        HotKeys(hot_keys)
    }
}
