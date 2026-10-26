//! The folder tree: this device's folders with the devices they're shared
//! with, then the hub's shares this device doesn't have.

use ratatui::Frame;
use ratatui::crossterm::event::{KeyCode, KeyEvent, MouseEvent, MouseEventKind};
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style, Stylize};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, List, ListItem};

use super::clickable_list::ClickableList;
use crate::commands::app::gesture::Gesture;
use crate::commands::app::state::{DeviceProgress, FolderState, State};
use crate::device_id::DeviceId;
use crate::folder_id::FolderId;

/// The tree: its rows come from the state, its selection is its own -
/// none until the first state it's given, its first row then.
#[derive(Default)]
pub struct Tree {
    selected: Option<Key>,
    list: ClickableList,
}

impl Tree {
    pub fn selected(&self) -> Option<&Key> {
        self.selected.as_ref()
    }

    /// The folder selected or under the selection: it shows the devices it
    /// could be shared with.
    pub fn expanded(&self) -> Option<&FolderId> {
        self.selected.as_ref().and_then(Key::folder)
    }

    /// Up and Down (k, j) move the selection; Enter activates the row.
    pub fn key(&mut self, state: &State, key: &KeyEvent) -> TreeResponse {
        match key.code {
            KeyCode::Up | KeyCode::Char('k') => self.step(state, -1),
            KeyCode::Down | KeyCode::Char('j') => self.step(state, 1),
            KeyCode::Enter => return self.activated(),
            _ => return TreeResponse::Ignored,
        }
        TreeResponse::Handled
    }

    /// The wheel moves the selection, a click selects a row - a double click
    /// activates it.
    pub fn mouse(&mut self, state: &State, mouse: &MouseEvent) -> TreeResponse {
        match mouse.kind {
            MouseEventKind::ScrollUp => self.step(state, -1),
            MouseEventKind::ScrollDown => self.step(state, 1),
            _ => return self.clicked(state, mouse).unwrap_or(TreeResponse::Ignored),
        }
        TreeResponse::Handled
    }

    /// Follows a reloaded `state`, keeping the selection where it can.
    pub fn reloaded(&mut self, state: &State) {
        let shown: Vec<Key> = self
            .rows(state)
            .iter()
            .filter_map(|row| row.key(state))
            .collect();
        // A hub share that just got here is a folder of this device now.
        let arrived = match &self.selected {
            Some(Key::Elsewhere(id)) => Some(Key::Folder(id.clone())),
            _ => None,
        };
        self.selected = self
            .selected
            .take()
            .into_iter()
            .chain(arrived)
            .find(|key| shown.contains(key))
            .or_else(|| shown.first().cloned());
    }

    pub fn draw(&self, frame: &mut Frame, area: Rect, state: &State) {
        let rows = self.rows(state);
        self.list.select(self.index(state, &rows));
        let expanded = self.expanded();
        let hub_id = state.hub_id();
        // The devices' names line up, " (hub)" included.
        let width = rows
            .iter()
            .filter_map(|row| row.device(state))
            .map(|id| state.name(id).chars().count() + if Some(id) == hub_id { 6 } else { 0 })
            .max()
            .unwrap_or(10);
        let items: Vec<ListItem> = rows
            .iter()
            .map(|row| ListItem::new(line(state, row, expanded, width)))
            .collect();
        let list = List::new(items).highlight_style(Style::new().add_modifier(Modifier::REVERSED));
        self.list
            .draw(frame, area, Block::bordered().title(" Folders "), list);
    }

    fn clicked(&mut self, state: &State, mouse: &MouseEvent) -> Option<TreeResponse> {
        let gesture = self.list.clicked(mouse)?;
        self.selected = Some(self.rows(state).get(gesture.at())?.key(state)?);
        Some(match gesture {
            Gesture::Click(_) => TreeResponse::Handled,
            Gesture::DoubleClick(_) => self.activated(),
        })
    }

    fn activated(&self) -> TreeResponse {
        self.selected
            .clone()
            .map_or(TreeResponse::Ignored, TreeResponse::Activated)
    }

