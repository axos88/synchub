//! `synchub mv`: move a Syncthing folder's directory, and repoint it and the
//! folders nested inside it.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result, bail};

use crate::api;
use crate::client;
use crate::paths;
use crate::settings;
use crate::side::Local;
use crate::{say, step, warn};

/// Move a directory that Syncthing syncs, and tell Syncthing about it.
///
/// SOURCE is a Syncthing folder: its directory, ID or label. mv pauses it and
/// any folders nested inside it, moves the directory to DEST, points the
/// folders at the new place, and resumes them. Nothing is re-synced: the
/// other devices don't notice the move.
///
/// As with mv, a DEST directory with something in it gets the directory
/// inside, under its own name. Otherwise DEST is the new path: one that
/// doesn't exist yet, or an empty directory, which the move replaces.
///
/// Example: synchub mv ~/projects/website ~/work/website
///
/// Example: synchub mv shy4a-ady3g ~/work/website
#[derive(clap::Args)]
pub struct Args {
    /// The Syncthing folder to move: its directory, ID or label
    source: String,
    /// Where to move it: into this directory if it has something in it,
    /// otherwise to this path
    dest: PathBuf,
    /// Allow moving out of, or into, a directory another Syncthing folder
    /// syncs (that folder will see the files disappear or appear)
    #[arg(short, long)]
    force: bool,
}

pub fn run(args: Args) -> Result<()> {
    let config = client::local()?.config()?;
    let folder = config.resolve_folder(&args.source)?;
    move_recursive(folder, &args.dest, args.force)
}

/// Moves `folder`'s directory into `dest` if that's a directory with
/// something in it, otherwise to `dest` (replacing an empty directory
/// there), repointing it and the folders nested inside it. In a dry run, it
/// only reports what it would do.
pub fn move_recursive(folder: &api::config::Folder<Local>, dest: &Path, force: bool) -> Result<()> {
    let client = client::local()?;
    let src = folder.local_path();
    // The config can point at a directory that's gone, or at a file.
    if !src.is_dir() {
        bail!(
            "folder {} is at {}, which isn't a directory",
            folder.name(),
            src.display()
        );
    }
    let dest = target(&src, dest)?;
    if !force && path_within_share(&src)? {
        bail!(
            "{} is inside another Syncthing folder, which would see its files disappear; use -f to move it anyway",
            src.display()
        );
    }
    if !force && path_within_share(&dest)? {
        bail!(
            "{} is inside another Syncthing folder, which would see the files appear; use -f to move it there anyway",
            dest.display()
        );
    }
    let folders = folders_inside(&src)?;

    // One rename moves them all, so they all pause first. Folders paused
    // already stay paused.
    let running: Vec<&api::config::Folder<Local>> = folders.iter().filter(|f| !f.paused).collect();
    for folder in &running {
        if let Err(err) = client.pause(folder) {
            return give_up(&running, err);
        }
    }
    if let Err(err) = move_dir(&src, &dest) {
        return give_up(&running, err);
    }
    // Each folder on its own: one failing mustn't leave the rest behind.
    let failed: Vec<(String, anyhow::Error)> = folders
        .iter()
        .map(|folder| (folder.name(), settle(folder, &src, &dest)))
        .filter_map(|(name, result)| result.err().map(|err| (name, err)))
        .collect();
    if !failed.is_empty() {
        bail!(
            "the directory moved, but these folders need fixing in the web UI: {}",
            warn_all(&failed)
        );
    }
    say!("done");
    Ok(())
}

/// Where `src` goes for `dest`, the way mv does it: into a directory with
/// something in it, otherwise to `dest` itself - where only an empty
/// directory may be.
fn target(src: &Path, dest: &Path) -> Result<PathBuf> {
    let dest = paths::canon(dest);
    let path = if dest.is_dir() && !empty_dir(&dest)? {
        dest.join(src.file_name().context("can't move /")?)
    } else {
        dest
    };
    if path.exists() && !empty_dir(&path)? {
        bail!(
            "{} already exists and isn't an empty directory; choose another destination",
            path.display()
        );
    }
    let parent = path.parent().context("no parent directory")?;
    if !parent.is_dir() {
        bail!(
            "{} doesn't exist; create it first, or choose another destination",
            parent.display()
        );
    }
    if paths::inside(&path, src) {
        bail!("can't move {} into itself", src.display());
    }
    Ok(path)
}

