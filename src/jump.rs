//! Turning whatever the user typed into a directory, then handing it to the shell.
//!
//! Resolution order for a target string (also used by the interactive menu, so typing a
//! path at the `scd>` prompt works too):
//!
//! 1. an existing directory path (`~`, relative paths and quoted paths all allowed),
//! 2. a menu number (`3` -> the third saved location),
//! 3. a saved name, strongest match tier first (see [`Config::match_name`]).

use std::fmt;
use std::path::{Path, PathBuf};

use crate::config::Config;
use crate::emit::Emitter;
use crate::paths;

/// A resolved jump destination.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Target {
    /// Index into `Config::locations`.
    Saved(usize),
    /// A directory that is not necessarily saved.
    Path(PathBuf),
}

/// Why a target could not be resolved.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    Empty,
    NotADirectory(String),
    IsAFile(String),
    Unknown(String),
    OutOfRange { index: usize, count: usize },
    /// Message is already complete, including the competing names.
    Ambiguous(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Empty => write!(f, "nothing to jump to"),
            Error::NotADirectory(path) => write!(f, "not a directory: {path}"),
            Error::IsAFile(path) => write!(f, "that is a file, not a directory: {path}"),
            Error::Unknown(query) => write!(f, "no saved location matches '{query}'"),
            Error::OutOfRange { index, count: 0 } => {
                write!(f, "nothing is saved yet, so [{index}] does not exist - run 'scd' and press [a]")
            }
            Error::OutOfRange { index, count } => write!(
                f,
                "[{index}] is out of range: {count} location(s) saved, use 1..{count}"
            ),
            Error::Ambiguous(message) => write!(f, "{message}"),
        }
    }
}

impl std::error::Error for Error {}

pub type Result<T> = std::result::Result<T, Error>;

/// Resolve a user-supplied target without touching the file system more than needed.
pub fn resolve(cfg: &Config, query: &str) -> Result<Target> {
    let text = query.trim();
    if text.is_empty() {
        return Err(Error::Empty);
    }

    let expanded = paths::expand(text);
    if expanded.is_dir() {
        return Ok(Target::Path(expanded));
    }
    if expanded.is_file() {
        return Err(Error::IsAFile(expanded.display().to_string()));
    }

    if let Some(index) = menu_number(text) {
        if index >= 1 && index <= cfg.locations.len() {
            return Ok(Target::Saved(index - 1));
        }
        // Out of range: it may still be somebody's folder name ("2024", "3d").
        let matches = cfg.match_name(text);
        if matches.len() == 1 {
            return Ok(Target::Saved(matches[0]));
        }
        if !matches.is_empty() {
            return Err(ambiguous(cfg, text, &matches));
        }
        return Err(Error::OutOfRange { index, count: cfg.locations.len() });
    }

    match cfg.match_name(text).as_slice() {
        [] => Err(Error::Unknown(text.to_string())),
        [only] => Ok(Target::Saved(*only)),
        matches => Err(ambiguous(cfg, text, matches)),
    }
}

/// Perform a resolved jump: remember the visit, then report the directory to the shell.
pub fn perform(
    cfg: &mut Config,
    config_path: &Path,
    target: Target,
    ui: &crate::ui::Ui,
    emitter: &Emitter,
) -> i32 {
    let (path, saved_index) = match target {
        Target::Path(path) => (path, None),
        Target::Saved(index) => (cfg.locations[index].path.clone(), Some(index)),
    };
    if !path.is_dir() {
        eprintln!("scd: {}", Error::NotADirectory(path.display().to_string()));
        return crate::exit::TARGET;
    }

    // Most-recently-used bookkeeping: the location moves to the top of the next menu.
    if let Some(index) = saved_index.or_else(|| cfg.index_of_path(&path)) {
        cfg.touch(index);
        if let Err(err) = cfg.save(config_path) {
            eprintln!("scd: {err}");
        }
    }
    emitter.emit_jump(&path, ui)
}

