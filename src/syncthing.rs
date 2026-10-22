//! The local Syncthing instance, over its REST API.

use std::fs;
use std::path::{Path, PathBuf};
use std::thread::sleep;
use std::time::{Duration, Instant};

use anyhow::{Context, Result, bail};
use reqwest::Method;
use reqwest::blocking::{Client as Http, RequestBuilder, Response};
use serde::Deserialize;
use serde::de::DeserializeOwned;
use serde_json::Value;

use crate::paths;

/// How to reach Syncthing.
#[derive(clap::Args)]
pub struct Connection {
    /// Syncthing's GUI/API address, e.g. 127.0.0.1:8384 [default: from config.xml]
    #[arg(long, env = "STGUIADDRESS", global = true)]
    gui_address: Option<String>,
    /// Syncthing's API key [default: from config.xml]
    #[arg(long, env = "STGUIAPIKEY", global = true, hide_env_values = true)]
    api_key: Option<String>,
    /// Syncthing's configuration directory, the one holding config.xml [default: Syncthing's default]
    #[arg(long, env = "STHOMEDIR", global = true)]
    home: Option<PathBuf>,
}

#[derive(Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Folder {
    pub id: String,
    pub label: String,
    pub path: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub paused: bool,
    pub send_ownership: bool,
    pub sync_ownership: bool,
}

impl Folder {
    pub fn local_path(&self) -> PathBuf {
        paths::canon(&paths::expand_tilde(&self.path))
    }

