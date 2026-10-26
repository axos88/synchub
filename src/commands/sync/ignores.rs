//! A folder's shared ignore list, `.stignore-shared`: hand-written and
//! synced, so the same on every device sharing the folder - included from
//! each device's own `.stignore`, which Syncthing doesn't sync. Setting it up
//! moves the patterns `.stignore` had into it.

use std::path::Path;

use anyhow::{Result, bail};

use super::warn_all;
use crate::api;
use crate::client;
use crate::paths;
use crate::settings;
use crate::side::Local;
use crate::step;

pub const FILE_NAME: &str = ".stignore-shared";

/// The line `.stignore` needs.
const INCLUDE: &str = "#include .stignore-shared";

/// What a new list holds. Its own name, at any depth, keeps it - and nested
/// folders' lists - synced whatever the patterns ignore.
const NEW_LIST: &str = "\
// Ignore patterns for every device sharing this folder: this file is synced.
// A device's own patterns go in its .stignore, which isn't - above its #include lines.
!.stignore-shared
";

/// Has every folder of this device include its shared list; whether anything
/// changed. Each folder on its own: one failing mustn't leave the rest behind.
pub fn include_everywhere() -> Result<bool> {
    let folders = client::local()?.config()?.folders;
    let results: Vec<(String, Result<bool>)> = folders
        .iter()
        .filter(|folder| !folder.is_receive_encrypted())
        .map(|folder| (folder.name(), include(folder)))
        .collect();
    let changed = results.iter().any(|(_, result)| matches!(result, Ok(true)));
    let failed: Vec<(String, anyhow::Error)> = results
        .into_iter()
        .filter_map(|(name, result)| result.err().map(|err| (name, err)))
        .collect();
    if !failed.is_empty() {
        bail!("these don't include {FILE_NAME} yet: {}", warn_all(&failed));
    }
    Ok(changed)
}

/// Has `folder`'s `.stignore` include its shared list, creating the list if
/// no device has one yet, and moving the folder's own patterns into it; whether
/// anything changed. While another device's list is still on its way, it waits
/// for it: an empty one made here would be the newer edit, and win over it.
pub fn include(folder: &api::config::Folder<Local>) -> Result<bool> {
    let root = folder.local_path();
    let stignore_file = root.join(".stignore");
    let stignore = paths::read_if_exists(&stignore_file)?.unwrap_or_default();
    if has_include(&stignore) {
        return Ok(false);
    }
    let list_file = root.join(FILE_NAME);
    let current_list = paths::read_if_exists(&list_file)?;
    let list = match &current_list {
        Some(list) => list.clone(),
        None if elsewhere(folder)? => {
            step!(
                Local,
                "{}: another device's {FILE_NAME} is on its way; it's included once it's here",
                folder.name()
            );
            return Ok(false);
        }
        None if !spreads(folder) => return Ok(false),
        None => NEW_LIST.to_string(),
    };
    let (new_stignore, new_list) = arrange(&stignore, &list, spreads(folder));
    // The list first: Syncthing fails on an included file that's missing.
    if current_list.as_deref() != Some(new_list.as_str()) {
        paths::write(&list_file, &new_list)?;
    }
    paths::write(&stignore_file, &new_stignore)?;
    load(folder, &root)?;
    Ok(true)
}

fn has_include(stignore: &str) -> bool {
    stignore.lines().any(|line| line.trim() == INCLUDE)
}

/// The folder's `.stignore` and shared list once the list is included:
/// with `migrate`, the `.stignore`'s own lines move into the list - after
/// what it has, skipping lines it has already - leaving `.stignore` its
/// includes only; else they stay, before the include.
fn arrange(stignore: &str, list: &str, migrate: bool) -> (String, String) {
    if !migrate {
        return (with_include(stignore), list.to_string());
    }
    let (includes, own): (Vec<&str>, Vec<&str>) = stignore
        .lines()
        .filter(|line| !line.trim().is_empty())
        .partition(|line| line.trim_start().starts_with("#include "));
    let listed: Vec<&str> = list.lines().map(str::trim).collect();
    let moved = own
        .into_iter()
        .filter(|line| !listed.contains(&line.trim()));
    let new_list = list.lines().chain(moved).fold(String::new(), push_line);
    (with_include(&includes.join("\n")), new_list)
}

