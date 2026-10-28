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
//!
//! Every renderer takes a [`Palette`]: with colour off the output is byte-identical to the
//! plain layout above, with colour on the aliases (bold green) and paths (bold cyan) pop.

use std::path::Path;

use crate::add;
use crate::color::Palette;
use crate::config::{Config, Location};
use crate::emit::Emitter;
use crate::jump;
use crate::settings;
use crate::time;
use crate::ui::{Ui, pad_to_width};

/// Run the menu until the user quits or jumps. Returns the process exit code.
pub fn run(cfg: &mut Config, config_path: &Path, ui: &Ui, emitter: &Emitter) -> i32 {
    let palette = ui.palette();
    if cfg.is_empty() {
        // First run: tell the user where their locations will be stored.
        ui.line(&format!(
            "{} {}",
            palette.dim("config:"),
            palette.path(&config_path.display().to_string())
        ));
        ui.blank();
    }

    let mut first_prompt = true;
    loop {
        render(cfg, ui);
        let Some(input) = ui.ask(&format!("{} ", palette.prompt("scd>"))) else {
            ui.blank();
            if first_prompt {
                ui.line(&palette.warn("no input on stdin - run scd in a terminal, or jump directly with:"));
                ui.line(&format!(
                    "{} {}   |   {} {}   |   {} {}   {}",
                    palette.name("scd"),
                    palette.dim("<path>"),
                    palette.name("scd"),
                    palette.dim("<number>"),
                    palette.name("scd"),
                    palette.dim("<name>"),
                    palette.dim("(see 'scd --help')")
                ));
            } else {
                ui.line(&palette.dim("bye"));
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
                ui.line(&palette.dim("bye"));
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
                ui.line(&format!("{} {}", palette.error_prefix(), palette.err(&err.to_string())));
                ui.blank();
            }
        }
    }
}

/// Print the menu.
pub fn render(cfg: &Config, ui: &Ui) {
    ui.lines(&menu_lines(cfg, &ui.palette()));
}

/// The menu as lines; kept separate from printing so both colour modes can be tested.
pub fn menu_lines(cfg: &Config, palette: &Palette) -> Vec<String> {
    let mut lines = vec![
        format!("{} configure scd", palette.key("c")),
        format!("{} add new location", palette.key("a")),
        String::new(),
    ];

    if cfg.locations.is_empty() {
        lines.push(palette.dim("no location saved yet"));
        lines.push(format!(
            "press {} to add one, or type a path to jump straight there",
            palette.key("a")
        ));
        lines.push(String::new());
        lines.push(quit_line(palette));
        lines.push(String::new());
        return lines;
    }

    lines.extend(rows(cfg.shown(), palette));

    let hidden = cfg.hidden_count();
    if hidden > 0 {
        lines.push(String::new());
        lines.push(format!(
            "{} saved, {} hidden - press {} to raise \"max locations to display\"",
            palette.number(&cfg.locations.len().to_string()),
            palette.number(&hidden.to_string()),
            palette.key("c")
        ));
    }
    lines.push(String::new());
    lines.push(quit_line(palette));
    lines.push(palette.dim(
        "tip: scd rename <n> <name> - scd mv <n> <pos> - scd up/down <n> - the order is yours",
    ));
    lines.push(String::new());
    lines
}

/// `[n] name  path` rows for the given locations, numbered from 1.
pub fn rows(locations: &[Location], palette: &Palette) -> Vec<String> {
    let width = name_width(locations);
    locations
        .iter()
        .enumerate()
        .map(|(index, location)| {
            format!(
                "{} {}",
                palette.key(&(index + 1).to_string()),
                entry(location, width, palette)
            )
        })
        .collect()
}

/// The whole list in menu order, flush left - printed after `mv` / `up` / `down` / `rename`
/// so the new arrangement is immediately visible.
pub fn order_lines(cfg: &Config, palette: &Palette) -> Vec<String> {
    rows(&cfg.locations, palette)
}

fn quit_line(palette: &Palette) -> String {
    format!("{} {}", palette.key("q"), palette.dim("quit"))
}

/// One menu row: `name  path`, names padded to a common *terminal* width so CJK and ASCII
/// names line up in the same column. Colour escapes are not counted as width.
pub fn entry(location: &Location, width: usize, palette: &Palette) -> String {
    let path = palette.path(&location.path.display().to_string());
    if location.name.is_empty() {
        return path;
    }
    format!("{}  {}", pad_to_width(&palette.name(&location.name), width), path)
}

