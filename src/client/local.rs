//! A Syncthing instance's REST API - this device's, made once, or another's
//! (the hub's builds on it).

use std::marker::PhantomData;
use std::sync::OnceLock;
use std::thread::sleep;
use std::time::{Duration, Instant};

use anyhow::{Context, Result, bail};
use itertools::Itertools;
use once_cell::sync::OnceCell;
use reqwest::blocking::{Client as Http, RequestBuilder, Response};
use reqwest::{Method, StatusCode};
use serde::de::DeserializeOwned;

use super::tls;
use crate::api;
use crate::device_id::DeviceId;
use crate::folder_id::FolderId;
use crate::settings;
use crate::side::{Hub, Local, Side};
use crate::syncthing::{self, Connection, Endpoint};
use crate::{say, step};

mod error;

pub use error::{NotFound, Refused};
use error::{is_404, is_cancelled};

static CONNECTION: OnceLock<Connection> = OnceLock::new();
static LOCAL: OnceCell<Client<Local>> = OnceCell::new();

/// Sets up how [`get`] reaches this device's Syncthing. Called once, before
/// anything else.
pub fn init(connection: Connection) {
    assert!(
        CONNECTION.set(connection).is_ok(),
        "client::local::init runs once"
    );
}

/// This device's Syncthing, connected on first use.
pub fn get() -> Result<&'static Client<Local>> {
    LOCAL.get_or_try_init(|| {
        ClientBuilder::local(CONNECTION.get().expect("client::local::init runs first")).connect()
    })
}

/// A Syncthing instance's API - `S` says whose. The operations that change
/// something report what they do (`step!`) - or, in a dry run, only what
/// they would do.
pub struct Client<S> {
    http: Http,
    /// What the requests' URLs start with.
    base: String,
    /// Where it is, for people: the URL, or the Unix socket.
    address: String,
    api_key: String,
    side: PhantomData<S>,
}

/// How a [`Client`] reaches its Syncthing; [`ClientBuilder::connect`] makes
/// the connected client. What fails while setting it up (e.g. reading
/// config.xml) is reported by `connect`.
pub struct ClientBuilder<S> {
    target: Result<Target>,
    side: PhantomData<S>,
}

/// This device's Syncthing: the address and API key from `connection`, or
/// else from its config.xml.
fn local_target(connection: &Connection) -> Result<Target> {
    let gui = syncthing::gui_config(connection)?;
    Ok(Target {
        endpoint: gui.endpoint()?,
        api_key: gui.api_key,
        timeout: None,
    })
}

/// Where the Syncthing is.
struct Target {
    endpoint: Endpoint,
    api_key: String,
    timeout: Option<Duration>,
}

impl ClientBuilder<Local> {
    /// A builder for this device's Syncthing: its address and API key from
    /// `connection`, or else from its config.xml.
    pub fn local(connection: &Connection) -> Self {
        ClientBuilder {
            target: local_target(connection),
            side: PhantomData,
        }
    }
}

impl ClientBuilder<Hub> {
    /// A builder for the hub's Syncthing, at `base`.
    pub fn hub(base: &str, api_key: &str) -> Self {
        ClientBuilder {
            target: Ok(Target {
                endpoint: Endpoint {
                    base: base.trim_end_matches('/').to_string(),
                    socket: None,
                },
                api_key: api_key.to_string(),
                timeout: Some(Duration::from_secs(60)),
            }),
            side: PhantomData,
        }
    }
}

impl<S: Side> ClientBuilder<S> {
    /// Builds the client, and checks that Syncthing answers.
    pub fn connect(self) -> Result<Client<S>> {
        let target = self.target?;
        let http = Http::builder()
            .timeout(target.timeout)
            .tls_backend_preconfigured(tls::config()?);
        #[cfg(unix)]
        let http = match &target.endpoint.socket {
            Some(socket) => http.unix_socket(socket.clone()),
            None => http,
        };
        let client = Client {
            http: http.build()?,
            address: target.endpoint.address(),
            base: target.endpoint.base,
            api_key: target.api_key,
            side: PhantomData,
        };
        client.ping().with_context(|| {
            format!(
                "can't reach {}'s API at {} - is it running, and is the API key right?",
                S::WHO,
                client.address
            )
        })?;
        Ok(client)
    }
}

impl<S: Side> Client<S> {
    /// Where its Syncthing is: the API's URL, or its Unix socket.
    pub fn address(&self) -> &str {
        &self.address
    }