/// `scd <target>` on the command line.
pub fn query(cfg: &mut Config, config_path: &Path, text: &str, emitter: &Emitter) -> i32 {
    let ui = emitter.ui();
    match resolve(cfg, text) {
        Ok(target) => perform(cfg, config_path, target, &ui, emitter),
        Err(err) => {
            let diagnostics = emitter.diagnostics();
            diagnostics.line(&format!("scd: {err}"));
            if matches!(err, Error::Unknown(_) | Error::IsAFile(_)) {
                diagnostics.line("hint: 'scd' opens the menu, 'scd add <path>' saves a location");
            }
            crate::exit::TARGET
        }
    }
}

/// Strict menu-number parsing: digits only, no sign, no wrapping.
fn menu_number(text: &str) -> Option<usize> {
    if !text.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    text.parse::<usize>().ok()
}

fn ambiguous(cfg: &Config, query: &str, matches: &[usize]) -> Error {
    let names: Vec<String> = matches.iter().map(|&index| cfg.locations[index].name.clone()).collect();
    let numbers: Vec<String> = matches.iter().map(|&index| (index + 1).to_string()).collect();
    Error::Ambiguous(format!(
        "'{query}' matches {} saved locations ({}); use a menu number instead: [{}]",
        matches.len(),
        names.join(", "),
        numbers.join("] [")
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Location;
    use std::fs;

    fn workspace(tag: &str) -> PathBuf {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("scd-jump-{tag}-{}-{nanos}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn config_with(dir: &Path, names: &[&str]) -> Config {
        let mut cfg = Config::default();
        for name in names {
            let path = dir.join(name);
            fs::create_dir_all(&path).unwrap();
            let mut location = Location::new(paths::absolute(&path));
            location.name = (*name).to_string();
            cfg.locations.push(location);
        }
        cfg
    }

    #[test]
    fn resolves_existing_directories() {
        let dir = workspace("dirs");
        let cfg = Config::default();
        let nested = dir.join("a").join("b");
        fs::create_dir_all(&nested).unwrap();

        assert_eq!(resolve(&cfg, nested.to_str().unwrap()).unwrap(), Target::Path(paths::absolute(&nested)));
        assert_eq!(resolve(&cfg, ".").unwrap(), Target::Path(paths::current_dir_absolute()));
        assert!(matches!(resolve(&cfg, "   "), Err(Error::Empty)));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn reports_files_and_missing_directories() {
        let dir = workspace("files");
        let cfg = Config::default();
        let file = dir.join("notes.txt");
        fs::write(&file, "hello").unwrap();
        assert!(matches!(
            resolve(&cfg, file.to_str().unwrap()),
            Err(Error::IsAFile(_))
        ));
        assert!(matches!(
            resolve(&cfg, dir.join("ghost").to_str().unwrap()),
            Err(Error::Unknown(_))
        ));
        let _ = fs::remove_dir_all(&dir);
    }

    /// A config whose locations point at synthetic paths. Nothing on disk can shadow a name,
    /// so the resolution order stays testable (a real, existing directory always wins).
    fn config_named(names: &[&str]) -> Config {
        let mut cfg = Config::default();
        for name in names {
            let path = if cfg!(windows) {
                PathBuf::from(format!(r"D:\scd-tests\{name}"))
            } else {
                PathBuf::from(format!("/scd-tests/{name}"))
            };
            let mut location = Location::new(path);
            location.name = (*name).to_string();
            cfg.locations.push(location);
        }
        cfg
    }

    #[test]
    fn resolves_numbers_and_names() {
        // "zz-" prefix: relative queries are resolved against the test runner's cwd first,
        // and the crate root does contain folders such as docs/ and src/.
        let cfg = config_named(&["zz-projects", "zz-docs", "zz-工作"]);

        assert_eq!(resolve(&cfg, "2").unwrap(), Target::Saved(1));
        assert_eq!(resolve(&cfg, "zz-docs").unwrap(), Target::Saved(1));
        assert_eq!(resolve(&cfg, "ZZ-DOCS").unwrap(), Target::Saved(1));
        assert_eq!(resolve(&cfg, "zz-工作").unwrap(), Target::Saved(2));
        assert_eq!(resolve(&cfg, "zz-p").unwrap(), Target::Saved(0), "unique name prefix");
        assert_eq!(resolve(&cfg, "工").unwrap(), Target::Saved(2), "unicode prefix");
        assert!(matches!(resolve(&cfg, "zz-"), Err(Error::Ambiguous(_))), "shared prefix is ambiguous");
        assert!(matches!(
            resolve(&cfg, "9"),
            Err(Error::OutOfRange { index: 9, count: 3 })
        ));
        assert!(matches!(resolve(&cfg, "0"), Err(Error::OutOfRange { index: 0, count: 3 })));
        assert!(matches!(resolve(&cfg, "nope"), Err(Error::Unknown(_))));
    }

    #[test]
    fn out_of_range_number_can_still_be_a_name() {
        let cfg = config_named(&["2024"]);
        assert_eq!(resolve(&cfg, "2024").unwrap(), Target::Saved(0));
        assert!(matches!(resolve(&cfg, "7"), Err(Error::OutOfRange { index: 7, count: 1 })));
    }

    #[test]
    fn a_real_directory_beats_a_saved_name_of_the_same_spelling() {
        let dir = workspace("shadow");
        let real = dir.join("real").join("zz-shadow");
        fs::create_dir_all(&real).unwrap();

        // Saved location with the same *name* but a different (missing) path.
        let mut cfg = Config::default();
        let mut saved = Location::new(dir.join("other").join("zz-shadow"));
        saved.name = "zz-shadow".to_string();
        cfg.locations.push(saved);

        assert_eq!(
            resolve(&cfg, real.to_str().unwrap()).unwrap(),
            Target::Path(paths::absolute(&real)),
            "an existing path wins"
        );
        assert_eq!(resolve(&cfg, "zz-shadow").unwrap(), Target::Saved(0), "the name still works");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn ambiguity_lists_the_candidates() {
        let cfg = config_named(&["zz-proj-a", "zz-proj-b"]);
        let err = resolve(&cfg, "zz-proj").unwrap_err();
        let message = err.to_string();
        assert!(message.contains("zz-proj-a") && message.contains("zz-proj-b"), "{message}");
        assert!(message.contains("[1] [2]"), "{message}");
    }

    #[test]
    fn perform_counts_the_visit_and_reports_the_path() {
        let dir = workspace("perform");
        let config_path = dir.join("config.json");
        let mut cfg = config_with(&dir, &["alpha", "beta"]);
        let emitter = Emitter::new(Some(dir.join("target.txt")), false);
        let target = dir.join("beta");

        let code = perform(&mut cfg, &config_path, Target::Saved(1), &emitter.ui(), &emitter);
        assert_eq!(code, crate::exit::OK);
        assert_eq!(cfg.locations[0].name, "beta", "used location moves to the front");
        assert_eq!(cfg.locations[0].visits, 1);
        let saved = Config::load(&config_path).unwrap();
        assert_eq!(saved.locations[0].name, "beta");
        let written = fs::read_to_string(dir.join("target.txt")).unwrap();
        assert_eq!(written.trim_end(), paths::absolute(&target).display().to_string());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn menu_number_rejects_signs_and_junk() {
        assert_eq!(menu_number("12"), Some(12));
        assert_eq!(menu_number("0"), Some(0));
        assert_eq!(menu_number("+1"), None);
        assert_eq!(menu_number("-1"), None);
        assert_eq!(menu_number("1a"), None);
        assert_eq!(menu_number(""), None);
    }

    #[test]
    fn error_messages_are_actionable() {
        assert_eq!(Error::Empty.to_string(), "nothing to jump to");
        assert!(Error::OutOfRange { index: 7, count: 0 }.to_string().contains("press [a]"));
        assert!(Error::OutOfRange { index: 7, count: 3 }.to_string().contains("1..3"));
        assert!(Error::NotADirectory("D:\\x".into()).to_string().contains("D:\\x"));
    }
}