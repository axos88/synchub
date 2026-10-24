# Project instructions

## Keep README.md up to date

Every change to synchub's behaviour, commands, options, configuration or supported platforms
updates `README.md` in the same change. The README starts with what synchub is for and how the
hub/leaf topology works (and why it means fewer conflicts and less sync lag); keep that
introduction accurate as the hub logic evolves.

## Code layout and style

- The commands never open screens: a missing argument is an error.
- `src/commands.rs` + `src/commands/`: one module per subcommand (`join`, `list`, `new`, `get`,
  `remove`, `mv`, `overwrite`, `gitignore`), children in a directory beside the module file (never
  `mod.rs`). Each command's operation is a `pub fn` named after it, taking typed values -
  `share::share`, `unshare::unshare`, `mv::move_recursive`, `get::get`, `remove::remove`, `new::create`,
  `sync::sync`, `gitignore::gitignore`, `overwrite::overwrite`, `join::join_hub`, ...; a command
  built from others calls theirs (`get` = `share::share` with this device,
  then `mv::move_recursive`) rather than reimplementing them. Each operation does its own dry run.
- `src/client.rs` + `src/client/`: the two API clients, globals made on first use and kept
  (`once_cell::sync::OnceCell::get_or_try_init` - std's `OnceLock` has no stable fallible
  version: a failure isn't kept, concurrent first uses wait for the one initializer running).
  `client::local()` is this device's `Client<Local>` (`client/local.rs`: also the generic
  `Client<S>` and its `ClientBuilder<S>`; `client::local::init` gets the `Connection` options
  first, in `main`). `client::hub()` is the hub's (`client/hub.rs`: `HubClient`, derefs to
  `Client<Hub>`, knows the hub's
  `id()`): on first use it reads the hub's settings, verifies the hub (sync port certificate, then
  `HubClient::connect` checks the API's device ID) and runs `commands::convert::convert` - so
  once per process. Nothing passes the clients around: an operation or helper
  fetches the one it uses (`let local = client::local()?;`). Exceptions, both taking the hub
  as a parameter: `commands::convert::convert`, which runs while `client::hub()` is
  being made (an initializer mustn't use its own cell: it would wait for itself), and `join`, which
  works with the hub it's joining, not the saved one.
- Whose Syncthing something belongs to is in its type (`src/side.rs`: the markers `Local` and
  `Hub`, with `Side::WHO` for messages and `Side::PLACE`, the `side::Place` value - `local` or
  `hub` - that starts a report: `say!`/`step!`/`warn!` take it first, as a bare `Local`/`Hub`
  or, in generic code, `S::PLACE`): `Client<S>`, `api::config::Config<S>`
  (written `Config<Local>`/`Config<Hub>`) and `api::config::Folder<S>` (written `Folder<Local>`/`Folder<Hub>`), so a
  hub folder can't be passed where this device's is meant - its path is a directory on the hub.
  Only `Folder<Local>` has `local_path()`/`overlaps()`, only `Config<Local>` finds folders by
  directory. Devices aren't split: names and IDs mean the same on both.
- Operations read what they need when they need it - no snapshot is loaded up front. A
  Syncthing's whole configuration is one request (`Client::config()`, `/rest/config`, ~30 KB;
  verified: a `ConfigSaved` event's data is exactly that object): use it wherever more than one
  list is needed, instead of separate `folders()`/`devices()` calls.
- There's no `hub` module: what's about the hub lives with what it works on. Verifying it by
  device ID is `HubClient`'s (`probe_device_id`, an associated function: `join` uses it before
  there's a client); where a new folder goes on it is `Config<Hub>::new_folder_path`; where it
  listens for devices by default is `settings::default_sync_address`; moving direct shares over
  is `commands::convert::convert`, which `client::hub()` runs on first connecting (the client
  calling into the commands for it) and `join` when joining - so `synchub convert` just
  connects. Sharing through the hub is
  `HubClient`'s (`share_folder`, `unshare_folder`); where a folder goes on a leaf is
  auto-accept's choice (see "Auto-accepting" below).
- `src/settings.rs`: synchub's own settings (`Settings`), global - named so it isn't confused
  with Syncthing's `api::config::Config`. `settings::init(file, dry_run)` in `main`; the file is
  read on first use (so commands not using the hub work with a broken or missing one).
  `settings::get()` returns a copy, `settings::update(|settings| ...)` changes and saves it (in a
  dry run only says it would). No guard is handed out: one held across a call that reads the
  settings again deadlocks (it did: a `match ...get()?...` kept it through the hub's connecting).
  `settings::dry_run()` is the `-n` flag.
- A FOLDER or DEVICE argument is resolved on a configuration - variables holding one are
  named `local_config` / `hub_config`, and likewise `local_folder` / `hub_folder` and
  `local_device` / `hub_device` for what's read from them (not `here` / `on_hub`) - each method failing with one clear error, so the
  commands don't add their own. `resolve_folder` is `find_folder` (an `Option`) plus that
  error. `local_config.find_folder(key)`: by directory, ID or label; the hub's: ID or label.
  `hub_config.resolve_folder(Some(&local_config), key)`: by ID or label on the hub, or by the
  directory of a folder this device has - looked up here, then on the hub by its ID - so it
  always returns the hub's folder (`Folder<Hub>`), never mixing in this device's. Commands acting
  on the hub (`share`, `unshare`, `remove`, `rename-folder`, `get`) use the hub's; `mv` uses this
  device's; `gitignore` and `mv`'s nesting check want the folder a path is inside
  (`Config<Local>::folder_containing`), and `overwrite` the one at a directory (`find_folder`). So
  a folder only this device has can't be named for those hub commands. `resolve_device(key)`
  (on either config) is used on the hub: since `sync` it knows every device a folder can be
  shared with, under the same names. Other lookups within a configuration are `Config`'s too
  (`find_device`, `folder`, `device`, `device_name`); comparing two configs is
  the caller's (`sync`'s). By ID alone the
  hub is asked with `Client::find_folder`/`find_device` (None on its 404, verified). Note: the
  hub answers 200 to a folder's device list holding an ID it doesn't know, and drops that ID
  silently (verified) - so never share with an ID that wasn't found on the hub.
- `share` takes this device too (then it accepts the folder); `unshare` refuses only the hub
  itself. `get --path` waits for the folder it had the hub share to arrive here
  (`Client::wait_for_folder`, up to half a minute; in a dry run it says it would), then
  moves this device's copy with `mv::move_recursive`.
- Operations report with `say!` (what was done; stdout), `warn!` and `step!` (a step as it starts,
  e.g. "pausing Notes"; stderr) from `src/output.rs`, never `println!`/`eprintln!`. Give a
  slow operation steps, announced before the slow part.
- Changes to Syncthing go through `Client`'s high-level operations (`pause`,
  `resume`, `set_path`, `share`, `unshare`, `add_folder`, `remove_folder`, `set_introducer`, ...),
  never raw JSON: each one reports itself with a descriptive `step!` (`here:` / `hub:` prefix),
  and its generic PATCH/POST helpers stay private. `share`/`unshare` read the folder's current
  device list first, so changes made meanwhile (e.g. an auto-accepted hub share) aren't lost.
