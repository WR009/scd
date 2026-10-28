//! The `c` flow from `docs/ARCHITECTURE.md`: ask about every configurable item, one
//! question at a time ("逐个询问需要修改的内容").
//!
//! Today there is exactly one setting - how many locations the menu lists - but the flow is
//! data driven: adding a setting means adding a [`Setting`] variant and one entry in [`ALL`].

use std::path::Path;

use crate::add::is_cancel;
use crate::color::Palette;
use crate::config::{Config, MAX_DISPLAY_LIMIT, MIN_MAX_DISPLAY};
use crate::ui::Ui;

/// Every configurable item, in the order they are asked.
pub const ALL: &[Setting] = &[Setting::MaxDisplay];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Setting {
    /// "显示多少个位置" - how many locations the menu lists.
    MaxDisplay,
}

impl Setting {
    pub fn key(&self) -> &'static str {
        match self {
            Setting::MaxDisplay => "max_display",
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            Setting::MaxDisplay => "max locations to display",
        }
    }

    pub fn hint(&self) -> &'static str {
        match self {
            Setting::MaxDisplay => "1-99, how many locations the menu lists",
        }
    }

    pub fn current(&self, cfg: &Config) -> String {
        match self {
            Setting::MaxDisplay => cfg.max_display.to_string(),
        }
    }

    /// The question line, e.g. `[1/1] max locations to display (1-99, ...; current: 9): `
    pub fn question(&self, index: usize, total: usize, cfg: &Config, palette: &Palette) -> String {
        format!(
            "{} {} ({}; current: {}): ",
            palette.key(&format!("{index}/{total}")),
            palette.bold(self.label()),
            palette.dim(self.hint()),
            palette.number(&self.current(cfg))
        )
    }

    /// Validate one answer and apply it. The error is shown to the user verbatim.
    pub fn apply(&self, cfg: &mut Config, input: &str) -> Result<(), String> {
        let text = input.trim();
        match self {
            Setting::MaxDisplay => {
                let value: usize = text.parse().map_err(|_| {
                    format!(
                        "\"{text}\" is not a whole number between {MIN_MAX_DISPLAY} and {MAX_DISPLAY_LIMIT}"
                    )
                })?;
                cfg.set_max_display(value)
            }
        }
    }
}

/// Walk through every setting, then persist the result if anything changed.
pub fn run(cfg: &mut Config, config_path: &Path, ui: &Ui) {
    let palette = ui.palette();
    ui.line(&palette.heading("configure scd"));
    ui.line(&format!(
        "{} {}",
        palette.dim("config file:"),
        palette.path(&config_path.display().to_string())
    ));
    ui.line(&palette.dim(&format!(
        "{} setting(s) - press enter to keep a value, type \"quit\" to cancel",
        ALL.len()
    )));
    ui.blank();

    let total = ALL.len();
    let mut changed = false;
    for (index, setting) in ALL.iter().enumerate() {
        loop {
            let Some(raw) = ui.ask(&setting.question(index + 1, total, cfg, &palette)) else {
                ui.blank();
                ui.line(&palette.dim("cancelled"));
                ui.blank();
                return;
            };
            let input = raw.trim();
            if input.is_empty() {
                break; // keep the current value, move on to the next setting
            }
            if is_cancel(input) {
                ui.line(&palette.dim("cancelled"));
                ui.blank();
                return;
            }
            match setting.apply(cfg, input) {
                Ok(()) => {
                    changed = true;
                    ui.line(&format!(
                        "{} {}",
                        palette.bold(setting.label()),
                        palette.number(&format!("= {}", setting.current(cfg)))
                    ));
                    break;
                }
                Err(message) => {
                    ui.line(&format!("{} {}", palette.error_prefix(), palette.err(&message)))
                }
            }
        }
    }

    if !changed {
        ui.line(&palette.dim("no changes"));
        ui.blank();
        return;
    }
    match cfg.save(config_path) {
        Ok(()) => ui.line(&format!(
            "{} {}",
            palette.ok("saved"),
            palette.path(&config_path.display().to_string())
        )),
        Err(err) => ui.line(&format!("{} {}", palette.error_prefix(), palette.err(&err))),
    }
    ui.blank();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::color::strip_ansi;

    #[test]
    fn exactly_one_setting_is_asked_today() {
        assert_eq!(ALL.len(), 1);
        assert_eq!(ALL[0], Setting::MaxDisplay);
        assert_eq!(ALL[0].key(), "max_display");
        assert_eq!(ALL[0].label(), "max locations to display");
    }

    #[test]
    fn question_mentions_index_hint_and_current_value() {
        let mut cfg = Config::default();
        cfg.set_max_display(4).unwrap();
        let question = Setting::MaxDisplay.question(1, ALL.len(), &cfg, &Palette::off());
        assert_eq!(
            question,
            "[1/1] max locations to display (1-99, how many locations the menu lists; current: 4): "
        );
    }

    #[test]
    fn coloured_question_keeps_its_text() {
        let mut cfg = Config::default();
        cfg.set_max_display(4).unwrap();
        let plain = Setting::MaxDisplay.question(1, ALL.len(), &cfg, &Palette::off());
        let colored = Setting::MaxDisplay.question(1, ALL.len(), &cfg, &Palette::on());
        assert_eq!(strip_ansi(&colored), plain);
        assert!(colored.contains("\x1b[1;33m1/1\x1b[0m"), "{colored:?}");
        assert!(colored.contains("\x1b[1;33m4\x1b[0m"), "current value is highlighted: {colored:?}");
        assert!(colored.ends_with("): \x1b[0m") || colored.ends_with("): "), "{colored:?}");
    }

    #[test]
    fn apply_validates_the_answer() {
        let mut cfg = Config::default();
        assert!(Setting::MaxDisplay.apply(&mut cfg, "5").is_ok());
        assert_eq!(cfg.max_display, 5);
        assert!(Setting::MaxDisplay.apply(&mut cfg, "  12  ").is_ok());
        assert_eq!(cfg.max_display, 12);

        for bad in ["0", "100", "-1", "abc", "1.5", ""] {
            let before = cfg.max_display;
            let err = Setting::MaxDisplay.apply(&mut cfg, bad).unwrap_err();
            assert_eq!(cfg.max_display, before, "{bad} must not change anything");
            assert!(err.contains(bad) || bad.is_empty(), "{err}");
        }
    }

    #[test]
    fn boundary_values_are_accepted() {
        let mut cfg = Config::default();
        assert!(Setting::MaxDisplay.apply(&mut cfg, &MIN_MAX_DISPLAY.to_string()).is_ok());
        assert_eq!(cfg.max_display, MIN_MAX_DISPLAY);
        assert!(Setting::MaxDisplay.apply(&mut cfg, &MAX_DISPLAY_LIMIT.to_string()).is_ok());
        assert_eq!(cfg.max_display, MAX_DISPLAY_LIMIT);
    }
}