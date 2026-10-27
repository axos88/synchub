//! Which build this is (build.rs).

/// `git describe`'s: e.g. `v0.2.0-3-gabc1234` - three commits after v0.2.0 -
/// or the short commit before there's a tag.
pub const VERSION: &str = env!("SYNCHUB_VERSION");

/// The commit it was built from, if known.
pub const COMMIT: Option<&str> = option_env!("SYNCHUB_COMMIT");

/// The release asset it's published as - set only for CI's release builds,
/// the ones that update themselves.
pub const ASSET: Option<&str> = option_env!("SYNCHUB_ASSET");
