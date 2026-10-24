//! A Syncthing folder's ID.

use std::fmt;
use std::str::FromStr;

use anyhow::bail;
use serde::{Deserialize, Serialize};

/// A folder's ID. Syncthing requires only that it's non-empty and unique
/// (lib/config/config.go); the web UI makes up random ones like `shy4a-ady3g`,
/// as does [`FolderId::generate`], but hand-made ones differ - e.g. `default`.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct FolderId(String);

impl FolderId {
    /// A random ID the way the web UI makes them: five and five lowercase letters or digits.
    pub fn generate() -> Self {
        const CHARS: &[u8] = b"abcdefghijklmnopqrstuvwxyz0123456789";
        let part = || -> String {
            (0..5)
                .map(|_| CHARS[rand::random_range(0..CHARS.len())] as char)
                .collect()
        };
        FolderId(format!("{}-{}", part(), part()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl FromStr for FolderId {
    type Err = anyhow::Error;

    fn from_str(id: &str) -> anyhow::Result<Self> {
        if id.is_empty() {
            bail!("a folder ID can't be empty");
        }
        Ok(FolderId(id.to_string()))
    }
}

impl fmt::Display for FolderId {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_ids_look_like_the_web_uis() {
        let id = FolderId::generate();
        let (a, b) = id.as_str().split_once('-').unwrap();
        assert_eq!((a.len(), b.len()), (5, 5));
        assert!(
            id.as_str()
                .chars()
                .all(|c| c == '-' || c.is_ascii_lowercase() || c.is_ascii_digit())
        );
    }

    #[test]
    fn any_non_empty_string_is_an_id() {
        assert_eq!("default".parse::<FolderId>().unwrap().as_str(), "default");
        assert!("".parse::<FolderId>().is_err());
    }

    #[test]
    fn serializes_as_a_plain_string() {
        let id: FolderId = serde_json::from_str("\"shy4a-ady3g\"").unwrap();
        assert_eq!(serde_json::to_string(&id).unwrap(), "\"shy4a-ady3g\"");
    }
}
