//! The running Syncthing itself (`/rest/system/...`).

use std::collections::BTreeMap;

use serde::Deserialize;

use crate::device_id::DeviceId;

/// `/rest/system/ping`: "pong".
#[derive(Deserialize)]
pub struct PingResponse {
    pub ping: String,
}

/// `/rest/system/version`.
#[derive(Deserialize)]
pub struct VersionResponse {
    /// e.g. `v2.1.7`.
    pub version: String,
}

/// `/rest/system/status`.
#[derive(Deserialize)]
pub struct StatusResponse {
    #[serde(rename = "myID")]
    pub my_id: DeviceId,
}

/// `/rest/system/connections`.
#[derive(Deserialize)]
pub struct ConnectionsResponse {
    /// By device ID.
    pub connections: BTreeMap<DeviceId, Connection>,
}

#[derive(Deserialize)]
pub struct Connection {
    pub connected: bool,
}
