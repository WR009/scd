//! Terminal colour, implemented without any dependency.
//!
//! Design rules:
//! * colour is decided **per stream**: a redirected stdout (e.g. `cd "$(scd proj)"`) must stay
//!   byte-clean, while stderr can still be pretty;
//! * `SCD_COLOR=always|never|auto` wins, then the usual conventions `NO_COLOR`,
//!   `CLICOLOR=0`, `CLICOLOR_FORCE`, `TERM=dumb`;
//! * on Windows the console is switched to VT processing (`ENABLE_VIRTUAL_TERMINAL_PROCESSING`)
//!   before the first escape is written, otherwise cmd.exe would print raw `[1;36m`;
//! * the machine-readable jump target on stdout is never coloured, whatever the settings say.
//!
//! Palette roles (aliases and paths are the two things that must pop):
//!
//! | role | SGR | used for |
//! |------|-----|----------|
//! | name | `1;32` bold green | saved alias / folder name |
//! | path | `1;36` bold cyan  | every directory or file path |
//! | key  | dim `[` + `1;33` + dim `]` | keys you can press: `[c] [a] [q] [1]` |
//! | prompt | `1;35` bold magenta | `scd>`, `new location:` |
//! | ok / warn / err | `1;32` / `33` / `1;31` | added-saved-removed / hints / errors |
//! | dim | `2` | secondary text: footers, `visits=`, `#` comments |

use std::env;

use crate::ui::Stream;

pub const ESC: char = '\x1b';
const RESET: &str = "\x1b[0m";

/// Whether colour should be emitted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Palette {
    pub enabled: bool,
}

/// User preference from `SCD_COLOR`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pref {
    Auto,
    Always,
    Never,
}

impl Palette {
    pub const fn off() -> Self {
        Palette { enabled: false }
    }

    pub const fn on() -> Self {
        Palette { enabled: true }
    }

    /// Detect the right setting for one output stream (and enable VT on Windows if needed).
    pub fn for_stream(stream: Stream) -> Self {
        let pref = pref_from(env::var("SCD_COLOR").ok().as_deref());
        let enabled = decide(
            pref,
            flag_set("NO_COLOR"),
            env::var("CLICOLOR").map(|v| v.trim() == "0").unwrap_or(false),
            flag_set("CLICOLOR_FORCE"),
            env::var("TERM").map(|v| v.trim() == "dumb").unwrap_or(false),
            is_tty(stream),
        );
        if enabled {
            enable_vt(stream);
        }
        Palette { enabled }
    }

    fn paint(&self, text: &str, sgr: &str) -> String {
        if !self.enabled || text.is_empty() {
            text.to_string()
        } else {
            format!("\x1b[{sgr}m{text}{RESET}")
        }
    }

    /// Saved alias / folder name - one of the two things that must stand out.
    pub fn name(&self, text: &str) -> String {
        self.paint(text, "1;32")
    }

    /// A directory or file path - the other thing that must stand out.
    pub fn path(&self, text: &str) -> String {
        self.paint(text, "1;36")
    }

    /// A key the user can press, rendered as `[x]` with dim brackets.
    pub fn key(&self, inner: &str) -> String {
        if !self.enabled {
            return format!("[{inner}]");
        }
        format!("{}{}{}", self.paint("[", "2"), self.paint(inner, "1;33"), self.paint("]", "2"))
    }

    pub fn prompt(&self, text: &str) -> String {
        self.paint(text, "1;35")
    }

    /// Section titles: bold + underline, which stays readable on light *and* dark themes
    /// (a fixed bright colour would vanish on one of them).
    pub fn heading(&self, text: &str) -> String {
        self.paint(text, "1;4")
    }

    pub fn ok(&self, text: &str) -> String {
        self.paint(text, "1;32")
    }

    pub fn warn(&self, text: &str) -> String {
        self.paint(text, "33")
    }

    pub fn err(&self, text: &str) -> String {
        self.paint(text, "1;31")
    }

    pub fn dim(&self, text: &str) -> String {
        self.paint(text, "2")
    }

    pub fn bold(&self, text: &str) -> String {
        self.paint(text, "1")
    }

    /// Numbers worth noticing: counts, `visits=3`, indices in prose.
    pub fn number(&self, text: &str) -> String {
        self.paint(text, "1;33")
    }

    /// `scd:` / `error:` style prefix in front of a message.
    pub fn error_prefix(&self) -> String {
        self.err("scd:")
    }
}

