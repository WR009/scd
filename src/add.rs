//! The `a` flow from `docs/ARCHITECTURE.md`:
//!
//! ```text
//!     type the new location, or type "here" to add the current location
//!     type "quit" to cancel
//!
//!     new location:
//! ```
//!
//! The same [`interpret`] function backs `scd add <path>`, so the interactive and
//! non-interactive paths can never drift apart.

use std::path::Path;

use crate::config::{Added, Config};
use crate::paths;
use crate::ui::Ui;

/// The two instruction lines, verbatim from the specification.
pub const HEADING: [&str; 2] = [
    "type the new location, or type \"here\" to add the current location",
    "type \"quit\" to cancel",
];

/// Prompt shown while waiting for the new location.
pub const PROMPT: &str = "new location: ";

/// What one round of input produced.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    /// The user backed out; nothing changed.
    Cancel,
    /// Show these lines, then ask again (invalid path, unwritable config, ...).
    Retry(Vec<String>),
    /// Show these lines and leave the flow (added, or already saved).
    Done(Vec<String>),
}

/// Interactive add: prompt until the input is usable or the user cancels.
pub fn run(cfg: &mut Config, config_path: &Path, ui: &Ui) {
    for line in HEADING {
        ui.line(line);
    }
    ui.blank();

    loop {
        let Some(raw) = ui.ask(PROMPT) else {
            ui.blank();
            ui.line("cancelled");
            ui.blank();
            return;
        };
        let input = raw.trim();
        if input.is_empty() {
            continue;
        }
        match interpret(cfg, config_path, input) {
            Outcome::Cancel => {
                ui.line("cancelled");
                ui.blank();
                return;
            }
            Outcome::Retry(lines) => {
                ui.lines(&lines);
                ui.blank();
            }
            Outcome::Done(lines) => {
                ui.lines(&lines);
                ui.blank();
                return;
            }
        }
    }
}

/// `scd add [path]`: one shot, no prompting. Defaults to the current directory.
pub fn non_interactive(cfg: &mut Config, config_path: &Path, target: Option<&str>) -> i32 {
    let input = target.map(str::trim).filter(|text| !text.is_empty()).unwrap_or("here");
    match interpret(cfg, config_path, input) {
        Outcome::Cancel => {
            crate::ui::print_line("cancelled");
            crate::exit::OK
        }
        Outcome::Done(lines) => {
            for line in &lines {
                crate::ui::print_line(line);
            }
            crate::exit::OK
        }
        Outcome::Retry(lines) => {
            for line in &lines {
                eprintln!("scd: {line}");
            }
            crate::exit::TARGET
        }
    }
}

/// Validate one answer and, when it is a directory, save it.
pub fn interpret(cfg: &mut Config, config_path: &Path, input: &str) -> Outcome {
    let text = input.trim();
    if is_cancel(text) {
        return Outcome::Cancel;
    }

    let candidate = if is_here(text) {
        paths::current_dir_absolute()
    } else {
        paths::expand(text)
    };

    if !candidate.is_dir() {
        let reason = if candidate.exists() { "that is a file, not a directory" } else { "no such directory" };
        return Outcome::Retry(vec![
            format!("{reason}: {}", candidate.display()),
            "type an absolute path, \"here\" for the current directory, or \"quit\" to cancel"
                .to_string(),
        ]);
    }

    let added = cfg.add(candidate);
    let index = match added {
        Added::New(index) | Added::Existing(index) => index,
    };
    let summary = cfg.locations[index].summary();
    if let Err(err) = cfg.save(config_path) {
        return Outcome::Retry(vec![err]);
    }

    match added {
        Added::New(_) => Outcome::Done(vec![format!("added [{}] {summary}", index + 1)]),
        Added::Existing(_) => Outcome::Done(vec![
            format!("already saved as [{}] {summary}", index + 1),
            "nothing changed".to_string(),
        ]),
    }
}

/// Words that cancel the flow.
pub fn is_cancel(text: &str) -> bool {
    matches!(text.to_ascii_lowercase().as_str(), "quit" | "q" | "cancel" | "exit" | "back")
}