    /// The Syncthing version running there, e.g. `v2.1.7`.
    pub fn version(&self) -> Result<String> {
        Ok(self
            .get::<api::system::VersionResponse>("system/version")?
            .version)
    }

    pub fn ping(&self) -> Result<()> {
        let ping: api::system::PingResponse = self.get("system/ping")?;
        if ping.ping != "pong" {
            bail!("{} answered the ping with {:?}", S::WHO, ping.ping);
        }
        Ok(())
    }

    /// This instance's own device ID.
    pub fn my_id(&self) -> Result<DeviceId> {
        Ok(self
            .get::<api::system::StatusResponse>("system/status")?
            .my_id)
    }

    pub fn devices(&self) -> Result<Vec<api::config::DeviceResponse>> {
        self.get("config/devices")
    }

    /// The device with the device ID `id`, if this instance knows it.
    pub fn find_device(&self, id: &DeviceId) -> Result<Option<api::config::DeviceResponse>> {
        self.find(&format!("config/devices/{}", encode(id.as_str())))
    }

    /// The IDs of the devices connected right now.
    pub fn connected_devices(&self) -> Result<Vec<DeviceId>> {
        let connections: api::system::ConnectionsResponse = self.get("system/connections")?;
        Ok(connections
            .connections
            .into_iter()
            .filter(|(_, connection)| connection.connected)
            .map(|(id, _)| id)
            .collect())
    }

    /// How far `device` has synced the folder `id`, as far as this instance knows.
    pub fn completion(
        &self,
        id: &FolderId,
        device: &DeviceId,
    ) -> Result<api::db::CompletionResponse> {
        self.get(&format!(
            "db/completion?folder={}&device={}",
            encode(id.as_str()),
            encode(device.as_str())
        ))
    }

    /// Adds a device: as the introducer whose folders are accepted
    /// automatically, or as a plain one.
    pub fn add_device(&self, id: &DeviceId, name: &str, introducer: bool) -> Result<()> {
        let (doing, would) = if introducer {
            let role = "as the introducer whose folders are accepted automatically";
            (
                format!("adding {name}, {role}"),
                format!("add {name}, {role}"),
            )
        } else {
            (format!("adding {name}"), format!("add {name}"))
        };
        if self.skip(doing, would) {
            return Ok(());
        }
        let device = api::config::NewDeviceRequest {
            id,
            name,
            addresses: vec!["dynamic".to_string()],
            introducer,
            auto_accept_folders: introducer,
            skip_introduction_removals: false,
        };
        self.send(self.request(Method::POST, "config/devices").json(&device))?;
        Ok(())
    }

    /// Gives the device `id` the name `name` - in this instance's
    /// configuration. Fails with [`NotFound`] if it doesn't know the device.
    pub fn rename_device(&self, id: &DeviceId, name: &str) -> Result<()> {
        let what = format!("device {id}");
        // A dry run sends nothing to fail with a 404.
        if settings::dry_run() && self.find_device(id)?.is_none() {
            return Err(self.not_found(what));
        }
        if self.skip(
            format!("renaming {what} to {name}"),
            format!("rename {what} to {name}"),
        ) {
            return Ok(());
        }
        let patch = api::config::DevicePatchRequest {
            name: Some(name.to_string()),
            ..Default::default()
        };
        self.found(self.update_device(id, &patch), what)
    }

    /// Makes `device` the introducer whose folders are accepted
    /// automatically - or neither.
    pub fn set_introducer(&self, device: &api::config::Device, on: bool) -> Result<()> {
        let name = device.display();
        let (doing, would) = if on {
            let role = "the introducer whose folders are accepted automatically";
            (
                format!("making {name} {role}"),
                format!("make {name} {role}"),
            )
        } else {
            let role = "no longer an introducer or auto-accepted";
            (
                format!("making {name} {role}"),
                format!("make {name} {role}"),
            )
        };
        if self.skip(doing, would) {
            return Ok(());
        }
        let patch = api::config::DevicePatchRequest {
            introducer: Some(on),
            auto_accept_folders: Some(on),
            skip_introduction_removals: on.then_some(false),
            ..Default::default()
        };
        self.update_device(&device.id, &patch)
    }