/// Parse `SCD_COLOR` (and friends) into a preference.
pub fn pref_from(value: Option<&str>) -> Pref {
    match value.map(|text| text.trim().to_ascii_lowercase()) {
        None => Pref::Auto,
        Some(ref v) if matches!(v.as_str(), "always" | "1" | "yes" | "true" | "force" | "on") => {
            Pref::Always
        }
        Some(ref v) if matches!(v.as_str(), "never" | "0" | "no" | "false" | "off" | "none") => {
            Pref::Never
        }
        Some(_) => Pref::Auto,
    }
}

/// Pure decision function so the whole policy can be unit tested without touching the env.
pub fn decide(
    pref: Pref,
    no_color: bool,
    clicolor_zero: bool,
    clicolor_force: bool,
    term_dumb: bool,
    is_tty: bool,
) -> bool {
    match pref {
        // An explicit `SCD_COLOR=always` beats everything: it is how you demo or test colour.
        Pref::Always => true,
        Pref::Never => false,
        Pref::Auto => {
            if no_color || clicolor_zero || term_dumb {
                return false;
            }
            is_tty || clicolor_force
        }
    }
}

fn flag_set(key: &str) -> bool {
    match env::var_os(key) {
        Some(value) => !value.is_empty(),
        None => false,
    }
}

/// Remove every CSI escape sequence; used by tests to prove colour never changes the text.
pub fn strip_ansi(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch != ESC {
            out.push(ch);
            continue;
        }
        if chars.peek() == Some(&'[') {
            chars.next();
            for next in chars.by_ref() {
                // Final byte of a CSI sequence is in 0x40..=0x7E.
                if ('@'..='~').contains(&next) {
                    break;
                }
            }
        }
    }
    out
}

// --------------------------------------------------------------------------- tty detection

#[cfg(windows)]
pub fn is_tty(stream: Stream) -> bool {
    console_mode(handle_of(stream)).is_some()
}

#[cfg(not(windows))]
pub fn is_tty(stream: Stream) -> bool {
    let fd = match stream {
        Stream::Stdout => 1,
        Stream::Stderr => 2,
    };
    unsafe { isatty(fd) == 1 }
}

#[cfg(not(windows))]
unsafe extern "C" {
    fn isatty(fd: i32) -> i32;
}

#[cfg(windows)]
const STD_OUTPUT_HANDLE: u32 = 4_294_967_285; // (u32)(-11)
#[cfg(windows)]
const STD_ERROR_HANDLE: u32 = 4_294_967_284; // (u32)(-12)
#[cfg(windows)]
const ENABLE_VIRTUAL_TERMINAL_PROCESSING: u32 = 0x0004;
#[cfg(windows)]
const INVALID_HANDLE: isize = -1;

#[cfg(windows)]
unsafe extern "system" {
    #[link_name = "GetStdHandle"]
    fn get_std_handle(which: u32) -> isize;
    #[link_name = "GetConsoleMode"]
    fn get_console_mode(handle: isize, mode: *mut u32) -> i32;
    #[link_name = "SetConsoleMode"]
    fn set_console_mode(handle: isize, mode: u32) -> i32;
}

#[cfg(windows)]
fn handle_of(stream: Stream) -> u32 {
    match stream {
        Stream::Stdout => STD_OUTPUT_HANDLE,
        Stream::Stderr => STD_ERROR_HANDLE,
    }
}

#[cfg(windows)]
fn console_mode(which: u32) -> Option<u32> {
    unsafe {
        let handle = get_std_handle(which);
        if handle == 0 || handle == INVALID_HANDLE {
            return None;
        }
        let mut mode = 0u32;
        if get_console_mode(handle, &mut mode) == 0 {
            return None; // not a console: pipe, file, or null device
        }
        Some(mode)
    }
}

/// Legacy conhost needs VT processing switched on; Windows Terminal already has it.
#[cfg(windows)]
fn enable_vt(stream: Stream) {
    let which = handle_of(stream);
    let Some(mode) = console_mode(which) else { return };
    if mode & ENABLE_VIRTUAL_TERMINAL_PROCESSING != 0 {
        return;
    }
    unsafe {
        let handle = get_std_handle(which);
        set_console_mode(handle, mode | ENABLE_VIRTUAL_TERMINAL_PROCESSING);
    }
}

