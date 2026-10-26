//! Syncthing's configuration objects (`/rest/config/...`).

use std::fmt;
use std::marker::PhantomData;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use itertools::Itertools;
use serde::de::IntoDeserializer;
use serde::{Deserialize, Serialize};

use crate::device_id::DeviceId;
use crate::folder_id::FolderId;
use crate::paths;
use crate::side::{Hub, Local, Side};

/// What `/rest/config` returns - and a `ConfigSaved` event's data.
pub type ConfigResponse<S> = Config<S>;

/// A Syncthing's configuration, as far as synchub reads it: its folders,
/// the devices it knows, and the folder defaults. `S` is whose it is.
#[derive(Deserialize, Clone)]
#[serde(bound = "")]
pub struct Config<S> {
    pub folders: Vec<Folder<S>>,
    pub devices: Vec<Device>,
    pub defaults: Defaults<S>,
}

/// The templates new folders and devices are made from.
#[derive(Deserialize, Clone)]
#[serde(bound = "")]
pub struct Defaults<S> {
    pub folder: Folder<S>,
}

/// What `/rest/config/folders` returns, and `/rest/config/defaults/folder`.
pub type FolderResponse<S> = Folder<S>;

/// A Syncthing folder, as a device's configuration has it. `S` is whose:
/// its path is a directory on that device.
#[derive(Deserialize, Clone)]
#[serde(rename_all = "camelCase", bound = "")]
pub struct Folder<S> {
    pub id: FolderId,
    pub label: String,
    pub path: String,
    #[serde(rename = "type")]
    pub folder_type: FolderType,
    pub paused: bool,
    pub send_ownership: bool,
    pub sync_ownership: bool,
    #[serde(default)]
    pub devices: Vec<FolderDevice>,
    #[serde(skip)]
    side: PhantomData<S>,
}

/// A device a folder is shared with - every field Syncthing has for it, so
/// writing a folder's list back loses nothing.
#[derive(Deserialize, Serialize, Clone, PartialEq, Debug)]
#[serde(rename_all = "camelCase")]
pub struct FolderDevice {
    #[serde(rename = "deviceID")]
    pub device_id: DeviceId,
    /// The introducer that shared it, if it came from one.
    #[serde(
        default,
        deserialize_with = "empty_as_none",
        skip_serializing_if = "Option::is_none"
    )]
    pub introduced_by: Option<DeviceId>,
    /// For a device that gets the folder encrypted.
    #[serde(
        default,
        deserialize_with = "empty_as_none",
        skip_serializing_if = "Option::is_none"
    )]
    pub encryption_password: Option<String>,
}

/// Syncthing writes an empty string where there's none.
fn empty_as_none<'de, D: serde::Deserializer<'de>, T: Deserialize<'de>>(
    deserializer: D,
) -> Result<Option<T>, D::Error> {
    let value = Option::<String>::deserialize(deserializer)?;
    value
        .filter(|v| !v.is_empty())
        .map(|v| T::deserialize(v.into_deserializer()))
        .transpose()
}

impl FolderDevice {
    pub fn new(device_id: &DeviceId) -> Self {
        FolderDevice {
            device_id: device_id.clone(),
            introduced_by: None,
            encryption_password: None,
        }
    }
}

/// A folder to add (`POST /rest/config/folders`); the settings not given
/// take the folder defaults.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NewFolderRequest<'a> {
    pub id: &'a FolderId,
    pub label: &'a str,
    pub path: &'a str,
    #[serde(rename = "type")]
    pub folder_type: FolderType,
    pub devices: Vec<FolderDevice>,
}

/// Changes to a folder's settings (`PATCH /rest/config/folders/{id}`), or to
/// the folder defaults (`PATCH /rest/config/defaults/folder`): only the
/// fields given change.
#[derive(Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct FolderPatchRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    pub folder_type: Option<FolderType>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub paused: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub send_ownership: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub devices: Option<Vec<FolderDevice>>,
}

/// What a device does with a folder's files.
#[derive(Deserialize, Serialize, Clone, Copy, PartialEq, Eq, Debug)]
#[serde(rename_all = "lowercase")]
pub enum FolderType {
    SendReceive,
    SendOnly,
    ReceiveOnly,
    /// Holds other devices' encrypted data, which this device can't read.
    ReceiveEncrypted,
}

