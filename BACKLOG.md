# Backlog

Next free ID: D-13

- D-12 · ~30 min · Drop `hub::hub_path` once Syncthing resolves relative folder paths - waiting for upstream
- D-7 · ~15 min · Join the real machines to the real hub (`synchub join`, needs the hub's API key)
- D-6 · ~30 min · Run the build workflow (binaries + rolling "latest" release): needs a push
- D-11 · ~½ day · Cache Syncthing's config in long-lived clients (the app), kept current by events
- D-8 · ~½ day · Prune a big file from the hub after it's ignored (`synchub prune`?) - waiting for Ákos
- D-10 · ~1 day · Same folder labels and device names on every device: a rename anywhere spreads (needs D-3)
- D-3 · ~1½ days · Background service: drops shares the hub stopped offering; later does gitignore too

### D-3 · Background service: leftover shares, then gitignore

The maintainer (2026-10-23): a background process, installed and started when a device joins
the hub (`synchub join`), removes from this device the shares the hub used to offer it but
doesn't any more - so a share removed on the hub (`synchub remove`, or unshared from this
device) disappears from every device without anyone running a command there. The same process
later takes over gitignore (below), and keeps folder labels and device names the same everywhere
(D-10). Not at app start: the service does it. Not implemented yet.

Decided (2026-10-23):
- Detecting a removal: this device shares the folder with the hub, but the hub doesn't share it
  with this device. (`synchub new` adds the folder here and then on the hub - shared back with
  this device - in one go, so that state is only momentary there; don't act on it mid-`new`.)
- No polling: watch the hub's event stream (`/rest/events` on the hub's API, config changes),
  and do a full check at start and whenever the hub's connection drops and comes back.
  Verified (2026-10-23): a config change on the hub - also a folder relabelled or a device
  renamed - comes as one `ConfigSaved` event on the hub's stream, carrying the whole config;
  nothing arrives on the leaves' own streams.

Deferred until the implementation:
- Whether removing a leftover keeps its files (dropping it from Syncthing's configuration only,
  as `synchub remove` without --delete-files does) or deletes them.
- Windows; service management as decided below for gitignores, but as
  `synchub service enable|start|stop|disable`, since the service does both jobs.

The gitignore part:

Since 2026-10-24 `gitignore` translates the .gitignore patterns themselves into Syncthing's, so a
new ignored file (a build output, a log) is ignored before it exists - no watcher needed for
that. What still needs a rerun is a changed .gitignore (or .git/info/exclude, core.excludesFile),
or a newly tracked file that a pattern matches (it gets a `!` line).

Decided (Ákos, 2026-10-22):
- Service management, `enable|start|stop|disable` (first planned as `synchub gitignores …`, now
  `synchub service …`, above). enable/disable control starting at boot and also start/stop;
  start/stop control the background process. On Linux a systemd user unit tied to
  syncthing.service (After=/PartOf=/WantedBy=; linger is on for akos), on macOS a launchd agent
  (RunAtLoad, KeepAlive).
- One service per machine; it takes the folder list from Syncthing and follows folders appearing
  and disappearing (ConfigSaved). Which folders it manages - opt-in ones (with a root
  .stignore-git) or all shares - is a config option in synchub's config.toml.
- Detection: Syncthing's event stream (`/rest/events`: LocalChangeDetected / RemoteChangeDetected
  for .gitignore files; reconnect + full pass after restarts) is enough now that the patterns
  are translated - an own file watcher was only needed to catch new ignored files quickly.
  .git/info/exclude, core.excludesFile and the index (tracked files) aren't in the folder's
  index, so they'd need polling or a file watcher of their own.

Next: build it - config option, the detection, the service management.

### D-6 · Run the build workflow

`.github/workflows/build.yml` builds Linux x86_64/x86/arm64 (static musl, via cross), macOS
x86_64/arm64 and Windows x86_64 on every push to master (artifacts on the run page) and attaches
them to the release of a `v*` tag; a `check` job runs fmt, clippy -D warnings and the tests.
After every push to master, the `latest` job replaces the "latest" pre-release (tag `latest`,
moved to the commit) with that commit's archives; README's install links point to
https://github.com/axos88/synchub/releases/download/latest/<archive> until proper releases.
Verified locally: the YAML parses, the check job's commands pass. Not verified: the builds
themselves - a local musl build fails only for lack of `x86_64-linux-musl-gcc`, which cross's
images provide.

Next: Ákos commits and pushes (or asks for it); then check the run and fix any target that
fails, or drop it if it's a pain (Ákos: "leave them out").
If ring fails to build for Windows or macOS, switch TLS to a pure-Rust rustls provider
(`rustls-pure`, TLS 1.3 only) - Ákos's decision, see D-5 in COMPLETED.md.

### D-7 · Join the real machines to the real hub

synchub is tested on a sandbox hub only. The real hub is https://syncthing.vandra.hu:8384
(certificate SHA-256 ED:34:08:AC:AE:03:4C:F3:57:BC:45:6D:3B:30:C8:87:DA:45:65:7F:E7:F1:78:5F:38:
72:7D:6E:14:AA:DC:1D, valid to 2027-05-14; device ID QIRKWUT-IDWX3B2-VQWCWIH-RA3D2AR-YIV2ZCM-
EY5UYI5-LK5OYWL-DSVLSQV).

Next: on each machine, `synchub join https://syncthing.vandra.hu:8384 <API key>` - it needs
the hub's API key, which Ákos has. On a machine whose folders are shared directly with devices
that have joined, join moves those shares to the hub.

### D-8 · Pruning a file from the hub after ignoring it