#[cfg(not(windows))]
fn enable_vt(_stream: Stream) {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn disabled_palette_is_byte_identical_to_the_input() {
        let off = Palette::off();
        for text in ["", "plain", r"D:\data\Projects\scd", "工作目录"] {
            assert_eq!(off.name(text), text);
            assert_eq!(off.path(text), text);
            assert_eq!(off.dim(text), text);
            assert_eq!(off.err(text), text);
        }
        assert_eq!(off.key("c"), "[c]");
        assert_eq!(off.key("1"), "[1]");
    }

    #[test]
    fn enabled_palette_wraps_sgr_codes() {
        let on = Palette::on();
        assert_eq!(on.name("scd"), "\x1b[1;32mscd\x1b[0m");
        assert_eq!(on.key("c"), "\x1b[2m[\x1b[0m\x1b[1;33mc\x1b[0m\x1b[2m]\x1b[0m");
        assert_eq!(on.prompt("scd> "), "\x1b[1;35mscd> \x1b[0m");
        assert_eq!(on.error_prefix(), "\x1b[1;31mscd:\x1b[0m");
        assert_eq!(on.dim("x"), "\x1b[2mx\x1b[0m");
        assert_eq!(on.err("x"), "\x1b[1;31mx\x1b[0m");
        assert_eq!(on.ok("x"), "\x1b[1;32mx\x1b[0m");
        assert_eq!(on.warn("x"), "\x1b[33mx\x1b[0m");
        assert_eq!(on.number("3"), "\x1b[1;33m3\x1b[0m");
        assert_eq!(on.heading("x"), "\x1b[1;4mx\x1b[0m");
        assert!(on.path("x").starts_with("\x1b[1;36m") && on.path("x").ends_with("\x1b[0m"));
        assert_eq!(strip_ansi(&on.path(r"D:\x")), r"D:\x");
    }

    #[test]
    fn empty_text_never_emits_escapes() {
        let on = Palette::on();
        let styled = [
            on.name(""),
            on.path(""),
            on.dim(""),
            on.err(""),
            on.ok(""),
            on.warn(""),
            on.bold(""),
            on.heading(""),
            on.number(""),
            on.prompt(""),
        ];
        for item in styled {
            assert_eq!(item, "", "empty input must stay empty");
        }
        assert_eq!(strip_ansi(&on.key("")), "[]");
    }

    #[test]
    fn strip_ansi_recovers_the_plain_text() {
        let on = Palette::on();
        let line = format!("{} {}  {}", on.key("1"), on.name("scd"), on.path(r"D:\data\Projects\scd"));
        assert!(line.contains('\x1b'));
        assert_eq!(strip_ansi(&line), "[1] scd  D:\\data\\Projects\\scd");
        assert_eq!(strip_ansi("no escapes here"), "no escapes here");
        assert_eq!(strip_ansi("\x1b[1;36m工作目录\x1b[0m"), "工作目录");
        assert_eq!(strip_ansi("\x1b[0m\x1b[2m\x1b[0m"), "");
    }

    #[test]
    fn preference_parsing() {
        assert_eq!(pref_from(None), Pref::Auto);
        assert_eq!(pref_from(Some("")), Pref::Auto);
        assert_eq!(pref_from(Some("garbage")), Pref::Auto);
        for value in ["always", "ALWAYS", "1", "yes", "true", "force", "on"] {
            assert_eq!(pref_from(Some(value)), Pref::Always, "{value}");
        }
        for value in ["never", "Never", "0", "no", "false", "off", "none"] {
            assert_eq!(pref_from(Some(value)), Pref::Never, "{value}");
        }
    }

    #[test]
    fn decision_table() {
        // auto: only a tty (or CLICOLOR_FORCE) enables colour
        assert!(decide(Pref::Auto, false, false, false, false, true));
        assert!(!decide(Pref::Auto, false, false, false, false, false));
        assert!(decide(Pref::Auto, false, false, true, false, false), "CLICOLOR_FORCE");
        // kill switches
        assert!(!decide(Pref::Auto, true, false, true, false, true), "NO_COLOR wins");
        assert!(!decide(Pref::Auto, false, true, false, false, true), "CLICOLOR=0 wins");
        assert!(!decide(Pref::Auto, false, false, false, true, true), "TERM=dumb wins");
        // explicit overrides
        assert!(decide(Pref::Always, true, true, false, true, false), "SCD_COLOR=always forces colour");
        assert!(!decide(Pref::Never, false, false, true, false, true), "SCD_COLOR=never forces plain");
    }

    #[test]
    fn redirected_streams_report_no_tty() {
        // Under `cargo test` stdout/stderr are pipes, so detection must say "no".
        assert!(!is_tty(Stream::Stdout));
        assert!(!is_tty(Stream::Stderr));
        assert!(!Palette::for_stream(Stream::Stdout).enabled);
    }

    #[test]
    fn enabling_vt_on_a_pipe_is_harmless() {
        enable_vt(Stream::Stdout);
        enable_vt(Stream::Stderr);
    }
}