//! `synchub overwrite`: replace a Syncthing folder's contents everywhere with a local directory.

use std::fs;
use std::path::{Path, PathBuf};
use std::thread::sleep;
use std::time::Duration;

use anyhow::{Context, Result, bail};

use crate::api;
use crate::client;
use crate::commands::{Arguments, flag};
use crate::folder_id::FolderId;
use crate::paths;
use crate::settings;
use crate::side::Local;
use crate::{say, warn};

/// Replace an existing Syncthing folder's contents on every device with a
/// local directory.
///
/// The folder FOLDER_ID is pointed at LOCAL_DIR, and every device sharing it
/// gets its copy replaced by LOCAL_DIR's contents: changed files are
/// overwritten, files LOCAL_DIR doesn't have are deleted there. LOCAL_DIR
/// itself is never modified (only a .stfolder marker is added if missing).
/// Files LOCAL_DIR's .stignore ignores are left alone on the other devices.
///
/// The folder's previous directory on this machine is left untouched; delete
/// it yourself once you no longer need it.
///
/// How: pause the folder, point it at LOCAL_DIR as send-only (so nothing can
/// be downloaded into it), resume and scan, "Override Changes" so this
/// device's files win everywhere, then switch the folder back to its original
/// type. The scan runs with "send ownership" on, so that every file is
/// re-read even where size and mtime match the old directory's; that setting
/// is restored too.
///
/// Example: synchub overwrite ~/projects/website shy4a-ady3g
#[derive(clap::Args, Debug, PartialEq)]
pub struct Args {
    /// The directory whose contents the folder gets
    pub local_dir: PathBuf,
    /// The existing Syncthing folder's ID (see the web UI, or `syncthing cli config folders list`)
    pub folder_id: FolderId,
    /// Allow LOCAL_DIR inside, or containing, another Syncthing folder
    #[arg(short, long)]
    pub force: bool,
}

impl Arguments for Args {
    fn arguments(&self) -> Vec<String> {
        [
            self.local_dir.display().to_string(),
            self.folder_id.to_string(),
        ]
        .into_iter()
        .chain(flag(self.force, "--force"))
        .collect()
    }
}

pub fn run(args: Args) -> Result<()> {
    let client = client::local()?;
    let folder = client.folder(&args.folder_id).with_context(|| {
        format!(
            "this device has no Syncthing folder with the ID {} (`synchub list` shows the folders)",
            args.folder_id
        )
    })?;
    if !args.local_dir.is_dir() {
        bail!(
            "{} isn't a directory; LOCAL_DIR must be the directory whose contents the folder gets",
            args.local_dir.display()
        );
    }
    let src = paths::canon(&args.local_dir);
    overwrite(&src, &folder, args.force)
}

/// Replaces `folder`'s contents on every device sharing it with the
/// directory `src`'s, pointing the folder at `src`.
pub fn overwrite(src: &Path, folder: &api::config::Folder<Local>, force: bool) -> Result<()> {
    let client = client::local()?;
    if folder.is_receive_encrypted() {
        bail!(
            "{} is receive-encrypted: it holds other devices' encrypted data, so it can't be made send-only to push this device's files",
            folder.name()
        );
    }
    if let Some(other) = other_folder_at(folder, src)? {
        bail!(
            "{} is already the directory of the Syncthing folder {}, and a directory can belong to one folder only; remove {} first, or push another directory",
            src.display(),
            other.name(),
            other.name()
        );
    }
    if !force && let Some(other) = overlapping(folder, src)? {
        bail!(
            "{} overlaps the Syncthing folder {} at {}, which would sync some of the same files; use -f to push anyway",
            src.display(),
            other.name(),
            other.path
        );
    }
    let new_path = src.display().to_string();
    if folder.send_ownership || folder.sync_ownership {
        warn!(
            "ownership is already synced, so files whose size and mtime match the old directory's may not be re-read"
        );
    }

    if let Err(err) = prepare(folder, src) {
        return give_up(folder, err);
    }
    if let Err(err) = push(folder, &new_path) {
        let _ = client.pause(folder);
        return Err(err.context(format!(
            "the push failed after {} was pointed at {new_path}: it's left there paused, send-only and sending ownership; in the web UI, set it back to {}{} and resume it",
            folder.name(),
            folder.folder_type,
            if folder.send_ownership {
                ""
            } else {
                " without sending ownership"
            }
        )));
    }
    finalize(folder)?;
    let files = client.status(&folder.id)?.local_files;
    say!(
        "done: {} holds {files} files; the other devices update as they connect",
        folder.name()
    );
    let old = folder.local_path();
    if old != src {
        say!(
            "{}, its directory until now, stays as it is; delete it once you don't need it",
            old.display()
        );
    }
    Ok(())
}

