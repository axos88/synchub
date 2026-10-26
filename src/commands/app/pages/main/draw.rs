//! Drawing the main page: this device and the hub, the folder tree, the log and the status
//! bar.

use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};

use crate::commands::app::App;
use crate::commands::app::components::dialog::Page;
use crate::commands::app::components::status_bar::{self, HotKeys};
use crate::commands::app::components::{hub_info, identicon_panel, log, syncthing_info};
use crate::commands::app::state::Log;

impl Page for App {
    /// Draws the page, with `hot_keys` in the status bar - its own, or a
    /// dialog's over it.
    fn draw_page(&self, frame: &mut Frame, hot_keys: &HotKeys) {
        // Until the state is loaded, there's nothing to show.
        let Some(state) = &self.state else {
            return;
        };
        let [header, tree_area, log_area, status_area] = layout(frame, &self.log);
        let [me_area, hub_area] = Layout::horizontal([Constraint::Percentage(50); 2]).areas(header);
        syncthing_info::draw(frame, me_area, &state.me);
        hub_info::draw(frame, hub_area, state.hub.as_ref());
        self.tree.draw(frame, tree_area, state);
        log::draw(frame, log_area, &self.log);
        status_bar::draw(frame, status_area, hot_keys);
    }
}

/// The header, the tree, the log and the status bar, top to bottom.
fn layout(frame: &Frame, log: &Log) -> [Rect; 4] {
    Layout::vertical([
        Constraint::Length(identicon_panel::HEIGHT),
        Constraint::Min(5),
        Constraint::Length(log::height(log)),
        Constraint::Length(status_bar::HEIGHT),
    ])
    .areas(frame.area())
}