/// The names the web UI uses.
impl fmt::Display for FolderType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(match self {
            FolderType::SendReceive => "Send & Receive",
            FolderType::SendOnly => "Send Only",
            FolderType::ReceiveOnly => "Receive Only",
            FolderType::ReceiveEncrypted => "Receive Encrypted",
        })
    }
}

impl<S> Config<S> {
    /// The folder `id`, if it has it.
    pub fn folder(&self, id: &FolderId) -> Option<&Folder<S>> {
        self.folders.iter().find(|f| f.id == *id)
    }

    /// The folder with the ID or label `key`; several with that label are
    /// an error.
    fn find_by_id_or_label(&self, key: &str) -> Result<Option<&Folder<S>>> {
        if let Some(folder) = self.folders.iter().find(|f| f.id.as_str() == key) {
            return Ok(Some(folder));
        }
        let labelled: Vec<&Folder<S>> = self.folders.iter().filter(|f| f.label == key).collect();
        match labelled.as_slice() {
            [] => Ok(None),
            [folder] => Ok(Some(folder)),
            _ => bail!("several folders are labelled {key}; use the folder ID"),
        }
    }

    /// The device `id`, if it knows it.
    pub fn device(&self, id: &DeviceId) -> Option<&Device> {
        self.devices.iter().find(|d| d.id == *id)
    }

    /// The device named `key`, or with that device ID (or its first seven
    /// characters, at least); several matching are an error.
    pub fn find_device(&self, key: &str) -> Result<Option<&Device>> {
        // Device IDs are upper case; people type them any way.
        let upper = key.to_ascii_uppercase();
        let matches: Vec<&Device> = self
            .devices
            .iter()
            .filter(|d| {
                let id = d.id.as_str();
                d.name == key || id == upper || (upper.len() >= 7 && id.starts_with(&upper))
            })
            .collect();
        match matches.as_slice() {
            [] => Ok(None),
            [device] => Ok(Some(device)),
            _ => bail!("several devices match {key}; use the device ID"),
        }
    }

    /// What it calls the device `id`: its name, or the ID's first block.
    pub fn device_name(&self, id: &DeviceId) -> String {
        self.device(id)
            .map_or_else(|| id.short().to_string(), Device::display)
    }
}

impl<S: Side> Config<S> {
    /// The device a DEVICE argument `key` names: by name, device ID, or the
    /// ID's first seven characters at least.
    pub fn resolve_device(&self, key: &str) -> Result<&Device> {
        match self.find_device(key)? {
            Some(device) => Ok(device),
            None => bail!(
                "{} knows no device named, or with the device ID, {key}",
                S::OWNER
            ),
        }
    }
}

impl Config<Hub> {
    /// Where a new folder goes on the hub: its folder defaults directory,
    /// under the folder's ID - unique, unlike a label.
    pub fn new_folder_path(&self, id: &FolderId) -> String {
        let base = match self.defaults.folder.path.trim_end_matches('/') {
            "" => "~",
            path => path,
        };
        format!("{base}/{id}")
    }

    /// The hub's folder a FOLDER argument `key` names: by ID or label - or,
    /// given `local_config`, by the directory of a folder this device has,
    /// which the hub has under the same ID.
    pub fn resolve_folder(
        &self,
        local_config: Option<&Config<Local>>,
        key: &str,
    ) -> Result<&Folder<Hub>> {
        let local_folder = match local_config {
            Some(config) if Path::new(key).is_dir() => config.find_folder(key)?,
            _ => None,
        };
        if let Some(local_folder) = local_folder {
            return self.folder(&local_folder.id).with_context(|| {
                format!(
                    "the hub doesn't have {}, the folder at {key} (`synchub list` shows them)",
                    local_folder.name()
                )
            });
        }
        match self.find_folder(key)? {
            Some(folder) => Ok(folder),
            None => bail!(
                "the hub has no folder with the ID or label {key} (`synchub list` shows them)"
            ),
        }
    }

    /// The folder with the ID or label `key`; several with that label are
    /// an error.
    pub fn find_folder(&self, key: &str) -> Result<Option<&Folder<Hub>>> {
        self.find_by_id_or_label(key)
    }
}

