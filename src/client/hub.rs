//! The hub's API client: verified to belong to the hub, and made once.

use std::net::{TcpStream, ToSocketAddrs};
use std::ops::Deref;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result, bail};
use itertools::Itertools;
use once_cell::sync::OnceCell;
use rustls::ClientConnection;
use rustls::pki_types::ServerName;

use super::{Client, ClientBuilder};
use crate::client;
use crate::commands::convert;
use crate::device_id::DeviceId;
use crate::folder_id::FolderId;
use crate::settings;
use crate::side::Hub;
use crate::{say, step};

static CLIENT: OnceCell<HubClient> = OnceCell::new();

/// The hub this device uses, connected and verified on first use.
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
    verify(&hub)?;
    let client = HubClient::connect(&hub.url, &hub.api_key, &hub.device_id)?;
    convert::convert(&client)?;
    Ok(client)
}

/// Checks that the hub is still the device set up with `synchub join`: the
/// certificate on its sync port must have the stored device ID.
fn verify(hub: &settings::Hub) -> Result<()> {
    let address = hub.sync_address();
    step!(Hub, "checking its device ID on its sync port {address}");
    let presented = HubClient::probe_device_id(&address).with_context(|| {
        format!("can't verify the hub: no device ID from its sync port {address}")
    })?;
    if presented != hub.device_id {
        bail!(
            "the Syncthing at {address} has device ID {presented}, not the hub's {} (from `synchub join`): the hub was replaced, or the address leads elsewhere; not using it - if the hub changed, run `synchub join` again",
            hub.device_id
        );
    }
    Ok(())
}

/// The hub's API, verified to belong to the hub: only [`HubClient::connect`]
/// makes one.
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
                "the API at {url} belongs to device {reported}, but the hub is {hub_id}: the address leads to another Syncthing than the hub's sync port"
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

    /// The device ID of the Syncthing listening for devices at `address` (host:port),
    /// from the certificate it presents - no key involved.
    pub fn probe_device_id(address: &str) -> Result<DeviceId> {
        let mut config = super::tls::config()?;
        config.alpn_protocols = vec![b"bep/1.0".to_vec()];
        let target = address
            .to_socket_addrs()?
            .next()
            .with_context(|| format!("{address}: no address"))?;
        let mut stream = TcpStream::connect_timeout(&target, Duration::from_secs(5))?;
        stream.set_read_timeout(Some(Duration::from_secs(5)))?;
        // Syncthing doesn't look at the server name.
        let name = ServerName::try_from("syncthing")?.to_owned();
        let mut connection = ClientConnection::new(Arc::new(config), name)?;
        // Syncthing wants a client certificate and gives up on us, but only after
        // its own certificate has been seen.
        while connection.is_handshaking() {
            if let Err(err) = connection.complete_io(&mut stream) {
                if connection.peer_certificates().is_none() {
                    return Err(err).with_context(|| format!("TLS with {address}"));
                }
                break;
            }
        }
        let certificate = connection
            .peer_certificates()
            .and_then(|certificates| certificates.first())
            .with_context(|| format!("{address} presented no certificate"))?;
        Ok(DeviceId::of_certificate(certificate))
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
