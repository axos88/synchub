//! `synchub list`: the hub's shares, and which of them this device has.

use anyhow::Result;
use itertools::Itertools;

use crate::api;
use crate::client;
use crate::device_id::DeviceId;
use crate::side::{Hub, Local};
use crate::{say, step};

/// List the hub's shares, which of them this device has, and who else has them
#[derive(clap::Args)]
pub struct Args {}

pub fn run(_args: Args) -> Result<()> {
    let (local, hub) = (client::local()?, client::hub()?);
    step!(Hub, "reading its folders and devices");
    let hub_config = hub.config()?;
    let listing = Listing {
        hub_id: hub.id().clone(),
        my_id: local.my_id()?,
        local_config: local.config()?,
        hub_config,
    };
    listing
        .hub_shares()
        .chain(listing.not_accepted())
        .for_each(|line| say!("{line}"));
    Ok(())
}

/// What the lines are made of.
struct Listing {
    hub_id: DeviceId,
    my_id: DeviceId,
    local_config: api::config::Config<Local>,
    hub_config: api::config::Config<Hub>,
}

impl Listing {
    /// A line for each of the hub's shares.
    fn hub_shares(&self) -> impl Iterator<Item = String> {
        self.hub_config
            .folders
            .iter()
            .sorted_by_key(|f| (f.label.to_lowercase(), f.id.clone()))
            .map(|folder| self.hub_share(folder))
    }

    /// A line for one of the hub's shares: whether it's here and where, and
    /// who else has it.
    fn hub_share(&self, folder: &api::config::Folder<Hub>) -> String {
        let local_folder = self.local_config.folder(&folder.id);
        let shared_here = local_folder.is_some() && folder.shared_with_device(&self.my_id);
        let others: Vec<String> = folder
            .shared_with_devices()
            .filter(|id| **id != self.hub_id && **id != self.my_id)
            .map(|id| self.device_name(id))
            .collect();
        format!(
            "[{}] {}  {}  also on: {}",
            if shared_here { "x" } else { " " },
            folder.name(),
            local_folder.map_or("-", |f| f.path.as_str()),
            if others.is_empty() {
                "-".to_string()
            } else {
                others.join(", ")
            }
        )
    }

    /// A line for each folder here shared with the hub that the hub doesn't
    /// have.
    fn not_accepted(&self) -> impl Iterator<Item = String> {
        self.local_config
            .folders
            .iter()
            .filter(|folder| {
                folder.shared_with_device(&self.hub_id)
                    && self.hub_config.folder(&folder.id).is_none()
            })
            .map(|folder| {
                format!(
                    "[x] {}  {}  (shared with the hub, but the hub hasn't accepted it)",
                    folder.name(),
                    folder.path
                )
            })
    }

    /// This device's name for the device `id` - or else the hub's.
    fn device_name(&self, id: &DeviceId) -> String {
        match self.local_config.device(id) {
            Some(device) => device.display(),
            None => self.hub_config.device_name(id),
        }
    }
}