impl Config<Local> {
    /// The folder a FOLDER argument `key` names: by its directory, ID or
    /// label.
    pub fn resolve_folder(&self, key: &str) -> Result<&Folder<Local>> {
        match self.find_folder(key)? {
            Some(folder) => Ok(folder),
            None => bail!(
                "this device has no folder at, or with the ID or label, {key} (`synchub list` shows them)"
            ),
        }
    }

    /// The folder at the directory `key`, or else with the ID or label
    /// `key`; several with that label are an error.
    pub fn find_folder(&self, key: &str) -> Result<Option<&Folder<Local>>> {
        if Path::new(key).is_dir() {
            let dir = paths::canon(Path::new(key));
            if let Some(folder) = self.folders.iter().find(|f| f.local_path() == dir) {
                return Ok(Some(folder));
            }
        }
        self.find_by_id_or_label(key)
    }

    /// The folders `path` is in, or at - the outermost first.
    pub fn enclosing(&self, path: &Path) -> Vec<&Folder<Local>> {
        let path = paths::canon(path);
        self.folders
            .iter()
            .filter(|f| paths::inside(&path, &f.local_path()))
            // They're inside one another, and a path sorts before what's inside it.
            .sorted_by_key(|f| f.local_path())
            .collect()
    }

    /// The folders at, around or inside `path`.
    pub fn related(&self, path: &Path) -> Vec<&Folder<Local>> {
        self.folders.iter().filter(|f| f.overlaps(path)).collect()
    }

    /// The innermost folder `path` is in, or at.
    pub fn folder_containing(&self, path: &Path) -> Option<&Folder<Local>> {
        self.enclosing(path).pop()
    }
}

impl<S> Folder<S> {
    pub fn shared_with_devices(&self) -> impl Iterator<Item = &DeviceId> {
        self.devices.iter().map(|d| &d.device_id)
    }

    pub fn shared_with_device(&self, id: &DeviceId) -> bool {
        self.shared_with_devices().any(|d| d == id)
    }

    /// The devices it's shared with that `introducer` didn't introduce:
    /// shared directly, not through it. This device included.
    pub fn direct_shares<'a>(
        &'a self,
        introducer: &'a DeviceId,
    ) -> impl Iterator<Item = &'a DeviceId> {
        self.devices
            .iter()
            .filter(move |d| d.introduced_by.as_ref() != Some(introducer))
            .map(|d| &d.device_id)
    }

    /// Holds other devices' encrypted data, which this device can't read:
    /// such a folder is left alone.
    pub fn is_receive_encrypted(&self) -> bool {
        self.folder_type == FolderType::ReceiveEncrypted
    }

    pub fn name(&self) -> String {
        if self.label.is_empty() {
            self.id.to_string()
        } else {
            format!("{} ({})", self.label, self.id)
        }
    }

    /// Its label, or its ID when it has none.
    pub fn label_or_id(&self) -> String {
        if self.label.is_empty() {
            self.id.to_string()
        } else {
            self.label.clone()
        }
    }
}

impl Folder<Local> {
    pub fn local_path(&self) -> PathBuf {
        paths::canon(&paths::expand_tilde(&self.path))
    }

    /// Whether `path` and this folder's directory overlap: one is the other,
    /// or lies inside it.
    pub fn overlaps(&self, path: &Path) -> bool {
        paths::related(path, &self.local_path())
    }
}

/// What `/rest/config/devices` returns.
pub type DeviceResponse = Device;

/// Another device, as a device's configuration has it.
#[derive(Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Device {
    #[serde(rename = "deviceID")]
    pub id: DeviceId,
    pub name: String,
    pub introducer: bool,
    pub auto_accept_folders: bool,
}

/// A device to add (`POST /rest/config/devices`); the settings not given
/// take Syncthing's defaults.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NewDeviceRequest<'a> {
    #[serde(rename = "deviceID")]
    pub id: &'a DeviceId,
    pub name: &'a str,
    pub addresses: Vec<String>,
    pub introducer: bool,
    pub auto_accept_folders: bool,
    pub skip_introduction_removals: bool,
}

/// Changes to a device's settings (`PATCH /rest/config/devices/{id}`): only
/// the fields given change.
#[derive(Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct DevicePatchRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub introducer: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auto_accept_folders: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub skip_introduction_removals: Option<bool>,
}

