# Completed

### D-1 · Old bash scripts in ~/Sync

2026-10-22: deleted `~/Sync/syncmv` and `~/Sync/syncput` (Ákos: "the bash scripts can be
deleted"); the deletion syncs to the other machines. The `syncmv`, `syncput` and `stgit` links
in `~/.local/bin` and the `sttools` binary were removed too: the tool is `syncstuff` now.

### D-2 · License

2026-10-22: MIT (Ákos's choice) - `LICENSE` added, `license = "MIT"` in Cargo.toml.

### D-4 · Hub-aware share management and node bootstrap

2026-10-22: built and tested on a sandbox (a hub with an HTTPS GUI, three leaves L1-L3 starting
as a direct mesh). What it does:
- `syncstuff hub join [URL] [API_KEY] [--fingerprint] [--folders-dir]`: trusts the hub's GUI
  certificate by SHA-256 fingerprint (confirmed, or given), HTTPS only; shows the hub's device
  ID; sets hub = introducer + auto-accept here (nobody else), folder defaults shared with no
  device; adds this device on the hub; saves url/api_key/device_id/cert_sha256 in
  `~/.config/syncstuff/config.toml` (0600). `hub show` shows them and the hub's state.
- Converting a network device by device (`src/hub/convert.rs`), at join and before every
  command that uses the hub: folders here shared directly with devices that have joined the hub
  get the hub folder (created if missing) shared with those devices, and here are shared with
  the hub instead; then the hub is paused and resumed so it introduces them again. Devices that
  haven't joined keep their direct share.
- `syncstuff share` (checkbox TUI), `share list|new|add|remove`; a missing argument opens a form
  (`share new`, `hub join`). Only this device's shares change.

Verified in the sandbox: join's refusals (http, wrong fingerprint, no terminal without
--fingerprint, wrong API key); L1, L2, L3 joining one by one converged to every leaf's folders
being self + hub + the others introduced by the hub, with files syncing at every step
(including to the not-yet-joined leaf); share new (to one device only), add, remove (not
re-added after 30 s, the others dropped the device); the TUI driven through tmux (manager
ticking and applying, the share-new form, the join form cancelled with Esc); a tampered pin is
refused. TLS moved from aws-lc-rs to ring meanwhile (D-5), re-verified against the hub.

### D-5 · Pure-Rust TLS provider?

2026-10-22: decided to stay on `ring` (Ákos: "stay on ring for now"). aws-lc-rs was already gone:
rustls uses ring, installed as the default provider in main.rs, reqwest built with
`rustls-no-provider`. Pure-Rust providers checked then: `rustls-rustcrypto` 0.0.2-alpha
(unaudited, last release 2024-04), `rustls-pure` 0.15.1 (new, TLS 1.3 only), `rustls-graviola`
0.4 (Rust + asm, x86_64/aarch64 only). Revisit only if ring causes build problems on Windows or
macOS - then switch to `rustls-pure` (the provider is set in `src/commands/hub/tls.rs` and
`src/main.rs`); D-6 checks this on the first CI run.

### D-6 · Run the build workflow

2026-10-26 (checked): it ran on the push of `bff0558` (2026-10-24, run 40128402328, 4m09s) and
every job passed - `check` (fmt, clippy -D warnings, tests) and the six builds: Linux x86_64, x86
and arm64 (static musl, via cross), macOS x86_64 and arm64, Windows x86_64. The `latest` job
published them as the "latest" pre-release (tag `latest` at `bff0558`, six archives of 2.9-3.7
MB). README's install links (`releases/download/latest/<archive>`) all answer 200. ring built
on every target, so the pure-Rust TLS fallback (D-5) isn't needed, and no target had to be
dropped. Later pushes re-run it and move `latest` along.
2026-10-26: the release holds the bare binaries now (`synchub-<platform>`, `.exe` on Windows),
not archives with README and LICENSE (Ákos: it's one file); README links them, with `chmod +x`
for Linux/macOS. Those links resolve once the workflow has run on the next push.

### D-9 · Naming a device this device doesn't know yet

2026-10-24: resolved by the maintainer's call - a FOLDER/DEVICE argument is looked up here first,
then on the hub (`api::config::Folder::resolve`, `Device::resolve`, by name/label or ID), since
the hub's labels and names are the reference now (`synchub sync`). So `share`/`unshare`/`new
--device`/`rename-device` name a device only the hub knows by its hub name or ID. Verified in
the sandbox with a device registered on the hub only ("Tablet"): `share aaa-new Tablet` shared
it (the following `unshare` found it on the folder). Why not just accept any well-formed ID: the
hub answers 200 to a folder's device list holding an ID it doesn't know, and drops it silently.
### D-14 · Text fields: a click doesn't move the cursor

2026-10-26: a click in a text field puts the cursor where it lands - in a form and in the
prompt dialog; on the field's border it only focuses it, past the end of the text the cursor
goes to the end. `FieldKind::mouse` takes the area the field was drawn at, passed by whoever
routed the click (the form's layout, the prompt's `field_area`), so the field records nothing
while drawing (in line with D-15); `TextInput::place_cursor(column, width)` works out where
the shown text starts the way `shown(width)` does - from the cursor and the width - so it needs
no remembered scroll position. The prompt now hands clicks on its field to the field (before,
they only moved the focus there). Tests: `TextInput` with and without scrolling, `TextField`
inside and on the border. On the sandbox: in the rename prompt, a click on the `t` of "Notes"
then X gave "NoXtes", a click on the border kept the cursor, a click past the end put it there;
in the new-share form's label field, "abcdef" then a click on its third character and X gave
"abXcdef".