/// `name  path` for one-line messages outside the aligned menu.
pub fn styled_summary(location: &Location, palette: &Palette) -> String {
    if location.name.is_empty() {
        return palette.path(&location.path.display().to_string());
    }
    format!(
        "{}  {}",
        palette.name(&location.name),
        palette.path(&location.path.display().to_string())
    )
}

pub fn name_width(locations: &[Location]) -> usize {
    locations
        .iter()
        .map(|location| crate::ui::display_width(&location.name))
        .max()
        .unwrap_or(0)
}

/// `scd list`: every saved location, including the ones the menu hides.
pub fn print_list(cfg: &Config, config_path: &Path, ui: &Ui) {
    ui.out_lines(&list_lines(cfg, config_path, &ui.palette()));
}

pub fn list_lines(cfg: &Config, config_path: &Path, palette: &Palette) -> Vec<String> {
    let mut lines = vec![
        format!("{} {}", palette.dim("# config:"), palette.path(&config_path.display().to_string())),
        format!(
            "{} {}, max locations to display: {}",
            palette.dim("#"),
            palette.number(&format!("{} location(s) saved", cfg.locations.len())),
            palette.number(&cfg.max_display.to_string())
        ),
    ];
    if cfg.locations.is_empty() {
        lines.push(palette.dim("# nothing saved yet - run 'scd' and press [a]"));
        return lines;
    }

    let width = name_width(&cfg.locations);
    for (index, location) in cfg.locations.iter().enumerate() {
        // Pad the plain word first: escapes would otherwise count towards the field width.
        let visibility = if index < cfg.max_display {
            palette.ok(&format!("{:<6}", "menu"))
        } else {
            palette.dim(&format!("{:<6}", "hidden"))
        };
        let last_used = if location.last_used_at == 0 {
            palette.dim("never")
        } else {
            palette.dim(&time::format_utc(location.last_used_at))
        };
        lines.push(format!(
            "{} {}  {}  {:<6}  visits={}  last_used={}",
            palette.key(&format!("{:>2}", index + 1)),
            pad_to_width(&palette.name(&location.name), width),
            palette.path(&location.path.display().to_string()),
            visibility,
            palette.number(&format!("{:<3}", location.visits)),
            last_used
        ));
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::color::strip_ansi;
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

    fn sample() -> Config {
        config(
            9,
            vec![
                location("scd", r"D:\data\Projects\scd"),
                location("docs", r"D:\docs"),
                location("工作目录", r"D:\工作目录"),
            ],
        )
    }

    #[test]
    fn empty_menu_matches_the_documented_shape() {
        let lines = menu_lines(&config(9, vec![]), &Palette::off());
        assert_eq!(lines[0], "[c] configure scd");
        assert_eq!(lines[1], "[a] add new location");
        assert_eq!(lines[2], "");
        assert!(lines[3].contains("no location saved yet"), "{lines:?}");
        assert!(lines.iter().any(|line| line == "[q] quit"));
        assert!(!lines.iter().any(|line| line.starts_with('[') && line[1..2].parse::<usize>().is_ok()));
    }

    #[test]
    fn menu_numbers_and_aligns_locations() {
        let cfg = sample();
        let width = name_width(cfg.shown());
        assert_eq!(width, 8, "工作目录 is the widest name (4 chars x 2 columns)");

        let lines = menu_lines(&cfg, &Palette::off());
        assert!(
            lines.contains(&format!("[1] {}  {}", pad_to_width("scd", width), r"D:\data\Projects\scd")),
            "{lines:?}"
        );
        assert!(
            lines.contains(&format!("[2] {}  {}", pad_to_width("docs", width), r"D:\docs")),
            "{lines:?}"
        );
        assert!(lines.iter().any(|line| line.starts_with("[3] ") && line.contains(r"D:\工作目录")));
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
        let lines = menu_lines(&cfg, &Palette::off());
        assert!(lines.iter().any(|line| line.starts_with("[1]")));
        assert!(lines.iter().any(|line| line.starts_with("[2]")));
        assert!(!lines.iter().any(|line| line.starts_with("[3]")));
        assert!(lines.iter().any(|line| line.contains("4 saved, 2 hidden")), "{lines:?}");
    }

    #[test]
    fn entry_falls_back_to_the_bare_path() {
        let off = Palette::off();
        let bare = location("", r"D:\only-path");
        assert_eq!(entry(&bare, 10, &off), r"D:\only-path");
        let named = location("proj", r"D:\proj");
        assert_eq!(entry(&named, 8, &off), "proj      D:\\proj");
        assert_eq!(name_width(&[named, bare]), 4);
    }

    #[test]
    fn cjk_and_ascii_names_line_up() {
        let cfg = config(
            9,
            vec![location("工作目录", r"D:\工作目录"), location("docs", r"D:\docs")],
        );
        let lines = menu_lines(&cfg, &Palette::off());
        let cjk = lines.iter().find(|line| line.starts_with("[1]")).unwrap();
        let ascii = lines.iter().find(|line| line.starts_with("[2]")).unwrap();
        assert_eq!(cjk, "[1] 工作目录  D:\\工作目录");
        assert_eq!(ascii, "[2] docs      D:\\docs");
        assert_eq!(name_width(cfg.shown()), 8);
    }

    #[test]
    fn colour_highlights_names_and_paths_and_changes_nothing_else() {
        let on = Palette::on();
        let cfg = sample();
        let colored = menu_lines(&cfg, &on);
        let plain = menu_lines(&cfg, &Palette::off());

        assert_eq!(colored.len(), plain.len());
        for (styled, flat) in colored.iter().zip(plain.iter()) {
            assert_eq!(strip_ansi(styled), *flat, "colour must not change the text");
        }
        // The two things that must pop: alias in bold green, path in bold cyan.
        let row = &colored[3];
        assert!(row.contains("\x1b[1;32mscd\x1b[0m"), "{row:?}");
        assert!(row.contains("\x1b[1;36mD:\\data\\Projects\\scd\x1b[0m"), "{row:?}");
        // Keys are yellow with dim brackets.
        assert!(colored[0].contains("\x1b[1;33mc\x1b[0m"), "{:?}", colored[0]);
        assert!(row.contains("\x1b[1;33m1\x1b[0m"), "{row:?}");
        // Alignment survives the escapes: the path column starts at the same place.
        let path_column = |line: &str| {
            let plain = strip_ansi(line);
            plain.find("D:").unwrap()
        };
        assert_eq!(path_column(&colored[3]), path_column(&colored[4]));
    }

    #[test]
    fn coloured_rows_still_align_in_terminal_columns() {
        let on = Palette::on();
        let cfg = config(
            9,
            vec![location("工作目录", r"D:\工作目录"), location("docs", r"D:\docs")],
        );
        let lines = menu_lines(&cfg, &on);
        let first = lines.iter().find(|line| strip_ansi(line).starts_with("[1]")).unwrap();
        let second = lines.iter().find(|line| strip_ansi(line).starts_with("[2]")).unwrap();
        let column = |line: &str| {
            let plain = strip_ansi(line);
            crate::ui::display_width(&plain[..plain.find("D:").unwrap()])
        };
        assert_eq!(column(first), column(second), "{first:?} / {second:?}");
    }

    #[test]
    fn list_includes_hidden_entries_and_metadata() {
        let mut cfg = config(1, vec![location("a", r"D:\a"), location("b", r"D:\b")]);
        cfg.locations[0].visits = 4;
        cfg.locations[0].last_used_at = 1_000_000_000;
        let path = Path::new(if cfg!(windows) { r"C:\scd\config.json" } else { "/opt/scd/config.json" });
        let lines = list_lines(&cfg, path, &Palette::off());
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

        let colored = list_lines(&cfg, path, &Palette::on());
        assert_eq!(colored.len(), lines.len());
        for (styled, flat) in colored.iter().zip(lines.iter()) {
            assert_eq!(strip_ansi(styled), *flat, "colour must not change the text");
        }
        assert!(colored[2].contains("\x1b[1;32ma\x1b[0m"), "{:?}", colored[2]);
    }

    #[test]
    fn list_reports_an_empty_config() {
        let lines = list_lines(&config(9, vec![]), Path::new("config.json"), &Palette::off());
        assert!(lines.iter().any(|line| line.contains("nothing saved yet")), "{lines:?}");
    }
}