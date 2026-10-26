//! A device's identicon with its title and details beside it: what this
//! device's Syncthing and the hub share at the top of the screen.

use itertools::Itertools;
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Style, Stylize};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use crate::device_id::{DeviceId, identicon};

/// Its height: the title and the identicon.
pub const HEIGHT: u16 = 7;

/// A detail: what it is, and its value.
pub type Detail = (&'static str, Span<'static>);

/// Draws `device_id`'s identicon in the `picture` style, `details` beside it.
pub fn draw(
    frame: &mut Frame,
    area: Rect,
    title: &str,
    device_id: &DeviceId,
    picture: Style,
    details: Vec<Detail>,
) {
    let rows = identicon::rows(device_id)
        .into_iter()
        .zip_longest(details.into_iter().map(Some))
        .map(|pair| {
            let (picture_row, detail) = pair.or_else(|| " ".repeat(10), || None);
            let detail = detail
                .into_iter()
                .flat_map(|(label, value)| [Span::raw(format!("{label:<9}  ")).dim(), value]);
            Line::from(
                [Span::styled(picture_row, picture), Span::raw("   ")]
                    .into_iter()
                    .chain(detail)
                    .collect::<Vec<_>>(),
            )
        });
    let title = Line::from(title.to_string().bold().underlined());
    let lines: Vec<Line> = std::iter::once(title).chain(rows).collect();
    frame.render_widget(Paragraph::new(lines), area);
}