impl Device {
    /// The name, or the ID's first block when it has none.
    pub fn display(&self) -> String {
        if self.name.is_empty() {
            self.id.short().to_string()
        } else {
            self.name.clone()
        }
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;

    #[test]
    fn folder_types_are_written_as_syncthing_writes_them() {
        for (folder_type, json) in [
            (FolderType::SendReceive, "\"sendreceive\""),
            (FolderType::SendOnly, "\"sendonly\""),
            (FolderType::ReceiveOnly, "\"receiveonly\""),
            (FolderType::ReceiveEncrypted, "\"receiveencrypted\""),
        ] {
            assert_eq!(serde_json::to_string(&folder_type).unwrap(), json);
            assert_eq!(
                serde_json::from_str::<FolderType>(json).unwrap(),
                folder_type
            );
        }
    }

    fn folder<S>(id: &str, label: &str, path: &Path) -> Folder<S> {
        Folder {
            id: id.parse().unwrap(),
            label: label.to_string(),
            path: path.display().to_string(),
            folder_type: FolderType::SendReceive,
            paused: false,
            send_ownership: false,
            sync_ownership: false,
            devices: Vec::new(),
            side: PhantomData,
        }
    }

    fn config<S>(folders: Vec<Folder<S>>, devices: Vec<Device>) -> Config<S> {
        Config {
            folders,
            devices,
            defaults: Defaults {
                folder: folder("default", "", Path::new("/defaults")),
            },
        }
    }

    #[test]
    fn folders_are_found_by_directory_id_or_label() {
        let dir = std::env::temp_dir().join(format!("synchub-find-{}", std::process::id()));
        let inner = dir.join("inner");
        fs::create_dir_all(&inner).unwrap();
        let config: Config<Local> = config(
            vec![
                folder("aaaaa-11111", "Outer", &dir),
                folder("bbbbb-22222", "Inner", &inner),
                folder("ccccc-33333", "Twin", Path::new("/nowhere/1")),
                folder("ddddd-44444", "Twin", Path::new("/nowhere/2")),
            ],
            Vec::new(),
        );
        let found = |key: &str| config.resolve_folder(key).ok().map(|f| f.id.to_string());
        assert_eq!(
            found(&inner.display().to_string()).as_deref(),
            Some("bbbbb-22222")
        );
        let below = inner.join("sub");
        fs::create_dir_all(&below).unwrap();
        assert_eq!(found(&below.display().to_string()), None);
        assert_eq!(
            config
                .folder_containing(&below)
                .map(|f| f.id.to_string())
                .as_deref(),
            Some("bbbbb-22222")
        );
        let enclosing: Vec<String> = config
            .enclosing(&below)
            .iter()
            .map(|f| f.id.to_string())
            .collect();
        assert_eq!(enclosing, ["aaaaa-11111", "bbbbb-22222"]);
        let ids = |folders: Vec<&Folder<Local>>| {
            folders.iter().map(|f| f.id.to_string()).collect::<Vec<_>>()
        };
        // Around it, and inside it.
        assert_eq!(ids(config.related(&below)), ["aaaaa-11111", "bbbbb-22222"]);
        assert_eq!(ids(config.related(&dir)), ["aaaaa-11111", "bbbbb-22222"]);
        assert_eq!(
            ids(config.related(Path::new("/nowhere"))),
            ["ccccc-33333", "ddddd-44444"]
        );
        assert_eq!(found("aaaaa-11111").as_deref(), Some("aaaaa-11111"));
        assert_eq!(found("Outer").as_deref(), Some("aaaaa-11111"));
        assert_eq!(found("Nothing"), None);
        let twins = config.resolve_folder("Twin").err().unwrap().to_string();
        assert!(twins.contains("several folders"), "{twins}");
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn the_hubs_folder_is_found_by_a_directory_here() {
        let dir = std::env::temp_dir().join(format!("synchub-hub-{}", std::process::id()));
        let (shared, alone) = (dir.join("shared"), dir.join("alone"));
        fs::create_dir_all(&shared).unwrap();
        fs::create_dir_all(&alone).unwrap();
        let local_config: Config<Local> = config(
            vec![
                folder("aaaaa-11111", "Shared", &shared),
                folder("bbbbb-22222", "Alone", &alone),
            ],
            Vec::new(),
        );
        let hub_config: Config<Hub> = config(
            vec![folder("aaaaa-11111", "Shared", Path::new("/hub/Shared"))],
            Vec::new(),
        );
        let found = |key: &Path, local| {
            hub_config
                .resolve_folder(local, &key.display().to_string())
                .map(|f| f.path.clone())
        };
        assert_eq!(found(&shared, Some(&local_config)).unwrap(), "/hub/Shared");
        let alone_err = found(&alone, Some(&local_config)).unwrap_err().to_string();
        assert!(
            alone_err.contains("the hub doesn't have Alone"),
            "{alone_err}"
        );
        assert!(found(&shared, None).is_err());
        assert_eq!(
            hub_config
                .resolve_folder(None, "Shared")
                .unwrap()
                .id
                .as_str(),
            "aaaaa-11111"
        );
        fs::remove_dir_all(&dir).unwrap();
    }

    /// A well-formed device ID starting with the group `first`, then `rest`s.
    fn id(first: &str, rest: char) -> String {
        let group: String = std::iter::repeat_n(rest, 7).collect();
        std::iter::once(first.to_string())
            .chain(std::iter::repeat_n(group, 7))
            .collect::<Vec<_>>()
            .join("-")
    }

    fn device(id: &str, name: &str) -> Device {
        Device {
            id: id.parse().unwrap(),
            name: name.to_string(),
            introducer: false,
            auto_accept_folders: false,
        }
    }

    #[test]
    fn devices_are_found_by_name_id_or_id_prefix() {
        let config: Config<Local> = config(
            Vec::new(),
            vec![
                device(&id("AAAAAAA", '2'), "laptop"),
                device(&id("AAAAAAA", '3'), "phone"),
                device(&id("BBBBBBB", '2'), "laptop2"),
            ],
        );
        let found = |key: &str| config.find_device(key).unwrap().map(|d| d.name.clone());
        assert_eq!(found("laptop").as_deref(), Some("laptop"));
        assert_eq!(found(&id("AAAAAAA", '3')).as_deref(), Some("phone"));
        assert_eq!(found("BBBBBBB").as_deref(), Some("laptop2"));
        assert_eq!(found("bbbbbbb").as_deref(), Some("laptop2"));
        assert_eq!(found("BBBBBB"), None);
        assert_eq!(found("tablet"), None);
        assert!(config.find_device("AAAAAAA").is_err());
    }

    #[test]
    fn direct_shares_are_those_the_introducer_didnt_introduce() {
        let [me, hub, friend, stranger] = ["AAAAAAA", "HHHHHHH", "FFFFFFF", "SSSSSSS"]
            .map(|first| -> DeviceId { id(first, '2').parse().unwrap() });
        let mut folder: Folder<Local> = folder("f", "", Path::new("/f"));
        folder.devices = [&me, &hub, &friend]
            .map(FolderDevice::new)
            .into_iter()
            .chain([FolderDevice {
                introduced_by: Some(hub.clone()),
                ..FolderDevice::new(&stranger)
            }])
            .collect();
        let direct: Vec<&DeviceId> = folder.direct_shares(&hub).collect();
        assert_eq!(direct, [&me, &hub, &friend]);
    }

    #[test]
    fn empty_folder_device_fields_are_none() {
        let (me, hub) = (id("AAAAAAA", 'A'), id("HHHHHHH", 'H'));
        let empty: FolderDevice = serde_json::from_str(&format!(
            r#"{{"deviceID": "{me}", "introducedBy": "", "encryptionPassword": ""}}"#
        ))
        .unwrap();
        assert_eq!(empty, FolderDevice::new(&me.parse().unwrap()));
        assert_eq!(
            serde_json::to_string(&empty).unwrap(),
            format!(r#"{{"deviceID":"{me}"}}"#)
        );

        let json = format!(
            r#"{{"deviceID":"{me}","introducedBy":"{hub}","encryptionPassword":"secret"}}"#
        );
        let set: FolderDevice = serde_json::from_str(&json).unwrap();
        assert_eq!(
            set.introduced_by.as_ref().map(DeviceId::as_str),
            Some(hub.as_str())
        );
        assert_eq!(set.encryption_password.as_deref(), Some("secret"));
        assert_eq!(serde_json::to_string(&set).unwrap(), json);
    }
}