- Clients are built: `ClientBuilder::local(&connection)` / `ClientBuilder::hub(url, api_key)`
  (setup errors wait for `connect()`), and its `connect()` builds, connects (pings) and returns
  the connected client; `HubClient::connect` builds on it and checks the hub's device ID. Both
  use one rustls configuration accepting any certificate (`client::tls::config()`; the probe of
  the hub's sync port uses it too, and reads the certificate off the connection, which rustls
  keeps although Syncthing then ends the handshake - verified): this device's GUI certificate
  is self-signed, and the hub is verified by device ID instead.
- A request Syncthing refuses fails with `client::local::Refused` (it reads "<who> answered
  <status>: <body>"). An operation on an object by ID that may not be there doesn't look it up
  first: it sends the change, and Syncthing's 404 becomes `client::local::NotFound` ("the hub has
  no folder abcde-12345") - e.g. `Client::rename_folder`/`rename_device`. A dry run sends
  nothing, so only it looks the object up, to fail the same way.
  Both live in `src/client/local/error.rs` (re-exported from `client::local`); their `Display`
  and `Error` come from `derive_more`.
- Names: the hub's folder labels and device names are the reference. A command changing them
  changes the hub only, then runs `commands::sync::sync()`, which compares the hub's with this
  device's and takes the hub's here (`synchub sync`; each folder/device on its own, failures
  collected). In a dry run the hub isn't changed, so the sync after it has nothing to show.
- Dry run (`-n`) is global, `settings::dry_run()`: the client's operations then only `say!`
  "would ..."; what's done outside the API (moving directories, writing files, deleting) checks
  it too. Nothing takes a dry-run argument.
- `src/api.rs` + `src/api/`: serde structs for Syncthing's REST API - every request and every
  response has one (`config` for `/rest/config/...`, `db` for `/rest/db/...`, `system` for
  `/rest/system/...`): responses carry the fields synchub reads, requests the fields it sends
  (`NewFolderRequest`, `FolderPatchRequest`, `NewDeviceRequest`, `DevicePatchRequest`). Only the
  API's requests and responses are suffixed (`...Request`, `...Response`); a type modelling a
  part of the system keeps its plain name (`Folder`, `Device`, `FolderStatus`, `Completion`,
  `FolderDevice`, `FolderType`, `Config`), with a `...Response` alias where the API returns it
  (`FolderResponse<S> = Folder<S>`, `ConfigResponse<S> = Config<S>`), used in the client's
  signatures. Never build or read raw JSON
  (`json!`, `serde_json::Value`, `["field"]`), and no public function returns it. Field names
  are checked against the docs and a live Syncthing (v2.1.7): e.g. a folder's device entries have
  exactly `deviceID`, `introducedBy`, `encryptionPassword` (`FolderDevice`), and `db/ignores`
  has an `error` the docs don't mention. `Folder` and `Device` are plain data with helpers
  reading their own fields only (`name()`, `shared_with_device()`, `local_path()`, `display()`, ...);
  anything looking across a list is `Config`'s - use it rather than matching by hand.
- Refer to them by module: `use crate::api;` and then `api::config::Folder`,
  `api::config::Device`, `api::db::FolderStatus` - never import the types themselves.
- Shared helpers at the root: `syncthing` (where this device's Syncthing is: the `Connection`
  options, else config.xml's `<gui>` - `gui_config`, then `GuiConfig::endpoint`; nothing else of
  config.xml is read, and it's never written), `paths`, `prompt` (the commands' y/n question).
- A folder's ID is the newtype `FolderId` in `folder_id` (not `api`: it's a Syncthing concept
  used everywhere, the API only carries it). Unlike the `api` types it's imported directly
  (`use crate::folder_id::FolderId;`) and written plainly as `FolderId`. Syncthing accepts any non-empty,
  unique folder ID (`lib/config/config.go`); only the web UI's generated IDs (and
  `FolderId::generate`) follow the `xxxxx-xxxxx` pattern, so never validate against it.
