//! Drawing a form: its title, its fields, and what the keys do.

use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::Stylize;
use ratatui::widgets::Paragraph;

use super::Field;
use crate::commands::app::components::status_bar::{self, HotKey, HotKeys};

/// Draws the form; returns each field's area.
pub(super) fn draw(
    frame: &mut Frame,
    title: &str,
    fields: &mut [Field],
    focus: usize,
) -> Vec<Rect> {
    let mut constraints = vec![Constraint::Length(2)];
    constraints.extend(fields.iter().map(|f| Constraint::Length(f.height())));
    constraints.push(Constraint::Min(0));
    constraints.push(Constraint::Length(status_bar::HEIGHT));
    let areas = Layout::vertical(constraints).split(frame.area());
    let [title_area, field_areas @ .., _filler, keys_area] = &*areas else {
        unreachable!("a title, the fields, a filler and the keys: four parts at least");
    };

    frame.render_widget(Paragraph::new(title.bold()), *title_area);
    let layout = fields
        .iter_mut()
        .zip(field_areas)
        .enumerate()
        .map(|(i, (field, &area))| {
            field.draw(frame, area, i == focus);
            area
        })
        .collect();
    let enter = if fields.iter().any(Field::missing) {
        "next required field"
    } else {
        "done"
    };
    let hot_keys = HotKeys(vec![
        HotKey::new(&["Tab", "Shift-Tab", "click"], "field"),
        HotKey::new(&["Space", "click"], "tick or choose"),
        HotKey::new(&["Enter"], enter),
        HotKey::new(&["Esc"], "cancel"),
    ]);
    status_bar::draw(frame, *keys_area, &hot_keys);
    layout
}
