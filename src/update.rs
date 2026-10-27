//! synchub updating itself. A release build checks GitHub's `latest` release
//! when it starts; if that was built from another commit, it downloads its own
//! platform's binary, checks it against the digest GitHub publishes, puts it
//! in its own place, and runs it instead - with the same arguments. A local
//! build (`cargo install`) never does.

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use anyhow::{Context, Result, bail};
use serde::Deserialize;

use crate::settings;
use crate::version;
use crate::{step, warn};

const RELEASE: &str = "https://api.github.com/repos/axos88/synchub/releases/tags/latest";

/// In the environment of the binary an update starts, so it doesn't update
/// again.
const UPDATED: &str = "SYNCHUB_UPDATED";

/// What GitHub says about a release (the fields read).
#[derive(Deserialize)]
struct Release {
    /// The commit it was built from.
    target_commitish: String,
    assets: Vec<Asset>,
}

#[derive(Deserialize)]
struct Asset {
    name: String,
    browser_download_url: String,
    /// `sha256:<hex>`.
    digest: Option<String>,
}

/// Updates synchub if the latest release was built from another commit, and
/// runs that one instead, not returning. If anything goes wrong, it says so,
/// and this one goes on.
pub fn update() {
    if env::var_os(UPDATED).is_some() {
        remove_previous();
        return;
    }
    if let Err(err) = try_update() {
        warn!("synchub wasn't updated: {err:#}");
    }
}

fn try_update() -> Result<()> {
    let (Some(commit), Some(asset_name)) = (version::COMMIT, version::ASSET) else {
        return Ok(());
    };
    let http = reqwest::blocking::Client::builder()
        .user_agent(concat!("synchub/", env!("SYNCHUB_VERSION")))
        .timeout(Duration::from_secs(5))
        .build()?;
    let release: Release = http.get(RELEASE).send()?.error_for_status()?.json()?;
    if release.target_commitish == commit {
        return Ok(());
    }
    let asset = release
        .assets
        .iter()
        .find(|asset| asset.name == asset_name)
        .with_context(|| format!("the latest release has no {asset_name}"))?;
    let short = &release.target_commitish[..release.target_commitish.len().min(7)];
    if settings::dry_run() {
        step!("would update synchub to {short}");
        return Ok(());
    }
    let exe = env::current_exe().context("can't tell where synchub is")?;
    // Before downloading: a binary that can't be replaced isn't worth
    // fetching on every start.
    let new = beside(&exe, "new");
    fs::write(&new, b"").with_context(|| format!("can't write {}", new.display()))?;
    step!("updating synchub to {short}: downloading {}", asset.name);
    let installed = download(&http, asset, &new).and_then(|()| install(&exe, &new));
    if installed.is_err() {
        let _ = fs::remove_file(&new);
    }
    installed?;
    step!("updated synchub to {short}; running it");
    restart(&exe)
}

/// Downloads `asset` into `file`, checked against its digest.
fn download(http: &reqwest::blocking::Client, asset: &Asset, file: &Path) -> Result<()> {
    let binary = http
        .get(&asset.browser_download_url)
        .timeout(Duration::from_secs(300))
        .send()?
        .error_for_status()?
        .bytes()?;
    check_digest(&binary, asset.digest.as_deref())?;
    fs::write(file, &binary).with_context(|| format!("can't write {}", file.display()))
}

/// Fails unless `binary`'s SHA-256 is `digest` (`sha256:<hex>`).
fn check_digest(binary: &[u8], digest: Option<&str>) -> Result<()> {
    let Some(expected) = digest.and_then(|digest| digest.strip_prefix("sha256:")) else {
        bail!("the release gives no SHA-256 digest to check the download against");
    };
    let actual = sha256_hex(binary);
    if actual != expected {
        bail!("the download's SHA-256 is {actual}, not the release's {expected}");
    }
    Ok(())
}

fn sha256_hex(bytes: &[u8]) -> String {
    ring::digest::digest(&ring::digest::SHA256, bytes)
        .as_ref()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

/// `exe` with `.suffix` after its name, in its directory.
fn beside(exe: &Path, suffix: &str) -> PathBuf {
    let mut name = exe.file_name().unwrap_or_default().to_os_string();
    name.push(format!(".{suffix}"));
    exe.with_file_name(name)
}

/// Gives `new` `exe`'s permissions, then renames it over `exe` - in one
/// step: the running process keeps the file it started from.
#[cfg(unix)]
fn install(exe: &Path, new: &Path) -> Result<()> {
    fs::set_permissions(new, fs::metadata(exe)?.permissions())?;
    fs::rename(new, exe).with_context(|| format!("can't replace {}", exe.display()))
}

/// Windows doesn't let a running program's file be overwritten, but renamed:
/// it moves aside - kept until the new one has started - and `new` takes its
/// place; if that fails, it moves back.
#[cfg(windows)]
fn install(exe: &Path, new: &Path) -> Result<()> {
    let old = beside(exe, "old");
    fs::rename(exe, &old).with_context(|| format!("can't move {} aside", exe.display()))?;
    if let Err(err) = fs::rename(new, exe) {
        let _ = fs::rename(&old, exe);
        return Err(err).with_context(|| format!("can't replace {}", exe.display()));
    }
    Ok(())
}

/// The binary an update on Windows moved aside, once its replacement - this
/// one - has started.
fn remove_previous() {
    if cfg!(windows)
        && let Ok(exe) = env::current_exe()
    {
        let _ = fs::remove_file(beside(&exe, "old"));
    }
}

/// Runs `exe` in this process's place, with the same arguments.
#[cfg(unix)]
fn restart(exe: &Path) -> Result<()> {
    use std::os::unix::process::CommandExt;
    let err = Command::new(exe)
        .args(env::args_os().skip(1))
        .env(UPDATED, "1")
        .exec();
    Err(err).context("can't run the updated synchub")
}

/// Windows has no exec: runs `exe` with the same arguments, and exits with
/// what it exits with.
#[cfg(windows)]
fn restart(exe: &Path) -> Result<()> {
    let status = Command::new(exe)
        .args(env::args_os().skip(1))
        .env(UPDATED, "1")
        .status()
        .context("can't run the updated synchub")?;
    std::process::exit(status.code().unwrap_or(1));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_download_must_match_its_digest() {
        let digest = format!("sha256:{}", sha256_hex(b"synchub"));
        assert!(check_digest(b"synchub", Some(&digest)).is_ok());
        assert!(check_digest(b"tampered", Some(&digest)).is_err());
        assert!(check_digest(b"synchub", None).is_err());
    }

    #[test]
    fn the_sha256_is_written_in_hex() {
        assert_eq!(
            sha256_hex(b""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }

    #[test]
    fn files_beside_the_binary_keep_its_name() {
        assert_eq!(
            beside(Path::new("/usr/local/bin/synchub"), "new"),
            PathBuf::from("/usr/local/bin/synchub.new")
        );
    }
}
