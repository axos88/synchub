//! `sttools mv` (syncmv): move a synced directory and repoint its Syncthing folders.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result, bail};
use serde_json::json;

use crate::paths;
use crate::syncthing::{Client, Folder};

/// Move a directory that Syncthing syncs, and tell Syncthing about it.
///
/// SOURCE is a local path; mv finds the Syncthing folder stored there by
/// itself, and any folders nested inside it. It pauses them, moves SOURCE to
/// DEST the way mv does (if DEST is an existing directory, SOURCE goes inside
/// it), points the folders at the new place, and resumes them. Nothing is
/// re-synced: the other devices don't notice the move.
///
/// Example: syncmv ~/projects/ruby/heroku ~/work/heroku
#[derive(clap::Args)]
pub struct Args {
    /// The directory to move
    source: PathBuf,
    /// Where to move it; if it's an existing directory, SOURCE goes inside it
    dest: PathBuf,
    /// Show what would happen, change nothing
    #[arg(short = 'n', long)]
    dry_run: bool,
    /// Allow moving out of, or into, a directory another Syncthing folder
    /// syncs (that folder will see the files disappear or appear)
    #[arg(short, long)]
    force: bool,
}

struct Repoint {
    folder: Folder,
    new_path: String,
}

pub fn run(client: &Client, args: Args) -> Result<()> {
    if !args.source.is_dir() {
        bail!("{}: not a directory", args.source.display());
    }
    let src = paths::canon(&args.source);
    let target = if args.dest.is_dir() {
        paths::canon(&args.dest).join(src.file_name().context("can't move /")?)
    } else {
        paths::canon(&args.dest)
    };
    if target.exists() {
        bail!("{} already exists", target.display());
    }
    let parent = target.parent().context("no parent directory")?;
    if !parent.is_dir() {
        bail!("{}: no such directory", parent.display());
    }
    if paths::inside(&target, &src) {
        bail!("can't move {} into itself", src.display());
    }

    let mut repoints = Vec::new();
    for folder in client.folders()? {
        let path = folder.local_path();
        if paths::inside(&path, &src) {
            let rel = path.strip_prefix(&src)?;
            let new = if rel.as_os_str().is_empty() {
                target.clone()
            } else {
                target.join(rel)
            };
            repoints.push(Repoint {
                new_path: paths::config_form(&folder.path, &new),
                folder,
            });
        } else if !args.force {
            for end in [&src, &target] {
                if paths::inside(end, &path) {
                    bail!(
                        "{} is inside Syncthing folder {} ({}); use -f to move it anyway",
                        end.display(),
                        folder.name(),
                        path.display()
                    );
                }
            }
        }
    }
    if repoints.is_empty() {
        bail!(
            "{} is not a Syncthing folder and contains none",
            src.display()
        );
    }

    for r in &repoints {
        println!("{}: {} -> {}", r.folder.name(), r.folder.path, r.new_path);
    }
    println!("mv {} {}", src.display(), target.display());
    if args.dry_run {
        return Ok(());
    }

    let set_paused = |paused: bool| -> Result<()> {
        for r in repoints.iter().filter(|r| !r.folder.paused) {
            client.update_folder(&r.folder.id, json!({ "paused": paused }))?;
        }
        Ok(())
    };
    if let Err(err) = set_paused(true).and_then(|()| move_dir(&src, &target)) {
        eprintln!("sttools: nothing was moved; resuming the folders");
        set_paused(false).context("resuming the folders")?;
        return Err(err);
    }
    for r in &repoints {
        client
            .update_folder(&r.folder.id, json!({ "path": r.new_path }))
            .context("failed after the move; the folders stay paused. Fix their paths in the web UI, then resume them")?;
    }
    set_paused(false)?;
    println!("done");
    Ok(())
}

fn move_dir(src: &Path, target: &Path) -> Result<()> {
    match fs::rename(src, target) {
        Ok(()) => Ok(()),
        // Another filesystem: let mv copy it across.
        Err(err) if err.kind() == io::ErrorKind::CrossesDevices => {
            let status = Command::new("mv").arg("--").arg(src).arg(target).status()?;
            if !status.success() {
                bail!("mv {} {} failed", src.display(), target.display());
            }
            Ok(())
        }
        Err(err) => Err(err)
            .with_context(|| format!("can't move {} to {}", src.display(), target.display())),
    }
}
