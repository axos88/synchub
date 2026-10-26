//! This device's Syncthing, at the top left.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Style, Stylize};
use ratatui::text::Span;

use super::{identicon_panel, syncthing_version};
use crate::commands::app::state::SyncthingInfo;

pub fn draw(frame: &mut Frame, area: Rect, info: &SyncthingInfo) {
    let details = vec![
        ("name", Span::raw(info.name.clone()).green().bold()),
        ("device ID", Span::raw(info.device_id.to_string())),
        ("address", Span::raw(info.address.clone())),
        (
            "version",
            Span::raw(syncthing_version(info.version.as_deref())),
        ),
    ];
    let picture = Style::new().magenta();
    identicon_panel::draw(
        frame,
        area,
        "This device",
        &info.device_id,
        picture,
        details,
    );
}