    fn rows(&self, state: &State) -> Vec<Row> {
        rows(state, self.expanded())
    }

    /// The selection's row among `rows`.
    fn index(&self, state: &State, rows: &[Row]) -> Option<usize> {
        let selected = self.selected.as_ref()?;
        rows.iter()
            .position(|row| row.key(state).as_ref() == Some(selected))
    }

    /// Moves the selection `by` rows, skipping headings.
    fn step(&mut self, state: &State, by: isize) {
        let rows = self.rows(state);
        let from = self.index(state, &rows).map_or(0, |i| i as isize);
        let next = (1..)
            .map(|n| from + by * n)
            .take_while(|i| (0..rows.len() as isize).contains(i))
            .find_map(|i| rows[i as usize].key(state));
        if next.is_some() {
            self.selected = next;
        }
    }
}

/// What the tree made of a key or the mouse.
pub enum TreeResponse {
    /// The selection moved, say.
    Handled,
    /// Not the tree's: the page's.
    Ignored,
    /// Enter, or a double click: the row's main action, for the page to do.
    Activated(Key),
}

/// What's selected - by what it is, not by row, as the rows change when a
/// folder expands.
#[derive(Clone, PartialEq, Debug)]
pub enum Key {
    Folder(FolderId),
    /// A device under a folder - shared with it, or one it could be shared with.
    Device(FolderId, DeviceId),
    /// A hub share this device doesn't have.
    Elsewhere(FolderId),
}

impl Key {
    /// The folder of this device it's on or under.
    pub fn folder(&self) -> Option<&FolderId> {
        match self {
            Key::Folder(id) | Key::Device(id, _) => Some(id),
            Key::Elsewhere(_) => None,
        }
    }
}

/// A line of the tree; the numbers index the state's lists.
enum Row {
    Folder(usize),
    Device {
        folder: usize,
        device: usize,
    },
    /// A device the folder could be shared with; shown while the folder is expanded.
    Candidate {
        folder: usize,
        id: DeviceId,
    },
    Heading(&'static str),
    Elsewhere(usize),
}

impl Row {
    /// What selecting it selects; a heading can't be.
    fn key(&self, state: &State) -> Option<Key> {
        match self {
            Row::Folder(f) => Some(Key::Folder(state.folders[*f].folder.id.clone())),
            Row::Device { folder, device } => {
                let view = &state.folders[*folder];
                Some(Key::Device(
                    view.folder.id.clone(),
                    view.devices[*device].id.clone(),
                ))
            }
            Row::Candidate { folder, id } => Some(Key::Device(
                state.folders[*folder].folder.id.clone(),
                id.clone(),
            )),
            Row::Heading(_) => None,
            Row::Elsewhere(e) => Some(Key::Elsewhere(state.elsewhere[*e].id.clone())),
        }
    }

