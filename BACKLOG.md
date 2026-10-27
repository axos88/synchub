# Backlog

Next free ID: D-18

By effort, smallest first; blocked items last.

- D-15 · ~½ day · Drawing changes nothing: layout computed apart from rendering, clicks hit-tested on it
- D-16 · ~½ day · Hovering highlights buttons and the status bar's hot keys; hot keys clickable like buttons
- D-8 · ~½ day · Prune a big file from the hub after it's ignored (`synchub prune`?) - waiting for Ákos
- D-11 · ~1 day · The app kept current by Syncthing's events: one event channel, a cached config
- D-3 · ~1½ days · Background service: drops shares the hub stopped offering; later does gitignore too
- D-13 · ~1½-2 days · A component tree: one trait for every part, retained UI state, pure layout (with D-15)
- D-17 · ~2-3 weeks · A cluster that doesn't trust its hub: encrypted hub, grant tree, signed logs - a new project
- D-7 · blocked · ~15 min · Join the real machines to the real hub - manual: Ákos runs `synchub join`
- D-12 · blocked · ~30 min · Drop `new_folder_path` once Syncthing resolves relative paths - syncthing#10937

### D-3 · Background service: leftover shares, then gitignore

The maintainer (2026-10-23): a background process, installed and started when a device joins
the hub (`synchub join`), removes from this device the shares the hub used to offer it but
doesn't any more - so a share removed on the hub (`synchub remove`, or unshared from this
device) disappears from every device without anyone running a command there. The same process
later takes over gitignore (below). (Keeping labels and names the same everywhere doesn't need it:
D-10 does that whenever synchub connects to the hub.) Not at app start: the service does it. Not
implemented yet.

Decided (2026-10-23):
- Detecting a removal: this device shares the folder with the hub, but the hub doesn't share it
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

### D-7 · Join the real machines to the real hub

synchub is tested on a sandbox hub only. The real hub is https://syncthing.vandra.hu:8384
(certificate SHA-256 ED:34:08:AC:AE:03:4C:F3:57:BC:45:6D:3B:30:C8:87:DA:45:65:7F:E7:F1:78:5F:38:
72:7D:6E:14:AA:DC:1D, valid to 2027-05-14; device ID QIRKWUT-IDWX3B2-VQWCWIH-RA3D2AR-YIV2ZCM-
EY5UYI5-LK5OYWL-DSVLSQV).

**Blocked:** a manual task - Ákos runs it on each machine himself.

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

3. Ákos (2026-10-26): pause all shares on the hub, remove the .stignore files, push the removal
   of all the ignored files to the hub, put the .stignore files back and resume all folders.
   To work out before building it: Syncthing propagates a deletion only for a file whose
   deletion it scanned - so the ignored files must exist where the deletion is made (a leaf
   that never had them can't delete them), and deleting them there loses that copy unless it's
   moved aside first (as in 1); while the hub's folders are paused, no deletion reaches it, so
   something has to be resumed for the push; and the other leaves would get the deletions too.

Next: Ákos picks one (recommended: 1, e.g. `synchub prune PATH`; 3 once its open points are
settled), or drops it.

### D-11 · The app kept current by events: one event channel, a cached config

The maintainer (2026-10-24): the config could be cached, with an events subscription updating it,
for a long-lived client like the app. Deferred: not needed for the commands, which are short.

Context:
- A Syncthing's whole configuration is one request: `Client::config()` (`/rest/config`, typed
  `api::config::Config<S>`; ~30 KB, a few ms on the sandbox). Verified 2026-10-24: the data of a
  `ConfigSaved` event (`/rest/events`) is exactly that object after the change.
- Today the app re-reads both configs (this device's and the hub's) on every load: at start,
  after each action, and on Ctrl-R (`app::state::State::load`).
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

### D-12 · Drop `new_folder_path` once Syncthing resolves relative folder paths

`POST /rest/config/folders` without a path gives the new folder the folder defaults' directory
itself (verified in the sandbox and in Syncthing's source, v2.1.7: `lib/api/confighandler.go`
starts from a copy of the defaults), so synchub computes `<defaults>/<folder ID>` for the hub in
`Config<Hub>::new_folder_path` (`src/api/config.rs`) - without being able to see whether that
directory exists on the hub's disk (the ID is unique, so two folders don't meet, but a directory
left there would be taken over). Auto-accept does it properly, server side.