Ákos (2026-10-23): can the hub's storage be pruned when a big file was synced and then added to
.stignore? Not by Syncthing itself: ignoring a file on a leaf never deletes it elsewhere, and
deleting it on that leaf afterwards doesn't propagate either (changes to ignored files aren't
synced) - the hub and the other leaves keep it. The REST API can set a device's ignore patterns
(`POST /rest/db/ignores`) but can't delete files (checked lib/api/api.go, v2.1.7).

Options offered:
1. Prune everywhere, keep the local copy: move the file out of the folder, wait until the hub has
   synced the deletion (completion 100%), add the pattern to .stignore, move it back. All via the
   API; the other leaves lose their copies too.
2. Prune only the hub: set the pattern on the hub via `POST /rest/db/ignores`, delete the file on
   the hub's disk - needs shell (SSH) access to the hub; the leaves keep theirs.
Also: old versions on the hub live in its .stversions and go only by the versioning settings.

Next: Ákos picks one (recommended: 1, e.g. `synchub prune PATH`), or drops it.

### D-10 · Same folder labels and device names on every device

The maintainer (2026-10-24): another job of synchub is keeping folder labels and device names
in sync - every folder is labelled, and every device named, the same way on all devices, and
changing one on any device (web UI, `synchub rename-folder`/`rename-device`, the app) spreads to
all of them. Not implemented yet.

What Syncthing does (verified in the sandbox, v2.1.7, 2026-10-23): a folder's label and a
device's name are each device's own; changing them on one Syncthing doesn't reach the others.
Each change emits one `ConfigSaved` event on the instance changed - its data is the whole
configuration, there's no "label/name changed" event - and nothing on the other devices.

Today (2026-10-24): `synchub sync` (`commands::sync::sync`) takes the hub's labels and names on
this device - the hub-to-leaf half, run by hand. `rename-folder` and `rename-device` change the
hub only, then run it; the other leaves take the new names when they `synchub sync`. Left: the
service running it when the hub's config changes, and the leaf-to-hub half (a rename made in a
leaf's web UI).

A sketch, part of the D-3 service (it watches the event streams anyway):
- The hub's labels and names are the reference.
- On this device's own stream: on `ConfigSaved`, compare labels/names with the last ones seen;
  one changed here (e.g. in the web UI) is set on the hub too.
- On the hub's stream: one changed there is set here - which is how a rename made on any leaf
  reaches all the others (that leaf's service puts it on the hub first).
- At start, and whenever the hub's connection comes back: take the hub's labels/names.
- Only folders the hub has, and devices both know; a leaf's own folders stay as they are.

To decide at the implementation:
- Conflicts: renamed on two devices at once, or on a leaf while it was offline - the hub's wins
  at reconnect, or the newest? (Syncthing keeps no timestamp per label.)
- A device's own name (its entry for itself, which its web UI shows as "Device Name") - synced
  too, so a device is called the same on itself as everywhere else?
- `synchub join`: whether joining adopts the hub's labels/names, overwriting this device's.
- Whether `rename-*` keep changing the hub and here directly, or only one place and let the
  service spread it.

### D-11 · Cache the config in long-lived clients, kept current by events

The maintainer (2026-10-24): the config could be cached, with an events subscription updating it,
for a long-lived client like the app. Deferred: not needed for the commands, which are short.

Context:
- A Syncthing's whole configuration is one request: `Client::config()` (`/rest/config`, typed
  `api::config::Config<S>`; ~30 KB, a few ms on the sandbox). Verified 2026-10-24: the data of a
  `ConfigSaved` event (`/rest/events`) is exactly that object after the change.
- Today the app re-reads both configs (this device's and the hub's) on every load: at start,
  after each action, and on `r` (`app::state::State::read`).
- Commands read the config when they need it and exit, so a cache gains them nothing.

Sketch: the client keeps the last config (e.g. behind a `Mutex`); a background thread long-polls
`/rest/events?events=ConfigSaved&since=<last id>` and replaces it with each event's data;
`config()` returns the cached one when the subscription is live, and re-reads (and re-subscribes)
after the connection dropped. Writes still re-read the one object they change right before
writing (as `Client::share`/`unshare` do), so a stale cache can't overwrite changes made
meanwhile. The D-3 service would subscribe the same way - share the code.

Next step: decide whether the app opts in (e.g. `Client::subscribe()`), or every client
subscribes on first `config()`.

### D-12 · Drop `hub::hub_path` once Syncthing resolves relative folder paths

`POST /rest/config/folders` without a path gives the new folder the folder defaults' directory
itself (verified in the sandbox and in Syncthing's source, v2.1.7: `lib/api/confighandler.go`
starts from a copy of the defaults), so synchub computes `<defaults>/<label>` for the hub in
`hub::hub_path` - without being able to see whether that directory exists on the hub's disk.
Auto-accept does it properly, server side.

2026-10-24: the maintainer's fork has a change (`axos88/syncthing`, branch
`api-folder-default-path`, commit 3b89afed; it replaced an earlier auto-accept-style attempt):
when a folder is created through `POST /rest/config/folders` or `PUT .../{id}`, a relative path
is resolved against the default folder path instead of Syncthing's working directory, and one
that exists and isn't an empty directory is refused with 409. Not yet proposed upstream:
Syncthing takes no unsolicited PRs and no mostly-AI-written code - an issue first, written by the
maintainer.

Next step: if it's released, `HubClient::create_folder`, `new` and `hub::convert` send the folder
ID as a relative path (a 409 means the directory is taken), and `hub_path` goes - only when
every hub runs that release (check its version first).

