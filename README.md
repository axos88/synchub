# sttools

Helpers for [Syncthing](https://syncthing.net/), driving the local instance over
its REST API:

| Command | Link name | What it does |
|---|---|---|
| `sttools mv SOURCE DEST` | `syncmv` | Moves a directory Syncthing syncs, and repoints its folder(s). Nothing is re-synced. |
| `sttools put LOCAL_DIR FOLDER_ID` | `syncput` | Makes an existing folder take its contents from `LOCAL_DIR`, replacing every other device's copy. |
| `sttools git [PATH]` | `stgit` | Makes Syncthing ignore what git ignores, via a synced `.stignore-git` per repository. |

`sttools <command> --help` explains each in full.

## Install

```sh
cargo install --git https://github.com/axos88/sttools
# the short names are links to the same binary:
for name in syncmv syncput stgit; do ln -sf ~/.cargo/bin/sttools ~/.local/bin/$name; done
```

Syncthing must be running for the user. sttools reads its GUI address and API
key from `config.xml` (`~/.local/state/syncthing` or `~/.config/syncthing` on
Linux, `~/Library/Application Support/Syncthing` on macOS); `--home`,
`--gui-address` and `--api-key` (or `STHOMEDIR`, `STGUIADDRESS`,
`STGUIAPIKEY`) override that.

## `syncmv`

```sh
syncmv ~/projects/ruby/heroku ~/work/heroku
```

Finds the Syncthing folder stored at the source (and any nested inside it),
pauses it, moves the directory like `mv` does, points the folder at the new
place and resumes it. Refuses to move into or out of another folder's
directory unless `-f` is given; `-n` shows the plan only.

## `syncput`

```sh
syncput ~/projects/ruby/heroku shy4a-ady3g
```

Points the existing folder at `LOCAL_DIR` and makes every other device's copy
match it: changed files are overwritten, files `LOCAL_DIR` lacks are deleted
there. `LOCAL_DIR` itself isn't modified (a `.stfolder` marker is added if
missing); the folder's previous directory is left in place.

It pauses the folder, switches it to send-only at the new path with "send
ownership" on (so every file is re-read, even where size and mtime match the
old directory's), scans, runs "Override Changes", and restores the folder's
type and settings. Files the folder's `.stignore` ignores stay as they are on
the other devices. Asks before starting unless `-y` is given.

## `stgit`

```sh
stgit .
```

Finds the Syncthing folder and the git repository `PATH` is in (or every
repository below `PATH`), and recursively their nested repositories and
submodules. For each, it writes what git ignores into a `.stignore-git` at the
repository's root, as anchored Syncthing patterns, and makes the folder's
`.stignore` `#include` it, with a `!` line that keeps the list itself synced.

- `.stignore-git` is synced; `.stignore` isn't, so run `stgit` once on each
  device. A device that lacks the ignored files keeps the entries git's rules
  still ignore, so the devices don't overwrite each other's lists.
- Your own `.stignore` lines stay. The generated `#include`s go last, so your
  `!` lines win - e.g. `!/config/master.key` keeps a git-ignored secret
  syncing.
- git only reports ignored paths that exist, so `stgit` offers to install a
  `post-commit` hook that reruns it. A file git ignores syncs until the next
  run after it appears.
- The hook goes into the repository's own `.git/hooks`. A `core.hooksPath`
  directory is never written to; if one is set, `stgit` says so, and its
  `post-commit` has to call the repository's hook.
