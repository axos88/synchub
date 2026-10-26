//! `synchub new`: share a directory of this device through the hub.

use std::path::{Path, PathBuf};

use anyhow::{Result, bail};
use itertools::Itertools;

use crate::api;
use crate::client;
use crate::commands::{Arguments, option, sync};
use crate::device_id::DeviceId;
use crate::folder_id::FolderId;
use crate::paths;
use crate::side::{Hub, Local};

/// Share a directory of this device through the hub.
///
/// The directory becomes a folder here, shared with the hub only. On the
/// hub, it goes in the folder defaults' directory under its folder ID, shared
/// with this device and the devices given, which get it automatically.
///
/// Example: synchub new ~/projects/notes --label Notes --device laptop
#[derive(clap::Args, Debug, PartialEq)]
pub struct Args {
    /// The directory to share
    pub path: PathBuf,
    /// The folder's label [default: none - like the web UI, the folder then
    /// shows by its ID]
    #[arg(long)]
    pub label: Option<String>,
    /// A device to share it with - its name on this device, or its device
    /// ID; repeat for more
    #[arg(long = "device")]
    pub devices: Vec<String>,
}

impl Arguments for Args {
    fn arguments(&self) -> Vec<String> {
        std::iter::once(self.path.display().to_string())
            .chain(option("--label", self.label.as_ref()))
            .chain(
                self.devices
                    .iter()
                    .flat_map(|device| ["--device".to_string(), device.clone()]),
            )
            .collect()
    }
}

pub fn run(args: Args) -> Result<()> {
    let hub_config = client::hub()?.config()?;
    let devices = args
        .devices
        .iter()
        .map(|key| Ok(hub_config.resolve_device(key)?.id.clone()))
        .collect::<Result<Vec<_>>>()?;
    create(&args.path, args.label.as_deref(), &devices)
}

/// Shares the directory `path`, labelled `label` (or not, as the web UI
/// does without one): here with the hub only; on the hub with this device
/// and `devices`.
pub fn create(path: &Path, label: Option<&str>, devices: &[DeviceId]) -> Result<()> {
    let (local, hub) = (client::local()?, client::hub()?);
    let (local_config, hub_config) = (local.config()?, hub.config()?);
    let path = existing_directory(path)?;
    check_no_overlap(&local_config, &path)?;
    let label = label.unwrap_or_default();
    let id = new_id(&local_config, &hub_config);
    let hub_path = hub_config.new_folder_path(&id);
    let my_id = local.my_id()?;
    let config_path = paths::relative_to_home(&path);
    let hub_devices: Vec<&DeviceId> = [hub.id(), &my_id]
        .into_iter()
        .chain(devices)
        .unique()
        .collect();

    // Here first: once the hub shares it with this device, the hub's offer
    // would otherwise be auto-accepted into the folder defaults' directory.
    local.add_folder(&id, label, &config_path, &[&my_id, hub.id()])?;
    hub.add_folder(&id, label, &hub_path, &hub_devices)?;
    include_shared_ignores(&id)
}

/// Has the new folder `id` include its shared ignore list.
fn include_shared_ignores(id: &FolderId) -> Result<()> {
    // A dry run added nothing.
    let Some(folder) = client::local()?.find_folder(id)? else {
        return Ok(());
    };
    sync::include_shared_ignores(&folder)?;
    Ok(())
}

/// `path`, in full; fails unless it's a directory.
fn existing_directory(path: &Path) -> Result<PathBuf> {
    let path = paths::canon(&paths::expand_tilde(&path.to_string_lossy()));
    if !path.is_dir() {
        bail!(
            "{} isn't a directory; PATH must be the directory to share",
            path.display()
        );
    }
    Ok(path)
}

/// Fails if `path` overlaps a folder this device syncs: Syncthing doesn't
/// support nesting folders.
fn check_no_overlap(local_config: &api::config::Config<Local>, path: &Path) -> Result<()> {
    if let Some(other) = local_config.folders.iter().find(|f| f.overlaps(path)) {
        bail!(
            "{} overlaps {} at {}, which this device syncs already; choose another directory",
            path.display(),
            other.name(),
            other.local_path().display()
        );
    }
    Ok(())
}

/// A new folder ID, not yet in use here or on the hub.
fn new_id(
    local_config: &api::config::Config<Local>,
    hub_config: &api::config::Config<Hub>,
) -> FolderId {
    loop {
        let id = FolderId::generate();
        let taken = local_config.folder(&id).is_some() || hub_config.folder(&id).is_some();
        if !taken {
            return id;
        }
    }
}
