//! synchub: helpers for Syncthing - move a synced directory, replace a
//! share's contents, make Syncthing ignore what git ignores, and choose shares
//! through a hub.

mod api;
mod client;
mod commands;
mod device_id;
mod folder_id;
mod output;
mod paths;
mod prompt;
mod settings;
mod side;
mod syncthing;
mod update;
mod version;

use std::path::PathBuf;
use std::process::ExitCode;

use clap::Parser;

/// Helpers for Syncthing.
///
/// Syncthing must be running for this user. Its GUI address and API key are
/// read from its config.xml; use the options below where that doesn't work.
#[derive(Parser)]
#[command(version = version::VERSION)]
struct Cli {
    #[command(flatten)]
    connection: syncthing::Connection,
    /// synchub's own settings file, holding the hub's details [default: ~/.config/synchub/config.toml]
    #[arg(long, env = "SYNCHUB_CONFIG", global = true)]
    config: Option<PathBuf>,
    /// Show what would be done, change nothing
    #[arg(short = 'n', long, global = true)]
    dry_run: bool,
    /// Don't update synchub to the latest release at start
    #[arg(
        long,
        env = "SYNCHUB_NO_UPDATE",
        global = true,
        value_parser = clap::builder::FalseyValueParser::new()
    )]
    no_update: bool,
    #[command(subcommand)]
    command: Option<commands::Command>,
}

fn main() -> ExitCode {
    // reqwest is built without a crypto provider: ring builds for every
    // release target with just a C compiler (aws-lc needs CMake/NASM on some).
    let _ = rustls::crypto::ring::default_provider().install_default();
    let cli = Cli::parse();
    settings::init(
        cli.config.unwrap_or_else(settings::default_path),
        cli.dry_run,
    );
    client::local::init(cli.connection);
    let command = cli
        .command
        .unwrap_or(commands::Command::App(Default::default()));
    let update = if cli.no_update {
        None
    } else {
        update::available()
    };
    let result = match (command, update) {
        // The app asks with a dialog of its own.
        (commands::Command::App(args), update) => commands::app::run(args, update),
        (command, update) => {
            update.into_iter().for_each(update::offer);
            commands::run(command)
        }
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("synchub: {err:#}");
            ExitCode::FAILURE
        }
    }
}
