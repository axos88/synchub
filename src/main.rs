//! sttools: helpers for Syncthing - move a synced directory, push a directory
//! into a share, make Syncthing ignore what git ignores.

mod git;
mod mv;
mod paths;
mod prompt;
mod put;
mod syncthing;

use std::ffi::OsString;
use std::path::Path;
use std::process::ExitCode;

use clap::{Parser, Subcommand};

/// Helpers for Syncthing.
///
/// Each command also runs under its own name when sttools is started through
/// a link with that name: syncmv (mv), syncput (put) and stgit (git).
///
/// Syncthing must be running for this user. Its GUI address and API key are
/// read from its config.xml; use the options below where that doesn't work.
#[derive(Parser)]
#[command(version)]
struct Cli {
    #[command(flatten)]
    connection: syncthing::Connection,
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    Mv(mv::Args),
    Put(put::Args),
    Git(git::Args),
}

/// The command line, with the subcommand filled in when started as syncmv, syncput or stgit.
fn args() -> Vec<OsString> {
    let mut args: Vec<OsString> = std::env::args_os().collect();
    let name = args
        .first()
        .and_then(|a| Path::new(a).file_name())
        .and_then(|n| n.to_str());
    let subcommand = match name {
        Some("syncmv") => "mv",
        Some("syncput") => "put",
        Some("stgit") => "git",
        _ => return args,
    };
    args[0] = "sttools".into();
    args.insert(1, subcommand.into());
    args
}

fn main() -> ExitCode {
    let cli = Cli::parse_from(args());
    let result = syncthing::Client::connect(&cli.connection).and_then(|client| match cli.command {
        Command::Mv(args) => mv::run(&client, args),
        Command::Put(args) => put::run(&client, args),
        Command::Git(args) => git::run(&client, args),
    });
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("sttools: {err:#}");
            ExitCode::FAILURE
        }
    }
}