/// Words that mean "the directory I am standing in".
pub fn is_here(text: &str) -> bool {
    matches!(text.to_ascii_lowercase().as_str(), "here" | "." | "./" | ".\\" | "cwd")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;

    fn workspace(tag: &str) -> PathBuf {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("scd-add-{tag}-{}-{nanos}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn keyword_detection() {
        assert!(is_cancel("quit") && is_cancel("QUIT") && is_cancel("q") && is_cancel("cancel"));
        assert!(!is_cancel("here") && !is_cancel("D:\\x"));
        assert!(is_here("here") && is_here("HERE") && is_here(".") && is_here("./"));
        assert!(!is_here("quit") && !is_here(".."));
    }

    #[test]
    fn adds_a_directory_and_persists_it() {
        let dir = workspace("adds");
        let target = dir.join("project");
        fs::create_dir_all(&target).unwrap();
        let config_path = dir.join("config.json");
        let mut cfg = Config::default();

        match interpret(&mut cfg, &config_path, target.to_str().unwrap()) {
            Outcome::Done(lines) => {
                assert_eq!(lines.len(), 1);
                assert!(lines[0].starts_with("added [1] project"), "{lines:?}");
                assert!(lines[0].contains(&target.display().to_string()), "{lines:?}");
            }
            other => panic!("expected Done, got {other:?}"),
        }

        let saved = Config::load(&config_path).unwrap();
        assert_eq!(saved.locations.len(), 1);
        assert_eq!(saved.locations[0].name, "project");
        assert!(paths::same_path(&saved.locations[0].path, &paths::absolute(&target)));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn second_add_is_reported_as_existing() {
        let dir = workspace("twice");
        let target = dir.join("project");
        fs::create_dir_all(&target).unwrap();
        let config_path = dir.join("config.json");
        let mut cfg = Config::default();
        assert!(matches!(interpret(&mut cfg, &config_path, target.to_str().unwrap()), Outcome::Done(_)));
        match interpret(&mut cfg, &config_path, target.to_str().unwrap()) {
            Outcome::Done(lines) => {
                assert!(lines[0].starts_with("already saved as [1]"), "{lines:?}");
                assert_eq!(cfg.locations.len(), 1);
            }
            other => panic!("expected Done, got {other:?}"),
        }
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn rejects_files_and_missing_directories() {
        let dir = workspace("rejects");
        let file = dir.join("notes.txt");
        fs::write(&file, "x").unwrap();
        let config_path = dir.join("config.json");
        let mut cfg = Config::default();

        match interpret(&mut cfg, &config_path, file.to_str().unwrap()) {
            Outcome::Retry(lines) => {
                assert!(lines[0].contains("file, not a directory"), "{lines:?}");
                assert!(lines[1].contains("\"here\""), "{lines:?}");
            }
            other => panic!("expected Retry, got {other:?}"),
        }
        match interpret(&mut cfg, &config_path, dir.join("ghost").to_str().unwrap()) {
            Outcome::Retry(lines) => assert!(lines[0].contains("no such directory"), "{lines:?}"),
            other => panic!("expected Retry, got {other:?}"),
        }
        assert!(cfg.is_empty());
        assert!(!config_path.exists(), "nothing is written when the input is unusable");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn cancel_changes_nothing() {
        let dir = workspace("cancel");
        let config_path = dir.join("config.json");
        let mut cfg = Config::default();
        assert_eq!(interpret(&mut cfg, &config_path, "quit"), Outcome::Cancel);
        assert!(cfg.is_empty());
        assert!(!config_path.exists());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn accepts_quoted_paths_with_spaces() {
        let dir = workspace("spaces");
        let target = dir.join("my projects");
        fs::create_dir_all(&target).unwrap();
        let config_path = dir.join("config.json");
        let mut cfg = Config::default();
        let quoted = format!("\"{}\"", target.display());
        match interpret(&mut cfg, &config_path, &quoted) {
            Outcome::Done(lines) => assert!(lines[0].contains("my projects"), "{lines:?}"),
            other => panic!("expected Done, got {other:?}"),
        }
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn non_interactive_defaults_to_here_and_reports_failures() {
        let dir = workspace("noninteractive");
        let config_path = dir.join("config.json");
        let mut cfg = Config::default();
        // The test process runs in the crate root, which is a real directory.
        assert_eq!(non_interactive(&mut cfg, &config_path, None), crate::exit::OK);
        assert_eq!(cfg.locations.len(), 1);
        assert!(!cfg.locations[0].path.as_os_str().is_empty());

        let mut broken = Config::default();
        let code = non_interactive(
            &mut broken,
            &config_path,
            Some(if cfg!(windows) { r"Z:\nope\nope" } else { "/nonexistent/nope" }),
        );
        assert_eq!(code, crate::exit::TARGET);
        assert!(broken.is_empty());
        let _ = fs::remove_dir_all(&dir);
    }
}