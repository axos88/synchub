//! The app `synchub` opens without a command: this device, the hub, and the
//! folders with the devices they're shared with - the operations a key away.
//!
//! Arranged by what things are: `pages` are whole screens, `components` the
//! parts they're drawn from; `window` (what has the terminal, and the loop
//! running it), `state`, `operation` and `terminal` serve them.

mod components;
mod gesture;
mod operation;
mod pages;
mod state;
mod terminal;
mod window;

use anyhow::{Result, bail};

use crate::commands::Arguments;
use components::tree::Tree;
use state::{Log, State};
use window::RunnableWindow;

/// Open the app: this device, the hub, and the folders - the commands a key away
///
/// It's what `synchub` runs without a command. It shows this device and the
/// hub side by side, and the folders with the devices they're shared with;
/// the status bar says which keys do what for the selection. A device that
/// hasn't joined a hub is asked to join one first.
///
/// Example: synchub
#[derive(clap::Args, Default, Debug, PartialEq)]
pub struct Args {}

impl Arguments for Args {
    fn arguments(&self) -> Vec<String> {
        Vec::new()
    }
}

pub fn run(_args: Args) -> Result<()> {
    if !terminal::interactive() {
        bail!("not a terminal; see `synchub --help` for the commands");
    }
    // The loading dialog, the join form and the main page, without leaving
    // the full screen in between.
    window::full_screen(|| {
        let mut app = App::new();
        app.reload("Loading this device and the hub")?;
        if app.state()?.hub.is_none() {
            app.join()?;
        }
        app.run()
    })
}

/// The app, on its main page; the other pages are brought up from it.
struct App {
    /// None until it's loaded.
    state: Option<State>,
    tree: Tree,
    /// The `synchub` command lines doing what the actions did.
    log: Log,
}
