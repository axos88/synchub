//! `synchub share`: have the hub share a folder with another device too.

use anyhow::Result;

use crate::client;
use crate::commands::Arguments;
use crate::device_id::DeviceId;
use crate::folder_id::FolderId;

/// Have the hub share a folder this device has with another device too.
///
/// The hub shares FOLDER also with DEVICE, which gets it automatically.
/// A folder the hub doesn't have yet is created there first.
///
/// Example: synchub share Website laptop
///
/// Example: synchub share ~/projects/website laptop
#[derive(clap::Args, Debug, PartialEq)]
pub struct Args {
    /// The folder: its directory, ID or label
    pub folder: String,
    /// The device to share it with: its name on this device, or its device ID
    pub device: String,
}

impl Arguments for Args {
    fn arguments(&self) -> Vec<String> {
        vec![self.folder.clone(), self.device.clone()]
    }
}

pub fn run(args: Args) -> Result<()> {
    let (local_config, hub_config) = (client::local()?.config()?, client::hub()?.config()?);
    let folder = hub_config.resolve_folder(Some(&local_config), &args.folder)?;
    let device = hub_config.resolve_device(&args.device)?;
    share(&folder.id, &device.id)
}

/// Has the hub share the folder `id` with `device` too - this device
/// included, which then accepts it.
pub fn share(id: &FolderId, device: &DeviceId) -> Result<()> {
    client::hub()?.share_folder(id, device)
}