- A device's ID is the newtype `DeviceId` in `device_id`, used the same way (imported directly,
  never a `String`/`&str` holding an ID). Parsing it (`FromStr`, which clap uses for
  `--device-id`) takes it as people write it - dashes and spaces optional, any case - and gives
  the canonical form; serde doesn't validate (Syncthing sends the canonical form).
  `DeviceId::of_certificate` computes it from a sync certificate; `short()` is the first group,
  which the web UI shows for a device without a name; `device_id::identicon` draws the web
  UI's picture of a device ID (`join` shows it when asking whether a device ID is the hub's).
  Name/ID matching takes the user's key as a
  string (`Config::find_device`).

## How a command is written (`src/commands/mv.rs` is the model)

- `run(args)` only turns the arguments into typed values (e.g. resolves SOURCE to an
  `api::config::Folder<Local>`) and calls the command's operation: a `pub` function taking those typed
  values. The clients and settings are globals (`client::local()`,
  `client::hub()`, `config`), never parameters.
- The operation reads as a short sequence of named steps; each step is a small function getting
  only what it needs. Helpers follow in the order they're called.
- No closures beyond one-liners (`with_context`, iterator adapters); no loops inside loops; no side
  effects inside `filter`.
- Changes go through `Client` operations; for what happens outside the API, check
  `settings::dry_run()` - nothing takes a dry-run argument.
- Output: `say!` results, `step!` progress, `warn!` problems - never `println!`/`eprintln!`.
- Over several folders: try each one, collect the failures (`map` then `filter_map`), warn about
  each, then fail listing them - one failing mustn't leave the rest behind.
- Errors name the folder or path, and say what to do about them.

## How the hub works (what synchub automates)

- The hub decides which device sees which folder: every folder is shared with the hub, and on the
  hub the devices that see it are ticked. The hub keeps a copy of every folder, unencrypted.
