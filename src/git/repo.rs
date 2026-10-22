//! Asking git about repositories.

use std::collections::BTreeSet;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use anyhow::{Context, Result, bail};

use crate::paths;

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

/// Absolute paths of what git ignores in `repo`; an ignored directory is
/// listed as itself, not file by file.
pub fn ignored(repo: &Path) -> Result<Vec<PathBuf>> {
    let out = git(
        repo,
        &[
            "ls-files",
            "-z",
            "--others",
            "--ignored",
            "--exclude-standard",
            "--directory",
        ],
    )?;
    let mut found = Vec::new();
    let mut dirs = Vec::new();
    for entry in entries(&out) {
        match entry.strip_suffix('/') {
            Some(dir) => dirs.push(repo.join(dir)),
            None => found.push(repo.join(entry)),
        }
    }
    // git also lists a directory as ignored when everything in it is, even if
    // no rule ignores the directory itself; then only its contents may go in,
    // or files added there later would never sync.
    while !dirs.is_empty() {
        let ignored: BTreeSet<PathBuf> = still_ignored(repo, &dirs)?.into_iter().collect();
        let mut children = Vec::new();
        for dir in dirs {
            if ignored.contains(&dir) {
                found.push(dir);
            } else {
                for child in fs::read_dir(&dir)? {
                    let child = child?;
                    if child.file_name() != ".git" {
                        children.push(child.path());
                    }
                }
            }
        }
        let ignored: BTreeSet<PathBuf> = still_ignored(repo, &children)?.into_iter().collect();
        dirs = Vec::new();
        for child in children {
            if ignored.contains(&child) {
                found.push(child);
            } else if child.is_dir() {
                dirs.push(child);
            }
        }
    }
    Ok(found)
}

/// Those of `paths` (absolute, inside `repo`) that git's rules ignore,
/// whether or not they exist here.
pub fn still_ignored(repo: &Path, paths: &[PathBuf]) -> Result<Vec<PathBuf>> {
    if paths.is_empty() {
        return Ok(Vec::new());
    }
    // A path that doesn't exist here is only matched by directory patterns
    // ("tmp/") when asked about with a trailing slash, so ask both ways.
    let mut input = Vec::new();
    for path in paths {
        let rel = path.strip_prefix(repo).unwrap_or(path).to_string_lossy();
        let suffixes: &[&str] = if path.is_file() { &[""] } else { &["", "/"] };
        for suffix in suffixes {
            input.extend_from_slice(rel.as_bytes());
            input.extend_from_slice(suffix.as_bytes());
            input.push(0);
        }
    }
    let mut child = git_in(repo)
        .args(["check-ignore", "-z", "--stdin"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .context("running git")?;
    let mut stdin = child.stdin.take().context("git's stdin")?;
    let writer = std::thread::spawn(move || stdin.write_all(&input));
    let out = child.wait_with_output()?;
    writer
        .join()
        .map_err(|_| anyhow::anyhow!("writing to git"))??;
    // check-ignore exits with 1 when nothing matched.
    if !matches!(out.status.code(), Some(0 | 1)) {
        bail!(
            "git check-ignore in {}: {}",
            repo.display(),
            String::from_utf8_lossy(&out.stderr).trim()
        );
    }
    let matched: BTreeSet<PathBuf> = entries(&out.stdout)
        .map(|e| repo.join(e.trim_end_matches('/')))
        .collect();
    Ok(matched.into_iter().collect())
}

/// The directory git keeps `repo`'s hooks in.
/// `repo`'s own hooks directory, inside its git directory - regardless of core.hooksPath.
pub fn private_hooks_dir(repo: &Path) -> Result<PathBuf> {
    let out = git(
        repo,
        &["rev-parse", "--path-format=absolute", "--git-common-dir"],
    )?;
    Ok(PathBuf::from(String::from_utf8_lossy(&out).trim()).join("hooks"))
}

/// The core.hooksPath setting in effect for `repo`, if any.
pub fn hooks_path(repo: &Path) -> Result<Option<String>> {
    let out = git_in(repo)
        .args(["config", "--get", "core.hooksPath"])
        .output()
        .context("running git")?;
    // `git config --get` exits with 1 when the setting is absent.
    match out.status.code() {
        Some(0) => Ok(Some(
            String::from_utf8_lossy(&out.stdout).trim().to_string(),
        )),
        Some(1) => Ok(None),
        _ => bail!(
            "git config in {}: {}",
            repo.display(),
            String::from_utf8_lossy(&out.stderr).trim()
        ),
    }
}

fn toplevel(dir: &Path) -> Option<PathBuf> {
    let out = git(dir, &["rev-parse", "--show-toplevel"]).ok()?;
    Some(paths::canon(Path::new(
        String::from_utf8_lossy(&out).trim(),
    )))
}

fn collect(repo: &Path, found: &mut Vec<PathBuf>) -> Result<()> {
    if found.iter().any(|f| f == repo) {
        return Ok(());
    }
    found.push(repo.to_path_buf());

    // Submodules: gitlinks in the index.
    let index = git(repo, &["ls-files", "-z", "--stage"])?;
    for entry in entries(&index) {
        if let Some((meta, path)) = entry.split_once('\t')
            && meta.starts_with("160000 ")
        {
            let sub = repo.join(path);
            if sub.join(".git").exists() {
                collect(&sub, found)?;
            }
        }
    }

    // Other repositories show up as untracked directories, possibly deeper
    // inside an untracked directory git lists as a whole.
    let untracked = git(
        repo,
        &[
            "ls-files",
            "-z",
            "--others",
            "--exclude-standard",
            "--directory",
        ],
    )?;
    for entry in entries(&untracked).filter(|e| e.ends_with('/')) {
        search(&repo.join(entry.trim_end_matches('/')), found)?;
    }
    Ok(())
}

fn search(dir: &Path, found: &mut Vec<PathBuf>) -> Result<()> {
    if dir.join(".git").exists() {
        return collect(dir, found);
    }
    let Ok(children) = fs::read_dir(dir) else {
        return Ok(());
    };
    for child in children {
        let child = child?;
        let name = child.file_name();
        if child.file_type()?.is_dir()
            && !matches!(name.to_str(), Some(".stversions" | ".stfolder"))
        {
            search(&child.path(), found)?;
        }
    }
    Ok(())
}

fn entries(out: &[u8]) -> impl Iterator<Item = &str> {
    out.split(|&b| b == 0)
        .filter(|e| !e.is_empty())
        .filter_map(|e| std::str::from_utf8(e).ok())
}

fn git(dir: &Path, args: &[&str]) -> Result<Vec<u8>> {
    let out = git_in(dir).args(args).output().context("running git")?;
    if !out.status.success() {
        bail!(
            "git {} in {}: {}",
            args.join(" "),
            dir.display(),
            String::from_utf8_lossy(&out.stderr).trim()
        );
    }
    Ok(out.stdout)
}

/// A git command run in `dir`, without the variables git sets for hooks
/// (GIT_DIR, GIT_INDEX_FILE, ...): they'd point every repository at the one
/// being committed to.
fn git_in(dir: &Path) -> Command {
    let mut command = Command::new("git");
    for (name, _) in std::env::vars_os() {
        if name.to_string_lossy().starts_with("GIT_") {
            command.env_remove(name);
        }
    }
    command.arg("-C").arg(dir);
    command
}
