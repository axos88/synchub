//! `synchub rename-device`: give a device a new name.

use anyhow::Result;

use crate::client;
use crate::commands::sync;
use crate::device_id::DeviceId;

/// Give a device a new name, on the hub and on this device.
///
/// Every device keeps its own name for each of the others; synchub finds
/// devices by this device's names. The device is renamed on the hub, and on
/// this device if it knows it. The other devices keep the names they have
/// for it.
///
/// Example: synchub rename-device laptop workstation
#[derive(clap::Args)]
pub struct Args {
    /// The device: its name on this device, or its device ID (in full, for
    /// one this device doesn't know)
    device: String,
    /// Its new name
    name: String,
}

pub fn run(args: Args) -> Result<()> {
    let hub_config = client::hub()?.config()?;
    let device = hub_config.resolve_device(&args.device)?;
    rename(&device.id, &args.name)
}

/// Gives the device `id` the name `name` on the hub, then takes the hub's
/// labels and names here.
pub fn rename(id: &DeviceId, name: &str) -> Result<()> {
    client::hub()?.rename_device(id, name)?;
    sync::sync()
}
