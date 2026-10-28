//! Terminal I/O helpers: indentation, prompts, line reading, column-aware padding and
//! per-stream colour.
//!
//! Two kinds of output exist and must not be confused:
//! * **menu chrome** ([`Ui::line`] / [`Ui::ask`]) - indented by four spaces, matching the
//!   layout in `docs/ARCHITECTURE.md`, sent to whichever stream [`crate::emit`] picked;
//! * **command output** ([`Ui::out`]) - flush left, always stdout, so `scd list` and
//!   `scd config` stay pipeable.

use std::io::{self, BufRead, Write};

use crate::color::Palette;

/// Indent used for every menu line.
pub const INDENT: &str = "    ";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stream {
    Stdout,
    Stderr,
}

/// A line-oriented view on one output stream, with its own colour decision.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Ui {
    stream: Stream,
    palette: Palette,
}

impl Ui {
    /// Detect colour support for `stream` (tty? `SCD_COLOR`? `NO_COLOR`?).
    pub fn new(stream: Stream) -> Self {
        Ui { stream, palette: Palette::for_stream(stream) }
    }

    pub fn with_palette(stream: Stream, palette: Palette) -> Self {
        Ui { stream, palette }
    }

    /// Never colour - used by tests that compare exact text.
    pub fn plain(stream: Stream) -> Self {
        Ui { stream, palette: Palette::off() }
    }

    pub fn stdout() -> Self {
        Ui::new(Stream::Stdout)
    }

    pub fn stderr() -> Self {
        Ui::new(Stream::Stderr)
    }

    pub fn stream(&self) -> Stream {
        self.stream
    }

    /// Palette callers use to style names, paths, keys, ... before handing text over.
    pub fn palette(&self) -> Palette {
        self.palette
    }

    /// Print one indented menu line (an empty `text` prints a blank line).
    pub fn line(&self, text: &str) {
        write_line(self.stream, &indented(text));
    }

    pub fn lines(&self, texts: &[String]) {
        for text in texts {
            self.line(text);
        }
    }

    pub fn blank(&self) {
        write_line(self.stream, "");
    }

    /// Print one flush-left line of command output.
    pub fn out(&self, text: &str) {
        write_line(self.stream, &flatten(text));
    }

    pub fn out_lines(&self, texts: &[String]) {
        for text in texts {
            self.out(text);
        }
    }

    /// Print a prompt without a newline, then read one line. `None` means end of input.
    pub fn ask(&self, prompt: &str) -> Option<String> {
        write_raw(self.stream, &indented(&flatten(prompt)));
        flush(self.stream);
        read_line()
    }
}

/// Prefix a non-empty line with the standard indent.
pub fn indented(text: &str) -> String {
    if text.is_empty() {
        String::new()
    } else {
        format!("{INDENT}{text}")
    }
}

/// Collapse CR/LF runs into a single space.
///
/// Windows `io::Error` messages embed newlines ("Access is denied.\r\n (os error 5)"), which
/// would otherwise spill onto a second line without the four-space indent and break the
/// layout of every message.
pub fn flatten(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut pending_space = false;
    for ch in text.chars() {
        if ch == '\r' || ch == '\n' {
            pending_space = true;
            continue;
        }
        if pending_space {
            if ch == ' ' || ch == '\t' {
                continue; // collapse the whole whitespace run
            }
            if !out.is_empty() && !out.ends_with(' ') {
                out.push(' ');
            }
            pending_space = false;
        }
        out.push(ch);
    }
    out
}

/// Width of `text` in terminal columns: East Asian wide/fullwidth characters (and emoji)
/// occupy two cells, so padding by `chars().count()` would misalign a menu that contains
/// both `docs` and `工作目录`.
pub fn display_width(text: &str) -> usize {
    // Colour escapes take no room on screen, so measure the text the user actually sees.
    crate::color::strip_ansi(text).chars().map(char_width).sum()
}

fn char_width(ch: char) -> usize {
    // Simplified East Asian Width (UAX #11) W + F classes, plus the emoji blocks that
    // terminals render wide.
    let code = ch as u32;
    let wide = matches!(code,
        0x1100..=0x115F
            | 0x2E80..=0x303E
            | 0x3041..=0x33FF
            | 0x3400..=0x4DBF
            | 0x4E00..=0x9FFF
            | 0xA000..=0xA4CF
            | 0xA960..=0xA97F
            | 0xAC00..=0xD7A3
            | 0xF900..=0xFAFF
            | 0xFE10..=0xFE19
            | 0xFE30..=0xFE6F
            | 0xFF00..=0xFF60
            | 0xFFE0..=0xFFE6
            | 0x1B000..=0x1B16F
            | 0x1F300..=0x1F64F
            | 0x1F900..=0x1F9FF
            | 0x20000..=0x3FFFD);
    if wide {
        2
    } else {
        1
    }
}

/// Pad `text` with spaces so that it occupies `width` terminal columns (never truncates).
/// Colour escapes inside `text` are ignored when measuring, so styled names still align.
pub fn pad_to_width(text: &str, width: usize) -> String {
    let missing = width.saturating_sub(display_width(text));
    if missing == 0 {
        text.to_string()
    } else {
        format!("{text}{}", " ".repeat(missing))
    }
}

/// Read one line from stdin.
///
/// Bytes that are not valid UTF-8 are replaced rather than fatal, because on Windows a
/// redirected stdin may carry the console code page. `None` signals end of input.
pub fn read_line() -> Option<String> {
    let stdin = io::stdin();
    let mut handle = stdin.lock();
    let mut buffer: Vec<u8> = Vec::new();
    match handle.read_until(b'\n', &mut buffer) {
        Ok(0) if buffer.is_empty() => None,
        Ok(_) => Some(clean_line(&buffer)),
        Err(_) => None,
    }
}

