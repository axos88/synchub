# Backlog

Next free ID: D-16

- D-7 · ~15 min · Join the real machines to the real hub (`synchub join`, needs the hub's API key)
- D-12 · ~30 min · Drop `hub::hub_path` once Syncthing resolves relative folder paths - waiting for upstream
- D-6 · ~30 min · Run the build workflow (binaries + rolling "latest" release): needs a push
- D-14 · ~1 h · Text fields: a click doesn't move the cursor to where it lands
- D-15 · ~½ day · Drawing changes nothing: layout computed apart from rendering, clicks hit-tested on it
- D-8 · ~½ day · Prune a big file from the hub after it's ignored (`synchub prune`?) - waiting for Ákos
- D-11 · ~1 day · The app kept current by Syncthing's events: one event channel, a cached config
- D-13 · ~1 day · A common interface for the app's components: sized, drawn, clickable - any child in a dialog
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

### D-11 · The app kept current by events: one event channel, a cached config

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

The app's event loop (discussed 2026-10-25): today the app redraws only on terminal input
(`event::read()` blocks in `pages/main/keys.rs`; a resize is an event too) and, polling every
100 ms, while `perform` or the first load runs. To redraw on server events, without going async:
- One `mpsc` channel of an `AppEvent` enum (`Terminal(crossterm Event)`, `Syncthing(Place,
  events)`, `Reloaded(Result<State>)`), fed by threads: one reading the terminal, one
  long-polling `/rest/events` per Syncthing (this device's and the hub's). The main loop draws,
  `recv()`s, handles - `perform`'s worker + channel, made permanent. Blocking `reqwest` stays.
- Only one thread may read the terminal: since 2026-10-25 every window (the main page, the
  dialogs, the form, the directory browser) is run by `app::window::RunnableWindow::run`, the only caller of
  `event::read()` - it takes the events from the channel instead (handed the receiver, as it's
  handed the terminal), or the reader thread and it compete for keystrokes. `perform`'s busy
  dialog then ends on a "done" event instead of `Window::finished` polling every 100 ms.
- Server events arriving while a form or the browser is up queue in the channel: batch them into
  one reload when it returns, don't drop them.
- Start with a debounced full reload on a relevant event (`FolderSummary`, `FolderCompletion`,
  `StateChanged`, `DeviceConnected`/`DeviceDisconnected`, `ConfigSaved`); `App::loaded` already
  keeps the selection. Patch single fields from events only if reloads turn out too slow.
- Keep the last event ID per Syncthing (`since`); start from the latest (`limit=1`). The hub's
  long-poll gets a timeout and retries with a delay, reporting failures as events, so the hub
  panel shows "unreachable" live.
- The async alternative - crossterm's `event-stream` feature with `tokio::select!` - would make
  every client call async (async `reqwest`): only worth it if many more sources come.
- With the app's own event type, `Window::event` takes it instead of crossterm's `Event`, and
  `Window::opened()` becomes a variant of it (agreed with Ákos, 2026-10-26): `Opened`, or
  `Activated` / `Deactivated` - fired by `RunnableWindow::run`, which knows from its thread-local
  whether a window is up; `Activated` also when a window opened over this one closes, so the
  form, which opens an empty directory field's browser only when it first appears, must remember
  it has. Until then `opened()` stays a provided method: crossterm's `Event` can't carry the
  app's own events, and wrapping it would make every window skip variants it doesn't use.

Next step: decide whether the app opts in (e.g. `Client::subscribe()`), or every client
subscribes on first `config()`; then the event channel above, with the config cache fed from the
same long-poll threads.

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

### D-13 · A common interface for the app's components

Deferred by Ákos (2026-10-26): "for now, let's punt on it". It keeps coming up; recommending
against it each time was Claude's call, not a decision made together.

What's missing: there's no common type for "a component" in the app. Each window routes clicks
by hand, each component draws through its own function taking the frame (the maintainer's
decision against ratatui's `Widget` trait, CLAUDE.md, still stands), and a dialog can only hold
its five built-in parts. Where it showed:
- `components/dialog.rs`: `Part` is a closed enum (`Text`, `Lines`, `Gap`, `Field`, `Buttons`);
  `draw` sizes each kind itself, as ratatui widgets don't report a preferred size, and only the
  field and buttons report where they landed, for clicks. A dialog can't hold any other child,
  e.g. the identicon panel.
- Click routing is per window and by hand (`App::mouse` offers clicks to the `Tree`, `Form::mouse`
  picks a field by area, the browser checks its path line, dialogs their buttons).
- The status bar's keys are composed by hand (the page's `hot_keys`; a `Dialog` gives its own,
  which `popup` puts in the page's status bar); the tree's own navigation keys aren't listed.
- Drawing records the layout for clicks (D-15, to be done with this).

Options discussed:
- `Part::Custom { width, height, draw: Box<dyn FnMut(&mut Frame, Rect)> }`: the caller states the
  size and draws - "any child" in a dialog, a small change; the five parts stay built in.
- A trait, e.g. `fn size(&self, room: usize) -> (usize, usize)`, `fn draw(&mut self, frame, area)`,
  plus a way to report clickable areas (and maybe its keys, for the status bar), implemented by
  the dialog's parts and the other components; dialogs (and pages) holding `Vec<Box<dyn …>>`.

Next step: Ákos to decide whether to go for the trait (and what it covers: size, drawing,
clicks, keys) or the custom part only.

### D-14 · Text fields: a click doesn't move the cursor

Deferred by Ákos (2026-10-26). A text field has a cursor now (`components/form/text_input.rs`:
`TextInput`, in characters; Left/Right, Home/End, Backspace/Delete at it), but a click only
focuses the field (`TextField::mouse`, `components/form/field/text.rs`, returns `Handled`) - the
cursor stays where it was. Same in the prompt dialog, which holds a `TextField`.

What it takes: the click's column, minus the field's left border (`area.x + 1`), plus where the
shown text starts - `TextInput::shown(width)` scrolls long text to keep the cursor in view, so the
start isn't always 0 - clamped to the text's length, becomes the cursor. The field doesn't know
its area when a click comes, so it would record it while drawing (like `ClickableList`, which
keeps its last area in a `Cell`), and `TextInput` would remember the scroll start of its last
`shown`, or offer `cursor_at(column, width)`. A test: `TextInput` placed at a column, with and
without scrolling. If D-15 is done first, the field's area comes from its layout instead of
being recorded while drawing.

Next step: do it.

### D-15 · Drawing changes nothing: layout apart from rendering

Deferred by Ákos (2026-10-26) "to when we make the component tree" - do it with D-13. His
principle: drawing should not modify state.

Today drawing writes what clicks and scrolling need later, as the layout exists only inside
`draw`:
- `components/clickable_list.rs`: `ClickableList` keeps `state` (`ListState` - ratatui's
  `render_stateful_widget` moves its scroll offset to keep the selection in view),
  `content_area` (where the items went, to map a click to an item) and `len` (to ignore clicks
  below the last item) in `Cell`s, written by `draw(&self)`; the `Cell`s exist because the main
  page is drawn through `&self` (`Page::draw_page`, `Tree::draw`). (Its `GestureDetector` isn't
  in one: it changes in `clicked(&mut self)` only.)
- The dialogs' `draw(&mut self)` store their button and field areas
  (`self.buttons = draw(frame, dialog).buttons`, `components/dialog/{alert,confirm,prompt}.rs`);
  the form and the directory browser record theirs the same way.

The plan discussed:
1. Each component gets a pure `layout(&self, area) -> …` (the main page's areas, a dialog's box
   with its field and buttons, a form's fields); `draw(&self, frame)` renders from it, and the
   event code calls the same function - one source, so they can't disagree.
2. `Window::event` gets the screen's size - `event(&mut self, event, area)`, from the run loop's
   `Terminal`, or kept on `Resize`.
3. The scroll offset moves in event handling: when the selection changes, by just enough to
   keep it in view, from the viewport height (1); `draw` renders a throwaway copy of the
   `ListState`. (It must persist: from offset 0 each frame, Up in a scrolled list would pin the
   highlight to the bottom row and scroll the list under it.)
4. `len` becomes an argument: `clicked(mouse, len)` - each caller knows its count. So does
   `select_next`/`select_previous`: ratatui's (`ratatui-widgets` 0.3.2, `list/state.rs`) don't
   know the length - next adds 1, previous from none jumps to `usize::MAX` - and `List`'s render
   clamps the selection to the last item (`list/rendering.rs`), so today the directory browser's
   Up/Down rely on the next draw to correct it; without that, they clamp themselves.
Then `draw` is `&self` everywhere: no `Cell`s, no recorded areas, no `draw(&mut self)` on
`Window`. Costs: the `Window` trait and every clickable component change (list, tree, dialogs,
form and fields, directory browser); the layout is computed per draw and per event (cheap);
it goes slightly against ratatui, which expects render to own the offset. Sandbox pass over
every clickable thing afterwards.

Rejected as the smaller step: drawing through `&mut self` everywhere, dropping only the
`Cell`s - the mutation stays, just honest.

Next step: take it up together with D-13.