- Leaves: the hub is their only introducer and the only device whose folders they auto-accept
  (`introducer`, `autoAcceptFolders`, `skipIntroductionRemovals: false`). `convert` turns both
  off only for the other devices the hub has (the maintainer's decision) - one that hasn't joined
  keeps its settings until it does. Folder defaults share with no device, so nothing is shared automatically.
- The hub: auto-accept off for every device, no introducers. If the hub auto-accepted, a device
  removed from a folder would be added back, because the device still announces the folder.
- A leaf learns from the hub which other devices share each folder; the leaves then sync with
  each other directly. Unticking a device on the hub makes the other leaves drop it too - but the
  hub removes only what it introduced: a device added to a folder by hand on a leaf stays.
- Removals go hub first, then the leaf: removed only on the leaf, the hub still offers the folder
  and the leaf auto-accepts it again. A leaf can't leave a folder on its own; ignoring the folder
  on the leaf doesn't help either (Syncthing issue #10935).
- Converting an existing network (`src/commands/convert.rs`): a folder shared directly with
  devices that have joined the hub is created/updated on the hub with those devices, and on the
  leaf shared with the hub instead; pausing and resuming the hub makes it introduce them again.
  Devices that have joined lose introducer and auto-accept here; devices that haven't keep their
  direct share and those settings. It runs when `client::hub()` first
  connects (which is all `synchub convert` does), so every command that uses the hub reads the
  hub's config once for it.

## Facts worth knowing

- Syncthing's ignore files (verified in its source, `lib/ignore/ignore.go`, `lib/fs/filesystem.go`):
  an `#include` path is resolved relative to the including file, but the included patterns are
  used as they are - anchored to the root of the folder reading them; the first matching
  pattern wins; a pattern without a leading `/` is compiled as itself and `**/` + itself (any
  depth); `foo/` becomes `foo/**` (the directory's contents; the empty directory still syncs);
  `{a,b}` is alternation (a `.gitignore` means braces literally, so they're escaped). `.stfolder`,
  `.stignore` and `.stversions` at a folder's root are "internal": ignored before any pattern
  is consulted, so `!/.stignore` can't make it sync; the same names further down are ordinary
  files - an enclosing folder syncs a nested folder's `.stignore` and `.stfolder`.
- `gitignore` translates patterns, it doesn't list ignored files (the maintainer's decision, so
  that a device without e.g. `target/` doesn't drop it from the synced list). Per repository
  (`gitignore/repo.rs`, through gitoxide, not by running `git`): the `.gitignore` files git
  reads (a directory walk of the files it doesn't ignore - so none in ignored directories or
  nested repositories), the deepest first; `.git/info/exclude` and `core.excludesFile`
  (falling back to `$XDG_CONFIG_HOME/git/ignore`) for the local list; the tracked files the
  rules match (from the index), which get `!` lines first, since git never ignores tracked
  files; `core.ignorecase` → a `(?i)` prefix. `gitignore/translate.rs` anchors each line to its
  `.gitignore`'s directory relative to the folder root (an unanchored pattern below the root
  becomes `/dir/**/x`; and since git's `**/` also matches no directory while Syncthing's needs
  one - its parser adds the variant only for unanchored `**/` lines - every `/**/` is also
  written collapsed to `/`: `/dir/x`, `a/**/b` → `/a/b` too; verified against git on the
  sandbox); from a `.gitignore`
  above the folder's root, the segments up to a `**` are matched against the way down
  (`gix::glob::wildmatch`). A negation re-including from a literally named directory the rules
  exclude is dropped, as git can't apply it. Known gap: an outer repository's any-depth
  patterns also reach into a nested repository. Each folder has `.stignore-git` (synced; `!`
  and the local list's name matched at any depth, for nested folders' lists) and
  `.stignore-git-local` (not synced), one section per repository (`// repo <rel>`, `.` for the
  root, `..` above it), the innermost first, all tracked `!` lines before the patterns. The synced
  list ends with `#include .stignore-git-local`, so `.stignore` needs one line; Syncthing fails
  on a missing included file ("file not found", `lib/ignore/ignore.go`), so the local list is
  always written, even empty, before `.stignore` includes the synced one.
  `PATH` is a scope, like `git status PATH`: the repositories at or around it (overlapping a
  folder) are read again and replace their sections in every folder at, around or inside it.
  Verified on the sandbox: for every file of the fixture (tracked file matching a pattern,
  negation, a negation git can't apply, nested repository, submodule, a nested folder),
  Syncthing's verdict (`/rest/db/file`) equals `git check-ignore`'s.
- A Syncthing GUI on a Unix socket (verified on the sandbox, and in Syncthing's
  `lib/config/guiconfiguration.go`): `config.xml` stores the socket's bare path as `<address>`, and
  its `tls` attribute still applies; `STGUIADDRESS` (our `--gui-address`) writes it
  `unix:///path`, or `unixs:///path` for TLS. reqwest's `ClientBuilder::unix_socket` (Unix only)
  sends every request through it - the URL's host is ignored, its scheme still picks TLS. A plain
  request to a TLS GUI gets redirected to https, which reqwest follows over the same socket.

- `remove` removes a folder from the hub and here; the other devices can't be reached, so they
  keep their copy (D-3's service is to drop such leftovers). `--delete-files` empties it
  everywhere first: it deletes the files here (keeping `.stfolder` and `.stignore`), rescans, and
  waits until the hub and every connected device sharing it have caught up (`Client::wait_synced`,
  10 minutes each, failing before anything is removed) - verified in the sandbox. It refuses
  receive-only/encrypted folders (deletions there don't spread) and folders with others nested
  inside.
- Adding a folder (`POST /rest/config/folders`, verified in the sandbox, v2.1.7, 2026-10-24):
  fields not sent take the folder defaults. A missing label is just empty; a missing path is the
  defaults' directory itself - not a directory under it, as the web UI and auto-accept make - so
  every folder added without a path would share it. `NewFolderRequest` therefore always sends
  a path (`Config<Hub>::new_folder_path` on the hub: its folder defaults' directory under the folder ID -
  unique, so no two folders meet; a label can repeat or change), absolute or `~/...`: a relative path isn't taken relative
  to the defaults' directory but to the Syncthing process's working directory, every time the
  folder is opened (`lib/fs/basicfs.go` `newBasicFilesystem`: `ExpandTilde`, then `filepath.Abs`).
- Auto-accepting (verified in the sandbox, v2.1.7, 2026-10-24): when the hub shares a folder
  with a leaf, the leaf adds it within about a second, at its folder defaults' directory under
  the folder's label - or, if that directory exists already (it had a file in it), under the
  folder's ID instead; it doesn't merge into an existing directory.
- Renaming (verified in the sandbox, v2.1.7): a folder's label and a device's name are each
  device's own - changing them on one Syncthing doesn't reach the others. Each change emits one
  `ConfigSaved` event on the instance changed (its data is the whole configuration; there's no
  specific "label/name changed" event), and nothing on the other devices.
- Syncthing allows a folder inside another one, but discourages it (its FAQ: possible "but it has
  its caveats"; circular shares must be avoided). synchub never creates one (`new` refuses
  overlaps via `Folder::overlaps`, `get --path` and `mv` via `mv`'s check, `overwrite` needs `-f`), but copes with existing ones: `mv`
  moves the nested folders along and repoints them (verified), and refuses to move a nested folder
  out of its parent without `-f`.
- `synchub mv` moves into DEST when that's a directory with something in it (like `mv`);
  otherwise DEST is the new path, and an empty directory there is replaced.
- TLS uses rustls with the `ring` provider (installed as the default in `main.rs`; reqwest is
  built with `rustls-no-provider`): ring builds for every release target with just a C
  compiler.
  If ring ever fails to build on Windows or macOS, switch to a pure-Rust provider (`rustls-pure`)
  rather than back to aws-lc-rs.
- The hub is verified by its device ID only (the maintainer's decision), at `join`: computed from the
  certificate on the hub's sync port (`HubClient::probe_device_id`: a TLS handshake with ALPN `bep/1.0`;
  Syncthing then rejects us for lacking a client certificate, but its own certificate has been
  seen). It's the sync certificate's hash, so it can't be faked there. Given with `--device-id` it
  is checked silently; with `-y` it's accepted unchecked; otherwise it's shown with the web UI's identicon (`src/device_id/identicon.rs`,
  identical to Syncthing's `identiconDirective.js`) for the user to confirm. The ID the API
  reports must match. `client::hub()` repeats both checks against the saved `device_id` (and
  `sync_address`, defaulting to port 22000 of the hub's host) on every command. The API's own
  certificate (`https-cert.pem`, not `cert.pem`) isn't checked
  at all - HTTPS only, any certificate - so the API key relies on TLS encryption alone.
- `join`'s positional API key has the clap id `hub_api_key`: `api_key` is the global option
  for the local Syncthing's key, and clap merges arguments with the same id.
- Changing a folder's devices works on its current state, fetched by ID - not the state loaded at
  the start of a command: `add --path --device` changes it in between.
- Deferred work is in `BACKLOG.md`, finished items in `COMPLETED.md`.
