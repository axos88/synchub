//! The parts the app's pages are drawn from.

pub mod clickable_list;
pub mod dialog;
pub mod dir_browser;
pub mod form;
pub mod hub_info;
pub mod identicon_panel;
pub mod log;
pub mod status_bar;
pub mod syncthing_info;
pub mod tree;

/// How a Syncthing's version reads in this device's and the hub's panels.
pub fn syncthing_version(version: Option<&str>) -> String {
    match version {
        Some(version) => format!("Syncthing {version}"),
        None => "unknown".to_string(),
    }
}
