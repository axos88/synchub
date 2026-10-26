//! `synchub unshare`: have the hub stop sharing a folder with another device.

use anyhow::Result;

use crate::client;
use crate::commands::Arguments;
use crate::device_id::DeviceId;
use crate::folder_id::FolderId;

/// Have the hub stop sharing a folder this device has with another device.
///
/// The hub no longer shares FOLDER with DEVICE, which keeps its copy until
/// it removes the folder itself. This device keeps the folder - to drop it
/// here, use `synchub remove`.
///
/// Example: synchub unshare Website phone
#[derive(clap::Args, Debug, PartialEq)]
pub struct Args {
    /// The folder: its directory, ID or label
    pub folder: String,
    /// The device to stop sharing it with: its name on this device, or its
    /// device ID
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
    unshare(&folder.id, &device.id)
}

/// Has the hub stop sharing the folder `id` with `device`, which keeps its
/// copy.
pub fn unshare(id: &FolderId, device: &DeviceId) -> Result<()> {
    client::hub()?.unshare_folder(id, device)
}
