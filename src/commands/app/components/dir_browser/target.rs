//! The path typed while choosing where to move something, with Tab
//! completing its directory names.

use std::fs;
use std::path::PathBuf;

use itertools::Itertools;
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Style, Stylize};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use super::base_name;
use crate::paths;

#[derive(Default)]
pub(super) struct Target {
    pub(super) path: String,
    /// Whether the keys go to the path rather than the list.
    pub(super) editing: bool,
    /// While Tab cycles through the directories the typed name begins.
    completion: Option<Completion>,
}

struct Completion {
    /// What was typed of the name before Tab.
    typed: String,
    names: Vec<String>,
    index: usize,
}

impl Target {
    /// Replaces the path.
    pub(super) fn set(&mut self, path: String) {
        self.path = path;
        self.completion = None;
    }

    /// Ends Tab's cycling: what it filled in stays.
    pub(super) fn stop_completing(&mut self) {
        self.completion = None;
    }

    /// The name being typed: what's after the last `/`.
    pub(super) fn name(&self) -> &str {
        self.path.rsplit('/').next().unwrap_or_default()
    }

    /// What's up to the last `/`, the `/` included.
    fn stem(&self) -> &str {
        &self.path[..self.path.len() - self.name().len()]
    }

    /// The subdirectories whose names begin with the name being typed.
    fn matches(&self) -> Vec<String> {
        let typed = self.name();
        let Some(dir) = self.dir().filter(|_| !typed.is_empty()) else {
            return Vec::new();
        };
        fs::read_dir(dir)
            .into_iter()
            .flatten()
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| p.is_dir())
            .map(|p| base_name(&p))
            .filter(|n| n.starts_with(typed) && (typed.starts_with('.') || !n.starts_with('.')))
            .sorted_by_key(|n| n.to_lowercase())
            .collect()
    }

    /// Tab (`forward`) or Shift-Tab: completes the name to the next or the
    /// previous directory it begins.
    pub(super) fn complete(&mut self, forward: bool) {
        let completion = match self.completion.take() {
            Some(mut c) => {
                let len = c.names.len();
                c.index = (c.index + if forward { 1 } else { len - 1 }) % len;
                c
            }
            None => {
                let names = self.matches();
                if names.is_empty() {
                    return;
                }
                Completion {
                    typed: self.name().to_string(),
                    index: if forward { 0 } else { names.len() - 1 },
                    names,
                }
            }
        };
        let kept = self.stem().len();
        self.path.truncate(kept);
        self.path.push_str(&completion.names[completion.index]);
        self.completion = Some(completion);
    }

    /// How much of the path's end Tab filled in.
    fn completed(&self) -> usize {
        self.completion
            .as_ref()
            .map_or(0, |c| c.names[c.index].len() - c.typed.len())
    }

    /// Ctrl-W.
    pub(super) fn delete_last_part(&mut self) {
        let kept = self
            .path
            .trim_end_matches('/')
            .rfind('/')
            .map_or(0, |i| i + 1);
        self.path.truncate(kept);
    }

    /// Shown after the path while it's edited: the rest of the first
    /// directory the name begins, or where Tab is in the cycle.
    fn hint(&self) -> Option<Span<'static>> {
        match &self.completion {
            Some(c) if c.names.len() > 1 => {
                Some(format!("  ({}/{})", c.index + 1, c.names.len()).dim())
            }
            Some(_) => None,
            None => {
                let typed = self.name();
                let matches = self.matches();
                if matches.iter().any(|n| n == typed) {
                    return None;
                }
                matches
                    .first()
                    .map(|n| n[typed.len()..].to_string().reversed())
            }
        }
    }

    pub(super) fn draw(&self, frame: &mut Frame, area: Rect) {
        const LABEL: &str = "To: ";
        let style = if self.editing {
            Style::new().yellow().underlined()
        } else {
            Style::new().bold()
        };
        // What Tab filled in stays inverted: the next Tab replaces it.
        let (typed, completed) = self.path.split_at(self.path.len() - self.completed());
        let spans = [
            Span::raw(LABEL).dim(),
            Span::styled(typed, style),
            Span::styled(completed, style.reversed()),
        ];
        let hint = self.editing.then(|| self.hint()).flatten();
        frame.render_widget(
            Paragraph::new(Line::from_iter(spans.into_iter().chain(hint))),
            area,
        );
        if self.editing {
            frame.set_cursor_position((
                area.x + (LABEL.len() + self.path.chars().count()) as u16,
                area.y,
            ));
        }
    }

    /// The directory the path is in - or is, when it ends in `/` - if it exists.
    pub(super) fn dir(&self) -> Option<PathBuf> {
        let path = paths::expand_tilde(&self.path);
        let dir = if self.path.ends_with('/') {
            Some(path.as_path())
        } else {
            path.parent()
        };
        dir.filter(|d| d.is_dir()).map(paths::canon)
    }

    /// The path, if it can be used; otherwise why not.
    pub(super) fn destination(&self) -> Result<PathBuf, String> {
        if self.path.trim().is_empty() {
            return Err("Type a path.".into());
        }
        let path = paths::expand_tilde(&self.path);
        if path.is_dir() {
            return match paths::is_empty_dir(&path) {
                Ok(true) => Ok(paths::canon(&path)),
                Ok(false) => Err(format!("{} isn't empty.", path.display())),
                Err(err) => Err(format!("can't read {}: {err}", path.display())),
            };
        }
        match path.parent() {
            _ if path.exists() => Err(format!("{} already exists.", path.display())),
            Some(parent) if !parent.is_dir() => {
                Err(format!("{}: no such directory", parent.display()))
            }
            _ => Ok(path),
        }
    }
}
