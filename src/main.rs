//! `scd` binary entry point. All logic lives in the `scd` library so that it can be
//! exercised by `tests/cli.rs` without spawning a process for every assertion.

use std::process::ExitCode;

fn main() -> ExitCode {
    // `args_os` + lossy conversion: an undecodable argument should not panic the program.
    let argv: Vec<String> = std::env::args_os()
        .map(|arg| arg.to_string_lossy().into_owned())
        .collect();
    let code = scd::run(argv);
    ExitCode::from(code.clamp(0, 255) as u8)
}