//! `synchub get`: get one of the hub's shares on this device.

use std::path::{Path, PathBuf};

use anyhow::Result;

use crate::client;
use crate::commands::{Arguments, option};
use crate::commands::{mv, share};
use crate::folder_id::FolderId;
use crate::say;

/// Get one of the hub's shares on this device.
///
/// The hub shares the folder with this device, which accepts it into its
/// folder defaults' directory, under the folder's label (or its ID, if a
/// directory has that name already). With --path, it's then moved there, as
/// `synchub mv` moves it. To have the hub share a folder with other devices,
/// use `synchub share`.
///
/// Example: synchub get Website --path ~/projects/website
#[derive(clap::Args, Debug, PartialEq)]
pub struct Args {
    /// The share: its label or folder ID (`synchub list` shows them)
    pub folder: String,
    /// Where to move it on this device [default: it stays in the folder defaults' directory]
    #[arg(long)]
    pub path: Option<PathBuf>,
}

impl Arguments for Args {
    fn arguments(&self) -> Vec<String> {
        std::iter::once(self.folder.clone())
            .chain(option(
                "--path",
                self.path.as_ref().map(|path| path.display()),
            ))
            .collect()
    }
}

pub fn run(args: Args) -> Result<()> {
    let (local_config, hub_config) = (client::local()?.config()?, client::hub()?.config()?);
    let folder = hub_config.resolve_folder(Some(&local_config), &args.folder)?;
    get(&folder.id, args.path.as_deref())
}

/// Has the hub share the folder `id` with this device, which accepts it -
/// and, given a `path`, moves it there once it's here.
pub fn get(id: &FolderId, path: Option<&Path>) -> Result<()> {
    let local = client::local()?;
    share::share(id, &local.my_id()?)?;
    let Some(path) = path else {
        return Ok(());
    };
    match local.wait_for_folder(id)? {
        Some(local_folder) => mv::move_recursive(&local_folder, path, false),
        None => {
            say!(Local, "would then move it to {}", path.display());
            Ok(())
        }
    }
}