    pub fn name(&self) -> String {
        if self.label.is_empty() {
            self.id.clone()
        } else {
            format!("{} ({})", self.label, self.id)
        }
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FolderStatus {
    pub state: String,
    #[serde(default)]
    pub error: String,
    pub local_files: u64,
}

pub struct Client {
    http: Http,
    base: String,
    api_key: String,
}

impl Client {
    pub fn connect(connection: &Connection) -> Result<Self> {
        let (address, api_key, tls) = match (&connection.gui_address, &connection.api_key) {
            (Some(address), Some(key)) => (address.clone(), key.clone(), false),
            _ => {
                let file = config_file(connection.home.as_deref())?;
                let gui = read_gui_config(&file)?;
                (
                    connection.gui_address.clone().unwrap_or(gui.address),
                    connection.api_key.clone().unwrap_or(gui.api_key),
                    gui.tls,
                )
            }
        };
        // Syncthing's GUI certificate is self-signed.
        let http = Http::builder()
            .timeout(None)
            .danger_accept_invalid_certs(true)
            .build()?;
        let client = Client {
            http,
            base: base_url(&address, tls)?,
            api_key,
        };
        client.get::<Value>("system/ping").with_context(|| {
            format!("can't reach Syncthing at {} - is it running?", client.base)
        })?;
        Ok(client)
    }

    pub fn folders(&self) -> Result<Vec<Folder>> {
        self.get("config/folders")
    }

    /// Changes some of a folder's settings, e.g. `json!({"paused": true})`.
    pub fn update_folder(&self, id: &str, changes: Value) -> Result<()> {
        let path = format!("config/folders/{}", encode(id));
        self.send(self.request(Method::PATCH, &path).json(&changes))?;
        Ok(())
    }

    pub fn status(&self, id: &str) -> Result<FolderStatus> {
        self.get(&format!("db/status?folder={}", encode(id)))
    }

    /// Scans the folder; returns when the scan is done.
    pub fn scan(&self, id: &str) -> Result<()> {
        self.send(self.request(Method::POST, &format!("db/scan?folder={}", encode(id))))?;
        Ok(())
    }

    /// "Override Changes" of a send-only folder: this device's files win everywhere.
    pub fn override_changes(&self, id: &str) -> Result<()> {
        self.send(self.request(Method::POST, &format!("db/override?folder={}", encode(id))))?;
        Ok(())
    }

    /// The error Syncthing found in the folder's ignore patterns, if any.
    pub fn ignores_error(&self, id: &str) -> Result<Option<String>> {
        let ignores: Value = self.get(&format!("db/ignores?folder={}", encode(id)))?;
        Ok(ignores["error"]
            .as_str()
            .filter(|e| !e.is_empty())
            .map(String::from))
    }

    pub fn wait_idle(&self, id: &str) -> Result<FolderStatus> {
        let deadline = Instant::now() + Duration::from_secs(3600);
        loop {
            if let Ok(status) = self.status(id) {
                match status.state.as_str() {
                    "idle" => return Ok(status),
                    "error" => bail!("folder {id}: {}", status.error),
                    _ => {}
                }
            }
            if Instant::now() > deadline {
                bail!("timed out waiting for folder {id} to become idle");
            }
            sleep(Duration::from_secs(1));
        }
    }

    fn request(&self, method: Method, path: &str) -> RequestBuilder {
        self.http
            .request(method, format!("{}/rest/{path}", self.base))
            .header("X-API-Key", &self.api_key)
    }

    fn send(&self, request: RequestBuilder) -> Result<Response> {
        let response = request.send()?;
        let status = response.status();
        if !status.is_success() {
            let body = response.text().unwrap_or_default();
            bail!("Syncthing answered {status}: {}", body.trim());
        }
        Ok(response)
    }

    fn get<T: DeserializeOwned>(&self, path: &str) -> Result<T> {
        Ok(self.send(self.request(Method::GET, path))?.json()?)
    }
}

struct GuiConfig {
    address: String,
    api_key: String,
    tls: bool,
}

fn config_file(home: Option<&Path>) -> Result<PathBuf> {
    let dirs = match home {
        Some(home) => vec![home.to_path_buf()],
        None => std::env::var_os("STCONFDIR")
            .map(PathBuf::from)
            .into_iter()
            .chain(default_config_dirs())
            .collect(),
    };
    dirs.iter()
        .map(|dir| dir.join("config.xml"))
        .find(|file| file.is_file())
        .with_context(|| {
            let tried: Vec<_> = dirs.iter().map(|d| d.display().to_string()).collect();
            format!(
                "no Syncthing config.xml in {}; use --home or --gui-address and --api-key",
                tried.join(", ")
            )
        })
}

fn default_config_dirs() -> Vec<PathBuf> {
    let home = paths::home();
    if cfg!(target_os = "macos") {
        return vec![home.join("Library/Application Support/Syncthing")];
    }
    let xdg = |var, default: &str| {
        std::env::var_os(var)
            .map(PathBuf::from)
            .unwrap_or_else(|| home.join(default))
    };
    vec![
        xdg("XDG_STATE_HOME", ".local/state").join("syncthing"),
        xdg("XDG_CONFIG_HOME", ".config").join("syncthing"),
    ]
}

fn read_gui_config(file: &Path) -> Result<GuiConfig> {
    let text = fs::read_to_string(file).with_context(|| format!("reading {}", file.display()))?;
    let doc =
        roxmltree::Document::parse(&text).with_context(|| format!("parsing {}", file.display()))?;
    let gui = doc
        .descendants()
        .find(|n| n.has_tag_name("gui"))
        .with_context(|| format!("no <gui> in {}", file.display()))?;
    let child = |name| {
        gui.children()
            .find(|n| n.has_tag_name(name))
            .and_then(|n| n.text())
            .unwrap_or("")
            .trim()
            .to_string()
    };
    Ok(GuiConfig {
        address: child("address"),
        api_key: child("apikey"),
        tls: gui.attribute("tls") == Some("true"),
    })
}

fn base_url(address: &str, tls: bool) -> Result<String> {
    if address.starts_with("unix://") || address.starts_with('/') {
        bail!(
            "Syncthing's GUI listens on a unix socket ({address}); pass --gui-address with a TCP address"
        );
    }
    if address.contains("://") {
        return Ok(address.trim_end_matches('/').to_string());
    }
    let address = match address
        .strip_prefix("0.0.0.0:")
        .or_else(|| address.strip_prefix("[::]:"))
        .or_else(|| address.strip_prefix(':'))
    {
        Some(port) => format!("127.0.0.1:{port}"),
        None => address.to_string(),
    };
    Ok(format!(
        "{}://{address}",
        if tls { "https" } else { "http" }
    ))
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
    fn base_url_maps_wildcard_addresses_to_localhost() {
        assert_eq!(
            base_url("0.0.0.0:8384", false).unwrap(),
            "http://127.0.0.1:8384"
        );
        assert_eq!(
            base_url("[::]:8384", true).unwrap(),
            "https://127.0.0.1:8384"
        );
        assert_eq!(base_url(":8384", false).unwrap(), "http://127.0.0.1:8384");
        assert_eq!(
            base_url("127.0.0.1:8384", false).unwrap(),
            "http://127.0.0.1:8384"
        );
        assert_eq!(
            base_url("https://host:1/", false).unwrap(),
            "https://host:1"
        );
        assert!(base_url("unix:///run/st.sock", false).is_err());
    }

    #[test]
    fn encode_escapes_reserved_characters() {
        assert_eq!(encode("shy4a-ady3g"), "shy4a-ady3g");
        assert_eq!(encode("a b/c"), "a%20b%2Fc");
    }

    #[test]
    fn reads_gui_settings() {
        let dir = std::env::temp_dir().join(format!("sttools-test-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let file = dir.join("config.xml");
        fs::write(
            &file,
            r#"<configuration><gui enabled="true" tls="true"><address>127.0.0.1:9999</address><apikey>KEY</apikey></gui></configuration>"#,
        )
        .unwrap();
        let gui = read_gui_config(&file).unwrap();
        fs::remove_file(&file).unwrap();
        assert_eq!(
            (gui.address.as_str(), gui.api_key.as_str(), gui.tls),
            ("127.0.0.1:9999", "KEY", true)
        );
    }
}