fn push_line(text: String, line: &str) -> String {
    text + line + "\n"
}

/// Whether another device has the folder's list - one not deleted.
fn elsewhere(folder: &api::config::Folder<Local>) -> Result<bool> {
    let file = client::local()?.find_file(&folder.id, FILE_NAME)?;
    Ok(file.is_some_and(|file| !file.global.deleted))
}

/// Whether a change to the list made here reaches the other devices: in a
/// receive-only folder, it would stay a local change.
fn spreads(folder: &api::config::Folder<Local>) -> bool {
    folder.folder_type != api::config::FolderType::ReceiveOnly
}

/// `stignore` with the [`INCLUDE`] line: after the device's own patterns,
/// so they win, and before the generated `.stignore-git` list, so the
/// hand-written shared patterns win over that.
fn with_include(stignore: &str) -> String {
    let lines: Vec<&str> = stignore.lines().collect();
    let at = lines
        .iter()
        .position(|line| line.trim() == "#include .stignore-git")
        .unwrap_or(lines.len());
    lines[..at]
        .iter()
        .chain([&INCLUDE])
        .chain(&lines[at..])
        .copied()
        .fold(String::new(), push_line)
}

/// Has Syncthing load `folder`'s ignore patterns right away - a scan does -
/// and fails if it finds an error in them.
fn load(folder: &api::config::Folder<Local>, root: &Path) -> Result<()> {
    if settings::dry_run() {
        return Ok(());
    }
    let local = client::local()?;
    local.scan(folder)?;
    match local.ignores_error(&folder.id)? {
        Some(err) => bail!(
            "Syncthing rejects the ignore patterns of {} ({err}); check {}/.stignore and {FILE_NAME}",
            folder.name(),
            root.display()
        ),
        None => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_include_goes_after_the_own_patterns_and_before_the_git_list() {
        assert_eq!(with_include(""), "#include .stignore-shared\n");
        assert_eq!(
            with_include("/tmp\n!/keep"),
            "/tmp\n!/keep\n#include .stignore-shared\n"
        );
        assert_eq!(
            with_include("/tmp\n#include .stignore-git\n"),
            "/tmp\n#include .stignore-shared\n#include .stignore-git\n"
        );
    }

    #[test]
    fn the_own_patterns_move_into_the_list() {
        let (stignore, list) = arrange(
            "// mine\n*.log\n\n#include .stignore-git\n/build/\n",
            NEW_LIST,
            true,
        );
        assert_eq!(
            stignore,
            "#include .stignore-shared\n#include .stignore-git\n"
        );
        assert_eq!(list, format!("{NEW_LIST}// mine\n*.log\n/build/\n"));
    }

    #[test]
    fn lines_the_list_has_already_arent_moved_twice() {
        let (stignore, list) = arrange("*.log\n/tmp/\n", "!.stignore-shared\n*.log\n", true);
        assert_eq!(stignore, "#include .stignore-shared\n");
        assert_eq!(list, "!.stignore-shared\n*.log\n/tmp/\n");
    }

    #[test]
    fn without_migrating_the_own_patterns_stay() {
        let (stignore, list) = arrange("*.log\n", "!.stignore-shared\n", false);
        assert_eq!(stignore, "*.log\n#include .stignore-shared\n");
        assert_eq!(list, "!.stignore-shared\n");
    }

    #[test]
    fn an_include_is_found_however_indented() {
        assert!(has_include("/tmp\n  #include .stignore-shared  \n"));
        assert!(!has_include("#include .stignore-git\n"));
    }
}
