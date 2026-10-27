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
            eprintln!("scd: {message}");
            eprintln!("try 'scd --help' for the user guide");
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
            println!("scd {}", env!("CARGO_PKG_VERSION"));
            return exit::OK;
        }
        _ => {}
    }

    let emitter = Emitter::from_env();
    let ui = emitter.ui();
    let config_path = config::resolve_path(parsed.config.as_deref());
    let mut cfg = match Config::load(&config_path) {
        Ok(cfg) => cfg,
        Err(err) => {
            eprintln!("scd: {err}");
            return exit::CONFIG;
        }
    };

    match parsed.command {
        Command::Interactive => menu::run(&mut cfg, &config_path, &ui, &emitter),
        Command::Jump(query) => jump::query(&mut cfg, &config_path, &query, &emitter),
        Command::List => {
            menu::print_list(&cfg, &config_path);
            exit::OK
        }
        Command::Add(target) => add::non_interactive(&mut cfg, &config_path, target.as_deref()),
        Command::Remove(target) => remove_location(&mut cfg, &config_path, &target),
        Command::ShowConfig => {
            println!("{}", config_path.display());
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
            eprintln!("scd: {err}");
            return exit::TARGET;
        }
    };
    let Some(index) = index else {
        eprintln!("scd: not a saved location: {target}");
        eprintln!("try 'scd list' to see what is saved");
        return exit::TARGET;
    };

    let removed = cfg.remove(index);
    if let Err(err) = cfg.save(config_path) {
        eprintln!("scd: {err}");
        return exit::CONFIG;
    }
    println!("removed [{}] {}", index + 1, removed.summary());
    exit::OK
}