2026-10-24: the maintainer's fork has a change (`axos88/syncthing`, branch
`api-folder-default-path`, commit 3b89afed; it replaced an earlier auto-accept-style attempt):
when a folder is created through `POST /rest/config/folders` or `PUT .../{id}`, a relative path
is resolved against the default folder path instead of Syncthing's working directory, and one
that exists and isn't an empty directory is refused with 409. Syncthing takes no unsolicited PRs
and no mostly-AI-written code, so it went upstream as an issue, written by the maintainer.

**Blocked on the upstream issue:** https://github.com/syncthing/syncthing/issues/10937 (opened
2026-10-23). Two maintainers replied (2026-10-24): relative paths resolving against the working
directory is intentional (portable installs, one config shared between OSes); a script should
read the default folder path from the API and build an absolute path itself - as synchub does.
Closed as "not planned" by calmh 2026-10-25, before Ákos could answer. Ákos is pushing back
(2026-10-26): resolve against the default folder path only when one is set (a fresh config's is
empty, so portable installs see no change); absolute paths couple API clients to the server's
filesystem; and a bug found while testing - with a default folder path set, a folder added
without a `path` gets that directory itself as its root, so it holds every auto-accepted folder
under it and shares them, both ways, with whatever device it's shared with (reproduced on v2.1.7
with three devices; the script and its output are in the comment).
Their answer (2026-10-26): changing what a non-absolute path means would be a breaking API change
- with over a million users, someone depends on every detail of the current contract - so it
won't change "at this stage of development". It didn't address an opt-in (a new query parameter
leaves today's contract as it is) or the no-path bug, which isn't about relative paths; Ákos to
ask about an opt-in, and to report the bug on its own (a separate issue, or privately).

Until that's settled synchub keeps `new_folder_path` and always sends a path (never none: that's
the bug above). A possible step meanwhile: before creating a hub folder, ask the hub's
`/rest/system/browse` whether the directory exists, and refuse if it does (it lists directories
only, so it can't tell an empty one from a full one).

Next step: follow the issue - only an opt-in can still unblock this; if none is accepted, drop D-12
(synchub keeps `new_folder_path`). If one is released, `HubClient::create_folder`
(`src/client/hub.rs`), `new::create` and `commands::convert::convert` send the folder ID as a
relative path (a 409 means the directory is taken), and `new_folder_path` goes - only when every
hub runs that release (check its version first).

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

Decided (Ákos, 2026-10-27), then punted ("let's punt on this"):
- **A component tree**, not just a custom dialog part: one `Component` trait for every part (text,
  buttons, fields, the list, the tree, the info panels), containers (dialog, form, pages) holding
  children, and D-15 folded in - a pure `layout(&self, area) -> Vec<Rect>` used both to draw and
  to hit-test, so drawing changes nothing. Sketch:
  `size(&self, room) -> Size`, `draw(&self, frame, area, focus)` (pure), `key(&mut self, key)`
  and `mouse(&mut self, gesture, area)` returning `Response<Self::Message>`.
- **Typed messages:** each component has `type Message`; key/mouse return
  `Response::Handled | Ignored | Message(m)`, and the parent maps its children's messages to its
  own (as `TreeResponse::Activated(Key)` and `Gesture` do today, made general).
- **Step by step**, each step building, tested, checked on the sandbox and reviewable on its own:
  1) the trait, `Response` and pure layout, `Window::event` getting the screen's area;
  2) dialogs and their parts; 3) the form and its fields; 4) the list, tree and browser;
  5) the main page and the info panels. Old and new coexist in between.
- **Two kinds of data** (Ákos): the application model isn't part of a component; a component's
  own UI state (focus, selection, scroll) is, and the component owns it. So no copying a
  derived "view" of the model into the component.
- **Hot keys aren't to be baked into the trait** (Ákos: they're application-specific): the
  trait must be extendable.

