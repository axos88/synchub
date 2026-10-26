//! The hub's API client: checked to be the hub's, and made once.

use std::ops::Deref;

use anyhow::{Context, Result, bail};
use itertools::Itertools;
use once_cell::sync::OnceCell;

use super::{Client, ClientBuilder};
use crate::client;
use crate::commands::{convert, sync};
use crate::device_id::DeviceId;
use crate::folder_id::FolderId;
use crate::settings;
use crate::side::Hub;
use crate::{say, step};

static CLIENT: OnceCell<HubClient> = OnceCell::new();

/// The hub this device uses, connected and checked on first use.
///
/// Everything that uses the hub comes through here, so connecting is also
/// when folders shared directly with devices that have joined the hub since
/// are moved over to the hub.
pub fn get() -> Result<&'static HubClient> {
    CLIENT.get_or_try_init(connect)
}

fn connect() -> Result<HubClient> {
    let hub = settings::get()?.hub.with_context(|| {
        format!(
            "this device isn't set up for a hub ({} has none); run `synchub join` first",
            settings::file().display()
        )
    })?;
    let client = HubClient::connect(&hub.url, &hub.api_key, &hub.device_id)?;
    convert::convert(&client)?;
    sync::on_connect(&client);
    Ok(client)
}

/// The hub's API, checked to report the hub's device ID: only
/// [`HubClient::connect`] makes one.
pub struct HubClient {
    client: Client<Hub>,
    id: DeviceId,
}

impl Deref for HubClient {
    type Target = Client<Hub>;

    fn deref(&self) -> &Client<Hub> {
        &self.client
    }
}

impl HubClient {
    /// Connects to the hub's API at `url` - which must belong to the device
    /// `hub_id`.
    pub fn connect(url: &str, api_key: &str, hub_id: &DeviceId) -> Result<Self> {
        let url = Self::api_url(url)?;
        step!(Hub, "connecting to its API at {url}");
        let client = ClientBuilder::hub(&url, api_key).connect()?;
        let reported = client.my_id()?;
        if reported != *hub_id {
            bail!(
                "the API at {url} belongs to device {reported}, but the hub is {hub_id}: the address leads to another Syncthing - or the hub was set up anew; then run `synchub join` again"
            );
        }
        Ok(HubClient {
            client,
            id: reported,
        })
    }

    /// The hub's API address `url`, with https:// if it has no scheme; plain
    /// http would send the API key unencrypted.
    pub fn api_url(url: &str) -> Result<String> {
        let url = url.trim().trim_end_matches('/');
        if url.starts_with("http://") {
            bail!("{url}: use https:// - over http the API key would travel unencrypted");
        }
        Ok(if url.starts_with("https://") {
            url.to_string()
        } else {
            format!("https://{url}")
        })
    }

    /// The hub's device ID.
    pub fn id(&self) -> &DeviceId {
        &self.id
    }

    /// Has the hub share the folder `id` with `device` too - this device
    /// included, which then accepts it. A folder the hub doesn't have yet is
    /// created there from this device's, shared with this device and `device`.
    pub fn share_folder(&self, id: &FolderId, device: &DeviceId) -> Result<()> {
        let Some(hub_folder) = self.find_folder(id)? else {
            return self.create_folder(id, device);
        };
        if hub_folder.shared_with_device(device) {
            say!(
                Hub,
                "{} is shared with {} already",
                hub_folder.name(),
                self.device_names(&[device])
            );
            return Ok(());
        }
        self.share(&hub_folder, &[device])
    }

    /// Has the hub stop sharing `id` with `device`, which keeps its copy.
    pub fn unshare_folder(&self, id: &FolderId, device: &DeviceId) -> Result<()> {
        if *device == self.id {
            bail!(
                "the hub has every folder shared through it; it can't stop sharing one with itself"
            );
        }
        let hub_folder = self.find_folder(id)?.with_context(|| {
            format!(
                "the hub doesn't have {id}, so it shares it with nobody to stop sharing it with"
            )
        })?;
        if !hub_folder.shared_with_device(device) {
            say!(
                Hub,
                "{} isn't shared with {}",
                hub_folder.name(),
                self.device_names(&[device])
            );
            return Ok(());
        }
        self.unshare(&hub_folder, &[device])
    }

    /// Creates the folder `id`, one this device has, on the hub - shared
    /// with this device and `device` - and shares it here with the hub.
    fn create_folder(&self, id: &FolderId, device: &DeviceId) -> Result<()> {
        let local = client::local()?;
        let folder = local
            .find_folder(id)?
            .with_context(|| format!("neither the hub nor this device has folder {id}"))?;
        let my_id = local.my_id()?;
        let path = self.config()?.new_folder_path(id);
        let devices: Vec<&DeviceId> = [&self.id, &my_id, device].into_iter().unique().collect();
        self.add_folder(id, &folder.label, &path, &devices)?;
        if !folder.shared_with_device(&self.id) {
            local.share(&folder, &[&self.id])?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_hubs_api_address_must_be_https() {
        assert_eq!(HubClient::api_url("hub:8384/").unwrap(), "https://hub:8384");
        assert_eq!(
            HubClient::api_url("https://hub:8384").unwrap(),
            "https://hub:8384"
        );
        assert!(HubClient::api_url("http://hub:8384").is_err());
    }
}
