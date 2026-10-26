# synchub

synchub manages a personal [Syncthing](https://syncthing.net/) network - your
laptops, desktops and servers - built around one always-on **hub**. It joins a
device to the hub, converts an existing network to the hub one device at a
time, and shows everything in an app where each device chooses its shares -
and who else sees them - a key at a time. It also carries a few helpers for
everyday Syncthing chores: moving a synced directory, replacing a share's
contents everywhere, and keeping what git ignores out of the sync.

It drives the local Syncthing (and the hub) over their REST API; it doesn't
replace Syncthing, and every setting it changes is visible in the web UI.

## The hub and the leaves

```
                 ┌──────────────────────┐
                 │         hub          │  always on; decides which
                 │ (has every folder)   │  device sees which folder
                 └──▲───────▲───────▲───┘
         introduces │       │       │ introduces
          ┌─────────┴┐ ┌────┴─────┐ ┌┴─────────┐
          │ laptop   │◄┤ desktop  ├►│ laptop 2 │   leaves
          └──────────┘ └──────────┘ └──────────┘
             leaves that share a folder also sync with each other directly
```

- Every folder is shared with the **hub**, and on the hub you tick the devices
  that see it. The hub keeps a copy of every folder.
- The other devices are **leaves**. A leaf accepts folders from the hub
  automatically, and uses the hub as its *introducer*: for each folder, the
  hub tells it which other leaves share it, and the leaves then also sync
  with each other directly - over the LAN, when they're on the same one.
- Only the hub decides. The hub doesn't auto-accept and has no introducers; the
  leaves accept nothing from anyone but the hub. Removing a device from a
  folder happens on the hub first - otherwise the hub offers the folder
  again, and the leaf accepts it again.
- Names are the same everywhere *(planned)*: Syncthing lets every device
  label each folder and name each device its own way. synchub is to keep
  them the same on all devices - renaming a folder or a device on one device
  renames it on all of them. Today `synchub rename-folder` and
  `rename-device` rename on the hub and then `synchub sync` the device they
  run on; the other devices take the new names when they `synchub sync`.

### Fewer conflicts

In a plain mesh, two devices exchange changes only while both are online. A
laptop that was closed while you edited on the desktop still has the old
version; edit the same file there, and Syncthing has two diverging versions
to reconcile - a `.sync-conflict` file. With devices that are rarely on at the
same time, this window is long.

With the hub, every change goes to the hub as soon as the device that made it
is online, and every device picks up the latest version from the hub as soon
as it comes online - typically before you start editing there. The window in
which two devices hold diverging versions shrinks from "until they happen to
be online together" to "until each has seen the hub", and conflicts become
rare.

The hub also keeps the *sharing* consistent: each folder's device list lives in
one place instead of being edited by hand on every device, so you don't get
folders shared one way only, devices re-adding each other, or removals that
come back.

### Less sync lag

In a mesh, a change reaches another device only when both are online at the
same time - possibly days later. The hub stores it and passes it on, so it
travels between devices that are never on at the same time. Devices that *are*
on together still sync directly, so the hub adds availability, not a detour.

The hub has a plain copy of every folder: it has to be a machine you trust.

## Install

There are no numbered releases yet. The
[`latest`](https://github.com/axos88/synchub/releases/tag/latest) release
always holds the binaries of the newest commit on master:

| Platform | Download |
|---|---|
| Linux x86_64 | [synchub-linux-x86_64.tar.gz](https://github.com/axos88/synchub/releases/download/latest/synchub-linux-x86_64.tar.gz) |
| Linux x86 (32-bit) | [synchub-linux-x86.tar.gz](https://github.com/axos88/synchub/releases/download/latest/synchub-linux-x86.tar.gz) |
| Linux arm64 | [synchub-linux-arm64.tar.gz](https://github.com/axos88/synchub/releases/download/latest/synchub-linux-arm64.tar.gz) |
| macOS x86_64 | [synchub-macos-x86_64.tar.gz](https://github.com/axos88/synchub/releases/download/latest/synchub-macos-x86_64.tar.gz) |
| macOS arm64 | [synchub-macos-arm64.tar.gz](https://github.com/axos88/synchub/releases/download/latest/synchub-macos-arm64.tar.gz) |
| Windows x86_64 | [synchub-windows-x86_64.zip](https://github.com/axos88/synchub/releases/download/latest/synchub-windows-x86_64.zip) |

The Linux binaries are static, so they run on any distribution. For example:

```sh
curl -L https://github.com/axos88/synchub/releases/download/latest/synchub-linux-x86_64.tar.gz | tar xz
install synchub-linux-x86_64/synchub ~/.local/bin/
```

Or build it: `cargo install --git https://github.com/axos88/synchub`.

Syncthing must be running for the user. synchub reads its GUI address and API
key from `config.xml` (`~/.local/state/syncthing` or `~/.config/syncthing` on
Linux, `~/Library/Application Support/Syncthing` on macOS,
`%LOCALAPPDATA%\Syncthing` on Windows); `--config-dir`, `--gui-address` and
`--api-key` (or `STHOMEDIR`, `STGUIADDRESS`, `STGUIAPIKEY`) override that. A
GUI on a Unix socket works too: in `config.xml` its address is the socket's
path; with `--gui-address` it's `unix:///path`, or `unixs:///path` with TLS.

synchub's own settings - the hub's address, API key, device ID and sync address - are in `~/.config/synchub/config.toml`
(`~/Library/Application Support/synchub` on macOS), readable by you only;
`--config` or `SYNCHUB_CONFIG` points elsewhere.

## Setting up a device

The hub itself is an ordinary Syncthing with its GUI on HTTPS, auto-accept off
for every device, no introducers, and folder defaults shared with no device.

```sh
synchub join https://hub.example.com:8384 <the hub's API key> [--device-id <the hub's device ID>]
```

(`synchub init` is the same command.)

Or just run `synchub`: on a device that hasn't joined a hub, the app starts
with a form asking for the hub's address, API key and the directory for new
folders - chosen in the
directory browser (Space or a click on the field) - then shows the
hub's device ID and picture for you to confirm, and joins. The log shows the
`synchub join` command that does the same - with the API key left out: it
runs with the key in the `SYNCHUB_HUB_API_KEY` environment variable. In every form, Enter moves to the
next required field that's still empty, and submits once none is.

`join` first checks that the address leads to the hub, by the **device ID**
the hub's API reports. With `--device-id`, it's checked against that without
asking; with `-y`, whatever ID is reported is accepted unchecked; otherwise
`join` shows the ID together with the picture the hub's web UI draws for it,
for you to compare:

```
The device ID of the Syncthing at https://hub.example.com:8384:

    ██      ██
    ██  ██  ██
    ██      ██
    ██      ██
        ██

    NLHZ5AC-IMOKBMU-4FWXEPU-CXFUK2Y-G5VFCRK-XGZBJ33-TV7VMVP-VPDVHAP
```

This catches an address that leads to another Syncthing; it doesn't
authenticate the hub. The API's (self-signed) certificate isn't checked, so
the API key is protected by TLS encryption only, not by verifying who receives
it - and an impostor can report any device ID.

Then it

- here: makes the hub the introducer whose folders are accepted automatically,
  and makes new folders shared with no device by default
  (`--default-folders-dir` sets where they go);
- on the hub: adds this device, without auto-accept or introducer;
- saves the hub's address, API key and device ID;
- converts the existing network (see below).

Every later command that uses the hub checks again that its API reports the
saved device ID - otherwise the command refuses to use it (the address leads
elsewhere, or the hub was set up anew: then `join` again).

## The app

```sh
synchub        # or: synchub app, synchub ui, synchub run
```

opens the app: this device and the hub side by side - each with the picture
its web UI draws for it, name, device ID, address and Syncthing version, and
for the hub whether its API is reachable and whether this device is connected
to it - and below them every folder here, with its state (in sync, syncing,
scanning, ...) and, for each device it's shared with, whether that device is
connected and how far it has synced. The hub's shares this device doesn't have
are listed at the end.

The highlighted folder opens up: under its devices it lists the hub's other
devices it isn't shared with yet, as `[ ]`, so sharing it with one is a key
away. The status bar lists the keys that do something for the highlighted row,
and what - e.g. `(Enter, Space, s) share with laptop` - leaving out the others:

| Key | On a folder | On a device under a folder | On a hub share that isn't here |
|---|---|---|---|
| Enter | move it to another directory or name (see below) | share with it / stop sharing with it | get it on this device |
| Space | - | share with it / stop sharing with it | - |
| `s` / `u` | - | share with it / stop sharing with it | - |
| `m` | move it | move its folder | - |
| `d` | remove it from this device and the hub, once you confirm - the files stay | - | - |
| `g` | gitignore it | gitignore its folder | - |
| `a` | - | - | get it on this device |
| `R` | rename it (on the hub and here) | rename the device (on the hub and here) | rename it on the hub |

Moving opens a directory browser with the destination path above it. The
path is the highlighted directory's (`./` is the one shown, `../` its
parent); → or a double click go into the highlighted directory, ← goes up.
Typing replaces the highlighted entry with a name in the directory shown (↑ on
the first row, or a click, edits the path as it is instead), and the list
follows the path as you type, highlighting the entry it names exactly - or
nothing. When the name only begins a directory's, the rest is shown inverted:
Tab completes it, and Tab / Shift-Tab again cycle through every directory the
name begins - the completed part stays inverted until you type on. ↓ goes
back to the list, at the highlighted entry. Enter, in the list or the path,
opens the highlighted directory (or goes up, on `../`); with `./` or nothing
highlighted it moves the folder to exactly the path - a new one, or an empty
directory, which it replaces. The keys line says what Enter does now; the
status line says where the folder goes, or why it can't. Ctrl-H goes to the
home directory.

And anywhere: `n` shares a new directory (a form for the directory, the label
and the devices; it starts in its directory browser - `j`/`k` move, Enter
opens, `-` goes up, Space chooses the highlighted directory, `c` the current
one, `~` or Ctrl-H home - and Space or a click on the directory field opens it
again), `r` refreshes, `q` quits - also while the app loads or refreshes,
without waiting for a slow hub. `j`/`k` or the arrow
keys move; the mouse selects, double-click is Enter, the wheel scrolls.

Every change is made on the hub, the way the hub works: sharing a folder with
another device means the hub shares it, and the other device gets it
automatically. An action runs in the background, with a small dialog saying
what's being done and listing its steps as they happen (e.g. for a move:
pausing the folder, moving the directory, pointing the folder at the new
place, resuming it). If it fails, the dialog shows the error and stays until
you press OK (Enter, Space, Esc or a click on the button) - so does an action
needing the hub while the hub can't be reached. A refresh that fails says why
the same way, and the app keeps showing what it last loaded. Questions are dialogs too, with
buttons: removing asks you to confirm, renaming asks for the new name (Tab, or
↓ and ↑, moves between the name and the OK and Cancel buttons; Enter in the
name presses the button chosen last). The status bar lists a dialog's keys. The log above the
status bar shows the commands that do the same from the command line - the
last three, e.g. `$ synchub share sdbyy-l7snl laptop`.

### Converting an existing network

You don't have to switch every device at once. When a device joins - and
before every later command that uses the hub - synchub looks for folders
here that are shared **directly** with devices that have joined the hub since.
For each, the hub gets the folder (if it doesn't have it yet) shared with the
same devices; here, those direct shares are replaced by the share with the
hub, which then introduces the devices again. Devices that have joined also
stop being introducers or auto-accepted here - only the hub is. Devices that
haven't joined keep their direct share, and their introducer and auto-accept
settings, until they do. The folders keep syncing throughout.

## Commands

Everything the app does is also a command, for scripts:

| Command | What it does |
|---|---|
| `synchub list` | The hub's shares, which are here, and which devices also have them. |
| `synchub convert` | Moves folders shared directly with devices that have joined the hub over to the hub (see "Converting an existing network" above). Every command that uses the hub also does this when it first connects. |
| `synchub new PATH [--label L] [--device D]...` | Shares a directory of this device - unlabelled unless `--label` is given, as the web UI does: here with the hub only, on the hub (in its folder defaults' directory, under the folder ID) with this device and the devices given. |
| `synchub get FOLDER [--path P]` | Gets one of the hub's shares here: the hub shares it with this device, which accepts it into its folder defaults' directory (under the label, or the folder ID if that directory exists already); with `--path`, it's then moved there, as `synchub mv` moves it. |
| `synchub remove FOLDER [--delete-files]` | Removes a folder from the hub and from this device; the other devices keep their copy. With `--delete-files`, it first empties the folder everywhere - deletes the files here and waits until every connected device sharing it has deleted them too - then deletes its directory here. |
| `synchub share FOLDER DEVICE` | Has the hub share a folder with a device too - this one included, which then accepts it. A folder only this device has is created on the hub first. |
| `synchub unshare FOLDER DEVICE` | Has the hub stop sharing a folder with a device; it keeps its copy until it removes it. |
| `synchub rename-folder FOLDER LABEL` | Gives a folder a new label on the hub, then syncs (below). Directories keep their names. |
| `synchub rename-device DEVICE NAME` | Renames a device on the hub, then syncs (below). |
| `synchub sync` | Takes the hub's folder labels and device names on this device - for the folders and devices both have, this one included, empty ones too. Every Syncthing keeps its own label for each folder and name for each device; this makes this device's the hub's. |

FOLDER is a folder's ID or label on the hub, or the directory of a folder this
device has (meaning the hub's folder with the same ID); `mv`, `overwrite` and
`gitignore` work on a directory here, so for them it's the directory, ID or
label of a folder on this device. DEVICE is looked up on the
hub: its name, its device ID, or the ID's first seven characters at least, in
any case. The hub's labels and
names are the reference (`synchub sync`). The
commands never open screens of their own - a missing argument is an error; the
app is `synchub` on its own. Each change is reported as it's made
(e.g. `local: pausing Notes`, `hub: sharing Notes with laptop`), on stderr.
`-n` / `--dry-run`, with any command, changes nothing: it prints what would be
done instead (`local: would pause Notes`).

## Helpers

| Command | What it does |
|---|---|
| `synchub mv SOURCE DEST` | Moves a Syncthing folder's directory - SOURCE is the folder's directory, ID or label - and repoints it, and the folders nested inside it. Like `mv`, into DEST if that's a directory with something in it; otherwise DEST is the new path (an empty directory there is replaced). Nothing is re-synced. |
| `synchub overwrite LOCAL_DIR FOLDER_ID` | Makes an existing folder take its contents from `LOCAL_DIR`, replacing every other device's copy (changed files overwritten, missing ones deleted). |
| `synchub gitignore [PATH]` | Makes Syncthing ignore what git ignores, through synced `.stignore-git` lists. |

### `overwrite`

Pauses the folder, points it at `LOCAL_DIR` as send-only with "send ownership"
on (so every file is re-read, even where size and mtime match the old
directory's), scans, runs "Override Changes", and restores the folder's type
and settings. `LOCAL_DIR` isn't modified (a `.stfolder` marker is added if
missing); the folder's previous directory is left in place. Files the other
devices have that `LOCAL_DIR` doesn't are deleted there - also ones this device
never had. It doesn't ask first; try it with `-n`.

### `gitignore`

Translates the git repositories' `.gitignore` patterns into Syncthing ignore
patterns, so whatever git ignores is ignored - whether it exists yet or not.
Like `git status PATH`, it works on the repositories at and around `PATH` (the
one `PATH` is in, or every one below it, and recursively their nested
repositories and submodules), in every Syncthing folder at, around or inside
`PATH`; other repositories' patterns stay as they are. Each folder has two lists:

```
<folder>/.stignore             your lines, then: #include .stignore-git
<folder>/.stignore-git         synced: the .gitignore files' patterns, and `!` lines for the
                               tracked files they match (git never ignores those); last:
                               #include .stignore-git-local
<folder>/.stignore-git-local   not synced: this device's own excludes - each repository's
                               .git/info/exclude and your core.excludesFile
```

- Each `.gitignore`'s patterns are anchored to its directory, in the order
  git applies them: the deepest file and the last line first, as Syncthing
  lets the first matching pattern win. A negation git can't apply - re-including
  from an excluded directory - is left out.
- `PATH` doesn't have to be shared itself: with only `PATH/ui` shared, a
  repository at `PATH/main` is left alone. Folders inside one another each get
  their own lists, anchored to their own root.
- `.stignore` doesn't sync, so run `gitignore` once on each device; the synced
  list's updates sync in from then on. Your own `.stignore` lines come first,
  so they win: an `!` line such as `!/config/master.key` keeps a git-ignored
  secret syncing.
- Run it again when a `.gitignore` changes, or when a file a pattern matches
  gets tracked. A background service that does it is planned. An outer
  repository's patterns that match at any depth also reach into a repository
  nested in it, which git wouldn't do.

## License

MIT
