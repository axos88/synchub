//! Whose Syncthing something belongs to: this device's or the hub's. The
//! types that differ between the two carry it - a folder's path is a
//! directory here only on this device - so they can't be mixed up.

use std::fmt;

/// This device's Syncthing, or the hub's.
pub trait Side {
    /// What errors call it, e.g. "the hub".
    const WHO: &'static str;
    /// Where its operations run, for their reports.
    const PLACE: Place;
    /// Whose it is, for what it has and hasn't: "this device", "the hub".
    const OWNER: &'static str;
}

/// This device's Syncthing.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Local {}

/// The hub's Syncthing.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Hub {}

impl Side for Local {
    const WHO: &'static str = "Syncthing";
    const PLACE: Place = Place::Local;
    const OWNER: &'static str = "this device";
}

impl Side for Hub {
    const WHO: &'static str = "the hub";
    const PLACE: Place = Place::Hub;
    const OWNER: &'static str = "the hub";
}

/// Where a step of an operation runs - what its report starts with, e.g.
/// "hub: sharing Notes with laptop".
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Place {
    Local,
    Hub,
}

impl fmt::Display for Place {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(match self {
            Place::Local => "local",
            Place::Hub => "hub",
        })
    }
}