/// Turn one raw stdin line into an answer: UTF-8 (lossy), no BOM, no line endings.
pub fn clean_line(buffer: &[u8]) -> String {
    let mut text = String::from_utf8_lossy(buffer).into_owned();
    // A UTF-8 BOM would otherwise make the first answer look like "\u{feff}a" and silently
    // fail to match the [a]/[c] menu keys (PowerShell pipes and `type file | scd` add one).
    if text.starts_with('\u{feff}') {
        text.remove(0);
    }
    text.trim_end_matches(['\r', '\n']).to_string()
}

fn write_line(stream: Stream, text: &str) {
    match stream {
        Stream::Stdout => {
            let mut handle = io::stdout();
            let _ = writeln!(handle, "{text}");
            let _ = handle.flush();
        }
        Stream::Stderr => {
            let mut handle = io::stderr();
            let _ = writeln!(handle, "{text}");
            let _ = handle.flush();
        }
    }
}

fn write_raw(stream: Stream, text: &str) {
    match stream {
        Stream::Stdout => {
            let mut handle = io::stdout();
            let _ = write!(handle, "{text}");
        }
        Stream::Stderr => {
            let mut handle = io::stderr();
            let _ = write!(handle, "{text}");
        }
    }
}

fn flush(stream: Stream) {
    match stream {
        Stream::Stdout => {
            let _ = io::stdout().flush();
        }
        Stream::Stderr => {
            let _ = io::stderr().flush();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn indents_non_empty_lines_only() {
        assert_eq!(indented("hello"), "    hello");
        assert_eq!(indented(""), "");
        assert_eq!(indented("[c] configure scd"), "    [c] configure scd");
    }

    #[test]
    fn clean_line_strips_endings_bom_and_bad_bytes() {
        assert_eq!(clean_line(b"a\n"), "a");
        assert_eq!(clean_line(b"a\r\n"), "a");
        assert_eq!(clean_line(b"a"), "a");
        assert_eq!(clean_line(b"\xef\xbb\xbfa\r\n"), "a", "leading BOM is dropped");
        assert_eq!(clean_line(b"\xef\xbb\xbf"), "");
        assert_eq!(clean_line(b"D:\\my dir\n"), "D:\\my dir");
        assert_eq!(clean_line("\u{feff}工作\n".as_bytes()), "工作");
        let lossy = clean_line(b"\xff\xfe bad\n");
        assert!(lossy.ends_with(" bad"), "{lossy:?}");
        assert!(lossy.contains('\u{fffd}'), "undecodable bytes become replacement chars: {lossy:?}");
    }

    #[test]
    fn flatten_keeps_messages_on_one_line() {
        assert_eq!(flatten("plain"), "plain");
        assert_eq!(flatten(""), "");
        assert_eq!(flatten("\r\nAccess is denied.\r\n (os error 5)"), "Access is denied. (os error 5)");
        assert_eq!(flatten("a\nb"), "a b");
        assert_eq!(flatten("a \nb"), "a b", "no double space when the text already ends with one");
        assert_eq!(flatten("name  path\nmore"), "name  path more", "intentional double spaces survive");
        assert_eq!(flatten("a\r\n\r\n\r\nb"), "a b");
        assert_eq!(flatten("trailing\r\n"), "trailing");
        assert_eq!(flatten("\n\nleading"), "leading");
        assert_eq!(flatten("工作\n目录"), "工作 目录");
    }

    #[test]
    fn line_output_never_contains_a_raw_newline_inside_the_text() {
        assert_eq!(
            indented(&flatten("cannot write x: \r\nAccess is denied.")),
            "    cannot write x: Access is denied."
        );
    }

    #[test]
    fn display_width_counts_cjk_as_two_columns() {
        assert_eq!(display_width(""), 0);
        assert_eq!(display_width("docs"), 4);
        assert_eq!(display_width("工作"), 4);
        assert_eq!(display_width("工作目录"), 8);
        assert_eq!(display_width("a工b"), 4);
        assert_eq!(display_width("ＤＯＣＳ"), 8, "fullwidth latin is wide too");
    }

    #[test]
    fn display_width_ignores_colour_escapes() {
        let on = Palette::on();
        assert_eq!(display_width(&on.name("docs")), 4, "escapes take no room on screen");
        assert_eq!(display_width(&on.path("工作")), 4);
        assert_eq!(pad_to_width(&on.name("docs"), 8), format!("{}    ", on.name("docs")));
        assert_eq!(display_width(&pad_to_width(&on.name("docs"), 8)), 8);
    }

    #[test]
    fn pad_to_width_aligns_mixed_scripts() {
        assert_eq!(pad_to_width("docs", 8), "docs    ");
        assert_eq!(pad_to_width("工作", 8), "工作    ");
        assert_eq!(pad_to_width("工作目录", 8), "工作目录");
        assert_eq!(pad_to_width("工作目录", 4), "工作目录", "never truncates");
        assert_eq!(display_width(&pad_to_width("工作", 8)), 8);
        assert_eq!(display_width(&pad_to_width("docs", 8)), 8);
    }

    #[test]
    fn ui_remembers_its_stream_and_palette() {
        assert_eq!(Ui::stdout().stream(), Stream::Stdout);
        assert_eq!(Ui::stderr().stream(), Stream::Stderr);
        assert_eq!(Ui::new(Stream::Stderr), Ui::stderr());
        assert!(!Ui::plain(Stream::Stdout).palette().enabled);
        assert!(Ui::with_palette(Stream::Stdout, Palette::on()).palette().enabled);
    }
}