Open when it's picked up again:
- How hot keys extend the trait: a subtrait for the app (`trait AppComponent: Component { fn
  hot_keys(&self) -> HotKeys; }`, containers generic over the child type, the core knowing
  nothing of them) - Claude's recommendation - or generic key bindings in the trait
  (`fn bindings(&self) -> Vec<Binding>`, the status bar being one way of showing them; D-16's
  clickable hot keys would follow).
- How the model reaches a component that shows it: passed in on each call, typed per component
  (`type Model`: the tree's the `State`, a button's `()`) - Claude's recommendation - or a shared
  `Rc<State>` the page hands the component on each reload, read but not owned.

Effort: ~1½-2 days with D-15 (the app is ~5,100 lines; it touches 8 windows, the 3 field kinds,
the dialogs' parts, the tree, the list and the directory browser).

Next step: Ákos to settle the two open points; then step 1.

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

### D-16 · Hover highlights buttons and hot keys; hot keys are clickable

Ákos (2026-10-26): moving the mouse over a button - e.g. a dialog's OK, focused or not - should
change how it looks, and so should the status bar's hot keys. Clicking buttons works; the hot
keys aren't buttons yet - clicking one should do what its key does.

What's there: `terminal::start` turns on crossterm's `EnableMouseCapture`, which includes
any-motion tracking, so moving the mouse comes as `MouseEventKind::Moved` events; nothing uses
them yet. Buttons are drawn by `components/dialog/draw.rs` (`buttons_line`: the focused or
chosen one on yellow or cyan), and their areas come back in `Drawn::buttons` for clicks.
`components/status_bar.rs` draws `HotKeys` as plain text (`lines`), recording no areas.

What it takes:
- A hovered button: the window keeps the mouse's last position (from `Moved`), and drawing a
  button under it styles it - a third look, distinct from focused (yellow) and chosen (cyan).
- Hot keys as buttons: the status bar says where each `HotKey` landed; a click on one becomes
  its first key, handed to the window as if pressed; hovered, it's highlighted like a button.
- Where the areas come from ties in with D-15 (layout apart from drawing): with it, hovering
  and clicking hit-test the layout instead of what drawing recorded; best done with or after
  D-15 and D-13.

Next step: decide the hover look (e.g. underlined, or reversed), then do it - after D-15/D-13,
or on today's recorded areas if it's wanted sooner.

### D-17 · A cluster that doesn't trust its hub (a redesign - likely a new project)

Discussed by Ákos and Claude (2026-10-27). Ákos: "we may want to revisit this, but it seems like
a completely new project" - and "not very much can be reused". Nothing decided to build; this
records the design as far as it went. Ákos's ideas and Claude's suggestions are marked.

**Why.** Today trust within the cluster is assumed (README, "Trust"): every leaf holds the hub's
API key - full control of the hub - and the hub keeps a plain copy of every folder. A device
can share any folder with itself, and a compromised device is a compromised hub.

**The design, in parts:**

1. **A service on the hub** (Ákos) - the only thing talking to the hub's Syncthing; leaves no
   longer hold its API key, they send requests. Channel (Claude's analysis): files inside the
   shares - no (they reach every member, the author isn't verifiable, and a new share can't be
   read before it's accepted); a control folder per leaf - authenticated by Syncthing's device
   certificates, works offline, but asynchronous; an API on the hub authenticated by the leaves'
   device certificates as TLS client certificates (the certificate's hash is the device ID) and
   serving the hub's own device certificate - so both sides are authenticated, which also closes
   today's "the hub isn't authenticated" gap. Claude recommended the API.
2. **An encrypted hub** (Ákos): Syncthing's untrusted-device encryption - the hub's copy of each
   folder is receive-encrypted (contents and names), so the hub relays and stores but can't read.
   What Syncthing doesn't do is distribute each folder's password: a folder's key is wrapped with
   each member's public key (Ákos), so the folder's owner decides who can read it. Claude's
   suggestions: a separate encryption key pair per device, its public half signed with the device
   key (rather than encrypting with the certificate's signing key), and an established format
   (`age`: X25519 and ChaCha20-Poly1305, anonymous recipients) rather than raw AES.
3. **Grants as a tree** (Ákos): A grants B and C, B grants D and E, D grants F and G. A can only
   remove its own grants (B, C); removing B removes B's whole subtree. A grant is a signed record
   carrying the folder key wrapped for the grantee; it's valid only if its granter's grant is, up
   to the folder's creator - anyone can check a chain, so the hub only stores and relays them.
   Claude's suggestions, open: one grant per device per folder (a second one refused - so
   removing a branch removes it without exceptions); a "may grant further" flag; no grant to an
   ancestor. Consequence: A can't remove D without removing B's branch (or asking B).
   Revocation rotates the folder **data's** password - which, as far as known, means removing and
   re-adding the folder on the hub, re-uploading it all (to verify) - and protects only what comes
   after: a removed device keeps what it already has.
4. **Leaves choose their own peers** (Claude): with an untrusted hub, the hub can't be the leaves'
   introducer - they sync with introduced devices directly, in plain text, so a hostile hub could
   introduce a device of its own and read everything. Each leaf verifies the grant tree and
   shares a folder directly only with devices whose chain checks out; no introducer, no
   auto-accepting from the hub. That needs something running on every leaf - D-3's service
   becomes required.
5. **The metadata** - device records (encryption public key signed with the device key, and the
   device certificate, checkable against the device ID), folder records, grants, revocations,
   wrapped keys - travels through Syncthing itself:
   - **Encrypted per record to its audience** (Ákos): a random key per record, the content
     encrypted once with it, the key wrapped for each recipient (envelope encryption - `age`'s
     multi-recipient format). Audience: the folder's members for its grants and revocations
     (every member checks every peer's chain), the whole cluster for device keys and admission.
     A removed device is simply no longer a recipient - no rotation needed for metadata. A new
     member needs the folder's history: the granter re-wraps the past records' keys for it, in
     the grant. Records are also signed by their author (encryption doesn't say who wrote one).
   - **Where**: not in the data folders (a new member needs its key before it can decrypt the
     folder holding it). One shared metadata folder, or one outbox per device - send-only on its
     owner, receive-only elsewhere (Claude leaned to outboxes: Syncthing then ensures only the
     author writes or deletes; one shared folder needs self-healing - each device archives what
     it saw and puts back what disappears). Immutable records, one file each, named by their
     hash: Syncthing never sees a conflict.
   - **A hash chain per author** (Ákos): each record carries the hash of its author's previous
     one, so a record can't go missing in the middle unnoticed - only the newest can be held back.
   - **Acknowledgements and pruning** (Ákos): each device publishes "I've handled A's records up
     to X"; once every stakeholder has, A may delete its records up to there. Claude's
     refinements: counters rather than timestamps (clocks disagree); acknowledgements carry the
     record's hash, so they also expose a held-back tail; a checkpoint (a signed summary of what's
     still in force - live grants, current keys - and where the chain stands) before pruning, so
     devices joining later still find live grants; removing a device from the cluster removes it
     as a stakeholder (a lost laptop would block pruning forever); two records claiming the same
     predecessor prove a misused key - warn loudly. Prior art: Secure Scuttlebutt's per-feed
     signed, hash-chained logs.
   - What remains against a hostile hub: a device that only ever talks to the hub can be kept
     stale; devices that also meet each other directly close that.

**What it would change.** The trust model goes from "every device trusts every other, and the
hub" to "a device sees a folder only if a chain of grants from its creator says so". The hub
still sees metadata (sizes, timing, which device syncs which folder). Admission of a new device
needs a step of its own (an existing device admits it, or it's done on the hub).

**Open questions:** one shared metadata folder or per-device outboxes; the grant rules above;
who may admit devices; the hub service's API; migration from today's clusters.

**To verify first, on the sandbox:** whether an untrusted device's folder password can be changed
without re-adding the folder; the key type of Syncthing's device certificates (v2.1.7); whether
receive-only devices pass on what they receive (for the hub relaying outboxes).

**Size:** roughly 2-3 weeks - the hub service, keys and grants, the encrypted hub, the leaf-side
service managing peers, the metadata logs, migration. Ákos: little of synchub carries over.

Next step: Ákos to decide whether to pursue it - as a new project or as synchub's next version.
