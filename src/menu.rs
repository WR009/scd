//! The interactive menu specified in `docs/ARCHITECTURE.md`:
//!
//! ```text
//!     [c] configure scd
//!     [a] add new location
//!
//!     [1] <location 1>
//!     [2] <location 2>
//! ```
//!
//! `[q]` leaves without jumping, and anything else is treated as a jump target, so typing a
//! full path at the prompt works exactly like `scd <path>` on the command line.

use std::path::Path;

use crate::add;
use crate::config::{Config, Location};
use crate::emit::Emitter;
use crate::jump;
use crate::settings;
use crate::time;
use crate::ui::Ui;

/// Run the menu until the user quits or jumps. Returns the process exit code.
pub fn run(cfg: &mut Config, config_path: &Path, ui: &Ui, emitter: &Emitter) -> i32 {
    if cfg.is_empty() {
        // First run: tell the user where their locations will be stored.
        ui.line(&format!("config: {}", config_path.display()));
        ui.blank();
    }

    let mut first_prompt = true;
    loop {
        render(cfg, ui);
        let Some(input) = ui.ask("scd> ") else {
            ui.blank();
            if first_prompt {
                ui.line("no input on stdin - run scd in a terminal, or jump directly with:");
                ui.line("scd <path>   |   scd <number>   |   scd <name>   (see 'scd --help')");
            } else {
                ui.line("bye");
            }
            return crate::exit::OK;
        };
        first_prompt = false;

        let choice = input.trim();
        if choice.is_empty() {
            continue;
        }
        match choice.to_ascii_lowercase().as_str() {
            "q" | "quit" | "exit" | "bye" => {
                ui.line("bye");
                return crate::exit::OK;
            }
            "c" | "config" | "configure" => {
                settings::run(cfg, config_path, ui);
                continue;
            }
            "a" | "add" => {
                add::run(cfg, config_path, ui);
                continue;
            }
            _ => {}
        }

        match jump::resolve(cfg, choice) {
            Ok(target) => return jump::perform(cfg, config_path, target, ui, emitter),
            Err(err) => {
                ui.line(&format!("scd: {err}"));
                ui.blank();
            }
        }
    }
}

/// Print the menu.
pub fn render(cfg: &Config, ui: &Ui) {
    ui.lines(&menu_lines(cfg));
}

/// The menu as plain lines; kept separate from printing so it can be tested.
pub fn menu_lines(cfg: &Config) -> Vec<String> {
    let mut lines = vec![
        "[c] configure scd".to_string(),
        "[a] add new location".to_string(),
        String::new(),
    ];

    if cfg.locations.is_empty() {
        lines.push("no location saved yet".to_string());
        lines.push("press [a] to add one, or type a path to jump straight there".to_string());
        lines.push(String::new());
        lines.push("[q] quit".to_string());
        lines.push(String::new());
        return lines;
    }

    let shown = cfg.shown();
    let width = name_width(shown);
    for (index, location) in shown.iter().enumerate() {
        lines.push(format!("[{}] {}", index + 1, entry(location, width)));
    }

    let hidden = cfg.hidden_count();
    if hidden > 0 {
        lines.push(String::new());
        lines.push(format!(
            "{} saved, {} hidden - press [c] to raise \"max locations to display\"",
            cfg.locations.len(),
            hidden
        ));
    }
    lines.push(String::new());
    lines.push("[q] quit".to_string());
    lines.push(String::new());
    lines
}

/// One menu row: `name  path`, names padded to a common *terminal* width so CJK and ASCII
/// names line up in the same column.
pub fn entry(location: &Location, width: usize) -> String {
    if location.name.is_empty() {
        return location.path.display().to_string();
    }
    format!("{}  {}", crate::ui::pad_to_width(&location.name, width), location.path.display())
}

pub fn name_width(locations: &[Location]) -> usize {
    locations
        .iter()
        .map(|location| crate::ui::display_width(&location.name))
        .max()
        .unwrap_or(0)
}

/// `scd list`: every saved location, including the ones the menu hides.
pub fn print_list(cfg: &Config, config_path: &Path) {
    for line in list_lines(cfg, config_path) {
        crate::ui::print_line(&line);
    }
}

