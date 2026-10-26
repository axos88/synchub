//! `synchub sync`: take the hub's folder labels and device names here, and
//! give the hub this device's where it has none.

use anyhow::{Result, bail};

use crate::api;
use crate::client::{self, HubClient};
use crate::commands::Arguments;
use crate::device_id::DeviceId;
use crate::folder_id::FolderId;
use crate::side::{Hub, Local};
use crate::{say, step, warn};

/// Take the hub's folder labels and device names here; give it this device's where it has none
///
/// Every device keeps its own label for each folder and its own name for
/// each device. sync gives this device's folders the labels the hub has for
/// them, and the devices both know - this one included - the names the hub
/// has for them. Where the hub's is empty, the hub takes this device's.
/// Folders and devices only one of them has are left alone.
///
/// It runs whenever synchub connects to the hub, and on every reload of the
/// app; this runs it on its own.
///
/// Example: synchub sync
#[derive(clap::Args, Debug, PartialEq)]
pub struct Args {}

impl Arguments for Args {
    fn arguments(&self) -> Vec<String> {
        Vec::new()
    }
}

pub fn run(_args: Args) -> Result<()> {
    if !sync(client::hub()?)? {
        say!(
            Local,
            "the folder labels and device names are the hub's already"
        );
    }
    Ok(())
}

/// [`sync`] on connecting to the hub, or reloading the app: a label or name
/// that can't be set doesn't stop what's going on - it's warned about, and
/// tried again next time.
pub fn on_connect(hub: &HubClient) {
    if let Err(err) = sync(hub) {
        warn!("{err:#}");
    }
}

/// Gives this device's folders and devices the hub's labels and names, and
/// the hub this device's where it has none; whether anything changed.
pub fn sync(hub: &HubClient) -> Result<bool> {
    let local = client::local()?;
    step!(Hub, "reading its folder labels and device names");
    let (local_config, hub_config) = (local.config()?, hub.config()?);
    let folders = folder_renames(&local_config, &hub_config);
    let devices = device_renames(&local_config, &hub_config);
    if folders.is_empty() && devices.is_empty() {
        return Ok(false);
    }

    let failed_folders = folders.iter().map(|rename| match rename {
        Rename::Here(id, label) => (format!("folder {id}"), local.rename_folder(id, label)),
        Rename::OnHub(id, label) => (
            format!("folder {id} on the hub"),
            hub.rename_folder(id, label),
        ),
    });
    let failed_devices = devices.iter().map(|rename| match rename {
        Rename::Here(id, name) => (format!("device {id}"), local.rename_device(id, name)),
        Rename::OnHub(id, name) => (
            format!("device {id} on the hub"),
            hub.rename_device(id, name),
        ),
    });

    // Each on its own: one failing mustn't leave the rest behind.
    let failed: Vec<(String, anyhow::Error)> = failed_folders
        .chain(failed_devices)
        .filter_map(|(what, result)| result.err().map(|err| (what, err)))
        .collect();
    if !failed.is_empty() {
        bail!("these weren't renamed: {}", warn_all(&failed));
    }
    Ok(true)
}

/// A label or name to set: the hub's here, or - the hub having none - this
/// device's on the hub.
#[derive(Debug, PartialEq)]
enum Rename<Id> {
    Here(Id, String),
    OnHub(Id, String),
}

/// What sync sets for `id`, called `local` here and `hub` on the hub.
fn rename<Id: Clone>(id: &Id, local: &str, hub: &str) -> Option<Rename<Id>> {
    match (local, hub) {
        ("", "") => None,
        (local, "") => Some(Rename::OnHub(id.clone(), local.to_string())),
        (local, hub) => (local != hub).then(|| Rename::Here(id.clone(), hub.to_string())),
    }
}

/// The labels to set for the folders both have.
fn folder_renames(
    local_config: &api::config::Config<Local>,
    hub_config: &api::config::Config<Hub>,
) -> Vec<Rename<FolderId>> {
    local_config
        .folders
        .iter()
        .filter_map(|local_folder| {
            let hub_folder = hub_config.folder(&local_folder.id)?;
            rename(&local_folder.id, &local_folder.label, &hub_folder.label)
        })
        .collect()
}

/// The names to set for the devices both know.
fn device_renames(
    local_config: &api::config::Config<Local>,
    hub_config: &api::config::Config<Hub>,
) -> Vec<Rename<DeviceId>> {
    local_config
        .devices
        .iter()
        .filter_map(|local_device| {
            let hub_device = hub_config.device(&local_device.id)?;
            rename(&local_device.id, &local_device.name, &hub_device.name)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_hubs_wins_and_an_empty_one_takes_this_devices() {
        assert_eq!(rename(&1, "Notes", "Notes"), None);
        assert_eq!(
            rename(&1, "Old", "New"),
            Some(Rename::Here(1, "New".into()))
        );
        assert_eq!(rename(&1, "", "New"), Some(Rename::Here(1, "New".into())));
        assert_eq!(
            rename(&1, "Mine", ""),
            Some(Rename::OnHub(1, "Mine".into()))
        );
        assert_eq!(rename(&1, "", ""), None::<Rename<i32>>);
    }
}