fn empty_dir(dir: &Path) -> Result<bool> {
    Ok(dir.is_dir()
        && paths::is_empty_dir(dir).with_context(|| format!("can't read {}", dir.display()))?)
}

/// Whether `path` is inside a Syncthing folder other than one at `path` itself.
fn path_within_share(path: &Path) -> Result<bool> {
    match path.parent() {
        Some(parent) => Ok(client::local()?
            .config()?
            .folder_containing(parent)
            .is_some()),
        None => Ok(false),
    }
}

/// The folders at or inside `dir`.
fn folders_inside(dir: &Path) -> Result<Vec<api::config::Folder<Local>>> {
    let client = client::local()?;
    Ok(client
        .folders()?
        .into_iter()
        .filter(|f| paths::inside(&f.local_path(), dir))
        .collect())
}

/// Moves the directory to `dest`, where [`target`] allows only an empty
/// directory: that makes way, and is put back if the move fails. (rename
/// would replace it on Unix, but not on Windows, and mv would move into it.)
fn move_dir(src: &Path, dest: &Path) -> Result<()> {
    if settings::dry_run() {
        say!(Local, "would move {} to {}", src.display(), dest.display());
        return Ok(());
    }
    step!(Local, "moving {} to {}", src.display(), dest.display());
    let replaced = dest.is_dir();
    if replaced {
        fs::remove_dir(dest).with_context(|| format!("can't remove {}", dest.display()))?;
    }
    let moved = rename(src, dest);
    if moved.is_err() && replaced {
        let _ = fs::create_dir(dest);
    }
    moved
}

/// Renames the directory - or, across filesystems, has mv copy it.
fn rename(src: &Path, dest: &Path) -> Result<()> {
    match fs::rename(src, dest) {
        Ok(()) => Ok(()),
        Err(err) if err.kind() == io::ErrorKind::CrossesDevices => {
            let status = Command::new("mv").arg("--").arg(src).arg(dest).status()?;
            if !status.success() {
                bail!("mv {} {} failed", src.display(), dest.display());
            }
            Ok(())
        }
        Err(err) => {
            Err(err).with_context(|| format!("can't move {} to {}", src.display(), dest.display()))
        }
    }
}

/// Points `folder` at its place now that `src` is at `dest`, and resumes
/// it if it was running. One that can't be repointed stays paused: its old
/// path is gone.
fn settle(folder: &api::config::Folder<Local>, src: &Path, dest: &Path) -> Result<()> {
    let client = client::local()?;
    let new_path = calculate_path(folder, src, dest);
    client.set_path(folder, &new_path).with_context(|| {
        format!(
            "can't point {} at {new_path}; it stays paused",
            folder.name()
        )
    })?;
    if !folder.paused {
        client
            .resume(folder)
            .with_context(|| format!("can't resume {}", folder.name()))?;
    }
    Ok(())
}

/// Where `folder`, at or inside `src`, is once `src` is at `dest`.
fn calculate_path(folder: &api::config::Folder<Local>, src: &Path, dest: &Path) -> String {
    let path = folder.local_path();
    let new = match path.strip_prefix(src) {
        Ok(rel) if !rel.as_os_str().is_empty() => dest.join(rel),
        _ => dest.to_path_buf(),
    };
    new.display().to_string()
}

/// Stops before anything moved: resumes the folders - every one it can -
/// and fails with `err`.
fn give_up(running: &[&api::config::Folder<Local>], err: anyhow::Error) -> Result<()> {
    let client = client::local()?;
    warn!("nothing was moved");
    let failed: Vec<(String, anyhow::Error)> = running
        .iter()
        .map(|folder| {
            let resumed = client
                .resume(folder)
                .with_context(|| format!("can't resume {}", folder.name()));
            (folder.name(), resumed)
        })
        .filter_map(|(name, result)| result.err().map(|err| (name, err)))
        .collect();
    if !failed.is_empty() {
        return Err(err.context(format!(
            "resume these folders in the web UI: {}",
            warn_all(&failed)
        )));
    }
    Err(err)
}

/// Warns about each folder's error; returns the folders' names.
fn warn_all(failed: &[(String, anyhow::Error)]) -> String {
    for (_, err) in failed {
        warn!("{err:#}");
    }
    let names: Vec<&str> = failed.iter().map(|(name, _)| name.as_str()).collect();
    names.join(", ")
}
