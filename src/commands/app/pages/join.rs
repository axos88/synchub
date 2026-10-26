//! Joining a hub from the app: the form asking for its details, shown when
//! the app starts without one, then checking the device ID its API reports.

use anyhow::Result;

use crate::client;
use crate::commands::app::App;
use crate::commands::app::components::dialog::{self, Dialog};
use crate::commands::app::components::form::{DirField, Form, TextField};
use crate::commands::app::window::{RunnableWindow, Window};
use crate::commands::{Command, join};
use crate::device_id::{DeviceId, identicon};

impl App {
    /// Joins a hub: asks for its details, has the user check the device ID
    /// its API reports, then joins it.
    pub fn join(&mut self) -> Result<()> {
        const DOING: &str = "Joining a hub";
        let current_dir = match client::local().and_then(|local| local.folder_defaults()) {
            Ok(defaults) => defaults.path,
            Err(err) => return self.alert(DOING, &err),
        };
        let Some(mut args) = hub_details_form(&current_dir).run()? else {
            return Ok(());
        };
        let hub = match join::find_hub(&args) {
            Ok(hub) => hub,
            Err(err) => return self.alert(DOING, &err),
        };
        if !self.confirm_hub_id(&hub.device_id)? {
            return Ok(());
        }
        args.device_id = Some(hub.device_id.clone());
        self.perform(
            DOING.to_string(),
            Some(Command::Join(args.clone())),
            move || join::join_hub(hub, args.default_folders_dir.as_deref()),
        )
    }

    /// Shows that `doing` failed with `err`.
    fn alert(&self, doing: &str, err: &anyhow::Error) -> Result<()> {
        let error = format!("{err:#}");
        dialog::alert(doing, &[], &error).popup(self)
    }

    /// Shows the hub's device ID and its picture, and asks whether they're
    /// the hub's.
    fn confirm_hub_id(&self, hub_id: &DeviceId) -> Result<bool> {
        let mut lines = identicon::rows(hub_id);
        lines.extend([
            String::new(),
            hub_id.to_string(),
            String::new(),
            "Compare them with the hub's own: in its web UI, Actions, Show ID.".to_string(),
        ]);
        let confirm = dialog::confirm(
            "The hub's device ID, as its API reports it",
            &lines,
            "Is it the hub's?",
            "Yes, join it",
            "No",
        );
        confirm.popup(self)
    }
}

/// A form asking for the hub's details - new folders going into
/// `current_dir` unless changed; None if cancelled.
fn hub_details_form(current_dir: &str) -> impl Window<Output = Option<join::Args>> + '_ {
    let mut form = Form::new("Join a hub");
    let url = form.add(
        TextField::new()
            .label("Hub address")
            .placeholder("https://host:8384")
            .required(),
    );
    let api_key = form.add(
        TextField::new()
            .label("Hub API key")
            .placeholder("on the hub: Actions, Settings, GUI")
            .required(),
    );
    let folders_dir = form.add(
        DirField::new()
            .label("Directory for new folders on this device")
            .value(current_dir),
    );
    form.map_some(move |answers| {
        let chosen = answers[folders_dir].path().trim();
        join::Args {
            url: answers[url].text().trim().to_string(),
            api_key: answers[api_key].text().trim().to_string(),
            device_id: None,
            default_folders_dir: (!chosen.is_empty() && chosen != current_dir)
                .then(|| chosen.to_string()),
            yes: false,
        }
    })
}
