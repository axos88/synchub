//! Asking git about repositories - through gitoxide, without running git.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use super::translate;
use crate::paths;
use anyhow::{Context, Result};
use gix::dir::entry::{Kind, Status};
use gix::dir::walk::EmissionMode;
use gix::index::entry::Mode;
use gix::worktree::stack::state::ignore::Source;
use itertools::Itertools;

/// The repositories to process for `path`: the one it's in - or, outside any,
/// those below it - and, recursively, the ones nested in them.
pub fn find(path: &Path) -> Result<Vec<PathBuf>> {
    let mut found = Vec::new();
    match toplevel(path) {
        Some(top) => collect(&top, &mut found)?,
        None => search(path, &mut found)?,
    }
    Ok(found)
}

/// What a repository ignores, as its files say it.
pub struct Ignores {
    /// Each .gitignore's directory and lines, the deepest first.
    pub gitignores: Vec<(PathBuf, Vec<String>)>,
    /// This device's own: the repository's .git/info/exclude, then the
    /// user's excludes file - relative to the repository's root.
    pub local: Vec<String>,
    /// The tracked files the rules match: git doesn't ignore those.
    pub tracked: Vec<PathBuf>,
    /// Whether its file names are matched ignoring case (core.ignorecase).
    pub fold_case: bool,
}

/// What `repo` ignores: its .gitignore files - the ones git reads, so not in
/// ignored directories or nested repositories - and this device's excludes.
pub fn ignores(repo: &Path) -> Result<Ignores> {
    let repository = gix::open(repo)?;
    let gitignores = walk(&repository, Wanted::Files)?
        .into_iter()
        .filter(|(rel, _)| rel.file_name().is_some_and(|name| name == ".gitignore"))
        .map(|(rel, _)| {
            let file = repo.join(rel);
            let lines = fs::read_to_string(&file)
                .with_context(|| format!("can't read {}", file.display()))?
                .lines()
                .map(String::from)
                .collect();
            let dir = file.parent().unwrap_or(repo).to_path_buf();
            Ok((dir, lines))
        })
        .collect::<Result<Vec<(PathBuf, Vec<String>)>>>()?
        .into_iter()
        .sorted_by_key(|(dir, _)| std::cmp::Reverse(dir.components().count()))
        .map(|(dir, lines)| {
            let lines = re_including(repo, &dir, lines)?;
            Ok((dir, lines))
        })
        .collect::<Result<_>>()?;

    let excludes_file = match repository
        .config_snapshot()
        .trusted_path("core.excludesFile")?
    {
        Some(file) => Some(file),
        None => std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .or_else(|| Some(paths::home().join(".config")))
            .map(|config| config.join("git").join("ignore")),
    };
    let local = [
        Some(repository.common_dir().join("info").join("exclude")),
        excludes_file,
    ]
    .into_iter()
    .flatten()
    .filter(|file| file.is_file())
    .map(|file| fs::read_to_string(&file).with_context(|| format!("can't read {}", file.display())))
    .collect::<Result<Vec<String>>>()?
    .iter()
    .flat_map(|text| text.lines().map(String::from))
    .collect();

    let index = repository.index_or_empty()?;
    let files: Vec<PathBuf> = index
        .entries()
        .iter()
        .filter(|e| e.mode != Mode::COMMIT)
        .map(|entry| Ok(repo.join(gix::path::from_bstr(entry.path(&index))?)))
        .collect::<Result<_>>()?;
    let tracked = matched(repo, &files)?;

    let fold_case = repository
        .config_snapshot()
        .boolean("core.ignorecase")
        .unwrap_or(false);
    Ok(Ignores {
        gitignores,
        local,
        tracked,
        fold_case,
    })
}

/// `lines` of the .gitignore in `dir` without the negations git can't
/// apply: re-including from a directory that's excluded.
fn re_including(repo: &Path, dir: &Path, lines: Vec<String>) -> Result<Vec<String>> {
    lines
        .into_iter()
        .map(|line| {
            let excluded = match translate::negated_parent(&line) {
                Some(parent) => !matched(repo, &[dir.join(parent)])?.is_empty(),
                None => false,
            };
            Ok((!excluded).then_some(line))
        })
        .filter_map(Result::transpose)
        .collect()
}

