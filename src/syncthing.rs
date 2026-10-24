//! Where this device's Syncthing is: the options saying so (`Connection`),
//! or else its config.xml.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};

use crate::paths;

/// How to reach Syncthing.
#[derive(clap::Args)]
pub struct Connection {
    /// Syncthing's GUI/API address, e.g. 127.0.0.1:8384, or a Unix socket: unix:///path (unixs:// with TLS) [default: from config.xml]
    #[arg(long, env = "STGUIADDRESS", global = true)]
    pub gui_address: Option<String>,
    /// Syncthing's API key [default: from config.xml]
    #[arg(long, env = "STGUIAPIKEY", global = true, hide_env_values = true)]
    pub api_key: Option<String>,
    /// Syncthing's configuration directory, the one holding config.xml [default: Syncthing's default]
    #[arg(long, env = "STHOMEDIR", global = true)]
    pub config_dir: Option<PathBuf>,
}

/// Where Syncthing's GUI/API listens, and its API key.
pub struct GuiConfig {
    /// host:port, a URL, or a Unix socket: a path, or unix:// (unixs:// with TLS) and one.
    pub address: String,
    pub api_key: String,
    pub tls: bool,
}

/// Where a Syncthing's API takes requests.
pub struct Endpoint {
    /// What the requests' URLs start with; over a socket only its scheme counts.
    pub base: String,
    /// The Unix socket the requests go through, if it listens on one.
    pub socket: Option<PathBuf>,
}

impl Endpoint {
    /// Where it is, for people: its URL, or its socket's.
    pub fn address(&self) -> String {
        match &self.socket {
            Some(path) if self.base.starts_with("https:") => format!("unixs://{}", path.display()),
            Some(path) => format!("unix://{}", path.display()),
            None => self.base.clone(),
        }
    }
}

/// The GUI/API's address and key from `connection`, the rest from Syncthing's
/// config.xml - which is read only if something is missing.
pub fn gui_config(connection: &Connection) -> Result<GuiConfig> {
    if let (Some(address), Some(api_key)) = (&connection.gui_address, &connection.api_key) {
        return Ok(GuiConfig {
            address: address.clone(),
            api_key: api_key.clone(),
            tls: false,
        });
    }
    let gui = read_gui_config(&config_file(connection.config_dir.as_deref())?)?;
    Ok(GuiConfig {
        address: connection.gui_address.clone().unwrap_or(gui.address),
        api_key: connection.api_key.clone().unwrap_or(gui.api_key),
        tls: gui.tls,
    })
}

impl GuiConfig {
    /// Where the API takes requests. Its address is read as Syncthing reads
    /// it (lib/config/guiconfiguration.go).
    pub fn endpoint(&self) -> Result<Endpoint> {
        let address = self.address.as_str();
        let socket = match address.split_once("://") {
            Some(("unix", path)) => Some((path, false)),
            Some(("unixs", path)) => Some((path, true)),
            None if address.starts_with('/') => Some((address, self.tls)),
            _ => None,
        };
        if let Some((path, tls)) = socket {
            if !cfg!(unix) {
                bail!(
                    "Syncthing's GUI listens on a Unix socket ({address}), which needs a Unix system"
                );
            }
            return Ok(Endpoint {
                base: format!("{}://localhost", scheme(tls)),
                socket: Some(PathBuf::from(path)),
            });
        }
        if address.contains("://") {
            return Ok(Endpoint {
                base: address.trim_end_matches('/').to_string(),
                socket: None,
            });
        }
        // Listening everywhere, it's reached on this machine.
        let address = match address
            .strip_prefix("0.0.0.0:")
            .or_else(|| address.strip_prefix("[::]:"))
            .or_else(|| address.strip_prefix(':'))
        {
            Some(port) => format!("127.0.0.1:{port}"),
            None => address.to_string(),
        };
        Ok(Endpoint {
            base: format!("{}://{address}", scheme(self.tls)),
            socket: None,
        })
    }
}

fn scheme(tls: bool) -> &'static str {
    if tls { "https" } else { "http" }
}

fn config_file(config_dir: Option<&Path>) -> Result<PathBuf> {
    let dirs = match config_dir {
        Some(dir) => vec![dir.to_path_buf()],
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
                "no Syncthing config.xml in {}; use --config-dir, or --gui-address and --api-key",
                tried.join(", ")
            )
        })
}

fn default_config_dirs() -> Vec<PathBuf> {
    let home = paths::home();
    if cfg!(target_os = "macos") {
        return vec![home.join("Library/Application Support/Syncthing")];
    }
    if cfg!(windows) {
        let local = std::env::var_os("LOCALAPPDATA")
            .map(PathBuf::from)
            .unwrap_or_else(|| home.join("AppData/Local"));
        return vec![local.join("Syncthing")];
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

#[cfg(test)]
mod tests {
    use super::*;

    fn endpoint(address: &str, tls: bool) -> (String, Option<PathBuf>) {
        let gui = GuiConfig {
            address: address.to_string(),
            api_key: String::new(),
            tls,
        };
        let endpoint = gui.endpoint().unwrap();
        (endpoint.base, endpoint.socket)
    }

    #[test]
    fn wildcard_addresses_are_reached_on_localhost() {
        let base = |address, tls| endpoint(address, tls).0;
        assert_eq!(base("0.0.0.0:8384", false), "http://127.0.0.1:8384");
        assert_eq!(base("[::]:8384", true), "https://127.0.0.1:8384");
        assert_eq!(base(":8384", false), "http://127.0.0.1:8384");
        assert_eq!(base("127.0.0.1:8384", false), "http://127.0.0.1:8384");
        assert_eq!(base("https://host:1/", false), "https://host:1");
    }

    #[test]
    fn unix_sockets_are_read_as_syncthing_reads_them() {
        let socket = Some(PathBuf::from("/run/st.sock"));
        let http = ("http://localhost".to_string(), socket.clone());
        let https = ("https://localhost".to_string(), socket);
        assert_eq!(endpoint("/run/st.sock", false), http);
        assert_eq!(endpoint("/run/st.sock", true), https);
        assert_eq!(endpoint("unix:///run/st.sock", true), http);
        assert_eq!(endpoint("unixs:///run/st.sock", false), https);
    }

    #[test]
    fn reads_gui_settings() {
        let dir = std::env::temp_dir().join(format!("synchub-test-{}", std::process::id()));
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