    /// Drops the connection to `device` and makes it again - which has an
    /// introducer send its folder list, and with it introduce the devices
    /// again.
    pub fn reconnect(&self, device: &DeviceId) -> Result<()> {
        let name = self.device_names(&[device]);
        if self.skip(
            format!("reconnecting to {name}"),
            format!("reconnect to {name}"),
        ) {
            return Ok(());
        }
        let path = format!("system/pause?device={}", encode(device.as_str()));
        self.send(self.request(Method::POST, &path))?;
        sleep(Duration::from_secs(2));
        let path = format!("system/resume?device={}", encode(device.as_str()));
        self.send(self.request(Method::POST, &path))?;
        Ok(())
    }

    /// Its whole configuration, in one go.
    pub fn config(&self) -> Result<api::config::ConfigResponse<S>> {
        self.get("config")
    }

    pub fn folders(&self) -> Result<Vec<api::config::FolderResponse<S>>> {
        self.get("config/folders")
    }

    pub fn folder(&self, id: &FolderId) -> Result<api::config::FolderResponse<S>> {
        self.get(&format!("config/folders/{}", encode(id.as_str())))
    }

    /// The folder `id`, if this instance has it.
    pub fn find_folder(&self, id: &FolderId) -> Result<Option<api::config::FolderResponse<S>>> {
        self.find(&format!("config/folders/{}", encode(id.as_str())))
    }

    /// Adds a folder shared with `devices`; settings not given take the
    /// folder defaults.
    pub fn add_folder(
        &self,
        id: &FolderId,
        label: &str,
        path: &str,
        devices: &[&DeviceId],
    ) -> Result<()> {
        let names = self.device_names(devices);
        // Named the way `Folder::name` names it.
        let name = if label.is_empty() {
            id.to_string()
        } else {
            format!("{label} ({id})")
        };
        if self.skip(
            format!("adding {name} at {path}, shared with {names}"),
            format!("add {name} at {path}, shared with {names}"),
        ) {
            return Ok(());
        }
        let folder = api::config::NewFolderRequest {
            id,
            label,
            path,
            folder_type: api::config::FolderType::SendReceive,
            devices: devices
                .iter()
                .map(|d| api::config::FolderDevice::new(d))
                .collect(),
        };
        self.send(self.request(Method::POST, "config/folders").json(&folder))?;
        Ok(())
    }

    /// Removes a folder from the configuration; its files stay on disk.
    pub fn remove_folder(&self, folder: &api::config::Folder<S>) -> Result<()> {
        let name = folder.name();
        if self.skip(format!("removing {name}"), format!("remove {name}")) {
            return Ok(());
        }
        let path = format!("config/folders/{}", encode(folder.id.as_str()));
        self.send(self.request(Method::DELETE, &path))?;
        Ok(())
    }

    /// Shares `folder` with `devices` too. Those it lists already now count
    /// as shared on purpose, no longer as introduced by another device.
    pub fn share(&self, folder: &api::config::Folder<S>, devices: &[&DeviceId]) -> Result<()> {
        let current = self.folder(&folder.id)?;
        let new: Vec<&DeviceId> = devices
            .iter()
            .copied()
            .filter(|id| !current.shared_with_device(id))
            .unique()
            .collect();
        let mut entries = current.devices.clone();
        for entry in &mut entries {
            if devices.contains(&&entry.device_id) {
                entry.introduced_by = None;
            }
        }
        entries.extend(new.iter().map(|id| api::config::FolderDevice::new(id)));
        if entries == current.devices {
            return Ok(());
        }
        let name = folder.name();
        let names = self.device_names(if new.is_empty() { devices } else { &new });
        let (doing, would) = if new.is_empty() {
            (
                format!("keeping {name} shared with {names}"),
                format!("keep {name} shared with {names}"),
            )
        } else {
            (
                format!("sharing {name} with {names}"),
                format!("share {name} with {names}"),
            )
        };
        if self.skip(doing, would) {
            return Ok(());
        }
        self.set_devices(folder, entries)
    }

    /// Stops sharing `folder` with `devices`; they keep their copy until
    /// they remove it.
    pub fn unshare(&self, folder: &api::config::Folder<S>, devices: &[&DeviceId]) -> Result<()> {
        let current = self.folder(&folder.id)?;
        let gone: Vec<&DeviceId> = devices
            .iter()
            .copied()
            .filter(|id| current.shared_with_device(id))
            .collect();
        if gone.is_empty() {
            return Ok(());
        }
        let (name, names) = (folder.name(), self.device_names(&gone));
        if self.skip(
            format!("unsharing {name} from {names}; they keep their copy"),
            format!("unshare {name} from {names}"),
        ) {
            return Ok(());
        }
        let entries: Vec<api::config::FolderDevice> = current
            .devices
            .into_iter()
            .filter(|d| !gone.contains(&&d.device_id))
            .collect();
        self.set_devices(folder, entries)
    }

