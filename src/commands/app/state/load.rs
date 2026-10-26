//! Reading the state from this device's Syncthing and the hub's.

use std::collections::BTreeMap;

use anyhow::Result;
use itertools::Itertools;

use super::{
    DeviceProgress, DeviceRow, FolderState, FolderView, HubApi, HubInfo, State, SyncthingInfo,
};
use crate::api;
use crate::client::{self, Client, HubClient};
use crate::device_id::DeviceId;
use crate::settings;
use crate::side::{self, Local};
use crate::step;

impl State {
    /// Loads it all.
    pub fn load() -> Result<Self> {
        let local = client::local()?;
        step!(
            Local,
            "reading this device's settings at {}",
            local.address()
        );
        let my_id = local.my_id()?;
        let local_config = local.config()?;
        let mut names: BTreeMap<DeviceId, String> = local_config
            .devices
            .iter()
            .map(|d| (d.id.clone(), d.display()))
            .collect();
        // Before client::hub: moving folders over to the hub reconnects to it.
        let connected = local.connected_devices()?;
        let me = SyncthingInfo {
            device_id: my_id.clone(),
            name: names.get(&my_id).cloned().unwrap_or_default(),
            address: local.address().to_string(),
            version: local.version().ok(),
        };
        let (hub, hub_config) = match settings::get()?.hub {
            None => (None, None),
            Some(settings) => {
                let (hub, config) = hub(&settings, &names, &connected);
                (Some(hub), config)
            }
        };
        if let Some(config) = &hub_config {
            for device in &config.devices {
                names
                    .entry(device.id.clone())
                    .or_insert_with(|| device.display());
            }
        }

        let hub_id = hub.as_ref().map(|hub| hub.device_id.clone());
        let others = match &hub_config {
            Some(config) => others(config, hub_id.as_ref(), &my_id),
            None => Vec::new(),
        };
        let hub_folders = hub_config.map(|config| config.folders);
        let folders: Vec<FolderView> = local_config
            .folders
            .into_iter()
            .sorted_by_key(|f| (f.label.to_lowercase(), f.id.clone()))
            .map(|folder| {
                let hub_folder = hub_folders.iter().flatten().find(|f| f.id == folder.id);
                let hub = hub_folder.zip(hub_id.as_ref());
                folder_view(local, folder, hub, &my_id, &connected, &others)
            })
            .collect();
        let elsewhere = hub_folders
            .unwrap_or_default()
            .into_iter()
            .filter(|f| !folders.iter().any(|v| v.folder.id == f.id))
            .sorted_by_key(|f| (f.label.to_lowercase(), f.id.clone()))
            .collect();
        Ok(State {
            me,
            hub,
            folders,
            elsewhere,
            others,
            names,
        })
    }
}

/// The hub set up in `settings`, and - if it can be reached - its
/// configuration.
fn hub(
    settings: &settings::Hub,
    names: &BTreeMap<DeviceId, String>,
    connected: &[DeviceId],
) -> (HubInfo, Option<api::config::Config<side::Hub>>) {
    let (api, config) = match reach_hub() {
        Ok((client, config)) => {
            let version = client.version().ok();
            (HubApi::Reachable { version }, Some(config))
        }
        Err(err) => (HubApi::Unreachable(format!("{err:#}")), None),
    };
    let info = HubInfo {
        device_id: settings.device_id.clone(),
        name: names.get(&settings.device_id).cloned().unwrap_or_default(),
        url: settings.url.clone(),
        api,
        connected: connected.contains(&settings.device_id),
    };
    (info, config)
}

/// Connects to the hub, and reads its configuration.
fn reach_hub() -> Result<(&'static HubClient, api::config::Config<side::Hub>)> {
    let client = client::hub()?;
    step!(Hub, "reading its folders and devices");
    Ok((client, client.config()?))
}

/// The hub's devices a share can be given to - not the hub, not this one -
/// by name.
fn others(
    hub_config: &api::config::Config<side::Hub>,
    hub_id: Option<&DeviceId>,
    my_id: &DeviceId,
) -> Vec<DeviceId> {
    hub_config
        .devices
        .iter()
        .filter(|d| Some(&d.id) != hub_id && d.id != *my_id)
        .sorted_by_key(|d| d.display().to_lowercase())
        .map(|d| d.id.clone())
        .collect()
}

/// This device's `folder`, with the devices it's shared with - the hub's
/// list, given its folder and ID (`hub`), and this device's direct shares -
/// and the hub's `others` it could be shared with.
fn folder_view(
    local: &Client<Local>,
    folder: api::config::Folder<Local>,
    hub: Option<(&api::config::Folder<side::Hub>, &DeviceId)>,
    my_id: &DeviceId,
    connected: &[DeviceId],
    others: &[DeviceId],
) -> FolderView {
    step!(Local, "reading {}'s sync status", folder.name());
    // A copy the hub no longer shares with this device: its device list doesn't apply here.
    let left_behind = hub.is_some_and(|(f, _)| !f.shared_with_device(my_id));
    // The hub decides who sees a folder; this device's own list lags behind
    // it: a device the hub introduced and stopped sharing it with is only
    // being removed here.
    let shared: Vec<&DeviceId> = match hub.filter(|_| !left_behind) {
        Some((hub_folder, hub_id)) => folder
            .direct_shares(hub_id)
            .chain(hub_folder.shared_with_devices())
            .collect(),
        None => folder.shared_with_devices().collect(),
    };
    let devices: Vec<DeviceRow> = shared
        .into_iter()
        .filter(|id| *id != my_id)
        .unique()
        .map(|id| DeviceRow {
            connected: connected.contains(id),
            progress: if folder.shared_with_device(id) {
                progress(local.completion(&folder.id, id))
            } else {
                DeviceProgress::NotIntroduced
            },
            id: id.clone(),
        })
        .collect();
    if left_behind {
        return FolderView {
            folder,
            state: FolderState::LeftBehind,
            devices,
            candidates: Vec::new(),
        };
    }
    let candidates = others
        .iter()
        .filter(|id| !devices.iter().any(|d| d.id == **id))
        .cloned()
        .collect();
    FolderView {
        state: folder_state(local, &folder),
        folder,
        devices,
        candidates,
    }
}

fn folder_state(local: &Client<Local>, folder: &api::config::Folder<Local>) -> FolderState {
    if folder.paused {
        return FolderState::Paused;
    }
    match local.status(&folder.id) {
        Ok(status) if status.state == "error" => FolderState::Error(status.error),
        Ok(status) if status.need_files > 0 => FolderState::Behind {
            state: status.state,
            files: status.need_files,
            bytes: status.need_bytes,
        },
        Ok(status) if status.state == "idle" => FolderState::InSync,
        Ok(status) => FolderState::Busy(status.state),
        Err(err) => FolderState::Unknown(format!("{err:#}")),
    }
}

fn progress(completion: Result<api::db::Completion>) -> DeviceProgress {
    match completion {
        Ok(c) if c.completion >= 100.0 => DeviceProgress::InSync,
        Ok(c) => DeviceProgress::Syncing {
            percent: c.completion,
            items: c.need_items,
            bytes: c.need_bytes,
        },
        Err(err) => DeviceProgress::Unknown(format!("{err:#}")),
    }
}