/// Those of `paths` (absolute, inside `repo`) that the repository's ignore
/// rules match, whether or not they exist here.
fn matched(repo: &Path, paths: &[PathBuf]) -> Result<Vec<PathBuf>> {
    if paths.is_empty() {
        return Ok(Vec::new());
    }
    let repository = gix::open(repo)?;
    let index = repository.index_or_empty()?;
    let mut excludes =
        repository.excludes(&index, None, Source::WorktreeThenIdMappingIfNotSkipped)?;
    let matched: BTreeSet<PathBuf> = paths
        .iter()
        .map(|path| -> Result<Option<PathBuf>> {
            let rel = path.strip_prefix(repo).unwrap_or(path);
            // What's in an ignored directory is ignored too. A path that
            // doesn't exist here is only matched by directory patterns
            // ("tmp/") when asked about as a directory, so ask both ways.
            let dirs = rel
                .ancestors()
                .skip(1)
                .filter(|d| !d.as_os_str().is_empty())
                .chain((!path.is_file()).then_some(rel));
            let as_file = path.is_file() || !path.exists();
            let ignored = as_file && excludes.at_path(rel, None)?.is_excluded();
            let ignored = dirs.into_iter().try_fold(ignored, |ignored, dir| {
                anyhow::Ok(ignored || excludes.at_path(dir, Some(Mode::DIR))?.is_excluded())
            })?;
            Ok(ignored.then(|| path.clone()))
        })
        .filter_map(Result::transpose)
        .collect::<Result<_>>()?;
    Ok(matched.into_iter().collect())
}

/// The working tree of the repository `dir` is in.
fn toplevel(dir: &Path) -> Option<PathBuf> {
    let repo = gix::discover(dir).ok()?;
    repo.workdir().map(paths::canon)
}

fn collect(repo: &Path, found: &mut Vec<PathBuf>) -> Result<()> {
    if found.iter().any(|f| f == repo) {
        return Ok(());
    }
    found.push(repo.to_path_buf());
    let repository = gix::open(repo)?;

    // Submodules: gitlinks in the index.
    let index = repository.index_or_empty()?;
    index
        .entries()
        .iter()
        .filter(|e| e.mode == Mode::COMMIT)
        .map(|entry| Ok(repo.join(gix::path::from_bstr(entry.path(&index))?)))
        .collect::<Result<Vec<_>>>()?
        .into_iter()
        .filter(|sub| sub.join(".git").exists())
        .try_for_each(|sub| collect(&sub, found))?;

    // Other repositories show up as untracked directories, possibly deeper
    // inside an untracked directory listed as a whole.
    walk(&repository, Wanted::Untracked)?
        .into_iter()
        .filter(|(_, kind)| matches!(kind, Some(Kind::Directory | Kind::Repository)))
        .try_for_each(|(rel, _)| search(&repo.join(rel), found))
}

fn search(dir: &Path, found: &mut Vec<PathBuf>) -> Result<()> {
    if dir.join(".git").exists() {
        return collect(dir, found);
    }
    let Ok(children) = fs::read_dir(dir) else {
        return Ok(());
    };
    children.into_iter().try_for_each(|child| {
        let child = child?;
        let name = child.file_name();
        if child.file_type()?.is_dir()
            && !matches!(name.to_str(), Some(".stversions" | ".stfolder"))
        {
            search(&child.path(), found)?;
        }
        Ok(())
    })
}

/// What a [`walk`] lists.
enum Wanted {
    /// Untracked directories as a whole, files outside them.
    Untracked,
    /// Every file git doesn't ignore, tracked or not.
    Files,
}

/// The `wanted` entries of `repo`'s working tree, a directory listed as a
/// whole when all of it is: (path relative to the repository, what it is on
/// disk).
fn walk(repo: &gix::Repository, wanted: Wanted) -> Result<Vec<(PathBuf, Option<Kind>)>> {
    let options = match wanted {
        Wanted::Untracked => repo
            .dirwalk_options()?
            .emit_untracked(EmissionMode::CollapseDirectory),
        Wanted::Files => repo
            .dirwalk_options()?
            .emit_tracked(true)
            .emit_untracked(EmissionMode::Matching),
    };
    let index = repo.index_or_empty()?;
    repo.dirwalk_iter(
        index,
        std::iter::empty::<&str>(),
        Default::default(),
        options,
    )?
    .map(|item| -> Result<Option<(PathBuf, Option<Kind>)>> {
        let entry = item?.entry;
        let keep = match wanted {
            Wanted::Untracked => entry.status == Status::Untracked,
            Wanted::Files => {
                matches!(entry.status, Status::Tracked | Status::Untracked)
                    && entry.disk_kind == Some(Kind::File)
            }
        };
        if !keep {
            return Ok(None);
        }
        let rel = gix::path::from_bstr(&entry.rela_path)?.into_owned();
        Ok(Some((rel, entry.disk_kind)))
    })
    .filter_map(Result::transpose)
    .collect()
}
