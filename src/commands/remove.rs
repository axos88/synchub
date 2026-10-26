//! `synchub remove`: drop a share from this device.

use std::fs;
use std::path::Path;
use std::time::Duration;

use anyhow::{Context, Result, bail};

use crate::api;
use crate::client;
use crate::commands::{Arguments, flag};
use crate::folder_id::FolderId;
use crate::paths;
use crate::settings;
use crate::side::{Hub, Local};
use crate::{say, step, warn};

/// Remove a folder from this device and from the hub.
///
/// The folder is removed on the hub and here. The other devices aren't
/// reachable from here: they keep their copy, files and all, which the hub
/// no longer offers them.
///
/// With --delete-files, the folder is emptied everywhere first: its files
/// are deleted here, and synchub waits until that deletion has reached the
/// hub and every connected device sharing the folder, so their copies are
/// emptied too; then it's removed, and its directory here deleted. A device
/// that isn't connected keeps its files.
///
/// Example: synchub remove Website
///
/// Example: synchub remove ~/projects/website --delete-files
#[derive(clap::Args, Debug, PartialEq)]
pub struct Args {
    /// The folder: its directory, ID or label on this device - or the ID of
    /// one only the hub has
    pub folder: String,
    /// Also delete the files: on every connected device sharing the folder,
    /// and its directory here
    #[arg(long)]
    pub delete_files: bool,
}

impl Arguments for Args {
    fn arguments(&self) -> Vec<String> {
        std::iter::once(self.folder.clone())
            .chain(flag(self.delete_files, "--delete-files"))
            .collect()
    }
}

pub fn run(args: Args) -> Result<()> {
    let (local_config, hub_config) = (client::local()?.config()?, client::hub()?.config()?);
    let folder = hub_config.resolve_folder(Some(&local_config), &args.folder)?;
    remove(&folder.id, args.delete_files)
}

/// How long a device may take to receive the deletion of a folder's files.
const CATCH_UP: Duration = Duration::from_secs(600);

/// Removes the folder `id` from the hub and from this device - emptying it
/// everywhere first, and deleting its directory here, if `delete_files`.
pub fn remove(id: &FolderId, delete_files: bool) -> Result<()> {
    let (local, hub) = (client::local()?, client::hub()?);
    let hub_folder = hub.find_folder(id)?;
    let local_folder = local.find_folder(id)?;
    if hub_folder.is_none() && local_folder.is_none() {
        bail!("neither the hub nor this device has a folder {id} (`synchub list` shows them)");
    }
    if delete_files && let Some(folder) = &local_folder {
        empty_everywhere(folder, hub_folder.as_ref())?;
    }
    if let Some(folder) = &hub_folder {
        hub.remove_folder(folder)?;
    }
    if let Some(folder) = &local_folder {
        local.remove_folder(folder)?;
        let dir = folder.local_path();
        if delete_files {
            delete_dir(&dir)?;
        } else {
            say!(Local, "its files stay in {}", dir.display());
        }
    }
    Ok(())
}

/// Deletes `folder`'s files here, and waits until the deletion has reached
/// the hub and every connected device sharing it - emptying their copies.
fn empty_everywhere(
    folder: &api::config::Folder<Local>,
    hub_folder: Option<&api::config::Folder<Hub>>,
) -> Result<()> {
    let (local, hub) = (client::local()?, client::hub()?);
    if !spreads_deletions(folder) {
        bail!(
            "{} is {}: files deleted here wouldn't be deleted on the other devices; remove it without --delete-files",
            folder.name(),
            folder.folder_type
        );
    }
    if let Some(inner) = nested_folder(&local.folders()?, folder) {
        bail!(
            "{} is inside {}, so emptying {} would empty it too; remove {} first",
            inner.name(),
            folder.name(),
            folder.name(),
            inner.name()
        );
    }
    delete_contents(folder)?;
    local.scan(folder)?;
    local.wait_idle(folder)?;
    let Some(hub_folder) = hub_folder else {
        return Ok(());
    };
    local.wait_synced(folder, hub.id(), CATCH_UP)?;
    let my_id = local.my_id()?;
    let connected = hub.connected_devices()?;
    let others = hub_folder
        .shared_with_devices()
        .filter(|id| *id != hub.id() && **id != my_id);
    for device in others {
        if connected.contains(device) {
            hub.wait_synced(hub_folder, device, CATCH_UP)?;
        } else {
            warn!(
                "{} isn't connected, so its copy of {} keeps the files",
                local.device_names(&[device]),
                folder.name()
            );
        }
    }
    Ok(())
}

/// Whether files deleted here are deleted on the other devices too.
fn spreads_deletions(folder: &api::config::Folder<Local>) -> bool {
    matches!(
        folder.folder_type,
        api::config::FolderType::SendReceive | api::config::FolderType::SendOnly
    )
}

/// Another folder of this device's inside `folder`.
fn nested_folder<'f>(
    local_folders: &'f [api::config::Folder<Local>],
    folder: &api::config::Folder<Local>,
) -> Option<&'f api::config::Folder<Local>> {
    let dir = folder.local_path();
    local_folders
        .iter()
        .find(|other| other.id != folder.id && paths::inside(&other.local_path(), &dir))
}

/// Deletes everything in `folder`'s directory but Syncthing's own
/// `.stfolder` marker and `.stignore`.
fn delete_contents(folder: &api::config::Folder<Local>) -> Result<()> {
    let dir = folder.local_path();
    if settings::dry_run() {
        say!(Local, "would delete the files in {}", dir.display());
        return Ok(());
    }
    step!(Local, "deleting the files in {}", dir.display());
    let entries = fs::read_dir(&dir).with_context(|| format!("can't read {}", dir.display()))?;
    for entry in entries {
        let path = entry?.path();
        if matches!(base_name(&path), ".stfolder" | ".stignore") {
            continue;
        }
        let deleted = if path.is_dir() && !path.is_symlink() {
            fs::remove_dir_all(&path)
        } else {
            fs::remove_file(&path)
        };
        deleted.with_context(|| format!("can't delete {}", path.display()))?;
    }
    Ok(())
}

fn base_name(path: &Path) -> &str {
    path.file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default()
}

fn delete_dir(dir: &Path) -> Result<()> {
    if settings::dry_run() {
        say!(Local, "would delete {}", dir.display());
        return Ok(());
    }
    step!(Local, "deleting {}", dir.display());
    fs::remove_dir_all(dir).with_context(|| format!("can't delete {}", dir.display()))
}
