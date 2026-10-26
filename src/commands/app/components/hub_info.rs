//! The hub, at the top right - or how to get one.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Style, Stylize};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use super::{identicon_panel, syncthing_version};
use crate::commands::app::state::{HubApi, HubInfo};

pub fn draw(frame: &mut Frame, area: Rect, hub: Option<&HubInfo>) {
    let Some(hub) = hub else {
        let lines = vec![
            Line::from("Hub".bold().underlined()),
            Line::from("none - restart synchub to join one, or run `synchub join`".yellow()),
        ];
        frame.render_widget(Paragraph::new(lines), area);
        return;
    };
    let (version, api) = match &hub.api {
        HubApi::Reachable { version } => (
            Span::raw(syncthing_version(version.as_deref())),
            Span::raw("reachable").green(),
        ),
        HubApi::Unreachable(err) => (
            Span::raw(syncthing_version(None)),
            Span::raw(err.clone()).red(),
        ),
    };
    let sync = if hub.connected {
        Span::raw("● connected").green()
    } else {
        Span::raw("○ not connected").yellow()
    };
    let details = vec![
        ("name", Span::raw(hub.name.clone()).green().bold()),
        ("device ID", Span::raw(hub.device_id.to_string())),
        ("address", Span::raw(hub.url.clone())),
        ("version", version),
        ("API", api),
        ("sync", sync),
    ];
    let picture = Style::new().cyan();
    identicon_panel::draw(frame, area, "Hub", &hub.device_id, picture, details);
}
