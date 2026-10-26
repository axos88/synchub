//! `synchub gitignore`: make Syncthing ignore what git ignores.

mod repo;
mod stignore;
mod translate;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use anyhow::{Result, bail};
use itertools::Itertools;

use crate::api;
use crate::client;
use crate::commands::Arguments;
use crate::paths;
use crate::side::Local;
use crate::{say, step};

/// Make Syncthing ignore what git ignores.
///
/// Translates the repositories' .gitignore patterns into Syncthing's, so
/// what git ignores is ignored whether it exists yet or not. Like `git status
/// PATH`, gitignore works on the repositories at and around PATH - the one
/// PATH is in, or every repository below it, and recursively the nested
/// repositories and submodules - in every Syncthing folder at, around or
/// inside PATH. Other repositories' patterns stay as they are.
///
/// Each folder gets two lists at its root. .stignore-git holds the .gitignore
/// files' patterns, and `!` lines for the tracked files they match - git
/// doesn't ignore those; Syncthing syncs it to the other devices.
/// .stignore-git-local holds this device's own excludes (the repositories'
/// .git/info/exclude and the user's core.excludesFile), and isn't synced;
/// .stignore-git includes it, last. The folder's .stignore gets one line,
/// `#include .stignore-git`, at its end.
///
/// .stignore isn't synced, so run gitignore once on every device; from then
/// on, updates sync in. Your own lines in .stignore come first and win: an
/// `!` line (e.g. `!/config/master.key`, to keep syncing a git-ignored
/// secret) overrides the generated patterns - but a line of yours that
/// matches .stignore-git would also stop the list from syncing. Run it again
/// when a .gitignore changes.
///
/// Example: synchub gitignore .
#[derive(clap::Args, Debug, PartialEq)]
pub struct Args {
    /// A directory at, around or inside a Syncthing folder
    #[arg(default_value = ".")]
    pub path: PathBuf,
}

impl Arguments for Args {
    fn arguments(&self) -> Vec<String> {
        vec![self.path.display().to_string()]
    }
}

pub fn run(args: Args) -> Result<()> {
    if !args.path.is_dir() {
        bail!("{} isn't a directory", args.path.display());
    }
    gitignore(&paths::canon(&args.path))
}

/// Brings the lists of the folders at, around or inside `path` up to date
/// for the repositories at or around `path`, and has Syncthing load them.
pub fn gitignore(path: &Path) -> Result<()> {
    let config = client::local()?.config()?;
    let folders = config.related(path);
    if folders.is_empty() {
        bail!(
            "no Syncthing folder at, around or inside {}, so there's nothing for Syncthing to ignore",
            path.display()
        );
    }

    step!(Local, "looking for git repositories in {}", path.display());
    let shared = |repo: &PathBuf| folders.iter().any(|folder| folder.overlaps(repo));
    let repos: Vec<PathBuf> = repo::find(path)?
        .into_iter()
        .filter(|repo| paths::related(repo, path) && shared(repo))
        .collect();
    if repos.is_empty() {
        bail!(
            "no git repository in a Syncthing folder at or around {}, so there's no .gitignore to follow",
            path.display()
        );
    }

    let ignores: BTreeMap<PathBuf, repo::Ignores> = repos
        .into_iter()
        .map(|repo| {
            step!(Local, "reading what {} ignores", repo.display());
            let ignores = repo::ignores(&repo)?;
            Ok((repo, ignores))
        })
        .collect::<Result<_>>()?;

    folders
        .into_iter()
        .try_for_each(|folder| update(folder, &ignores))
}

