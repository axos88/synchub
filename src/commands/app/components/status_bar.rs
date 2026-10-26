//! The status bar: the keys that do something now.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::Stylize;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

pub const HEIGHT: u16 = 2;

/// What the keys do now; none while a dialog has them.
#[derive(Default)]
pub struct HotKeys(pub Vec<HotKey>);

/// Keys doing the same thing.
pub struct HotKey {
    /// e.g. `Enter`, `m`.
    pub keys: &'static [&'static str],
    pub action: String,
}

impl HotKey {
    pub fn new(keys: &'static [&'static str], action: impl Into<String>) -> Self {
        HotKey {
            keys,
            action: action.into(),
        }
    }
}

/// Draws the `hot_keys` - each kept together when the bar wraps.
pub fn draw(frame: &mut Frame, area: Rect, hot_keys: &HotKeys) {
    frame.render_widget(Paragraph::new(lines(hot_keys, area.width)), area);
}

/// The bar's lines, wrapping between key bindings - never inside one.
fn lines(hot_keys: &HotKeys, width: u16) -> Vec<Line<'static>> {
    const GAP: &str = "   ";
    let width = usize::from(width);
    let mut lines = vec![Vec::new()];
    let mut used = 0;
    for HotKey { keys, action } in &hot_keys.0 {
        let keys = format!("({})", keys.join(", "));
        let len = keys.chars().count() + 1 + action.chars().count();
        if used > 0 && used + GAP.len() + len > width {
            lines.push(Vec::new());
            used = 0;
        }
        let line = lines.last_mut().expect("one line at least");
        if used > 0 {
            line.push(Span::raw(GAP));
            used += GAP.len();
        }
        line.push(Span::raw(keys).bold());
        line.push(Span::raw(format!(" {action}")));
        used += len;
    }
    lines.into_iter().map(Line::from).collect()
}
