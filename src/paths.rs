//! Path handling: quotes, `~`, relative paths, normalisation and pretty printing.
//!
//! Two rules shape this module:
//! * never hand a `\\?\` verbatim path back to the user (that is what `fs::canonicalize`
//!   produces on Windows and it confuses both humans and `cd`),
//! * compare paths the way the platform does, so re-adding `D:\Code` and `d:\code` is
//!   recognised as the same location.

use std::env;
use std::path::{Component, Path, PathBuf};

/// `$USERPROFILE` on Windows, `$HOME` elsewhere.
pub fn home_dir() -> Option<PathBuf> {
    let key = if cfg!(windows) { "USERPROFILE" } else { "HOME" };
    env::var_os(key)
        .map(PathBuf::from)
        .filter(|path| !path.as_os_str().is_empty())
}

/// Current working directory, or `.` when it cannot be determined.
pub fn current_dir() -> PathBuf {
    env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
}

/// Absolute, normalised current working directory (what "here" means).
pub fn current_dir_absolute() -> PathBuf {
    absolute(&current_dir())
}

/// Strip one layer of surrounding quotes and whitespace.
///
/// Users paste paths from "copy as path" menus and from shell history, both of which add
/// quotes; `"D:\my dir"` must work exactly like `D:\my dir`.
pub fn unquote(input: &str) -> &str {
    let trimmed = input.trim();
    let bytes = trimmed.as_bytes();
    if bytes.len() >= 2 {
        let first = bytes[0];
        let last = bytes[bytes.len() - 1];
        if (first == b'"' && last == b'"') || (first == b'\'' && last == b'\'') {
            return &trimmed[1..trimmed.len() - 1];
        }
    }
    trimmed
}

/// Expand what the user typed into an absolute, normalised path.
pub fn expand(input: &str) -> PathBuf {
    expand_with(input, home_dir().as_deref(), &current_dir())
}

/// [`expand`] with injectable home/cwd, so it can be tested without touching the env.
pub fn expand_with(input: &str, home: Option<&Path>, cwd: &Path) -> PathBuf {
    let text = unquote(input);
    let expanded = if text == "~" {
        match home {
            Some(home) => home.to_path_buf(),
            None => PathBuf::from(text),
        }
    } else if let Some(rest) = tilde_tail(text) {
        match home {
            Some(home) => home.join(rest),
            None => PathBuf::from(text),
        }
    } else {
        PathBuf::from(text)
    };
    absolute_from(&expanded, cwd)
}

fn tilde_tail(text: &str) -> Option<&str> {
    // "~user" style expansion is intentionally not supported: a bare "~x" stays literal.
    text.strip_prefix("~/").or_else(|| text.strip_prefix("~\\"))
}

/// Make `path` absolute against `cwd`, normalise it, and use the real on-disk spelling
/// when the path exists.
pub fn absolute_from(path: &Path, cwd: &Path) -> PathBuf {
    let base = if path.is_relative() { cwd.join(path) } else { path.to_path_buf() };
    let lexical = normalize(&base);
    match std::fs::canonicalize(&lexical) {
        Ok(real) => unverbatim(&real),
        Err(_) => lexical,
    }
}

/// [`absolute_from`] against the current working directory.
pub fn absolute(path: &Path) -> PathBuf {
    absolute_from(path, &current_dir())
}

/// Collapse `.`, `..` and duplicated separators without touching the file system.
///
/// `..` that would escape a root is dropped (`C:\..\a` -> `C:\a`); at the start of a
/// relative path it is preserved (`..\a` stays `..\a`).
pub fn normalize(path: &Path) -> PathBuf {
    let mut head = String::new();
    let mut segments: Vec<std::ffi::OsString> = Vec::new();
    let mut leading_parents = 0usize;

    for component in path.components() {
        match component {
            Component::Prefix(prefix) => head.push_str(&prefix.as_os_str().to_string_lossy()),
            Component::RootDir => {
                let separator = if cfg!(windows) { '\\' } else { '/' };
                if !head.ends_with(separator) {
                    head.push(separator);
                }
            }
            Component::CurDir => {}
            Component::ParentDir => {
                if segments.pop().is_none() && head.is_empty() {
                    leading_parents += 1;
                }
            }
            Component::Normal(part) => segments.push(part.to_os_string()),
        }
    }

    let mut out = PathBuf::from(head);
    for _ in 0..leading_parents {
        out.push("..");
    }
    for segment in segments {
        out.push(segment);
    }
    if out.as_os_str().is_empty() {
        out.push(".");
    }
    out
}

