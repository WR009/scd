//! `scd` (super cd) - jump to your favourite directories from anywhere in the terminal.
//!
//! The crate is intentionally dependency free so that it builds offline from a clean
//! checkout; the little JSON it needs lives in [`json`].
//!
//! | module       | responsibility                                            |
//! |--------------|-----------------------------------------------------------|
//! | [`cli`]      | command line parsing and the `--help` user guide           |
//! | [`config`]   | `config.json` model, lookup order, atomic save             |
//! | [`json`]     | minimal JSON reader/writer used by [`config`]              |
//! | [`paths`]    | `~` / relative path expansion and normalisation            |
//! | [`menu`]     | the interactive `[c] [a] [1..n] [q]` menu                  |
//! | [`add`]      | the `a` ("add new location") flow                          |
//! | [`settings`] | the `c` ("configure scd") flow, one question at a time     |
//! | [`jump`]     | target resolution (path / number / name) and the jump      |
//! | [`emit`]     | how the chosen directory is handed back to the shell       |
//! | [`ui`]       | indented terminal output and line reading                  |
//! | [`time`]     | UTC timestamps stored with each location                   |

pub mod add;
pub mod cli;
pub mod color;
pub mod config;
pub mod emit;
pub mod json;
pub mod jump;
pub mod menu;
pub mod paths;
pub mod settings;
pub mod time;
pub mod ui;

use std::path::Path;

use cli::Command;
use config::Config;
use emit::Emitter;
use ui::Ui;

/// Process exit codes.
pub mod exit {
    /// Everything went fine, including "user quit the menu without jumping".
    pub const OK: i32 = 0;
    /// The requested target could not be resolved or is not a directory.
    pub const TARGET: i32 = 1;
    /// `config.json` could not be read, parsed or written.
    pub const CONFIG: i32 = 2;
    /// The command line itself was wrong (unknown flag, missing value, ...).
    pub const USAGE: i32 = 64;
}

/// Program entry point, shared by `main.rs` and the integration tests.
pub fn run(argv: Vec<String>) -> i32 {
    let parsed = match cli::parse(argv) {
        Ok(cli) => cli,
        Err(message) => {
            let diagnostics = Ui::stderr();
            let palette = diagnostics.palette();
            diagnostics.out(&format!("{} {}", palette.error_prefix(), palette.err(&message)));
            diagnostics.out(&format!(
                "{} {}",
                palette.dim("try"),
                palette.dim(&palette.bold("'scd --help'").to_string())
            ));
            return exit::USAGE;
        }
    };

    // --help / --version must stay usable even when config.json is broken.
    match parsed.command {
        Command::Help => {
            print!("{}", cli::help_text());
            return exit::OK;
        }
        Command::Version => {
            let out = Ui::stdout();
            let palette = out.palette();
            out.out(&format!(
                "{} {}",
                palette.name("scd"),
                palette.number(env!("CARGO_PKG_VERSION"))
            ));
            return exit::OK;
        }
        _ => {}
    }

    let emitter = Emitter::from_env();
    // Menu chrome follows the emitter's stream; command output (`list`, `config`, `add`,
    // `rm`) always goes to stdout so it stays pipeable.
    let ui = emitter.ui();
    let out = Ui::stdout();
    let config_path = config::resolve_path(parsed.config.as_deref());
    let mut cfg = match Config::load(&config_path) {
        Ok(cfg) => cfg,
        Err(err) => {
            let diagnostics = Ui::stderr();
            let palette = diagnostics.palette();
            diagnostics.out(&format!("{} {}", palette.error_prefix(), palette.err(&err)));
            return exit::CONFIG;
        }
    };

    match parsed.command {
        Command::Interactive => menu::run(&mut cfg, &config_path, &ui, &emitter),
        Command::Jump(query) => jump::query(&mut cfg, &config_path, &query, &emitter),
        Command::List => {
            menu::print_list(&cfg, &config_path, &out);
            exit::OK
        }
        Command::Add(target) => add::non_interactive(&mut cfg, &config_path, target.as_deref(), &out),
        Command::Remove(target) => remove_location(&mut cfg, &config_path, &target),
        Command::Move { target, position } => {
            move_location(&mut cfg, &config_path, &target, position)
        }
        Command::Shift { target, delta } => shift_location(&mut cfg, &config_path, &target, delta),
        Command::Rename { target, name } => {
            rename_location(&mut cfg, &config_path, &target, &name)
        }
        Command::ShowConfig => {
            let palette = out.palette();
            out.out(&palette.path(&config_path.display().to_string()));
            exit::OK
        }
        Command::Help | Command::Version => exit::OK,
    }
}

/// `scd rm <target>`: drop one saved location and persist the result.
fn remove_location(cfg: &mut Config, config_path: &Path, target: &str) -> i32 {
    let index = match jump::resolve(cfg, target) {
        Ok(jump::Target::Saved(index)) => Some(index),
        Ok(jump::Target::Path(path)) => cfg.index_of_path(&path),
        Err(err) => {
            fail(&err.to_string(), None);
            return exit::TARGET;
        }
    };
    let Some(index) = index else {
        fail(&format!("not a saved location: {target}"), Some("try 'scd list' to see what is saved"));
        return exit::TARGET;
    };

    let removed = cfg.remove(index);
    if let Err(err) = cfg.save(config_path) {
        fail(&err, None);
        return exit::CONFIG;
    }
    let out = Ui::stdout();
    let palette = out.palette();
    out.out(&format!(
        "{} {} {}",
        palette.ok("removed"),
        palette.key(&(index + 1).to_string()),
        menu::styled_summary(&removed, &palette)
    ));
    exit::OK
}

