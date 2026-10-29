//! Handing the chosen directory back to the shell.
//!
//! A child process cannot change the working directory of its parent shell, so `scd`
//! resolves the target and reports it while a three-line shell function performs the `cd`
//! (see `shell/scd.ps1`, `shell/scd.bash`, `shell/scd.zsh`). Two contracts are supported:
//!
//! * `SCD_TARGET_FILE` is set - what the shipped wrappers do. The directory is written to
//!   that file (UTF-8, no decoration) and the user interface goes to stdout, so the program
//!   behaves like any normal console tool and non-ASCII paths survive the trip.
//! * otherwise - the directory is printed as the only stdout line, which makes
//!   `cd "$(scd ...)"` work, and the user interface is redirected to stderr to keep stdout
//!   machine readable. A one-off hint explains the missing `cd`.

use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use crate::ui::{Stream, Ui};

// Colour roles for the jump report live in crate::color; the machine-readable channels
// (SCD_TARGET_FILE payload and the single stdout line) are deliberately never styled.

pub struct Emitter {
    target_file: Option<PathBuf>,
    stream: Stream,
    hint: bool,
}

impl Emitter {
    /// Build an emitter from `SCD_TARGET_FILE`, `SCD_CD` and `SCD_NO_HINT`.
    pub fn from_env() -> Self {
        let target_file = env::var_os("SCD_TARGET_FILE")
            .map(PathBuf::from)
            .filter(|path| !path.as_os_str().is_empty());
        let wrapped = target_file.is_some() || flag_enabled("SCD_CD");
        Self::new(target_file, !wrapped && !flag_enabled("SCD_NO_HINT"))
    }

    /// Explicit constructor; `hint` says whether the shell-integration note should print.
    pub fn new(target_file: Option<PathBuf>, hint: bool) -> Self {
        let stream = if target_file.is_some() { Stream::Stdout } else { Stream::Stderr };
        Emitter { target_file, stream, hint }
    }

    /// Stream used for menus and prompts.
    pub fn ui(&self) -> Ui {
        Ui::new(self.stream)
    }

    /// Stream used for diagnostics; errors never mix with machine-readable output.
    pub fn diagnostics(&self) -> Ui {
        Ui::stderr()
    }

    pub fn uses_target_file(&self) -> bool {
        self.target_file.is_some()
    }

    /// Report a successful jump and return the process exit code.
    pub fn emit_jump(&self, target: &Path, ui: &Ui) -> i32 {
        let palette = ui.palette();
        let text = target.display().to_string();
        ui.line(&format!("{} {}", palette.ok("->"), palette.path(&text)));

        if let Some(file) = &self.target_file {
            let payload = format!("{text}\n");
            if let Err(err) = fs::write(file, payload.as_bytes()) {
                let errors = Ui::stderr();
                let error_palette = errors.palette();
                errors.out(&format!(
                    "{} cannot write {}: {}",
                    error_palette.error_prefix(),
                    error_palette.path(&file.display().to_string()),
                    error_palette.err(&crate::ui::flatten(&err.to_string()))
                ));
                return crate::exit::CONFIG;
            }
        } else {
            println!("{text}");
        }

        if self.hint {
            ui.line(&palette.warn("scd cannot change this shell's directory by itself."));
            ui.line(&format!(
                "{}  {} {}",
                palette.dim("for now:"),
                palette.bold("cd"),
                palette.path(&format!("\"{text}\""))
            ));
            ui.line(&format!(
                "{}     install the shell function from {} (see {}).",
                palette.dim("once:"),
                palette.path("shell/"),
                palette.bold("'scd --help'")
            ));
        }
        crate::exit::OK
    }
}

/// Truthy environment flag: unset or empty/0/false/no/off means disabled.
fn flag_enabled(key: &str) -> bool {
    match env::var_os(key) {
        Some(value) => {
            let text = value.to_string_lossy().trim().to_ascii_lowercase();
            !matches!(text.as_str(), "" | "0" | "false" | "no" | "off")
        }
        None => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn workspace(tag: &str) -> PathBuf {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("scd-emit-{tag}-{}-{nanos}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        // CI runners hand out temp dirs that are not in canonical form (8.3 short names
        // on the Windows runners, /var -> /private/var on macOS). scd reports canonical
        // paths, so the sandbox root has to be canonical too or the two spellings of the
        // same directory never compare equal.
        crate::paths::absolute(&dir)
    }

    #[test]
    fn target_file_moves_the_ui_to_stdout() {
        let without = Emitter::new(None, false);
        assert_eq!(without.ui().stream(), Stream::Stderr);
        assert!(!without.uses_target_file());

        let with = Emitter::new(Some(PathBuf::from("out.txt")), true);
        assert_eq!(with.ui().stream(), Stream::Stdout);
        assert!(with.uses_target_file());
        assert_eq!(with.diagnostics().stream(), Stream::Stderr);
    }

    #[test]
    fn emit_jump_writes_the_path_to_the_target_file() {
        let dir = workspace("emit");
        let file = dir.join("target.txt");
        let emitter = Emitter::new(Some(file.clone()), true);
        let code = emitter.emit_jump(&dir, &emitter.ui());
        assert_eq!(code, crate::exit::OK);
        let written = fs::read_to_string(&file).unwrap();
        assert_eq!(written.trim_end(), dir.to_string_lossy().trim_end_matches(['\\', '/']));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn emit_jump_reports_an_unwritable_target_file() {
        let emitter = Emitter::new(Some(PathBuf::from(if cfg!(windows) {
            r"Z:\definitely\not\here\scd.txt"
        } else {
            "/proc/definitely/not/here/scd.txt"
        })), false);
        let code = emitter.emit_jump(Path::new(if cfg!(windows) { r"C:\" } else { "/" }), &Ui::stderr());
        assert_eq!(code, crate::exit::CONFIG);
    }
}