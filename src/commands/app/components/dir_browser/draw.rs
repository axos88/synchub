//! Drawing the browser: the path, the directory's entries, and what the keys do.

use std::path::PathBuf;

use ratatui::Frame;
use ratatui::layout::{Constraint, Layout};
use ratatui::style::{Modifier, Style, Stylize};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, List, ListItem, Paragraph};

use super::target::Target;
use super::{Browser, Entry, base_name, dir_shown};
use crate::commands::app::components::status_bar::{self, HotKey, HotKeys};
use crate::paths;

impl Browser<'_> {
    pub(super) fn draw_browser(&mut self, frame: &mut Frame) {
        let [title_area, dir_area, list_area, status_area, help_area] = Layout::vertical([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Min(3),
            Constraint::Length(1),
            Constraint::Length(status_bar::HEIGHT),
        ])
        .areas(frame.area());
        frame.render_widget(Paragraph::new(self.title.bold()), title_area);
        match &self.target {
            None => frame.render_widget(Paragraph::new(dir_shown(&self.dir)), dir_area),
            Some(target) => target.draw(frame, dir_area),
        }
        let items: Vec<ListItem> = self
            .entries
            .iter()
            .map(|entry| entry_item(entry, self.marked))
            .collect();
        let list = List::new(items).highlight_style(Style::new().add_modifier(Modifier::REVERSED));
        self.list.draw(frame, list_area, Block::bordered(), list);
        frame.render_widget(Paragraph::new(self.status()), status_area);
        status_bar::draw(frame, help_area, &self.hot_keys());
        self.path_area = dir_area;
    }

    /// Why Enter didn't work, where it moves the thing to, or which
    /// Syncthing folder this directory is in.
    fn status(&self) -> Span<'static> {
        if !self.message.is_empty() {
            return self.message.clone().yellow();
        }
        match self
            .target
            .as_ref()
            .filter(|_| self.moves())
            .map(Target::destination)
        {
            Some(Ok(dest)) => {
                return format!("Moves it to {}", paths::relative_to_home(&dest)).green();
            }
            Some(Err(problem)) => return problem.yellow(),
            None => {}
        }
        match self
            .marked
            .iter()
            .find(|(path, _)| paths::inside(&self.dir, path))
        {
            Some((_, folder)) => {
                format!("This directory is inside the Syncthing folder {folder}.").yellow()
            }
            None => "".into(),
        }
    }

    /// What the keys do now.
    fn hot_keys(&self) -> HotKeys {
        HotKeys(match &self.target {
            None => vec![
                HotKey::new(&["j", "k", "click"], "move"),
                HotKey::new(&["Enter", "double-click"], "open"),
                HotKey::new(&["-"], "up"),
                HotKey::new(&["Space"], "choose highlighted"),
                HotKey::new(&["c"], "choose this one"),
                HotKey::new(&["."], "hidden"),
                HotKey::new(&["~", "Ctrl-H"], "home"),
                HotKey::new(&["Esc"], "cancel"),
            ],
            Some(target) if target.editing => vec![
                HotKey::new(&["Enter"], self.enter_does()),
                HotKey::new(&["Tab", "Shift-Tab"], "complete the directory name"),
                HotKey::new(&["Ctrl-W"], "delete the last part"),
                HotKey::new(&["Ctrl-U"], "clear"),
                HotKey::new(&["Ctrl-H"], "home"),
                HotKey::new(&["↓"], "to the list"),
                HotKey::new(&["Esc"], "cancel"),
            ],
            Some(_) => vec![
                HotKey::new(&["Enter"], self.enter_does()),
                HotKey::new(&["↑", "↓", "click"], "highlight"),
                HotKey::new(&["→", "double-click"], "open"),
                HotKey::new(&["←"], "up"),
                HotKey::new(&["letters"], "type a name"),
                HotKey::new(&["↑ at the top", "click"], "edit the path"),
                HotKey::new(&["Ctrl-H"], "home"),
                HotKey::new(&["Esc"], "cancel"),
            ],
        })
    }

    /// What Enter does now, for the help line.
    fn enter_does(&self) -> String {
        match self.selected() {
            Some(Entry::Dir(dir)) => format!("open {}/", base_name(dir)),
            Some(Entry::Up) => "go up".into(),
            Some(Entry::Here) => "move it here".into(),
            None => "move it there".into(),
        }
    }
}

/// An entry's line: a directory says whether it is, or holds, a Syncthing folder.
fn entry_item(entry: &Entry, marked: &[(PathBuf, String)]) -> ListItem<'static> {
    let dir = match entry {
        Entry::Here => return ListItem::new("./"),
        Entry::Up => return ListItem::new("../"),
        Entry::Dir(dir) => dir,
    };
    let note = marked
        .iter()
        .find_map(|(path, folder)| {
            if path == dir {
                Some(format!("  Syncthing folder {folder}"))
            } else if paths::inside(path, dir) {
                Some("  contains a Syncthing folder".to_string())
            } else {
                None
            }
        })
        .unwrap_or_default();
    ListItem::new(Line::from(vec![
        Span::raw(format!("{}/", base_name(dir))),
        Span::raw(note).dim(),
    ]))
}
