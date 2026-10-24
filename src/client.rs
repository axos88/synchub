//! The Syncthing API clients: this device's and the hub's, each connected on
//! first use and kept from then on.

pub mod hub;
pub mod local;
pub mod tls;

pub use hub::{HubClient, get as hub};
pub use local::{Client, ClientBuilder, get as local};