/// Resolve a target to an index in `Config::locations`, or explain why it is not saved.
fn saved_index(cfg: &Config, target: &str) -> Result<usize, String> {
    match jump::resolve(cfg, target) {
        Ok(jump::Target::Saved(index)) => Ok(index),
        Ok(jump::Target::Path(path)) => cfg
            .index_of_path(&path)
            .ok_or_else(|| format!("not a saved location: {}", path.display())),
        Err(err) => Err(err.to_string()),
    }
}

fn saved_index_or_fail(cfg: &Config, target: &str) -> Option<usize> {
    match saved_index(cfg, target) {
        Ok(index) => Some(index),
        Err(err) => {
            fail(&err, Some("try 'scd list' to see what is saved"));
            None
        }
    }
}

/// Persist an edit and show the resulting order, so the effect is immediately visible.
fn save_and_show_order(cfg: &Config, config_path: &Path, out: &Ui, headline: String) -> i32 {
    if let Err(err) = cfg.save(config_path) {
        fail(&err, None);
        return exit::CONFIG;
    }
    out.out(&headline);
    out.out_lines(&menu::order_lines(cfg, &out.palette()));
    exit::OK
}

fn moved_headline(cfg: &Config, from: usize, to: usize, palette: &color::Palette) -> String {
    format!(
        "{} {}: {} {} {}",
        palette.ok("moved"),
        palette.name(&cfg.locations[to].name),
        palette.key(&(from + 1).to_string()),
        palette.dim("->"),
        palette.key(&(to + 1).to_string())
    )
}

/// `scd mv <target> <position>`: the user puts a location exactly where they want it.
fn move_location(cfg: &mut Config, config_path: &Path, target: &str, position: usize) -> i32 {
    let out = Ui::stdout();
    let palette = out.palette();
    let Some(from) = saved_index_or_fail(cfg, target) else { return exit::TARGET };
    match cfg.move_to(from, position) {
        Ok(to) if to == from => {
            out.out(&format!(
                "{} {} {}",
                palette.dim("already at position"),
                palette.key(&(from + 1).to_string()),
                palette.name(&cfg.locations[from].name)
            ));
            exit::OK
        }
        Ok(to) => {
            let headline = moved_headline(cfg, from, to, &palette);
            save_and_show_order(cfg, config_path, &out, headline)
        }
        Err(err) => {
            fail(&err, None);
            exit::TARGET
        }
    }
}

/// `scd up <target>` / `scd down <target>`.
fn shift_location(cfg: &mut Config, config_path: &Path, target: &str, delta: i32) -> i32 {
    let out = Ui::stdout();
    let palette = out.palette();
    let Some(from) = saved_index_or_fail(cfg, target) else { return exit::TARGET };
    match cfg.shift(from, delta) {
        Ok(to) if to == from => {
            let edge = if delta < 0 { "already at the top:" } else { "already at the bottom:" };
            out.out(&format!(
                "{} {} {}",
                palette.dim(edge),
                palette.key(&(from + 1).to_string()),
                palette.name(&cfg.locations[from].name)
            ));
            exit::OK
        }
        Ok(to) => {
            let headline = moved_headline(cfg, from, to, &palette);
            save_and_show_order(cfg, config_path, &out, headline)
        }
        Err(err) => {
            fail(&err, None);
            exit::TARGET
        }
    }
}

/// `scd rename <target> <new name>`.
fn rename_location(cfg: &mut Config, config_path: &Path, target: &str, name: &str) -> i32 {
    let out = Ui::stdout();
    let palette = out.palette();
    let Some(index) = saved_index_or_fail(cfg, target) else { return exit::TARGET };
    let old = cfg.locations[index].name.clone();
    if let Err(err) = cfg.rename(index, name) {
        fail(&err, None);
        return exit::TARGET;
    }
    if let Err(err) = cfg.save(config_path) {
        fail(&err, None);
        return exit::CONFIG;
    }
    out.out(&format!(
        "{} {} {} {} {}",
        palette.ok("renamed"),
        palette.key(&(index + 1).to_string()),
        palette.dim(&old),
        palette.dim("->"),
        palette.name(&cfg.locations[index].name)
    ));
    out.out(&format!(
        "{} {}",
        palette.dim("path:"),
        palette.path(&cfg.locations[index].path.display().to_string())
    ));
    exit::OK
}

/// One red `scd: <message>` line on stderr, plus an optional dim hint.
fn fail(message: &str, hint: Option<&str>) {
    let diagnostics = Ui::stderr();
    let palette = diagnostics.palette();
    diagnostics.out(&format!("{} {}", palette.error_prefix(), palette.err(message)));
    if let Some(hint) = hint {
        diagnostics.out(&palette.dim(hint));
    }
}