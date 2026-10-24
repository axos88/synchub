//! A Syncthing device's ID, and the picture the web UI draws for it.

pub mod identicon;

use std::fmt;
use std::str::FromStr;

use anyhow::bail;
use data_encoding::BASE32_NOPAD;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// A device's ID: the SHA-256 of its sync certificate in base32, with a Luhn
/// check character after every 13, in eight groups of seven - e.g.
/// `NLHZ5AC-IMOKBMU-4FWXEPU-CXFUK2Y-G5VFCRK-XGZBJ33-TV7VMVP-VPDVHAP`.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct DeviceId(String);

impl DeviceId {
    /// The device ID of a device's sync certificate (`der`).
    pub fn of_certificate(der: &[u8]) -> Self {
        let base32 = BASE32_NOPAD.encode(&Sha256::digest(der));
        let mut checked = Vec::new();
        for chunk in base32.as_bytes().chunks(13) {
            checked.extend_from_slice(chunk);
            checked.push(luhn_base32(chunk));
        }
        DeviceId(grouped(&checked))
    }

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

/// Syncthing's Luhn mod 32 check character (lib/protocol/luhn.go).
fn luhn_base32(chars: &[u8]) -> u8 {
    let (mut factor, mut sum) = (1, 0);
    for &c in chars {
        let codepoint = BASE32.iter().position(|&b| b == c).unwrap_or(0);
        let addend = factor * codepoint;
        factor = if factor == 2 { 1 } else { 2 };
        sum += addend / 32 + addend % 32;
    }
    BASE32[(32 - sum % 32) % 32]
}

#[cfg(test)]
mod tests {
    use rustls::pki_types::CertificateDer;
    use rustls::pki_types::pem::PemObject;

    use super::*;

    /// A throwaway test instance's sync certificate, and the device ID its Syncthing reported.
    const CERT: &str = "-----BEGIN CERTIFICATE-----
MIIBoDCCAVKgAwIBAgIJAK34EqACEpCZMAUGAytlcDBKMRIwEAYDVQQKEwlTeW5j
dGhpbmcxIDAeBgNVBAsTF0F1dG9tYXRpY2FsbHkgR2VuZXJhdGVkMRIwEAYDVQQD
EwlzeW5jdGhpbmcwHhcNMjYxMDIyMDAwMDAwWhcNNDYxMDE3MDAwMDAwWjBKMRIw
EAYDVQQKEwlTeW5jdGhpbmcxIDAeBgNVBAsTF0F1dG9tYXRpY2FsbHkgR2VuZXJh
dGVkMRIwEAYDVQQDEwlzeW5jdGhpbmcwKjAFBgMrZXADIQDv4iIARG+cJmiFCnnj
jTXe+BTGyepU7IxpobSoc7eEFKNVMFMwDgYDVR0PAQH/BAQDAgWgMB0GA1UdJQQW
MBQGCCsGAQUFBwMBBggrBgEFBQcDAjAMBgNVHRMBAf8EAjAAMBQGA1UdEQQNMAuC
CXN5bmN0aGluZzAFBgMrZXADQQDbMFnrRU71ORvs1lApILE9ilnMEyrkmitp/JDe
yuqFUufgBp6vUSX/ikeqJqcMquB948M5B85/v6bs2ODOrFEK
-----END CERTIFICATE-----";
    const ID: &str = "NLHZ5AC-IMOKBMU-4FWXEPU-CXFUK2Y-G5VFCRK-XGZBJ33-TV7VMVP-VPDVHAP";

    #[test]
    fn computes_syncthings_device_id() {
        let der = CertificateDer::from_pem_slice(CERT.as_bytes()).unwrap();
        assert_eq!(DeviceId::of_certificate(&der).as_str(), ID);
    }

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
