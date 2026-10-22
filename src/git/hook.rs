//! The post-commit hook that runs stgit.

use std::fs;
use std::io;
use std::path::Path;

use anyhow::{Context, Result};

use super::repo;
use crate::prompt;

#[derive(Clone, Copy, clap::ValueEnum)]
pub enum HookMode {
    /// Ask in each repository that doesn't have it yet
    Ask,
    /// Install it without asking
    Yes,
    /// Don't install it
    No,
}

const MARKER: &str = "# sttools git: keep .stignore-git in step with what git ignores";
// Commits from IDEs often run hooks with a minimal PATH.
const COMMAND: &str =
    r#"PATH="$PATH:$HOME/.cargo/bin:$HOME/.local/bin" sttools git --hook no . || true"#;

/// Installs the hook in the repository's own hooks directory. A core.hooksPath
/// directory is never written to: it's shared by all repositories (global) or
/// committed with the project.
pub fn ensure(repo_dir: &Path, mode: HookMode) -> Result<()> {
    if matches!(mode, HookMode::No) {
        return Ok(());
    }
    let dir = repo::private_hooks_dir(repo_dir)?;
    let file = dir.join("post-commit");
    if fs::symlink_metadata(&file).is_ok_and(|m| m.file_type().is_symlink()) {
        eprintln!(
            "sttools: {} is a link, leaving it alone; to run stgit after each commit, add:\n  {COMMAND}",
            file.display()
        );
        return Ok(());
    }
    let existing = match fs::read_to_string(&file) {
        Ok(text) => Some(text),
        Err(err) if err.kind() == io::ErrorKind::NotFound => None,
        Err(err) => return Err(err).with_context(|| format!("reading {}", file.display())),
    };
    if existing
        .as_deref()
        .is_some_and(|text| text.contains(MARKER))
    {
        return Ok(());
    }
    if let Some(text) = &existing
        && !text
            .lines()
            .next()
            .is_some_and(|l| l.starts_with("#!") && l.contains("sh"))
    {
        eprintln!(
            "sttools: {} isn't a shell script; to run stgit after each commit, add:\n  {COMMAND}",
            file.display()
        );
        return Ok(());
    }
    if matches!(mode, HookMode::Ask) {
        let question = format!(
            "Install a post-commit hook in {} that runs stgit after each commit?",
            repo_dir.display()
        );
        match prompt::confirm(&question)? {
            Some(true) => {}
            Some(false) => return Ok(()),
            None => {
                eprintln!(
                    "sttools: {} has no stgit hook; run stgit there with --hook yes to install it",
                    repo_dir.display()
                );
                return Ok(());
            }
        }
    }

    match existing {
        Some(text) => {
            let separator = if text.ends_with('\n') { "" } else { "\n" };
            fs::write(&file, format!("{text}{separator}{MARKER}\n{COMMAND}\n"))
                .with_context(|| format!("writing {}", file.display()))?;
        }
        None => {
            fs::create_dir_all(&dir)?;
            fs::write(&file, format!("#!/bin/sh\n{MARKER}\n{COMMAND}\n"))
                .with_context(|| format!("writing {}", file.display()))?;
            make_executable(&file)?;
        }
    }
    println!("installed {}", file.display());
    if let Some(hooks_path) = repo::hooks_path(repo_dir)? {
        eprintln!(
            "sttools: note: core.hooksPath is {hooks_path}, so git runs the hooks there instead of {}; make their post-commit run it",
            file.display()
        );
    }
    Ok(())
}

#[cfg(unix)]
fn make_executable(file: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(file, fs::Permissions::from_mode(0o755))?;
    Ok(())
}

#[cfg(not(unix))]
fn make_executable(_: &Path) -> Result<()> {
    Ok(())
}