    /// The device it's about, if any.
    fn device<'s>(&'s self, state: &'s State) -> Option<&'s DeviceId> {
        match self {
            Row::Device { folder, device } => Some(&state.folders[*folder].devices[*device].id),
            Row::Candidate { id, .. } => Some(id),
            _ => None,
        }
    }
}

/// The tree's rows, with the folder `expanded` showing the devices it could be shared with.
fn rows(state: &State, expanded: Option<&FolderId>) -> Vec<Row> {
    let mut rows = Vec::new();
    for (f, view) in state.folders.iter().enumerate() {
        rows.push(Row::Folder(f));
        rows.extend((0..view.devices.len()).map(|device| Row::Device { folder: f, device }));
        if expanded == Some(&view.folder.id) {
            rows.extend(view.candidates.iter().map(|id| Row::Candidate {
                folder: f,
                id: id.clone(),
            }));
        }
    }
    if !state.elsewhere.is_empty() {
        rows.push(Row::Heading("On the hub, not on this device"));
        rows.extend((0..state.elsewhere.len()).map(Row::Elsewhere));
    }
    rows
}

/// A row's line, the device names `width` wide.
fn line<'a>(state: &'a State, row: &Row, expanded: Option<&FolderId>, width: usize) -> Line<'a> {
    let hub_id = state.hub_id();
    match row {
        Row::Folder(f) => {
            let view = &state.folders[*f];
            let open = expanded == Some(&view.folder.id);
            Line::from(vec![
                Span::raw(if open { "▾ " } else { "▸ " }).blue(),
                Span::raw(view.folder.label_or_id()).bold(),
                Span::raw(format!("  {}", view.folder.id)).dim(),
                Span::raw(format!("  {}", view.folder.path)).blue(),
                Span::raw("  "),
                folder_state(&view.state),
            ])
        }
        Row::Device { folder, device } => {
            let device = &state.folders[*folder].devices[*device];
            let is_hub = Some(&device.id) == hub_id;
            let name = if is_hub {
                format!("{} (hub)", state.name(&device.id))
            } else {
                state.name(&device.id)
            };
            let name = Span::raw(format!("{name:<width$}"));
            let link = if device.connected {
                Span::raw(format!("{:<15}", "● connected")).green()
            } else {
                Span::raw(format!("{:<15}", "○ not connected")).yellow()
            };
            Line::from(vec![
                Span::raw("    [x] "),
                if is_hub { name.cyan() } else { name },
                Span::raw("  "),
                link,
                Span::raw("  "),
                progress(&device.progress),
            ])
        }
        Row::Candidate { id, .. } => Line::from(vec![
            Span::raw("    [ ] ").dim(),
            Span::raw(format!("{:<width$}", state.name(id))).dim(),
            Span::raw("  not shared").dim(),
        ]),
        Row::Heading(text) => Line::from(Span::raw(*text).bold().dim()),
        Row::Elsewhere(e) => {
            let folder = &state.elsewhere[*e];
            let others: Vec<String> = folder
                .shared_with_devices()
                .filter(|id| Some(*id) != hub_id)
                .map(|id| state.name(id))
                .collect();
            let on = if others.is_empty() {
                "the hub only".to_string()
            } else {
                others.join(", ")
            };
            Line::from(vec![
                Span::raw("+ ").green(),
                Span::raw(folder.label_or_id()).bold(),
                Span::raw(format!("  {}", folder.id)).dim(),
                Span::raw(format!("  on: {on}")).dim(),
            ])
        }
    }
}

fn folder_state(state: &FolderState) -> Span<'static> {
    match state {
        FolderState::Paused => Span::raw("paused"),
        FolderState::LeftBehind => {
            Span::raw("the hub no longer shares it with this device - a leftover copy").yellow()
        }
        FolderState::InSync => Span::raw("✔ in sync").green(),
        FolderState::Behind {
            state,
            files,
            bytes,
        } => Span::raw(format!(
            "{state}, {files} file(s), {} to sync",
            size(*bytes)
        ))
        .yellow(),
        FolderState::Busy(state) => Span::raw(state.clone()).yellow(),
        FolderState::Error(err) => Span::raw(format!("error: {err}")).red(),
        FolderState::Unknown(err) => Span::raw(format!("unknown: {err}")).red(),
    }
}

/// How far a device has synced the folder.
fn progress(progress: &DeviceProgress) -> Span<'static> {
    match progress {
        DeviceProgress::InSync => Span::raw("✔ in sync").green(),
        DeviceProgress::Syncing {
            percent,
            items,
            bytes,
        } => Span::raw(format!(
            "{percent:.0}% · {items} item(s), {} to go",
            size(*bytes)
        ))
        .yellow(),
        DeviceProgress::NotIntroduced => Span::raw("not introduced yet").yellow(),
        DeviceProgress::Unknown(err) => Span::raw(format!("unknown: {err}")).red(),
    }
}

/// Bytes, in the binary units Syncthing's web UI uses.
fn size(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KiB", "MiB", "GiB", "TiB"];
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit + 1 < UNITS.len() {
        value /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{bytes} B")
    } else {
        format!("{value:.1} {}", UNITS[unit])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sizes_use_binary_units() {
        assert_eq!(size(512), "512 B");
        assert_eq!(size(1536), "1.5 KiB");
        assert_eq!(size(3 * 1024 * 1024 * 1024), "3.0 GiB");
    }
}