    pub fn pause(&self, folder: &api::config::Folder<S>) -> Result<()> {
        let name = folder.name();
        if self.skip(format!("pausing {name}"), format!("pause {name}")) {
            return Ok(());
        }
        self.update_folder(
            &folder.id,
            &api::config::FolderPatchRequest {
                paused: Some(true),
                ..Default::default()
            },
        )
    }

    pub fn resume(&self, folder: &api::config::Folder<S>) -> Result<()> {
        let name = folder.name();
        if self.skip(format!("resuming {name}"), format!("resume {name}")) {
            return Ok(());
        }
        self.update_folder(
            &folder.id,
            &api::config::FolderPatchRequest {
                paused: Some(false),
                ..Default::default()
            },
        )
    }

    /// Gives the folder `id` the label `label` - in this instance's
    /// configuration. Fails with [`NotFound`] if it doesn't have the folder.
    pub fn rename_folder(&self, id: &FolderId, label: &str) -> Result<()> {
        let what = format!("folder {id}");
        // A dry run sends nothing to fail with a 404.
        if settings::dry_run() && self.find_folder(id)?.is_none() {
            return Err(self.not_found(what));
        }
        if self.skip(
            format!("renaming {what} to {label}"),
            format!("rename {what} to {label}"),
        ) {
            return Ok(());
        }
        let patch = api::config::FolderPatchRequest {
            label: Some(label.to_string()),
            ..Default::default()
        };
        self.found(self.update_folder(id, &patch), what)
    }

    /// Points `folder` at `path`, written the way its config should keep it.
    pub fn set_path(&self, folder: &api::config::Folder<S>, path: &str) -> Result<()> {
        let name = folder.name();
        if self.skip(
            format!("pointing {name} at {path}"),
            format!("point {name} at {path}"),
        ) {
            return Ok(());
        }
        self.update_folder(
            &folder.id,
            &api::config::FolderPatchRequest {
                path: Some(path.to_string()),
                ..Default::default()
            },
        )
    }

    /// Sets `folder`'s type, and whether it sends file ownership.
    pub fn set_folder_type(
        &self,
        folder: &api::config::Folder<S>,
        folder_type: api::config::FolderType,
        send_ownership: bool,
    ) -> Result<()> {
        let name = folder.name();
        let ownership = if send_ownership {
            ", sending ownership"
        } else {
            ""
        };
        if self.skip(
            format!("making {name} {folder_type}{ownership}"),
            format!("make {name} {folder_type}{ownership}"),
        ) {
            return Ok(());
        }
        self.update_folder(
            &folder.id,
            &api::config::FolderPatchRequest {
                folder_type: Some(folder_type),
                send_ownership: Some(send_ownership),
                ..Default::default()
            },
        )
    }

    /// The template new folders are made from: a folder without an ID.
    pub fn folder_defaults(&self) -> Result<api::config::FolderResponse<S>> {
        self.get("config/defaults/folder")
    }

    /// New folders: shared with no device but `own_id` (this instance's own),
    /// and put into `dir` if given.
    pub fn set_folder_defaults(&self, own_id: &DeviceId, dir: Option<&str>) -> Result<()> {
        let into = dir.map(|d| format!(", in {d}")).unwrap_or_default();
        if self.skip(
            format!("sharing new folders with no device by default{into}"),
            format!("share new folders with no device by default{into}"),
        ) {
            return Ok(());
        }
        let defaults = api::config::FolderPatchRequest {
            devices: Some(vec![api::config::FolderDevice::new(own_id)]),
            path: dir.map(String::from),
            ..Default::default()
        };
        self.send(
            self.request(Method::PATCH, "config/defaults/folder")
                .json(&defaults),
        )?;
        Ok(())
    }

    /// Changes the folder's settings given in `patch`.
    fn update_folder(&self, id: &FolderId, patch: &api::config::FolderPatchRequest) -> Result<()> {
        let path = format!("config/folders/{}", encode(id.as_str()));
        self.send(self.request(Method::PATCH, &path).json(patch))?;
        Ok(())
    }

    /// Replaces the list of devices `folder` is shared with.
    fn set_devices(
        &self,
        folder: &api::config::Folder<S>,
        devices: Vec<api::config::FolderDevice>,
    ) -> Result<()> {
        let patch = api::config::FolderPatchRequest {
            devices: Some(devices),
            ..Default::default()
        };
        self.update_folder(&folder.id, &patch)
    }

