//! `synchub sync`: take the hub's folder labels and device names here.

use anyhow::{Result, bail};

use crate::api;
use crate::client;
use crate::device_id::DeviceId;
use crate::folder_id::FolderId;
use crate::side::{Hub, Local};
use crate::{say, step, warn};

/// Take the hub's folder labels and device names on this device.
///
/// Every device keeps its own label for each folder and its own name for
/// each device. sync gives this device's folders the labels the hub has for
/// them, and the devices both know - this one included - the names the hub
/// has for them, empty ones too. Folders and devices only one of them has are
/// left alone.
///
/// Example: synchub sync
#[derive(clap::Args)]
pub struct Args {}

pub fn run(_args: Args) -> Result<()> {
    sync()
}

/// Gives this device's folders and devices the hub's labels and names.
pub fn sync() -> Result<()> {
    let local = client::local()?;
    step!(Hub, "reading its folder labels and device names");
    let (local_config, hub_config) = (local.config()?, client::hub()?.config()?);
    let relabelled_folders = changed_folder_labels(&local_config, &hub_config);
    let renamed_devices = changed_device_names(&local_config, &hub_config);
    if relabelled_folders.is_empty() && renamed_devices.is_empty() {
        say!(
            Local,
            "the folder labels and device names are the hub's already"
        );
        return Ok(());
    }

    let failed_folders = relabelled_folders
        .iter()
        .map(|(id, label)| (format!("folder {id}"), local.rename_folder(id, label)));

    let failed_devices = renamed_devices
        .iter()
        .map(|(id, name)| (format!("device {id}"), local.rename_device(id, name)));

    // Each on its own: one failing mustn't leave the rest behind.
    let failed: Vec<(String, anyhow::Error)> = failed_folders
        .chain(failed_devices)
        .filter_map(|(what, result)| result.err().map(|err| (what, err)))
        .collect();
    if !failed.is_empty() {
        bail!("these kept their names here: {}", warn_all(&failed));
    }
    Ok(())
}

/// This device's folders the hub labels differently: their IDs, and the
/// hub's labels.
fn changed_folder_labels(
    local_config: &api::config::Config<Local>,
    hub_config: &api::config::Config<Hub>,
) -> Vec<(FolderId, String)> {
    local_config
        .folders
        .iter()
        .filter_map(|local_folder| {
            let hub_folder = hub_config.folder(&local_folder.id)?;
            (hub_folder.label != local_folder.label)
                .then(|| (local_folder.id.clone(), hub_folder.label.clone()))
        })
        .collect()
}

/// The devices this device knows that the hub names differently: their IDs,
/// and the hub's names.
fn changed_device_names(
    local_config: &api::config::Config<Local>,
    hub_config: &api::config::Config<Hub>,
) -> Vec<(DeviceId, String)> {
    local_config
        .devices
        .iter()
        .filter_map(|local_device| {
            let hub_device = hub_config.device(&local_device.id)?;
            (hub_device.name != local_device.name)
                .then(|| (local_device.id.clone(), hub_device.name.clone()))
        })
        .collect()
}

/// Warns about each error; returns what they were about.
fn warn_all(failed: &[(String, anyhow::Error)]) -> String {
    for (_, err) in failed {
        warn!("{err:#}");
    }
    let whats: Vec<&str> = failed.iter().map(|(what, _)| what.as_str()).collect();
    whats.join(", ")
}
