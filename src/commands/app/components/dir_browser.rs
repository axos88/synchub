//! A directory browser, after vim's netrw: for picking a directory, or a
//! new path to move something to.

mod draw;
mod events;
mod target;

use std::fs;
use std::path::{Path, PathBuf};

use std::ops::ControlFlow;

use anyhow::Result;
use itertools::Itertools;
use ratatui::Frame;
use ratatui::crossterm::event::Event;
use ratatui::layout::Rect;

use super::clickable_list::ClickableList;
use crate::commands::app::terminal::pressed;
use crate::commands::app::window::Window;
use crate::paths;
use target::Target;

/// A browser for picking a directory, starting at `start`; `None` if
/// cancelled. It marks the directories `marked` (path, name): the Syncthing
/// folders.
pub fn choose_dir<'a>(
    title: &'a str,
    start: &Path,
    marked: &'a [(PathBuf, String)],
) -> impl Window<Output = Option<PathBuf>> + 'a {
    Browser::new(title, marked, paths::canon(start), None, None)
}

/// A browser for picking a new path for something: one that doesn't exist
/// yet, or an empty directory. The path shows the highlighted directory;
/// typing adds to it. Opens in the directory `current` is in, highlighting
/// it; `None` if cancelled.
pub fn choose_target<'a>(
    title: &'a str,
    current: &Path,
    marked: &'a [(PathBuf, String)],
) -> impl Window<Output = Option<PathBuf>> + 'a {
    let current = paths::canon(current);
    let start = current.parent().unwrap_or(&current).to_path_buf();
    Browser::new(
        title,
        marked,
        start,
        Some(Target::default()),
        Some(&current),
    )
}

enum Entry {
    /// The directory shown, when choosing a target in it is an option.
    Here,
    Up,
    Dir(PathBuf),
}

struct Browser<'a> {
    title: &'a str,
    /// The directories it marks (path, name).
    marked: &'a [(PathBuf, String)],
    dir: PathBuf,
    entries: Vec<Entry>,
    list: ClickableList,
    hidden: bool,
    message: String,
    /// The path being chosen, when choosing a target.
    target: Option<Target>,
    /// Where the path was drawn, for clicks.
    path_area: Rect,
}

impl<'a> Browser<'a> {
    /// Opens `dir`, highlighting `select` if it's there.
    fn new(
        title: &'a str,
        marked: &'a [(PathBuf, String)],
        dir: PathBuf,
        target: Option<Target>,
        select: Option<&Path>,
    ) -> Self {
        let mut browser = Browser {
            title,
            marked,
            dir,
            entries: Vec::new(),
            list: ClickableList::default(),
            hidden: false,
            message: String::new(),
            target,
            path_area: Rect::default(),
        };
        browser.load(select);
        browser.retarget();
        browser
    }

    /// Lists the current directory, selecting `select` if it's there.
    fn load(&mut self, select: Option<&Path>) {
        let dirs: Vec<PathBuf> = match fs::read_dir(&self.dir) {
            Ok(entries) => entries
                .filter_map(|e| e.ok())
                .map(|e| e.path())
                .filter(|p| p.is_dir())
                .filter(|p| self.hidden || !base_name(p).starts_with('.'))
                .sorted_by_key(|p| base_name(p).to_lowercase())
                .collect(),
            Err(err) => {
                self.message = format!("can't read {}: {err}", self.dir.display());
                Vec::new()
            }
        };
        let here = self.target.as_ref().map(|_| Entry::Here);
        let up = self.dir.parent().map(|_| Entry::Up);
        self.entries = here
            .into_iter()
            .chain(up)
            .chain(dirs.into_iter().map(Entry::Dir))
            .collect();
        let index = select
            .and_then(|s| {
                self.entries
                    .iter()
                    .position(|e| matches!(e, Entry::Dir(p) if p == s))
            })
            .unwrap_or(0);
        self.list
            .select((!self.entries.is_empty()).then_some(index));
    }

