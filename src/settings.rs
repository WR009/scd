//! The `c` flow from `docs/ARCHITECTURE.md`: ask about every configurable item, one
//! question at a time ("逐个询问需要修改的内容").
//!
//! Today there is exactly one setting - how many locations the menu lists - but the flow is
//! data driven: adding a setting means adding a [`Setting`] variant and one entry in [`ALL`].

use std::path::Path;

use crate::add::is_cancel;
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

    /// The question line, e.g. `[1/1] max locations to display (1-99, ...; current: 9): `.
    pub fn question(&self, index: usize, total: usize, cfg: &Config) -> String {
        format!(
            "[{index}/{total}] {} ({}; current: {}): ",
            self.label(),
            self.hint(),
            self.current(cfg)
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
    ui.line("configure scd");
    ui.line(&format!("config file: {}", config_path.display()));
    ui.line(&format!(
        "{} setting(s) - press enter to keep a value, type \"quit\" to cancel",
        ALL.len()
    ));
    ui.blank();

    let total = ALL.len();
    let mut changed = false;
    for (index, setting) in ALL.iter().enumerate() {
        loop {
            let Some(raw) = ui.ask(&setting.question(index + 1, total, cfg)) else {
                ui.blank();
                ui.line("cancelled");
                ui.blank();
                return;
            };
            let input = raw.trim();
            if input.is_empty() {
                break; // keep the current value, move on to the next setting
            }
            if is_cancel(input) {
                ui.line("cancelled");
                ui.blank();
                return;
            }
            match setting.apply(cfg, input) {
                Ok(()) => {
                    changed = true;
                    ui.line(&format!("{} = {}", setting.label(), setting.current(cfg)));
                    break;
                }
                Err(message) => ui.line(&format!("scd: {message}")),
            }
        }
    }

    if !changed {
        ui.line("no changes");
        ui.blank();
        return;
    }
    match cfg.save(config_path) {
        Ok(()) => ui.line(&format!("saved {}", config_path.display())),
        Err(err) => ui.line(&format!("scd: {err}")),
    }
    ui.blank();
}

#[cfg(test)]
mod tests {
    use super::*;

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
        let question = Setting::MaxDisplay.question(1, ALL.len(), &cfg);
        assert_eq!(
            question,
            "[1/1] max locations to display (1-99, how many locations the menu lists; current: 4): "
        );
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