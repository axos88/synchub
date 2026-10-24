//! Path helpers shared by the commands.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

pub fn home() -> PathBuf {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/"))
}

/// Expands a leading `~`, the way Syncthing does for folder paths.
pub fn expand_tilde(path: &str) -> PathBuf {
    if path == "~" {
        home()
    } else if let Some(rest) = path.strip_prefix("~/") {
        home().join(rest)
    } else {
        PathBuf::from(path)
    }
}

/// Absolute and symlink-free; the trailing components that don't exist yet are kept as written.
pub fn canon(path: &Path) -> PathBuf {
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir().unwrap_or_default().join(path)
    };
    let mut existing = absolute.as_path();
    let mut missing = Vec::new();
    loop {
        if let Ok(resolved) = existing.canonicalize() {
            return missing
                .iter()
                .rev()
                .fold(resolved, |path, name| path.join(name));
        }
        match (existing.parent(), existing.file_name()) {
            (Some(parent), Some(name)) => {
                missing.push(name);
                existing = parent;
            }
            _ => return absolute,
        }
    }
}

/// Whether `path` is `dir` or lies below it.
pub fn inside(path: &Path, dir: &Path) -> bool {
    path.starts_with(dir)
}

/// Whether one of `a` and `b` is the other, or lies inside it.
pub fn related(a: &Path, b: &Path) -> bool {
    inside(a, b) || inside(b, a)
}

/// `path` relative to `base` (which it must be inside), with `/` separators.
pub fn relative(path: &Path, base: &Path) -> String {
    let rel = path.strip_prefix(base).unwrap_or(path);
    rel.components()
        .map(|c| c.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
}

pub fn is_empty_dir(dir: &Path) -> io::Result<bool> {
    Ok(fs::read_dir(dir)?.next().is_none())
}

/// `path` the way a person would write it: under home in `~/` form,
/// elsewhere in full.
pub fn relative_to_home(path: &Path) -> String {
    let home = canon(&home());
    if !inside(path, &home) {
        return path.display().to_string();
    }
    let rel = relative(path, &home);
    if rel.is_empty() {
        "~".to_string()
    } else {
        format!("~/{rel}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canon_keeps_missing_components() {
        let tmp = canon(&std::env::temp_dir());
        assert_eq!(canon(&tmp.join("no/such/dir")), tmp.join("no/such/dir"));
    }

    #[test]
    fn canon_strips_trailing_slash() {
        let tmp = canon(&std::env::temp_dir());
        assert_eq!(canon(Path::new(&format!("{}/", tmp.display()))), tmp);
    }

    #[test]
    fn inside_is_by_component() {
        assert!(inside(Path::new("/a/b"), Path::new("/a")));
        assert!(inside(Path::new("/a"), Path::new("/a")));
        assert!(!inside(Path::new("/ab"), Path::new("/a")));
    }

    #[test]
    fn related_is_either_way() {
        let (dir, sub) = (Path::new("/a/b"), Path::new("/a/b/c"));
        assert!(related(dir, sub) && related(sub, dir) && related(dir, dir));
        assert!(!related(dir, Path::new("/a/bc")));
    }

    #[test]
    fn paths_relative_to_home() {
        let home = canon(&home());
        assert_eq!(relative_to_home(&home.join("y/z")), "~/y/z");
        assert_eq!(relative_to_home(&home), "~");
        assert_eq!(relative_to_home(Path::new("/elsewhere")), "/elsewhere");
    }
}