    /// Changes the device's settings given in `patch`.
    fn update_device(&self, id: &DeviceId, patch: &api::config::DevicePatchRequest) -> Result<()> {
        let path = format!("config/devices/{}", encode(id.as_str()));
        self.send(self.request(Method::PATCH, &path).json(patch))?;
        Ok(())
    }

    pub fn status(&self, id: &FolderId) -> Result<api::db::FolderStatusResponse> {
        self.get(&format!("db/status?folder={}", encode(id.as_str())))
    }

    /// Scans the folder; returns when the scan is done.
    pub fn scan(&self, folder: &api::config::Folder<S>) -> Result<()> {
        let name = folder.name();
        if self.skip(format!("rescanning {name}"), format!("rescan {name}")) {
            return Ok(());
        }
        let path = format!("db/scan?folder={}", encode(folder.id.as_str()));
        self.scan_retrying(&path, &name, 3)
    }

    /// A scan asked for while the folder restarts - Syncthing restarts one
    /// whenever its configuration changes, e.g. an introducer adding a device
    /// to it - is refused; it's asked for again once the restart is over.
    fn scan_retrying(&self, path: &str, name: &str, tries: u32) -> Result<()> {
        match self.send(self.request(Method::POST, path)) {
            Err(err) if is_cancelled(&err) && tries > 1 => {
                step!(S::PLACE, "{name} is restarting; rescanning it again");
                sleep(Duration::from_secs(1));
                self.scan_retrying(path, name, tries - 1)
            }
            result => result.map(drop),
        }
    }

    /// "Override Changes" of a send-only folder: this device's files win everywhere.
    pub fn override_changes(&self, folder: &api::config::Folder<S>) -> Result<()> {
        let name = folder.name();
        if self.skip(
            format!("overriding the other devices' changes to {name}"),
            format!("override the other devices' changes to {name}"),
        ) {
            return Ok(());
        }
        self.send(self.request(
            Method::POST,
            &format!("db/override?folder={}", encode(folder.id.as_str())),
        ))?;
        Ok(())
    }

    /// The file `name` of the folder `id`, as the folder's index has it; None
    /// if no device has ever had it (Syncthing answers 404).
    pub fn find_file(&self, id: &FolderId, name: &str) -> Result<Option<api::db::FileResponse>> {
        self.find(&format!(
            "db/file?folder={}&file={}",
            encode(id.as_str()),
            encode(name)
        ))
    }

    /// The error Syncthing found in the folder's ignore patterns, if any.
    pub fn ignores_error(&self, id: &FolderId) -> Result<Option<String>> {
        let ignores: api::db::IgnoresResponse =
            self.get(&format!("db/ignores?folder={}", encode(id.as_str())))?;
        Ok(ignores.error.filter(|e| !e.is_empty()))
    }

    /// Waits until `device` has everything of `folder` this instance has -
    /// for up to `timeout`.
    pub fn wait_synced(
        &self,
        folder: &api::config::Folder<S>,
        device: &DeviceId,
        timeout: Duration,
    ) -> Result<()> {
        let (name, device_name) = (folder.name(), self.device_names(&[device]));
        if self.skip(
            format!("waiting for {device_name} to catch up on {name}"),
            format!("wait for {device_name} to catch up on {name}"),
        ) {
            return Ok(());
        }
        let deadline = Instant::now() + timeout;
        loop {
            let completion = self.completion(&folder.id, device)?;
            if completion.completion >= 100.0 && completion.need_items == 0 {
                break;
            }
            if Instant::now() > deadline {
                bail!(
                    "{device_name} still hasn't caught up on {name} after {} minutes ({} item(s) to go); nothing was removed - try again later",
                    timeout.as_secs() / 60,
                    completion.need_items
                );
            }
            sleep(Duration::from_secs(1));
        }
        step!(S::PLACE, "{device_name} has caught up on {name}");
        Ok(())
    }