    fn open(&mut self, dir: PathBuf, select: Option<PathBuf>) {
        self.dir = paths::canon(&dir);
        self.load(select.as_deref());
        self.retarget();
    }

    fn up(&mut self) {
        if let Some(parent) = self.dir.parent().map(Path::to_path_buf) {
            let from = self.dir.clone();
            self.open(parent, Some(from));
        }
    }

    fn selected(&self) -> Option<&Entry> {
        self.list.selected().and_then(|i| self.entries.get(i))
    }

    fn open_selected(&mut self) {
        match self.selected() {
            Some(Entry::Up) => self.up(),
            Some(Entry::Here) | None => {}
            Some(Entry::Dir(dir)) => {
                let dir = dir.clone();
                self.open(dir, None);
            }
        }
    }

    /// Highlights the entry `by` places away, putting its path into the path.
    fn step(&mut self, by: isize) {
        if by < 0 {
            self.list.select_previous();
        } else {
            self.list.select_next();
        }
        self.retarget();
    }

    /// Puts the highlighted directory's path into the path.
    fn retarget(&mut self) {
        let dir = match self.selected() {
            Some(Entry::Dir(dir)) => dir.clone(),
            Some(Entry::Up) => self.dir.parent().unwrap_or(&self.dir).to_path_buf(),
            Some(Entry::Here) | None => self.dir.clone(),
        };
        if let Some(target) = &mut self.target {
            target.set(dir_shown(&dir));
        }
    }

    /// Shows the directory of the path being typed, as far as it exists,
    /// highlighting the entry it names, if any: `./` for the directory itself.
    fn follow(&mut self) {
        let Some(dir) = self.target.as_ref().map(Target::dir) else {
            return;
        };
        if let Some(dir) = &dir
            && *dir != self.dir
        {
            self.dir = dir.clone();
            self.load(None);
        }
        let Some(target) = &self.target else {
            return;
        };
        let typed = target.name();
        let index = self.entries.iter().position(|e| match e {
            Entry::Here => typed.is_empty() && dir.as_ref() == Some(&self.dir),
            Entry::Dir(dir) => base_name(dir) == typed,
            Entry::Up => false,
        });
        self.list.select(index);
    }

    /// Enter, while choosing a target: opens the highlighted directory, or
    /// with `./` or nothing highlighted, chooses the path. `Some` when chosen.
    fn enter(&mut self) -> Option<Option<PathBuf>> {
        if !self.moves() {
            self.open_selected();
            return None;
        }
        if matches!(self.selected(), Some(Entry::Here)) {
            self.retarget();
        }
        match self.target.as_ref()?.destination() {
            Ok(dest) => Some(Some(dest)),
            Err(problem) => {
                self.message = problem;
                None
            }
        }
    }

    /// Whether Enter chooses the path now, rather than opening a directory.
    fn moves(&self) -> bool {
        matches!(self.selected(), Some(Entry::Here) | None)
    }
}

/// A directory's path as it would be typed, ending in `/` for more to be typed after it.
fn dir_shown(dir: &Path) -> String {
    format!("{}/", paths::relative_to_home(dir).trim_end_matches('/'))
}

fn base_name(path: &Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.display().to_string())
}

/// The browser, until a directory is chosen (`Some`), or it's cancelled.
impl Window for Browser<'_> {
    type Output = Option<PathBuf>;

    fn draw(&mut self, frame: &mut Frame) {
        self.draw_browser(frame);
    }

    fn event(&mut self, event: Event) -> Result<ControlFlow<Option<PathBuf>>> {
        if let Event::Mouse(mouse) = event {
            self.mouse(&mouse);
        } else if let Some(key) = pressed(&event)
            && let Some(done) = self.key(key)
        {
            return Ok(ControlFlow::Break(done));
        }
        Ok(ControlFlow::Continue(()))
    }
}