/// The folder other than `folder` that's at `src` already, if there is one.
fn other_folder_at(
    folder: &api::config::Folder<Local>,
    src: &Path,
) -> Result<Option<api::config::Folder<Local>>> {
    let local_config = client::local()?.config()?;
    Ok(local_config
        .find_folder(&src.to_string_lossy())?
        .filter(|other| other.id != folder.id)
        .cloned())
}

/// A folder other than `folder` that `src` is inside, or that is inside `src`.
fn overlapping(
    folder: &api::config::Folder<Local>,
    src: &Path,
) -> Result<Option<api::config::Folder<Local>>> {
    let client = client::local()?;
    Ok(client
        .folders()?
        .into_iter()
        .filter(|other| other.id != folder.id)
        .find(|other| other.overlaps(src)))
}

/// Pauses the folder and makes it send-only, so nothing can be downloaded
/// into `src` meanwhile.
fn prepare(folder: &api::config::Folder<Local>, src: &Path) -> Result<()> {
    let client = client::local()?;
    client.pause(folder)?;
    // Ownership the stored index doesn't have makes the scan re-read every
    // file instead of trusting matching size and mtime.
    client.set_folder_type(folder, api::config::FolderType::SendOnly, true)?;
    if !settings::dry_run() {
        let marker = src.join(".stfolder");
        fs::create_dir_all(&marker)
            .with_context(|| format!("can't create Syncthing's marker {}", marker.display()))?;
    }
    Ok(())
}

/// Stops before anything was pushed: puts the folder's settings back, and
/// fails with `err`.
fn give_up(folder: &api::config::Folder<Local>, err: anyhow::Error) -> Result<()> {
    let client = client::local()?;
    warn!("nothing was pushed; restoring {}", folder.name());
    client
        .set_folder_type(folder, folder.folder_type, folder.send_ownership)
        .with_context(|| {
            format!(
                "can't restore {}: set it back to {} in the web UI",
                folder.name(),
                folder.folder_type
            )
        })?;
    if !folder.paused {
        client
            .resume(folder)
            .with_context(|| format!("can't resume {}; resume it in the web UI", folder.name()))?;
    }
    Err(err)
}

/// Points the folder at its new directory, scans it, and has its files win
/// on every device.
fn push(folder: &api::config::Folder<Local>, new_path: &str) -> Result<()> {
    let client = client::local()?;
    client.set_path(folder, new_path)?;
    client.resume(folder)?;
    client.wait_idle(folder)?;
    client.scan(folder)?;
    client.wait_idle(folder)?;
    client.override_changes(folder)?;
    sleep(Duration::from_secs(2));
    client.wait_idle(folder)?;
    Ok(())
}

/// Puts back the folder's type and ownership setting, and pauses it again
/// if it was paused.
fn finalize(folder: &api::config::Folder<Local>) -> Result<()> {
    let client = client::local()?;
    client.set_folder_type(folder, folder.folder_type, folder.send_ownership)?;
    if folder.paused {
        client.pause(folder)?;
    }
    Ok(())
}