/// Replaces the sections of `ignores`' repositories in `folder`'s lists.
fn update(
    folder: &api::config::Folder<Local>,
    ignores: &BTreeMap<PathBuf, repo::Ignores>,
) -> Result<()> {
    let root = folder.local_path();
    let synced_file = root.join(stignore::FILE_NAME);
    let local_file = root.join(stignore::LOCAL_FILE_NAME);
    let previous_synced = paths::read_if_exists(&synced_file)?;
    let previous_local = paths::read_if_exists(&local_file)?;
    let theirs: Vec<(&PathBuf, &repo::Ignores)> = ignores
        .iter()
        .filter(|(repo, _)| folder.overlaps(repo))
        .collect();
    if previous_synced.is_none() && theirs.is_empty() {
        // No list, and no repository in it: none of git's business.
        return Ok(());
    }

    let mut synced = stignore::List::parse(previous_synced.as_deref().unwrap_or_default());
    let mut local = stignore::List::parse(previous_local.as_deref().unwrap_or_default());
    theirs.into_iter().for_each(|(repo, ignores)| {
        let key = section(repo, &root);
        synced
            .sections
            .insert(key.clone(), synced_section(ignores, &root));
        local
            .sections
            .insert(key, local_section(repo, ignores, &root));
    });

    let synced_text = synced.render_synced();
    say!(
        "{}: {} patterns, {} tracked files kept syncing{}",
        synced_file.display(),
        synced.patterns(),
        synced.tracked(),
        unchanged(&previous_synced, &synced_text)
    );
    write_if_changed(&synced_file, previous_synced.as_deref(), &synced_text)?;
    let local_text = local.render_local();
    say!(
        "{}: {} patterns of this device's own{}",
        local_file.display(),
        local.patterns(),
        unchanged(&previous_local, &local_text)
    );
    write_if_changed(&local_file, previous_local.as_deref(), &local_text)?;
    include_list(&root)?;
    load(folder)
}

/// The synced section for a repository: the tracked files its rules match,
/// and its .gitignore files' patterns - the deepest file first, and in each,
/// the last line first.
fn synced_section(ignores: &repo::Ignores, root: &Path) -> stignore::Section {
    stignore::Section {
        tracked: ignores
            .tracked
            .iter()
            .filter(|file| paths::inside(file, root))
            .map(|file| format!("!{}", stignore::anchored(&paths::relative(file, root))))
            .collect(),
        patterns: ignores
            .gitignores
            .iter()
            .flat_map(|(dir, lines)| {
                lines
                    .iter()
                    .rev()
                    .flat_map(move |line| translate::translate(line, dir, root, ignores.fold_case))
            })
            // The first one decides; a repeated one never would.
            .unique()
            .collect(),
    }
}

/// The local section for `repo`: its own excludes, relative to its root.
fn local_section(repo: &Path, ignores: &repo::Ignores, root: &Path) -> stignore::Section {
    stignore::Section {
        tracked: Vec::new(),
        patterns: ignores
            .local
            .iter()
            .rev()
            .flat_map(|line| translate::translate(line, repo, root, ignores.fold_case))
            .unique()
            .collect(),
    }
}

/// The section key of `repo` in the folder at `root`: its path relative to
/// the root - `.` for the root itself, `..` for each level above it.
fn section(repo: &Path, root: &Path) -> String {
    if paths::inside(repo, root) {
        let rel = paths::relative(repo, root);
        return if rel.is_empty() { ".".to_string() } else { rel };
    }
    let up = root
        .strip_prefix(repo)
        .map_or(0, |rel| rel.components().count());
    vec![".."; up].join("/")
}

fn unchanged(previous: &Option<String>, content: &str) -> &'static str {
    if previous.as_deref() == Some(content) {
        ", unchanged"
    } else {
        ""
    }
}

/// Has the folder's .stignore include the synced list - which includes the
/// local one.
fn include_list(root: &Path) -> Result<()> {
    let file = root.join(".stignore");
    let current = paths::read_if_exists(&file)?;
    let updated = stignore::with_include(current.as_deref().unwrap_or_default());
    write_if_changed(&file, current.as_deref(), &updated)
}

/// Has Syncthing load `folder`'s ignore patterns right away - a scan does -
/// and fails if it finds an error in them.
fn load(folder: &api::config::Folder<Local>) -> Result<()> {
    let client = client::local()?;
    client.scan(folder)?;
    match client.ignores_error(&folder.id)? {
        Some(err) => bail!(
            "Syncthing rejects the ignore patterns of {} ({err}); check {}/.stignore and the .stignore-git lists",
            folder.name(),
            folder.path
        ),
        None => Ok(()),
    }
}

fn write_if_changed(file: &Path, current: Option<&str>, content: &str) -> Result<()> {
    if current == Some(content) {
        return Ok(());
    }
    paths::write(file, content)
}
