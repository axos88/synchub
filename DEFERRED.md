# Deferred

Next free ID: D-4

- D-1 · ~5 min · The bash syncmv/syncput in ~/Sync: keep, or delete now that sttools replaces them
- D-2 · ~5 min · No license yet: the public repo is "all rights reserved" until one is chosen
- D-3 · ~1 day · stgit: new git-ignored files sync until the next run; translate .gitignore rules?

### D-1 · Old bash scripts in ~/Sync

`~/Sync/syncmv` and `~/Sync/syncput` are the bash versions sttools replaces. Syncthing copies
them to every machine, so deleting them deletes them everywhere, including machines that don't
have sttools installed yet (the MacBook). On akos-workstation, `~/.local/bin/syncmv` and
`syncput` already point at `~/.cargo/bin/sttools`.

Next: Ákos decides whether to delete them (after installing sttools on the other machines) or
keep them as a fallback.

### D-2 · License

The repository is public without a LICENSE file, which means all rights reserved.

Next: Ákos picks a license (e.g. MIT or Apache-2.0); add LICENSE and `license` in Cargo.toml.

### D-3 · stgit only knows ignored files that exist

`git ls-files --others --ignored` lists existing paths only, so a file git ignores (a new build
output, a new log) syncs to the other devices until stgit runs again (the post-commit hook).
Verified in the sandbox test: a `new.log` created before a commit reached the other device.

Options:
- Also run stgit from post-checkout / post-merge hooks, or periodically - narrows the window.
- Translate the .gitignore rules themselves into Syncthing patterns, so future files match too.
  Needs care: git's last-match-wins vs Syncthing's first-match-wins, negations, dir-only
  rules, per-directory .gitignore anchoring, .git/info/exclude and core.excludesFile. A wrong
  translation could stop tracked files from syncing.

Next: Ákos decides whether the window matters enough to do either.
