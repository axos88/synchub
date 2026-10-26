//! Everything the app shows, loaded in one go (`load`).

mod load;

use std::collections::BTreeMap;
use std::path::PathBuf;

use crate::api;
use crate::device_id::DeviceId;
use crate::folder_id::FolderId;
use crate::side::{self, Local};

/// This device's Syncthing.
pub struct SyncthingInfo {
    pub device_id: DeviceId,
    pub name: String,
    /// Where its API is.
    pub address: String,
    /// None if it couldn't be read.
    pub version: Option<String>,
}

/// The hub this device joined.
pub struct HubInfo {
    pub device_id: DeviceId,
    pub name: String,
    pub url: String,
    pub api: HubApi,
    /// Whether this device's Syncthing is connected to the hub's.
    pub connected: bool,
}

pub enum HubApi {
    /// It answers: its Syncthing's version, None if it couldn't be read.
    Reachable { version: Option<String> },
    /// Why it doesn't.
    Unreachable(String),
}

/// A device a folder is shared with.
pub struct DeviceRow {
    pub id: DeviceId,
    pub connected: bool,
    pub progress: DeviceProgress,
}

/// How far a device has synced a folder.
pub enum DeviceProgress {
    InSync,
    Syncing {
        percent: f64,
        items: u64,
        bytes: u64,
    },
    /// The hub shares the folder with it, but hasn't introduced it to this device yet.
    NotIntroduced,
    /// Why it can't be told.
    Unknown(String),
}

pub struct FolderView {
    pub folder: api::config::Folder<Local>,
    pub state: FolderState,
    /// The devices it's shared with, this one excepted.
    pub devices: Vec<DeviceRow>,
    /// The hub's other devices it isn't shared with.
    pub candidates: Vec<DeviceId>,
}

/// Where a folder of this device is at.
pub enum FolderState {
    Paused,
    /// The hub no longer shares it with this device: a leftover copy.
    LeftBehind,
    InSync,
    /// Files still to sync, in Syncthing's `state` (e.g. syncing, scanning).
    Behind {
        state: String,
        files: u64,
        bytes: u64,
    },
    /// Nothing to sync, but not idle: Syncthing's state (e.g. scanning).
    Busy(String),
    Error(String),
    /// Why its status can't be read.
    Unknown(String),
}

/// The `synchub` command lines doing what the app's actions did - kept by
/// the app beside the [`State`], which every reload replaces.
pub struct Log {
    /// Oldest first.
    commands: Vec<String>,
    /// How many of the latest are shown.
    pub max_lines: usize,
}

impl Default for Log {
    fn default() -> Self {
        Log {
            commands: Vec::new(),
            max_lines: 3,
        }
    }
}

impl Log {
    pub fn add(&mut self, command: String) {
        self.commands.push(command);
    }

    /// The latest commands, as many as are shown, oldest first.
    pub fn latest(&self) -> &[String] {
        &self.commands[self.commands.len().saturating_sub(self.max_lines)..]
    }
}

pub struct State {
    pub me: SyncthingInfo,
    /// None while no hub is set up.
    pub hub: Option<HubInfo>,
    pub folders: Vec<FolderView>,
    /// The hub's shares this device doesn't have.
    pub elsewhere: Vec<api::config::Folder<side::Hub>>,
    /// The hub's devices a share can be given to - not the hub, not this
    /// one - by name.
    pub others: Vec<DeviceId>,
    pub names: BTreeMap<DeviceId, String>,
}

impl State {
    pub fn name(&self, id: &DeviceId) -> String {
        self.names
            .get(id)
            .cloned()
            .unwrap_or_else(|| id.short().to_string())
    }

    pub fn hub_id(&self) -> Option<&DeviceId> {
        self.hub.as_ref().map(|hub| &hub.device_id)
    }

    /// Whether the hub is usable.
    pub fn hub_ready(&self) -> bool {
        self.hub
            .as_ref()
            .is_some_and(|hub| matches!(hub.api, HubApi::Reachable { .. }))
    }

    /// The folder `id` of this device.
    pub fn folder(&self, id: &FolderId) -> Option<&FolderView> {
        self.folders.iter().find(|f| f.folder.id == *id)
    }

    /// The hub's share `id` that this device doesn't have.
    pub fn elsewhere(&self, id: &FolderId) -> Option<&api::config::Folder<side::Hub>> {
        self.elsewhere.iter().find(|f| f.id == *id)
    }

    /// A folder's label - here, or else on the hub - or its ID when it has
    /// none.
    pub fn label(&self, id: &FolderId) -> String {
        self.folder(id)
            .map(|v| v.folder.label_or_id())
            .or_else(|| self.elsewhere(id).map(|f| f.label_or_id()))
            .unwrap_or_else(|| id.to_string())
    }

    /// Whether this device's folder `folder` is shared with `device`.
    pub fn shared(&self, folder: &FolderId, device: &DeviceId) -> bool {
        self.folder(folder)
            .is_some_and(|v| v.devices.iter().any(|d| d.id == *device))
    }

    /// This device's folders, (directory, name): for the directory browser
    /// to mark.
    pub fn synced(&self) -> Vec<(PathBuf, String)> {
        self.folders
            .iter()
            .map(|v| (v.folder.local_path(), v.folder.name()))
            .collect()
    }
}
