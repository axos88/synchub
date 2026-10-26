//! A Syncthing device's ID, and the picture the web UI draws for it.

pub mod identicon;

use std::fmt;
use std::str::FromStr;

use anyhow::bail;
use serde::{Deserialize, Serialize};

/// A device's ID: the SHA-256 of its sync certificate in base32, with a Luhn
/// check character after every 13, in eight groups of seven - e.g.
/// `NLHZ5AC-IMOKBMU-4FWXEPU-CXFUK2Y-G5VFCRK-XGZBJ33-TV7VMVP-VPDVHAP`.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct DeviceId(String);

impl DeviceId {
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Its first group, which the web UI shows for a device without a name.
    pub fn short(&self) -> &str {
        self.0.get(..7).unwrap_or(&self.0)
    }
}

/// A device ID as given by a person: dashes and spaces optional, any case.
impl FromStr for DeviceId {
    type Err = anyhow::Error;

    fn from_str(id: &str) -> anyhow::Result<Self> {
        let chars: String = id
            .chars()
            .filter(|c| !matches!(c, '-' | ' '))
            .map(|c| c.to_ascii_uppercase())
            .collect();
        if chars.len() != 56 || !chars.bytes().all(|b| BASE32.contains(&b)) {
            bail!("{id}: not a Syncthing device ID");
        }
        Ok(DeviceId(grouped(chars.as_bytes())))
    }
}

impl fmt::Display for DeviceId {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(&self.0)
    }
}

const BASE32: &[u8; 32] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ234567";

/// `chars` in groups of seven, joined by dashes.
fn grouped(chars: &[u8]) -> String {
    chars
        .chunks(7)
        .map(|group| String::from_utf8_lossy(group).into_owned())
        .collect::<Vec<_>>()
        .join("-")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A device ID, as Syncthing writes it.
    const ID: &str = "NLHZ5AC-IMOKBMU-4FWXEPU-CXFUK2Y-G5VFCRK-XGZBJ33-TV7VMVP-VPDVHAP";

    #[test]
    fn parses_ids_as_people_write_them() {
        let id: DeviceId = ID.replace('-', "").to_lowercase().parse().unwrap();
        assert_eq!(id.as_str(), ID);
        assert!("NLHZ5AC-IMOKBMU".parse::<DeviceId>().is_err());
    }

    #[test]
    fn its_short_form_is_the_first_group() {
        assert_eq!(ID.parse::<DeviceId>().unwrap().short(), "NLHZ5AC");
    }

    #[test]
    fn serializes_as_a_plain_string() {
        let id: DeviceId = serde_json::from_str(&format!("\"{ID}\"")).unwrap();
        assert_eq!(serde_json::to_string(&id).unwrap(), format!("\"{ID}\""));
    }
}