/// Remove Windows verbatim prefixes that `fs::canonicalize` adds.
pub fn unverbatim(path: &Path) -> PathBuf {
    let text = path.to_string_lossy();
    if let Some(rest) = text.strip_prefix(r#"\\?\UNC\"#) {
        return PathBuf::from(format!(r#"\\{rest}"#));
    }
    if let Some(rest) = text.strip_prefix(r#"\\?\"#) {
        return PathBuf::from(rest.to_string());
    }
    if let Some(rest) = text.strip_prefix(r#"\??\"#) {
        return PathBuf::from(rest.to_string());
    }
    path.to_path_buf()
}

/// Platform-appropriate equality for two already-normalised paths.
pub fn same_path(left: &Path, right: &Path) -> bool {
    if left == right {
        return true;
    }
    if !cfg!(windows) {
        return false;
    }
    let fold = |path: &Path| {
        path.to_string_lossy().to_lowercase().replace('/', "\\").trim_end_matches('\\').to_string()
    };
    fold(left) == fold(right)
}

/// Last path component, used to derive a friendly default name.
pub fn file_name(path: &Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().trim().to_string())
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| path.display().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(text: &str) -> PathBuf {
        PathBuf::from(text)
    }

    #[test]
    fn unquote_removes_matching_quotes_only() {
        assert_eq!(unquote(r#""D:\my dir""#), r"D:\my dir");
        assert_eq!(unquote("'/tmp/x y'"), "/tmp/x y");
        assert_eq!(unquote("  D:\\plain  "), "D:\\plain");
        assert_eq!(unquote("\""), "\"");
        assert_eq!(unquote("\"mismatched'"), "\"mismatched'");
        assert_eq!(unquote(""), "");
    }

    #[test]
    fn normalize_collapses_dots() {
        if cfg!(windows) {
            assert_eq!(normalize(&p(r"C:\a\.\b\..\c")), p(r"C:\a\c"));
            assert_eq!(normalize(&p(r"C:\a\b\..")), p(r"C:\a"));
            assert_eq!(normalize(&p(r"C:\..\a")), p(r"C:\a"));
            assert_eq!(normalize(&p(r"C:\a\b\\")), p(r"C:\a\b"));
            assert_eq!(normalize(&p(r"C:\a\b\..\..\")), p(r"C:\"));
        }
        assert_eq!(normalize(&p("a/./b/../c")), p("a/c"));
        assert_eq!(normalize(&p("a/b/..")), p("a"));
        assert_eq!(normalize(&p("..")), p(".."));
        assert_eq!(normalize(&p("../..")), p(if cfg!(windows) { r"..\.." } else { "../.." }));
        assert_eq!(normalize(&p(".")), p("."));
        assert_eq!(normalize(&p("")), p("."));
    }

    #[test]
    fn normalize_keeps_roots_intact() {
        if cfg!(windows) {
            assert_eq!(normalize(&p(r"\\server\share\a\..\b")).to_string_lossy(), r"\\server\share\b");
            assert_eq!(normalize(&p(r"C:")).to_string_lossy(), "C:");
        } else {
            assert_eq!(normalize(&p("/a/../")), p("/"));
        }
    }

    #[test]
    fn expand_applies_home_and_cwd() {
        let home = if cfg!(windows) { p(r"C:\Users\me") } else { p("/home/me") };
        let cwd = if cfg!(windows) { p(r"C:\work") } else { p("/work") };

        assert_eq!(expand_with("~", Some(&home), &cwd), home);
        let joined = expand_with(if cfg!(windows) { r"~\docs" } else { "~/docs" }, Some(&home), &cwd);
        assert_eq!(joined, home.join("docs"));
        assert_eq!(
            expand_with(if cfg!(windows) { r"sub\dir" } else { "sub/dir" }, Some(&home), &cwd),
            cwd.join(if cfg!(windows) { r"sub\dir" } else { "sub/dir" })
        );
        assert_eq!(
            expand_with(if cfg!(windows) { r#""C:\my dir""# } else { r#"'/my dir'"# }, Some(&home), &cwd),
            if cfg!(windows) { p(r"C:\my dir") } else { p("/my dir") }
        );
        // Without a home directory the tilde is left alone instead of panicking.
        assert_eq!(expand_with("~", None, &cwd), cwd.join("~"));
    }

    #[test]
    fn absolute_from_resolves_existing_directories() {
        let temp = env::temp_dir();
        let resolved = absolute_from(&p("."), &temp);
        assert!(resolved.is_absolute(), "{resolved:?}");
        assert!(same_path(&resolved, &absolute_from(&p("."), &temp)));
        assert!(!resolved.to_string_lossy().starts_with(r"\\?\"), "{resolved:?}");
    }

    #[test]
    fn absolute_from_keeps_missing_paths_lexical() {
        let cwd = if cfg!(windows) { p(r"C:\work") } else { p("/work") };
        let missing = cwd.join(if cfg!(windows) { r"nope\..\there" } else { "nope/../there" });
        let resolved = absolute_from(&missing, &cwd);
        assert_eq!(resolved, cwd.join("there"));
    }

    #[test]
    fn unverbatim_strips_windows_prefixes() {
        assert_eq!(unverbatim(&p(r"\\?\C:\a\b")), p(r"C:\a\b"));
        assert_eq!(unverbatim(&p(r"\\?\UNC\server\share")), p(r"\\server\share"));
        assert_eq!(unverbatim(&p("/tmp/x")), p("/tmp/x"));
    }

    #[test]
    fn same_path_follows_platform_rules() {
        if cfg!(windows) {
            assert!(same_path(&p(r"C:\A\b"), &p(r"c:\a\B")));
            assert!(same_path(&p(r"C:\a\b"), &p(r"C:/a/b")));
            assert!(same_path(&p(r"C:\a\b\"), &p(r"C:\a\b")));
            assert!(!same_path(&p(r"C:\a\b"), &p(r"C:\a\c")));
        } else {
            assert!(same_path(&p("/a/b"), &p("/a/b")));
            assert!(!same_path(&p("/a/b"), &p("/A/B")));
        }
    }

    #[test]
    fn file_name_falls_back_to_the_whole_path() {
        assert_eq!(file_name(&p(if cfg!(windows) { r"C:\a\proj" } else { "/a/proj" })), "proj");
        if cfg!(windows) {
            assert_eq!(file_name(&p(r"C:\")), r"C:\");
        } else {
            assert_eq!(file_name(&p("/")), "/");
        }
    }
}