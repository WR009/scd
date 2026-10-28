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

use crate::color::Palette;
use crate::config::{Added, Config};
use crate::menu::styled_summary;
use crate::paths;
use crate::ui::Ui;

/// Prompt shown while waiting for the new location.
pub const PROMPT: &str = "new location: ";

/// The two instruction lines, verbatim from the specification (colour added on top).
pub fn heading(palette: &Palette) -> Vec<String> {
    vec![
        format!(
            "type the new location, or type {} to add the current location",
            palette.bold("\"here\"")
        ),
        format!("type {} to cancel", palette.bold("\"quit\"")),
    ]
}

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
    let palette = ui.palette();
    ui.lines(&heading(&palette));
    ui.blank();

    loop {
        let Some(raw) = ui.ask(&palette.prompt(PROMPT)) else {
            ui.blank();
            ui.line(&palette.dim("cancelled"));
            ui.blank();
            return;
        };
        let input = raw.trim();
        if input.is_empty() {
            continue;
        }
        match interpret(cfg, config_path, input, &palette) {
            Outcome::Cancel => {
                ui.line(&palette.dim("cancelled"));
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
pub fn non_interactive(cfg: &mut Config, config_path: &Path, target: Option<&str>, ui: &Ui) -> i32 {
    let input = target.map(str::trim).filter(|text| !text.is_empty()).unwrap_or("here");
    match interpret(cfg, config_path, input, &ui.palette()) {
        Outcome::Cancel => {
            ui.out(&ui.palette().dim("cancelled"));
            crate::exit::OK
        }
        Outcome::Done(lines) => {
            ui.out_lines(&lines);
            crate::exit::OK
        }
        Outcome::Retry(lines) => {
            let palette = Palette::for_stream(crate::ui::Stream::Stderr);
            for line in &lines {
                eprintln!("{} {}", palette.error_prefix(), palette.err(line));
            }
            crate::exit::TARGET
        }
    }
}

/// Validate one answer and, when it is a directory, save it.
pub fn interpret(cfg: &mut Config, config_path: &Path, input: &str, palette: &Palette) -> Outcome {
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
        let reason =
            if candidate.exists() { "that is a file, not a directory" } else { "no such directory" };
        return Outcome::Retry(vec![
            format!("{}: {}", palette.err(reason), palette.path(&candidate.display().to_string())),
            format!(
                "type an absolute path, {} for the current directory, or {} to cancel",
                palette.bold("\"here\""),
                palette.bold("\"quit\"")
            ),
        ]);
    }

    let added = cfg.add(candidate);
    let index = match added {
        Added::New(index) | Added::Existing(index) => index,
    };
    let summary = styled_summary(&cfg.locations[index], palette);
    if let Err(err) = cfg.save(config_path) {
        return Outcome::Retry(vec![palette.err(&err)]);
    }

    match added {
        Added::New(_) => {
            let mut lines = vec![format!(
                "{} {} {}",
                palette.ok("added"),
                palette.key(&(index + 1).to_string()),
                summary
            )];
            // Entries are appended, so a new one can land outside the visible part of the menu.
            if index >= cfg.max_display {
                lines.push(palette.warn(&format!(
                    "note: [{}] is past \"max locations to display\" ({}), so the menu hides it - \
                     raise the limit in [c], or bring it up with 'scd mv {} 1'",
                    index + 1,
                    cfg.max_display,
                    index + 1
                )));
            }
            Outcome::Done(lines)
        }
        Added::Existing(_) => Outcome::Done(vec![
            format!(
                "{} {} {}",
                palette.warn("already saved as"),
                palette.key(&(index + 1).to_string()),
                summary
            ),
            palette.dim("nothing changed"),
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
    use crate::color::strip_ansi;
    use crate::ui::Stream;
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

    fn out() -> Ui {
        Ui::plain(Stream::Stdout)
    }

    #[test]
    fn heading_is_the_specification_text_verbatim() {
        let lines = heading(&Palette::off());
        assert_eq!(lines[0], "type the new location, or type \"here\" to add the current location");
        assert_eq!(lines[1], "type \"quit\" to cancel");

        let colored = heading(&Palette::on());
        assert_eq!(strip_ansi(&colored[0]), lines[0]);
        assert_eq!(strip_ansi(&colored[1]), lines[1]);
        assert!(colored[0].contains("\x1b[1m\"here\"\x1b[0m"), "{:?}", colored[0]);
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

        match interpret(&mut cfg, &config_path, target.to_str().unwrap(), &Palette::off()) {
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
    fn coloured_confirmation_highlights_name_and_path() {
        let dir = workspace("colored");
        let target = dir.join("project");
        fs::create_dir_all(&target).unwrap();
        let mut cfg = Config::default();
        match interpret(&mut cfg, &dir.join("config.json"), target.to_str().unwrap(), &Palette::on())
        {
            Outcome::Done(lines) => {
                assert_eq!(strip_ansi(&lines[0]).trim_start(), strip_ansi(&lines[0]).trim_start());
                assert!(lines[0].starts_with("\x1b[1;32madded\x1b[0m "), "{:?}", lines[0]);
                assert!(lines[0].contains("\x1b[1;32mproject\x1b[0m"), "{:?}", lines[0]);
                assert!(
                    lines[0].contains(&format!("\x1b[1;36m{}\x1b[0m", target.display())),
                    "{:?}",
                    lines[0]
                );
            }
            other => panic!("expected Done, got {other:?}"),
        }
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn second_add_is_reported_as_existing() {
        let dir = workspace("twice");
        let target = dir.join("project");
        fs::create_dir_all(&target).unwrap();
        let config_path = dir.join("config.json");
        let mut cfg = Config::default();
        let off = Palette::off();
        assert!(matches!(
            interpret(&mut cfg, &config_path, target.to_str().unwrap(), &off),
            Outcome::Done(_)
        ));
        match interpret(&mut cfg, &config_path, target.to_str().unwrap(), &off) {
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
        let off = Palette::off();

        match interpret(&mut cfg, &config_path, file.to_str().unwrap(), &off) {
            Outcome::Retry(lines) => {
                assert!(lines[0].contains("file, not a directory"), "{lines:?}");
                assert!(lines[1].contains("\"here\""), "{lines:?}");
            }
            other => panic!("expected Retry, got {other:?}"),
        }
        match interpret(&mut cfg, &config_path, dir.join("ghost").to_str().unwrap(), &off) {
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
        assert_eq!(interpret(&mut cfg, &config_path, "quit", &Palette::off()), Outcome::Cancel);
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
        match interpret(&mut cfg, &config_path, &quoted, &Palette::off()) {
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
        let ui = out();
        // The test process runs in the crate root, which is a real directory.
        assert_eq!(non_interactive(&mut cfg, &config_path, None, &ui), crate::exit::OK);
        assert_eq!(cfg.locations.len(), 1);
        assert!(!cfg.locations[0].path.as_os_str().is_empty());

        let mut broken = Config::default();
        let code = non_interactive(
            &mut broken,
            &config_path,
            Some(if cfg!(windows) { r"Z:\nope\nope" } else { "/nonexistent/nope" }),
            &ui,
        );
        assert_eq!(code, crate::exit::TARGET);
        assert!(broken.is_empty());
        let _ = fs::remove_dir_all(&dir);
    }
}