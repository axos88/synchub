//! A .gitignore line as Syncthing patterns.
//!
//! git reads a pattern relative to the directory of its .gitignore; Syncthing
//! reads every pattern relative to the folder's root. Only that is translated
//! here: git lets the last matching pattern win and Syncthing the first, so
//! the order is the caller's.

use std::path::Path;

use gix::glob::wildmatch;
use itertools::Itertools;

use crate::paths;

/// The Syncthing patterns for `line`, a line of the .gitignore in `dir`, in
/// the folder at `root` - none for a comment, or a pattern that can't match
/// inside the folder. `fold_case` for a repository that ignores case.
pub fn translate(line: &str, dir: &Path, root: &Path, fold_case: bool) -> Vec<String> {
    let Some(rule) = Rule::parse(line) else {
        return Vec::new();
    };
    let prefix = format!(
        "{}{}",
        if rule.negated { "!" } else { "" },
        if fold_case { "(?i)" } else { "" }
    );
    let suffix = if rule.dir_only { "/" } else { "" };
    rule.bases(dir, root, fold_case)
        .iter()
        .flat_map(|base| with_empty_stars(base))
        .unique()
        .map(|base| format!("{prefix}{base}{suffix}"))
        .collect()
}

/// `base` and, for each `/**/` in it, the same with it collapsed to `/`: in
/// git `**/` also matches no directory at all, in Syncthing at least one.
fn with_empty_stars(base: &str) -> Vec<String> {
    let mut parts = base.split("/**/");
    let first = parts.next().unwrap_or_default().to_string();
    parts.fold(vec![first], |variants, part| {
        variants
            .iter()
            .flat_map(|variant| [format!("{variant}/**/{part}"), format!("{variant}/{part}")])
            .collect()
    })
}

/// The directory a negation re-includes from, relative to the .gitignore's -
/// if it names one literally: git can't re-include from an excluded
/// directory, so the caller drops the negation then.
pub fn negated_parent(line: &str) -> Option<String> {
    let rule = Rule::parse(line).filter(|rule| rule.negated)?;
    let (_, parents) = rule.segments.split_last()?;
    let literal = parents
        .iter()
        .all(|segment| !segment.contains(['*', '?', '[', '\\']));
    (!parents.is_empty() && literal).then(|| parents.join("/"))
}

/// A .gitignore pattern, taken apart.
struct Rule {
    negated: bool,
    /// Ends with `/`: matches directories only.
    dir_only: bool,
    /// Has a `/` before its end: relative to the .gitignore's directory, not
    /// matching at any depth below it.
    anchored: bool,
    /// Its parts between the slashes, as git writes them.
    segments: Vec<String>,
}

impl Rule {
    fn parse(line: &str) -> Option<Self> {
        let line = trim_trailing_spaces(line.trim_end_matches('\r'));
        if line.is_empty() || line.starts_with('#') {
            return None;
        }
        let (negated, line) = match line.strip_prefix('!') {
            Some(rest) => (true, rest),
            None => (false, line),
        };
        let (dir_only, line) = match line.strip_suffix('/') {
            Some(rest) => (true, rest),
            None => (false, line),
        };
        // git's rule: a slash anywhere but at the end anchors the pattern to
        // its .gitignore's directory - `/foo` and `foo/bar` are anchored,
        // `foo` and `foo/` match at any depth. The trailing one is gone by now.
        let anchored = line.contains('/');
        let line = line.strip_prefix('/').unwrap_or(line);
        (!line.is_empty()).then(|| Rule {
            negated,
            dir_only,
            anchored,
            segments: line.split('/').map(String::from).collect(),
        })
    }

    /// The patterns, without the negation and the trailing `/`.
    fn bases(&self, dir: &Path, root: &Path, fold_case: bool) -> Vec<String> {
        let pattern = |segments: &[String]| {
            segments
                .iter()
                .map(|segment| syncthing(segment))
                .collect::<Vec<_>>()
                .join("/")
        };
        let whole = pattern(&self.segments);
        if paths::inside(dir, root) {
            let at = paths::relative(dir, root);
            return match (self.anchored, at.is_empty()) {
                (true, true) => vec![format!("/{whole}")],
                (true, false) => vec![format!("/{at}/{whole}")],
                // Syncthing matches these at any depth itself.
                (false, true) => vec![whole],
                (false, false) => vec![format!("/{at}/**/{whole}")],
            };
        }
        if !paths::inside(root, dir) {
            return Vec::new();
        }
        if !self.anchored {
            return vec![whole];
        }
        // From above the folder: the segments up to a `**` have to match the
        // way down to the root; the rest is what's matched inside it.
        let names: Vec<String> = root
            .strip_prefix(dir)
            .unwrap_or(root)
            .components()
            .map(|c| c.as_os_str().to_string_lossy().into_owned())
            .collect();
        let star = self
            .segments
            .iter()
            .position(|segment| segment == "**")
            .unwrap_or(self.segments.len());
        let mode = if fold_case {
            wildmatch::Mode::NO_MATCH_SLASH_LITERAL | wildmatch::Mode::IGNORE_CASE
        } else {
            wildmatch::Mode::NO_MATCH_SLASH_LITERAL
        };
        let on_the_way = self.segments[..star.min(names.len())]
            .iter()
            .zip(&names)
            .all(|(segment, name)| {
                gix::glob::wildmatch(segment.as_str().into(), name.as_str().into(), mode)
            });
        if !on_the_way {
            return Vec::new();
        }
        if star < self.segments.len() && star <= names.len() {
            // The `**` reaches into the folder: what follows it matches at any depth.
            return match &self.segments[star + 1..] {
                [] => vec!["**".to_string()],
                rest => vec![pattern(rest)],
            };
        }
        if self.segments.len() > names.len() {
            vec![format!("/{}", pattern(&self.segments[names.len()..]))]
        } else {
            // It matches the root, or a directory above it.
            Vec::new()
        }
    }
}

