//! Sharing a new directory from the app: a form with the directory - its
//! browser open as the form opens - its label, and the devices that see it.

use std::path::PathBuf;

use anyhow::Result;

use crate::commands::app::App;
use crate::commands::app::components::form::{Check, ChecksField, DirField, Form, TextField};
use crate::commands::app::state::State;
use crate::commands::app::window::{RunnableWindow, Window};
use crate::commands::{Command, new};
use crate::device_id::DeviceId;
use crate::paths;

/// What the form asks for: the directory to share, its label (if any) and
/// the devices that see it.
type Answers = (PathBuf, Option<String>, Vec<DeviceId>);

impl App {
    /// Asks for the directory and the details, then creates the share.
    pub fn new_share(&mut self) -> Result<()> {
        let state = self.state()?;
        let Some((path, label, devices)) = share_details_form(state).run()? else {
            return Ok(());
        };
        let command = Command::New(new::Args {
            path: paths::relative_to_home(&path).into(),
            label: label.clone(),
            devices: devices.iter().map(|id| state.name(id)).collect(),
        });
        self.perform(
            format!("Sharing {}", paths::relative_to_home(&path)),
            Some(command),
            move || new::create(&path, label.as_deref(), &devices),
        )
    }
}

/// A form asking for the directory to share and the details; None if
/// cancelled.
fn share_details_form(state: &State) -> impl Window<Output = Option<Answers>> + '_ {
    let mut form = Form::new("Share a directory through the hub");
    let dir = form.add(
        DirField::new()
            .label("Directory to share")
            .required()
            .marking(state.synced()),
    );
    let label = form.add(
        TextField::new()
            .label("Label")
            .placeholder("optional - without one, the folder shows by its ID"),
    );
    let devices = form.add(
        ChecksField::new()
            .label("Devices that see it (the hub and this device always do)")
            .items(
                state
                    .others
                    .iter()
                    .map(|id| Check {
                        key: id.clone(),
                        label: state.name(id),
                        ticked: false,
                    })
                    .collect(),
            ),
    );
    form.map_some(move |answers| {
        let typed = answers[label].text().trim();
        (
            paths::expand_tilde(answers[dir].path()),
            (!typed.is_empty()).then(|| typed.to_string()),
            answers[devices].ticked().cloned().collect(),
        )
    })
}
