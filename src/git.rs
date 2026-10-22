//! `sttools git` (stgit): make Syncthing ignore what git ignores.

mod hook;
mod repo;
mod stignore;

use std::collections::BTreeSet;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};

use crate::paths;
use crate::syncthing::Client;

pub use hook::HookMode;

/// Make Syncthing ignore what git ignores.
///
/// PATH is a directory inside a Syncthing folder. stgit finds that folder and
/// the git repository PATH is in - or, when PATH isn't in one, every
/// repository below it - and, recursively for nested repositories and
/// submodules too, lists what git ignores there. The list goes into a
/// .stignore-git file at each repository's root (at the folder's root when the
/// folder lies inside the repository), as Syncthing ignore patterns.
///
/// The .stignore-git files are ordinary files: Syncthing syncs them to the
/// other devices. stgit makes the folder's .stignore #include them, and adds a
/// `!` line keeping each one synced. .stignore itself isn't synced, so run
/// stgit once on every device; from then on, updates sync in with
/// .stignore-git. Your own lines in .stignore are kept, and an `!` pattern of
/// yours (e.g. `!/config/master.key`, to keep syncing a git-ignored secret)
/// wins over the generated ones.
///
/// git only lists ignored paths that exist, so stgit runs again after each
/// commit from a post-commit hook; it offers to install one in each
/// repository.
///
/// Example: stgit .
#[derive(clap::Args)]
pub struct Args {
    /// A directory inside a Syncthing folder
    #[arg(default_value = ".")]
    path: PathBuf,
    /// Install the post-commit hook that runs stgit
    #[arg(long, value_enum, default_value_t = HookMode::Ask)]
    hook: HookMode,
    /// Show what would change, write nothing
    #[arg(short = 'n', long)]
    dry_run: bool,
}

pub fn run(client: &Client, args: Args) -> Result<()> {
    if !args.path.is_dir() {
        bail!("{}: not a directory", args.path.display());
    }
    let path = paths::canon(&args.path);
    let folders = client.folders()?;
    let folder = folders
        .iter()
        .filter(|f| paths::inside(&path, &f.local_path()))
        .max_by_key(|f| f.local_path().components().count())
        .with_context(|| format!("{} is not inside any Syncthing folder", path.display()))?;
    let root = folder.local_path();
    let repos = repo::find(&path)?;
    if repos.is_empty() {
        bail!("no git repository at or below {}", path.display());
    }

    let mut lists = Vec::new();
    for repo in &repos {
        let list = if paths::inside(repo, &root) {
            repo.join(stignore::LIST)
        } else {
            root.join(stignore::LIST)
        };
        let previous = read_optional(&list)?;
        let mut ignored: BTreeSet<PathBuf> = repo::ignored(repo)?.into_iter().collect();
        // The list is synced and other devices lack the ignored files, so keep
        // what git's rules still ignore rather than only what exists here.
        let listed: Vec<PathBuf> = previous
            .as_deref()
            .map(stignore::listed)
            .unwrap_or_default()
            .into_iter()
            .map(|rel| root.join(rel))
            .filter(|p| paths::inside(p, repo))
            .collect();
        ignored.extend(repo::still_ignored(repo, &listed)?);
        let patterns: Vec<String> = ignored
            .iter()
            .filter(|p| paths::inside(p, &root) && !stignore::is_syncthing_file(p))
            .map(|p| stignore::anchored(&paths::relative(p, &root)))
            .collect();
        let source = paths::relative(repo, &root);
        let content = stignore::list_file(&source, &patterns);
        let unchanged = previous.as_deref() == Some(content.as_str());
        println!(
            "{}: {} ignored path{}{}",
            list.display(),
            patterns.len(),
            if patterns.len() == 1 { "" } else { "s" },
            if unchanged { ", unchanged" } else { "" }
        );
        if !unchanged && !args.dry_run {
            fs::write(&list, content).with_context(|| format!("writing {}", list.display()))?;
        }
        lists.push(list);
    }

    let stignore_file = root.join(".stignore");
    let current = read_optional(&stignore_file)?.unwrap_or_default();
    let updated = stignore::with_lists(&current, &root, &lists);
    if updated != current {
        println!("{}: updated", stignore_file.display());
        if !args.dry_run {
            fs::write(&stignore_file, &updated)
                .with_context(|| format!("writing {}", stignore_file.display()))?;
        }
    }
    if args.dry_run {
        return Ok(());
    }

    // The scan makes Syncthing load the new ignore patterns right away.
    client.scan(&folder.id)?;
    if let Some(err) = client.ignores_error(&folder.id)? {
        bail!(
            "Syncthing rejects the ignore patterns of {}: {err}",
            folder.name()
        );
    }
    for repo in &repos {
        hook::ensure(repo, args.hook)?;
    }
    Ok(())
}

fn read_optional(file: &Path) -> Result<Option<String>> {
    match fs::read_to_string(file) {
        Ok(text) => Ok(Some(text)),
        Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(err) => Err(err).with_context(|| format!("reading {}", file.display())),
    }
}
