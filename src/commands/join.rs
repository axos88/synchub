//! `synchub join`: set this device up to get its folders from the hub.

use anyhow::{Result, bail};

use crate::client::{self, Client, ClientBuilder, HubClient};
use crate::commands::convert;
use crate::commands::{Arguments, flag, option};
use crate::device_id::DeviceId;
use crate::device_id::identicon;
use crate::prompt;
use crate::say;
use crate::settings;
use crate::side::Hub;

/// Set this device up to use the hub.
///
/// Checks first that the address leads to the hub, by the device ID its API
/// reports. Given with --device-id, it's checked against that; with -y, it's
/// accepted unchecked; otherwise it's shown, with the picture the hub's web UI
/// draws for it, for you to compare. Then, here:
/// the hub becomes the introducer whose folders are accepted automatically,
/// and new folders are shared with no device by default. On the hub: this device is added, without auto-accept or
/// introducer. Last, the hub's address, API key and device ID are saved for
/// the share commands.
///
/// Then the folders here that are shared directly with devices that have
/// joined the hub move over to it: the hub gets the folder, shared with the
/// same devices, and here the folder is shared with the hub instead; the
/// hub's other devices stop being introducers or auto-accepted here. Devices
/// that haven't joined keep their direct share - every later command that
/// uses the hub moves them over once they have - so a network can switch to
/// the hub one device at a time.
///
/// Missing arguments are asked for in a form.
///
/// Example: synchub join https://hub.example.com:8384 <API key>
#[derive(clap::Args, Clone, Debug, PartialEq)]
pub struct Args {
    /// The hub's GUI/API address, https://host:8384
    pub url: String,
    /// The hub's API key (on the hub: Actions, Settings, GUI)
    // Its own ID: "api_key" is the global option for the local Syncthing's key.
    #[arg(
        id = "hub_api_key",
        value_name = "API_KEY",
        env = "SYNCHUB_HUB_API_KEY",
        hide_env_values = true
    )]
    pub api_key: String,
    /// The hub's device ID; when given, the hub is checked against it without asking
    #[arg(long)]
    pub device_id: Option<DeviceId>,
    /// Accept the hub's device ID without asking - trusting whatever answers at the address
    #[arg(short, long)]
    pub yes: bool,
    /// The directory new folders go into on this device [default: unchanged]
    #[arg(long)]
    pub default_folders_dir: Option<String>,
}

/// The API key is left out - a secret, kept out of the log: the command
/// takes it from SYNCHUB_HUB_API_KEY then.
impl Arguments for Args {
    fn arguments(&self) -> Vec<String> {
        std::iter::once(self.url.clone())
            .chain(option("--device-id", self.device_id.as_ref()))
            .chain(flag(self.yes, "--yes"))
            .chain(option(
                "--default-folders-dir",
                self.default_folders_dir.as_ref(),
            ))
            .collect()
    }
}

pub fn run(args: Args) -> Result<()> {
    let hub = find_hub(&args)?;
    match &args.device_id {
        Some(expected) if *expected == hub.device_id => {
            say!(Hub, "device ID {} checked at {}", hub.device_id, hub.url);
        }
        Some(expected) => bail!(
            "the Syncthing at {} has device ID {}, not the hub's {expected}: check the address and --device-id; nothing changed",
            hub.url,
            hub.device_id
        ),
        None if args.yes => say!(
            Hub,
            "device ID {} at {}, accepted unchecked (-y)",
            hub.device_id,
            hub.url
        ),
        None => confirm_hub_id(&hub.device_id, &hub.url)?,
    }
    join_hub(hub, args.default_folders_dir.as_deref())
}

/// The hub `args` names, with the device ID its API reports - for checking
/// before joining it.
pub fn find_hub(args: &Args) -> Result<settings::Hub> {
    let url = HubClient::api_url(&args.url)?;
    let device_id = ClientBuilder::hub(&url, &args.api_key).connect()?.my_id()?;
    Ok(settings::Hub {
        url,
        api_key: args.api_key.clone(),
        device_id,
    })
}

/// Joins the checked `hub`: makes it this device's only introducer, has it
/// know this device, moves direct shares over to it, and saves its details.
pub fn join_hub(hub: settings::Hub, default_folders_dir: Option<&str>) -> Result<()> {
    let local = client::local()?;
    let client = HubClient::connect(&hub.url, &hub.api_key, &hub.device_id)?;
    let my_id = local.my_id()?;
    if hub.device_id == my_id {
        bail!("this device is the hub itself; run `synchub join` on the leaf devices only");
    }
    trust_hub(&client, &hub.device_id)?;
    local.set_folder_defaults(&my_id, default_folders_dir)?;
    register_with_hub(&client, &my_id)?;
    convert::convert(&client)?;
    settings::update(|settings| settings.hub = Some(hub))?;
    say!("Run `synchub` to choose this device's folders.");
    Ok(())
}

/// Shows the device ID and its picture, and asks whether it's the hub's.
fn confirm_hub_id(hub_id: &DeviceId, url: &str) -> Result<()> {
    say!("The device ID of the Syncthing at {url}:\n");
    say!("{}", identicon::render(hub_id));
    say!("    {hub_id}\n");
    say!(
        "Compare it with the hub's own (in its web UI: Actions, Show ID) - the ID and the picture."
    );
    match prompt::confirm("Is it the hub's?")? {
        Some(true) => Ok(()),
        Some(false) => bail!("not trusted; nothing changed"),
        None => bail!(
            "not a terminal; give the hub's device ID with --device-id, or accept it unchecked with -y"
        ),
    }
}

/// Makes the hub this device's introducer whose folders are accepted
/// automatically. Its other devices stop being either in [`convert`].
fn trust_hub(hub: &Client<Hub>, hub_id: &DeviceId) -> Result<()> {
    let local = client::local()?;
    if let Some(hub_device) = local.find_device(hub_id)? {
        return local.set_introducer(&hub_device, true);
    }
    // The hub's name, as it calls itself.
    let name = hub.find_device(hub_id)?.map(|d| d.name).unwrap_or_default();
    local.add_device(hub_id, &name, true)
}

/// Has the hub know this device, if it doesn't yet.
fn register_with_hub(hub: &Client<Hub>, my_id: &DeviceId) -> Result<()> {
    let local = client::local()?;
    if hub.devices()?.iter().any(|d| d.id == *my_id) {
        say!(Hub, "already knows this device");
        return Ok(());
    }
    let my_name = local
        .devices()?
        .into_iter()
        .find(|d| d.id == *my_id)
        .map(|d| d.name)
        .unwrap_or_default();
    hub.add_device(my_id, &my_name, false)
}
