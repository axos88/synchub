//! `synchub convert`: move direct shares over to the hub, one device at a
//! time.

use std::collections::BTreeSet;

use anyhow::Result;

use crate::api;
use crate::client;
use crate::client::HubClient;
use crate::device_id::DeviceId;
use crate::say;
use crate::side::{Hub, Local};

/// Move folders shared directly with devices of the hub's over to the hub.
///
/// A folder here shared directly with devices that have joined the hub is
/// put on the hub - shared with this device and those devices - and here
/// shared with the hub instead, which introduces them again. Devices that
/// haven't joined keep their direct share, so a network moves to the hub one
/// device at a time. Devices that have joined the hub stop being introducers
/// whose folders are accepted automatically here: only the hub is one. This
/// also happens whenever a command first connects to the hub.
///
/// Example: synchub convert
#[derive(clap::Args)]
pub struct Args {}

pub fn run(_args: Args) -> Result<()> {
    // Connecting to the hub converts: see `client::hub`.
    client::hub()?;
    say!("done");
    Ok(())
}

/// For each folder here that is shared directly with devices that have
/// joined the hub, the hub gets the folder (if it lacks it) and shares it
/// with this device and those devices; here, the folder is then shared with
/// the hub instead of them, and the hub introduces them again. Devices that
/// haven't joined keep their direct share, so a network can move to the hub
/// one device at a time. Devices that have joined the hub stop being
/// introducers or auto-accepted here.
///
/// It takes the hub's client rather than using [`client::hub`]: it runs
/// while that is being made, and while joining a hub.
pub fn convert(hub: &HubClient) -> Result<()> {
    let local = client::local()?;
    let my_id = local.my_id()?;
    let hub_config = hub.config()?;
    let members: BTreeSet<&DeviceId> = hub_config.devices.iter().map(|d| &d.id).collect();
    let joined = |id: &DeviceId| *id != my_id && id != hub.id() && members.contains(id);
    let local_config = local.config()?;
    let trusted = local_config
        .devices
        .iter()
        .filter(|d| (d.introducer || d.auto_accept_folders) && joined(&d.id));
    for device in trusted {
        local.set_introducer(device, false)?;
    }
    let moving: Vec<(&api::config::Folder<Local>, Vec<&DeviceId>)> = local_config
        .folders
        .iter()
        .filter(|folder| !folder.is_receive_encrypted())
        .map(|folder| {
            let direct: Vec<&DeviceId> = folder
                .direct_shares(hub.id())
                .filter(|id| joined(id))
                .collect();
            (folder, direct)
        })
        .filter(|(_, direct)| !direct.is_empty())
        .collect();
    for (folder, direct) in &moving {
        share_on_hub(hub, &hub_config, folder, &my_id, direct)?;
        hand_over(folder, &my_id, hub.id(), direct)?;
    }
    if !moving.is_empty() {
        // Reconnecting makes the hub send its folder list again, and with it
        // introduce the devices it shares each folder with.
        local.reconnect(hub.id())?;
    }
    Ok(())
}

/// Has the hub share `folder` with this device and the `direct` ones -
/// creating it on the hub if it isn't there.
fn share_on_hub(
    hub: &HubClient,
    hub_config: &api::config::Config<Hub>,
    folder: &api::config::Folder<Local>,
    my_id: &DeviceId,
    direct: &[&DeviceId],
) -> Result<()> {
    let wanted: Vec<&DeviceId> = std::iter::once(my_id)
        .chain(direct.iter().copied())
        .collect();
    match hub_config.folder(&folder.id) {
        Some(hub_folder) => hub.share(hub_folder, &wanted),
        None => {
            let path = hub_config.new_folder_path(&folder.id);
            let devices: Vec<&DeviceId> = std::iter::once(hub.id())
                .chain(wanted.iter().copied())
                .collect();
            hub.add_folder(&folder.id, &folder.label, &path, &devices)
        }
    }
}

/// Here, shares `folder` with the hub instead of the `direct` devices: the
/// hub introduces them again.
fn hand_over(
    folder: &api::config::Folder<Local>,
    my_id: &DeviceId,
    hub_id: &DeviceId,
    direct: &[&DeviceId],
) -> Result<()> {
    let local = client::local()?;
    local.share(folder, &[my_id, hub_id])?;
    local.unshare(folder, direct)
}