pub fn list_lines(cfg: &Config, config_path: &Path) -> Vec<String> {
    let mut lines = vec![
        format!("# config: {}", config_path.display()),
        format!(
            "# {} location(s) saved, max locations to display: {}",
            cfg.locations.len(),
            cfg.max_display
        ),
    ];
    if cfg.locations.is_empty() {
        lines.push("# nothing saved yet - run 'scd' and press [a]".to_string());
        return lines;
    }

    let width = name_width(&cfg.locations);
    for (index, location) in cfg.locations.iter().enumerate() {
        let visibility = if index < cfg.max_display { "menu  " } else { "hidden" };
        let last_used = if location.last_used_at == 0 {
            "never".to_string()
        } else {
            time::format_utc(location.last_used_at)
        };
        lines.push(format!(
            "[{:>2}] {}  {}  visits={:<3} last_used={}",
            index + 1,
            entry(location, width),
            visibility,
            location.visits,
            last_used
        ));
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn location(name: &str, path: &str) -> Location {
        Location {
            name: name.to_string(),
            path: PathBuf::from(path),
            visits: 0,
            added_at: 0,
            last_used_at: 0,
        }
    }

    fn config(max_display: usize, locations: Vec<Location>) -> Config {
        Config { max_display, locations }
    }

    #[test]
    fn empty_menu_matches_the_documented_shape() {
        let lines = menu_lines(&config(9, vec![]));
        assert_eq!(lines[0], "[c] configure scd");
        assert_eq!(lines[1], "[a] add new location");
        assert_eq!(lines[2], "");
        assert!(lines[3].contains("no location saved yet"), "{lines:?}");
        assert!(lines.iter().any(|line| line == "[q] quit"));
        assert!(!lines.iter().any(|line| line.starts_with('[') && line[1..2].parse::<usize>().is_ok()));
    }

    #[test]
    fn menu_numbers_and_aligns_locations() {
        let cfg = config(
            9,
            vec![
                location("scd", r"D:\data\Projects\scd"),
                location("docs", r"D:\docs"),
                location("工作", r"D:\工作"),
            ],
        );
        let lines = menu_lines(&cfg);
        assert!(lines.contains(&format!("[1] {}  {}", "scd ", r"D:\data\Projects\scd")), "{lines:?}");
        assert!(lines.contains(&"[2] docs  D:\\docs".to_string()), "{lines:?}");
        // CJK names are two columns wide in a terminal but one char for padding purposes;
        // the important part is that the path is still on the same line.
        assert!(lines.iter().any(|line| line.starts_with("[3] ") && line.contains(r"D:\工作")));
    }

    #[test]
    fn menu_hides_entries_beyond_max_display_and_says_so() {
        let cfg = config(
            2,
            vec![
                location("a", r"D:\a"),
                location("b", r"D:\b"),
                location("c", r"D:\c"),
                location("d", r"D:\d"),
            ],
        );
        let lines = menu_lines(&cfg);
        assert!(lines.iter().any(|line| line.starts_with("[1]")));
        assert!(lines.iter().any(|line| line.starts_with("[2]")));
        assert!(!lines.iter().any(|line| line.starts_with("[3]")));
        assert!(
            lines.iter().any(|line| line.contains("4 saved, 2 hidden")),
            "{lines:?}"
        );
    }

    #[test]
    fn entry_falls_back_to_the_bare_path() {
        let bare = location("", r"D:\only-path");
        assert_eq!(entry(&bare, 10), r"D:\only-path");
        let named = location("proj", r"D:\proj");
        assert_eq!(entry(&named, 8), "proj      D:\\proj");
        assert_eq!(name_width(&[named, bare]), 4);
    }

    #[test]
    fn cjk_and_ascii_names_line_up() {
        let cfg = config(
            9,
            vec![location("工作目录", r"D:\工作目录"), location("docs", r"D:\docs")],
        );
        let lines = menu_lines(&cfg);
        let cjk = lines.iter().find(|line| line.starts_with("[1]")).unwrap();
        let ascii = lines.iter().find(|line| line.starts_with("[2]")).unwrap();
        // Both names occupy 8 columns, so both paths start at the same column.
        assert_eq!(cjk, "[1] 工作目录  D:\\工作目录");
        assert_eq!(ascii, "[2] docs      D:\\docs");
        assert_eq!(name_width(cfg.shown()), 8);
    }

    #[test]
    fn list_includes_hidden_entries_and_metadata() {
        let mut cfg = config(1, vec![location("a", r"D:\a"), location("b", r"D:\b")]);
        cfg.locations[0].visits = 4;
        cfg.locations[0].last_used_at = 1_000_000_000;
        let lines = list_lines(&cfg, Path::new(if cfg!(windows) { r"C:\scd\config.json" } else { "/opt/scd/config.json" }));
        assert!(lines[0].starts_with("# config: "), "{lines:?}");
        assert!(lines[1].contains("2 location(s) saved"), "{lines:?}");
        assert!(lines[1].contains("max locations to display: 1"), "{lines:?}");
        assert!(
            lines.iter().any(|line| line.contains("[ 1]")
                && line.contains("menu")
                && line.contains("visits=4")
                && line.contains("last_used=2001-09-09T01:46:40Z")),
            "{lines:?}"
        );
        assert!(
            lines.iter().any(|line| line.contains("[ 2]")
                && line.contains("hidden")
                && line.contains("visits=0")
                && line.contains("last_used=never")),
            "{lines:?}"
        );
    }

    #[test]
    fn list_reports_an_empty_config() {
        let lines = list_lines(&config(9, vec![]), Path::new("config.json"));
        assert!(lines.iter().any(|line| line.contains("nothing saved yet")), "{lines:?}");
    }
}