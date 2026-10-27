//! Bakes into the binary which build it is: its version - `git describe`'s,
//! e.g. `v0.2.0-3-gabc1234`, or the short commit before there's a tag - its
//! commit, and, for the release builds CI makes, the release asset it's
//! published as: only those update themselves. CI passes them in; a local
//! build asks git.

use std::env;
use std::process::Command;

fn main() {
    for var in ["SYNCHUB_VERSION", "SYNCHUB_COMMIT", "SYNCHUB_ASSET"] {
        println!("cargo:rerun-if-env-changed={var}");
    }
    // A local build's version follows the checkout, and whether it's dirty.
    for path in [".git/HEAD", ".git/refs", ".git/index", "src"] {
        println!("cargo:rerun-if-changed={path}");
    }
    // `v*` only: the rolling `latest` tag isn't a version.
    let describe = ["describe", "--tags", "--always", "--dirty", "--match", "v*"];
    let version = env::var("SYNCHUB_VERSION")
        .ok()
        .or_else(|| git(&describe))
        .unwrap_or_else(|| env!("CARGO_PKG_VERSION").to_string());
    println!("cargo:rustc-env=SYNCHUB_VERSION={version}");
    if let Some(commit) = env::var("SYNCHUB_COMMIT")
        .ok()
        .or_else(|| git(&["rev-parse", "HEAD"]))
    {
        println!("cargo:rustc-env=SYNCHUB_COMMIT={commit}");
    }
    if let Ok(asset) = env::var("SYNCHUB_ASSET") {
        println!("cargo:rustc-env=SYNCHUB_ASSET={asset}");
    }
}

/// What `git args` prints; None if it fails, e.g. outside a checkout.
fn git(args: &[&str]) -> Option<String> {
    let output = Command::new("git").args(args).output().ok()?;
    let text = String::from_utf8_lossy(&output.stdout).trim().to_string();
    (output.status.success() && !text.is_empty()).then_some(text)
}
