//! The shapes of Syncthing's REST API that synchub reads and sends, by area
//! of the API - only the fields synchub uses (a response's others are
//! ignored; a request's are left to Syncthing's defaults).

pub mod config;
pub mod db;
pub mod system;
