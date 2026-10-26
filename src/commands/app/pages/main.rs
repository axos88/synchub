//! The main page: this device and the hub, the folder tree, the log and the status bar.

mod actions;
mod draw;
mod events;

pub use events::quits;

use std::ops::ControlFlow;

use anyhow::{Context, Result};
use ratatui::Frame;
use ratatui::crossterm::event::Event;

use crate::commands::app::App;
use crate::commands::app::components::dialog::Page;
use crate::commands::app::components::tree::Tree;
use crate::commands::app::state::State;
use crate::commands::app::terminal;
use crate::commands::app::window::Window;

impl App {
    /// The app, its state still to be loaded.
    pub fn new() -> Self {
        App {
            state: None,
            tree: Tree::default(),
            log: Default::default(),
        }
    }

    /// The state; an error before it's loaded.
    pub fn state(&self) -> Result<&State> {
        loaded(&self.state)
    }

    /// Takes the reloaded state; the tree keeps its selection where it can.
    pub fn loaded(&mut self, state: State) {
        let state = self.state.insert(state);
        self.tree.reloaded(state);
    }
}

/// The `state`; an error before it's loaded. Takes the field rather than the
/// app, to borrow it beside the tree.
pub(super) fn loaded(state: &Option<State>) -> Result<&State> {
    state.as_ref().context("the app's state isn't loaded yet")
}

/// The main page, until the app is quit.
impl Window for App {
    type Output = ();

    fn draw(&mut self, frame: &mut Frame) {
        if let Some(state) = &self.state {
            self.draw_page(frame, &self.hot_keys(state));
        }
    }

    fn event(&mut self, event: Event) -> Result<ControlFlow<()>> {
        if self.state.is_none() {
            return Ok(ControlFlow::Continue(()));
        }
        match (terminal::pressed(&event), event) {
            (_, Event::Mouse(mouse)) => self.mouse(mouse).map(ControlFlow::Continue),
            (Some(key), _) => self.key(key),
            _ => Ok(ControlFlow::Continue(())),
        }
    }
}