/// `line` without trailing spaces, unless they're escaped.
fn trim_trailing_spaces(line: &str) -> &str {
    let trimmed = line.trim_end_matches(' ');
    if trimmed.ends_with('\\') && trimmed.len() < line.len() {
        &line[..trimmed.len() + 1]
    } else {
        trimmed
    }
}

/// A .gitignore glob as Syncthing's: those also know `{a,b}`, which a
/// .gitignore means literally.
fn syncthing(segment: &str) -> String {
    segment
        .chars()
        .scan(false, |escaped, c| {
            let out = match c {
                '{' | '}' if !*escaped => format!("\\{c}"),
                _ => c.to_string(),
            };
            *escaped = c == '\\' && !*escaped;
            Some(out)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(dir: &str, line: &str) -> Vec<String> {
        translate(line, Path::new(dir), Path::new("/f"), false)
    }

    #[test]
    fn comments_and_blank_lines_are_nothing() {
        assert!(at("/f", "# a comment").is_empty());
        assert!(at("/f", "").is_empty());
        assert!(at("/f", "   ").is_empty());
    }

    #[test]
    fn at_the_root_unanchored_patterns_match_anywhere() {
        assert_eq!(at("/f", "*.log"), ["*.log"]);
        assert_eq!(at("/f", "target/"), ["target/"]);
        assert_eq!(at("/f", "!keep.log"), ["!keep.log"]);
    }

    #[test]
    fn a_slash_anchors_to_the_gitignores_directory() {
        assert_eq!(at("/f", "/build"), ["/build"]);
        assert_eq!(at("/f", "docs/*.pdf"), ["/docs/*.pdf"]);
        assert_eq!(at("/f/sub", "/build"), ["/sub/build"]);
        assert_eq!(at("/f/sub", "out/tmp/"), ["/sub/out/tmp/"]);
        // A slash in the middle anchors; only a trailing one doesn't.
        assert_eq!(at("/f/sub", "foo/bar"), ["/sub/foo/bar"]);
        assert_eq!(at("/f/sub", "foo/"), ["/sub/**/foo/", "/sub/foo/"]);
    }

    #[test]
    fn a_double_star_also_matches_no_directory() {
        assert_eq!(at("/f", "**/foo/bar"), ["/**/foo/bar", "/foo/bar"]);
        assert_eq!(at("/f", "a/**/b"), ["/a/**/b", "/a/b"]);
        assert_eq!(
            at("/f/sub", "a/**/b/**/c"),
            [
                "/sub/a/**/b/**/c",
                "/sub/a/**/b/c",
                "/sub/a/b/**/c",
                "/sub/a/b/c"
            ]
        );
        assert_eq!(at("/f", "a/**"), ["/a/**"]);
    }

    #[test]
    fn below_the_root_unanchored_patterns_match_at_any_depth_below_it() {
        assert_eq!(at("/f/sub", "*.log"), ["/sub/**/*.log", "/sub/*.log"]);
        assert_eq!(at("/f/sub", "!keep/"), ["!/sub/**/keep/", "!/sub/keep/"]);
    }

    #[test]
    fn from_above_the_root_only_whats_inside_it_is_kept() {
        assert_eq!(at("/", "*.log"), ["*.log"]);
        assert_eq!(at("/", "f/tmp"), ["/tmp"]);
        assert_eq!(at("/", "*/tmp"), ["/tmp"]);
        assert_eq!(at("/", "other/tmp"), Vec::<String>::new());
        assert_eq!(at("/", "**/tmp"), ["tmp"]);
        assert_eq!(at("/", "f/**"), ["**"]);
        // The folder itself.
        assert_eq!(at("/", "/f"), Vec::<String>::new());
    }

    #[test]
    fn braces_are_literal_in_git_but_not_in_syncthing() {
        assert_eq!(at("/f", "a{b}"), ["a\\{b\\}"]);
        assert_eq!(at("/f", "a\\{b"), ["a\\{b"]);
    }

    #[test]
    fn trailing_spaces_go_unless_escaped() {
        assert_eq!(at("/f", "foo  "), ["foo"]);
        assert_eq!(at("/f", "foo\\ "), ["foo\\ "]);
        assert_eq!(at("/f", "\\#hash"), ["\\#hash"]);
    }

    #[test]
    fn case_folding_goes_after_the_negation() {
        let lines = translate("!Readme", Path::new("/f"), Path::new("/f"), true);
        assert_eq!(lines, ["!(?i)Readme"]);
    }

    #[test]
    fn a_negations_literal_parent_is_found() {
        assert_eq!(negated_parent("!build/keep").as_deref(), Some("build"));
        assert_eq!(negated_parent("!/a/b/c").as_deref(), Some("a/b"));
        assert_eq!(negated_parent("!keep"), None);
        assert_eq!(negated_parent("!*/keep"), None);
        assert_eq!(negated_parent("build/keep"), None);
    }
}
