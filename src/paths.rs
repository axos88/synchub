//! Path helpers shared by the commands.

use std::path::{Path, PathBuf};

pub fn home() -> PathBuf {
    std::env::var_os("HOME")
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

/// `path` relative to `base` (which it must be inside), with `/` separators.
pub fn relative(path: &Path, base: &Path) -> String {
    let rel = path.strip_prefix(base).unwrap_or(path);
    rel.components()
        .map(|c| c.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
}

/// How to write `path` into a folder's config: in `~/` form if the old value used it.
pub fn config_form(old: &str, path: &Path) -> String {
    let home = canon(&home());
    if old.starts_with('~') && inside(path, &home) {
        let rel = relative(path, &home);
        return if rel.is_empty() {
            "~".to_string()
        } else {
            format!("~/{rel}")
        };
    }
    path.display().to_string()
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
    fn config_form_follows_the_old_value() {
        let home = canon(&home());
        assert_eq!(config_form("~/x", &home.join("y/z")), "~/y/z");
        assert_eq!(
            config_form("/abs/x", &home.join("y")),
            home.join("y").display().to_string()
        );
        assert_eq!(config_form("~/x", Path::new("/elsewhere")), "/elsewhere");
    }
}
