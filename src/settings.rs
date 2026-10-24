//! synchub's own settings: the hub this device is joined to - and, for this
//! run, whether it's a dry run. They're global: [`init`] sets them up once,
//! and [`get`], [`update`] and [`dry_run`] reach them from anywhere.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard, OnceLock, PoisonError};

use anyhow::{Context, Result};
use once_cell::sync::OnceCell;
use serde::{Deserialize, Serialize};

use crate::device_id::DeviceId;
use crate::paths;
use crate::say;

#[derive(Serialize, Deserialize, Default, Clone)]
pub struct Settings {
    pub hub: Option<Hub>,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct Hub {
    /// The hub's GUI/API address, https://...
    pub url: String,
    pub api_key: String,
    /// The hub's device ID, verified on every connection.
    pub device_id: DeviceId,
    /// Where the hub listens for devices (host:port); its certificate there
    /// proves the device ID. Empty in files from before it was stored.
    #[serde(default)]
    pub sync_address: String,
}

impl Hub {
    pub fn sync_address(&self) -> String {
        if self.sync_address.is_empty() {
            default_sync_address(&self.url)
        } else {
            self.sync_address.clone()
        }
    }
}

/// What this run was started with.
struct Run {
    file: PathBuf,
    dry_run: bool,
    /// The settings file's contents, read when first needed: commands that
    /// don't use the hub work without it.
    settings: OnceCell<Mutex<Settings>>,
}

static RUN: OnceLock<Run> = OnceLock::new();

/// Sets up the settings for this run: they're read from, and saved in,
/// `file`. Called once, before anything else.
pub fn init(file: PathBuf, dry_run: bool) {
    let run = Run {
        file,
        dry_run,
        settings: OnceCell::new(),
    };
    assert!(RUN.set(run).is_ok(), "settings::init runs once");
}

fn run() -> &'static Run {
    RUN.get().expect("settings::init runs first")
}

/// A copy of the settings file's contents.
pub fn get() -> Result<Settings> {
    Ok(contents()?.clone())
}

/// Changes the settings file's contents with `change`, and saves them - or,
/// in a dry run, only says it would.
pub fn update(change: impl FnOnce(&mut Settings)) -> Result<()> {
    let file = file();
    if dry_run() {
        say!("would save synchub's settings in {}", file.display());
        return Ok(());
    }
    let mut settings = contents()?;
    change(&mut settings);
    write(file, &settings)?;
    say!("saved synchub's settings in {}", file.display());
    Ok(())
}

/// The settings file's contents, locked: they're only ever handed out
/// copied, or changed in [`update`], so no lock is held for long.
fn contents() -> Result<MutexGuard<'static, Settings>> {
    let run = run();
    let settings = run
        .settings
        .get_or_try_init(|| load(&run.file).map(Mutex::new))?;
    Ok(settings.lock().unwrap_or_else(PoisonError::into_inner))
}

/// The settings file.
pub fn file() -> &'static Path {
    &run().file
}

/// Whether this is a dry run: nothing is changed, what would be is reported.
pub fn dry_run() -> bool {
    run().dry_run
}

/// Where the hub at `url` listens for devices unless told otherwise: port 22000 of its host.
pub fn default_sync_address(url: &str) -> String {
    format!("{}:22000", host(url))
}

/// The host part of an https:// URL (with brackets, for IPv6).
fn host(url: &str) -> &str {
    let rest = url.strip_prefix("https://").unwrap_or(url);
    let authority = rest.split('/').next().unwrap_or(rest);
    if authority.starts_with('[') {
        return authority
            .split(']')
            .next()
            .map_or(authority, |h| &authority[..h.len() + 1]);
    }
    authority.split(':').next().unwrap_or(authority)
}

pub fn default_path() -> PathBuf {
    let base = if cfg!(target_os = "macos") {
        paths::home().join("Library/Application Support")
    } else {
        std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| paths::home().join(".config"))
    };
    base.join("synchub").join("config.toml")
}

fn load(file: &Path) -> Result<Settings> {
    match fs::read_to_string(file) {
        Ok(text) => toml::from_str(&text)
            .with_context(|| format!("can't read synchub's settings in {}", file.display())),
        Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(Settings::default()),
        Err(err) => Err(err).with_context(|| format!("can't read {}", file.display())),
    }
}

/// Writes `settings`, readable by the user only: it holds the hub's API key.
fn write(file: &Path, settings: &Settings) -> Result<()> {
    if let Some(dir) = file.parent() {
        fs::create_dir_all(dir)?;
    }
    let text = toml::to_string(settings)?;
    let mut options = fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
        options.mode(0o600);
        // An existing file keeps its mode on open; tighten it too.
        if file.exists() {
            fs::set_permissions(file, fs::Permissions::from_mode(0o600))?;
        }
    }
    io::Write::write_all(&mut options.open(file)?, text.as_bytes())
        .with_context(|| format!("can't write {}", file.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    const ID: &str = "NLHZ5AC-IMOKBMU-4FWXEPU-CXFUK2Y-G5VFCRK-XGZBJ33-TV7VMVP-VPDVHAP";

    #[test]
    fn sync_address_defaults_to_port_22000_of_the_hub() {
        assert_eq!(
            default_sync_address("https://hub.example.com:8384"),
            "hub.example.com:22000"
        );
        assert_eq!(default_sync_address("https://hub/x"), "hub:22000");
        assert_eq!(default_sync_address("https://[::1]:8384"), "[::1]:22000");
    }

    #[test]
    fn round_trips_and_is_private() {
        let dir = std::env::temp_dir().join(format!("synchub-settings-{}", std::process::id()));
        let file = dir.join("config.toml");
        let settings = Settings {
            hub: Some(Hub {
                url: "https://hub:8384".into(),
                api_key: "KEY".into(),
                device_id: ID.parse().unwrap(),
                sync_address: "hub:22000".into(),
            }),
        };
        write(&file, &settings).unwrap();
        let loaded = load(&file).unwrap().hub.unwrap();
        assert_eq!(
            (loaded.url.as_str(), loaded.device_id.as_str()),
            ("https://hub:8384", ID)
        );
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                fs::metadata(&file).unwrap().permissions().mode() & 0o777,
                0o600
            );
        }
        fs::remove_dir_all(&dir).unwrap();
        assert!(load(&file).unwrap().hub.is_none());
    }
}
