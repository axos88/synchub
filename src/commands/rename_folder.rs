//! `synchub rename-folder`: give a folder a new label.

use anyhow::Result;

use crate::client;
use crate::commands::Arguments;
use crate::commands::sync;
use crate::folder_id::FolderId;

/// Give a folder a new label, on the hub and on this device.
///
/// Every device keeps its own label for each folder. The folder is
/// relabelled on the hub, and on this device if it has it; the other devices
/// keep theirs. Its directories keep their names.
///
/// Example: synchub rename-folder Website Homepage
#[derive(clap::Args, Debug, PartialEq)]
pub struct Args {
    /// The folder: its directory, ID or label on this device - or the ID of
    /// one only the hub has
    pub folder: String,
    /// Its new label
    pub label: String,
}

impl Arguments for Args {
    fn arguments(&self) -> Vec<String> {
        vec![self.folder.clone(), self.label.clone()]
    }
}

pub fn run(args: Args) -> Result<()> {
    let (local_config, hub_config) = (client::local()?.config()?, client::hub()?.config()?);
    let folder = hub_config.resolve_folder(Some(&local_config), &args.folder)?;
    rename(&folder.id, &args.label)
}

/// Gives the folder `id` the label `label` on the hub, then takes the hub's
/// labels and names here.
pub fn rename(id: &FolderId, label: &str) -> Result<()> {
    client::hub()?.rename_folder(id, label)?;
    sync::sync()
}
