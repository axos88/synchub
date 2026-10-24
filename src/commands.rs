//! The subcommands, one module each.

pub mod convert;
pub mod get;
pub mod gitignore;
pub mod join;
pub mod list;
pub mod mv;
pub mod new;
pub mod overwrite;
pub mod remove;
pub mod rename_device;
pub mod rename_folder;
pub mod share;
pub mod sync;
pub mod unshare;

use anyhow::Result;
use clap::Subcommand;

#[derive(Subcommand)]
pub enum Command {
    #[command(visible_alias = "init")]
    Join(join::Args),
    Convert(convert::Args),
    List(list::Args),
    New(new::Args),
    Get(get::Args),
    Remove(remove::Args),
    Share(share::Args),
    Unshare(unshare::Args),
    Sync(sync::Args),
    RenameDevice(rename_device::Args),
    RenameFolder(rename_folder::Args),
    Mv(mv::Args),
    Overwrite(overwrite::Args),
    Gitignore(gitignore::Args),
}

pub fn run(command: Command) -> Result<()> {
    match command {
        Command::Join(args) => join::run(args),
        Command::Convert(args) => convert::run(args),
        Command::List(args) => list::run(args),
        Command::New(args) => new::run(args),
        Command::Get(args) => get::run(args),
        Command::Remove(args) => remove::run(args),
        Command::Share(args) => share::run(args),
        Command::Unshare(args) => unshare::run(args),
        Command::Sync(args) => sync::run(args),
        Command::RenameDevice(args) => rename_device::run(args),
        Command::RenameFolder(args) => rename_folder::run(args),
        Command::Mv(args) => mv::run(args),
        Command::Overwrite(args) => overwrite::run(args),
        Command::Gitignore(args) => gitignore::run(args),
    }
}