    /// Waits - for up to half a minute - until this instance has the folder
    /// `id`, e.g. one the hub just shared with it, which it accepts on its
    /// own. Returns it - or, in a dry run, where nothing arrives, None if it
    /// isn't here.
    pub fn wait_for_folder(&self, id: &FolderId) -> Result<Option<api::config::Folder<S>>> {
        if let Some(folder) = self.find_folder(id)? {
            return Ok(Some(folder));
        }
        if self.skip(
            format!("waiting for folder {id} to arrive from the hub"),
            format!("wait for folder {id} to arrive from the hub"),
        ) {
            return Ok(None);
        }
        let deadline = Instant::now() + Duration::from_secs(30);
        loop {
            sleep(Duration::from_millis(500));
            if let Some(folder) = self.find_folder(id)? {
                return Ok(Some(folder));
            }
            if Instant::now() > deadline {
                bail!(
                    "folder {id} hasn't arrived after half a minute - is this device connected to the hub? It arrives once it is"
                );
            }
        }
    }

    /// Waits until the folder is done scanning and syncing - for up to an hour.
    pub fn wait_idle(&self, folder: &api::config::Folder<S>) -> Result<()> {
        let name = folder.name();
        if self.skip(
            format!("waiting for {name} to finish scanning and syncing"),
            format!("wait for {name} to finish scanning and syncing"),
        ) {
            return Ok(());
        }
        let deadline = Instant::now() + Duration::from_secs(3600);
        loop {
            let state = match self.status(&folder.id) {
                Ok(status) if status.state == "idle" => break,
                Ok(status) if status.state == "error" => bail!(
                    "{name} reports an error: {}; see it in the web UI",
                    status.error
                ),
                Ok(status) => status.state,
                Err(err) => format!("unknown ({err:#})"),
            };
            if Instant::now() > deadline {
                bail!(
                    "{name} still isn't idle after an hour (it's {state}); check it in the web UI"
                );
            }
            sleep(Duration::from_secs(1));
        }
        step!(S::PLACE, "{name} is idle");
        Ok(())
    }

    fn not_found(&self, what: String) -> anyhow::Error {
        NotFound { who: S::WHO, what }.into()
    }

    /// The `result` of a change to `what` (e.g. "folder abcde-12345") by its
    /// ID, with Syncthing's 404 turned into [`NotFound`].
    fn found(&self, result: Result<()>, what: String) -> Result<()> {
        match result {
            Err(err) if is_404(&err) => Err(self.not_found(what)),
            result => result,
        }
    }

    /// Reports a change about to be made (`doing`) - or, in a dry run, what
    /// it would be (`would`); true when the change is to be skipped.
    fn skip(&self, doing: String, would: String) -> bool {
        let dry_run = settings::dry_run();
        if dry_run {
            say!(S::PLACE, "would {would}");
        } else {
            step!(S::PLACE, "{doing}");
        }
        dry_run
    }

    /// The devices' names, as this instance knows them.
    pub fn device_names(&self, ids: &[&DeviceId]) -> String {
        let devices = self.devices().unwrap_or_default();
        ids.iter()
            .map(|id| match devices.iter().find(|d| d.id == **id) {
                Some(device) => device.display(),
                None => id.short().to_string(),
            })
            .collect::<Vec<_>>()
            .join(", ")
    }

    fn request(&self, method: Method, path: &str) -> RequestBuilder {
        self.http
            .request(method, format!("{}/rest/{path}", self.base))
            .header("X-API-Key", &self.api_key)
    }

    fn send(&self, request: RequestBuilder) -> Result<Response> {
        self.check(request.send()?)
    }

    /// Fails with [`Refused`] unless `response` reports success.
    fn check(&self, response: Response) -> Result<Response> {
        let status = response.status();
        if !status.is_success() {
            let body = response.text().unwrap_or_default();
            return Err(Refused {
                who: S::WHO,
                status,
                body: body.trim().to_string(),
            }
            .into());
        }
        Ok(response)
    }

    fn get<T: DeserializeOwned>(&self, path: &str) -> Result<T> {
        Ok(self.send(self.request(Method::GET, path))?.json()?)
    }

    /// GETs the object at `path`; None if there's none (Syncthing answers
    /// 404 for an unknown ID).
    fn find<T: DeserializeOwned>(&self, path: &str) -> Result<Option<T>> {
        let response = self.request(Method::GET, path).send()?;
        match response.status() {
            StatusCode::NOT_FOUND => Ok(None),
            _ => Ok(Some(self.check(response)?.json()?)),
        }
    }
}

fn encode(value: &str) -> String {
    value
        .bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                (b as char).to_string()
            }
            _ => format!("%{b:02X}"),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encode_escapes_reserved_characters() {
        assert_eq!(encode("shy4a-ady3g"), "shy4a-ady3g");
        assert_eq!(encode("a b/c"), "a%20b%2Fc");
    }
}
