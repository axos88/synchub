//! `sttools put` (syncput): push a local directory into an existing Syncthing folder.

use std::fs;
use std::path::PathBuf;
use std::thread::sleep;
use std::time::Duration;

use anyhow::{Context, Result, bail};
use serde_json::json;

use crate::paths;
use crate::prompt;
use crate::syncthing::Client;

/// Push a local directory into an existing Syncthing folder, replacing every
/// other device's copy.
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
/// Example: syncput ~/projects/ruby/heroku shy4a-ady3g
#[derive(clap::Args)]
pub struct Args {
    /// The directory whose contents the folder gets
    local_dir: PathBuf,
    /// The existing Syncthing folder's ID (see the web UI, or `syncthing cli config folders list`)
    folder_id: String,
    /// Show what would happen, change nothing
    #[arg(short = 'n', long)]
    dry_run: bool,
    /// Allow LOCAL_DIR inside, or containing, another Syncthing folder
    #[arg(short, long)]
    force: bool,
    /// Don't ask for confirmation
    #[arg(short, long)]
    yes: bool,
}

pub fn run(client: &Client, args: Args) -> Result<()> {
    if !args.local_dir.is_dir() {
        bail!("{}: not a directory", args.local_dir.display());
    }
    let src = paths::canon(&args.local_dir);
    let folders = client.folders()?;
    let folder = folders
        .iter()
        .find(|f| f.id == args.folder_id)
        .with_context(|| format!("{}: no such Syncthing folder", args.folder_id))?;
    if folder.kind == "receiveencrypted" {
        bail!(
            "{} is receive-encrypted; its type can't be changed",
            folder.name()
        );
    }
    for other in folders.iter().filter(|f| f.id != folder.id) {
        let path = other.local_path();
        if path == src {
            bail!(
                "{} is already Syncthing folder {}; remove that folder first",
                src.display(),
                other.name()
            );
        }
        if !args.force && (paths::inside(&src, &path) || paths::inside(&path, &src)) {
            bail!(
                "{} overlaps Syncthing folder {} ({}); use -f to push anyway",
                src.display(),
                other.name(),
                path.display()
            );
        }
    }

    let old = folder.local_path();
    let new_path = paths::config_form(&folder.path, &src);
    println!("{}: {} -> {}", folder.name(), folder.path, new_path);
    println!(
        "Every device sharing {} gets its copy replaced by the contents of {}.",
        folder.name(),
        src.display()
    );
    if old != src {
        println!("{} is left as it is.", old.display());
    }
    if args.dry_run {
        return Ok(());
    }
    if !args.yes {
        match prompt::confirm("Continue?")? {
            Some(true) => {}
            Some(false) => bail!("aborted"),
            None => bail!("not a terminal; use -y to confirm"),
        }
    }
    if folder.send_ownership || folder.sync_ownership {
        eprintln!(
            "sttools: warning: ownership is already synced, so files whose size and mtime match the old directory's may not be re-read"
        );
    }

    let id = folder.id.as_str();
    let restore = json!({ "type": folder.kind, "sendOwnership": folder.send_ownership, "paused": folder.paused });
    let prepare = || -> Result<()> {
        client.update_folder(id, json!({ "paused": true }))?;
        // Ownership the stored index doesn't have makes the scan re-read every
        // file instead of trusting matching size and mtime.
        client.update_folder(id, json!({ "type": "sendonly", "sendOwnership": true }))?;
        fs::create_dir_all(src.join(".stfolder")).context("creating the .stfolder marker")?;
        Ok(())
    };
    if let Err(err) = prepare() {
        eprintln!("sttools: nothing was pushed; restoring {}", folder.name());
        client
            .update_folder(id, restore)
            .context("restoring the folder's settings")?;
        return Err(err);
    }

    let push = || -> Result<()> {
        client.update_folder(id, json!({ "path": new_path, "paused": false }))?;
        println!("scanning {} ...", src.display());
        client.wait_idle(id)?;
        client.scan(id)?;
        client.wait_idle(id)?;
        println!("overriding the other devices' changes ...");
        client.override_changes(id)?;
        sleep(Duration::from_secs(2));
        client.wait_idle(id)?;
        Ok(())
    };
    if let Err(err) = push() {
        let _ = client.update_folder(id, json!({ "paused": true }));
        return Err(err.context(format!(
            "failed after repointing; {} is left paused at {new_path} as send-only (was {}), with send ownership on. Check it in the web UI",
            folder.name(),
            folder.kind
        )));
    }

    client.update_folder(id, restore)?;
    let status = client.status(id)?;
    println!(
        "done: {} files pushed; the other devices update as they connect",
        status.local_files
    );
    Ok(())
}
