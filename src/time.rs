//! UTC timestamps for the per-location metadata in `config.json`.
//!
//! Implemented with the civil-calendar algorithm (Howard Hinnant) instead of pulling in a
//! date crate, so the project stays dependency free.

use std::time::{SystemTime, UNIX_EPOCH};

pub const SECONDS_PER_DAY: i64 = 86_400;

/// Current unix time in seconds; `0` if the system clock is before the epoch.
pub fn unix_now() -> i64 {
    match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(duration) => duration.as_secs() as i64,
        Err(_) => 0,
    }
}

/// Format as `YYYY-MM-DDTHH:MM:SSZ` (always UTC).
pub fn format_utc(seconds: i64) -> String {
    let days = seconds.div_euclid(SECONDS_PER_DAY);
    let of_day = seconds.rem_euclid(SECONDS_PER_DAY);
    let (year, month, day) = civil_from_days(days);
    let (hour, minute, second) = (of_day / 3600, (of_day % 3600) / 60, of_day % 60);
    format!("{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}Z")
}

/// Inverse of [`format_utc`]; tolerates a missing `Z` and fractional seconds.
pub fn parse_utc(text: &str) -> Option<i64> {
    let text = text.trim();
    let text = match text.strip_suffix('Z').or_else(|| text.strip_suffix('z')) {
        Some(stripped) => stripped,
        None => text,
    };
    let (date, clock) = text.split_once('T')?;

    let mut date_parts = date.split('-');
    let year: i64 = date_parts.next()?.trim().parse().ok()?;
    let month: u32 = date_parts.next()?.trim().parse().ok()?;
    let day: u32 = date_parts.next()?.trim().parse().ok()?;
    if date_parts.next().is_some() {
        return None;
    }

    let mut clock_parts = clock.split(':');
    let hour: i64 = clock_parts.next()?.trim().parse().ok()?;
    let minute: i64 = clock_parts.next()?.trim().parse().ok()?;
    let second_text = clock_parts.next()?.trim();
    if clock_parts.next().is_some() {
        return None;
    }
    let second: i64 = second_text.split('.').next()?.parse().ok()?;

    if !(1..=12).contains(&month) || !(1..=31).contains(&day) {
        return None;
    }
    if !(0..=23).contains(&hour) || !(0..=59).contains(&minute) || !(0..=60).contains(&second) {
        return None;
    }

    Some(days_from_civil(year, month, day) * SECONDS_PER_DAY + hour * 3600 + minute * 60 + second)
}

/// Days since 1970-01-01 -> (year, month, day) in the civil calendar.
pub fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let shifted = days + 719_468;
    let era = if shifted >= 0 { shifted } else { shifted - 146_096 } / 146_097;
    let day_of_era = (shifted - era * 146_097) as u64;
    let year_of_era =
        (day_of_era - day_of_era / 1460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let year = year_of_era as i64 + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let shifted_month = (5 * day_of_year + 2) / 153;
    let day = (day_of_year - (153 * shifted_month + 2) / 5 + 1) as u32;
    let month = if shifted_month < 10 { shifted_month + 3 } else { shifted_month - 9 } as u32;
    (year + i64::from(month <= 2), month, day)
}

/// (year, month, day) in the civil calendar -> days since 1970-01-01.
pub fn days_from_civil(year: i64, month: u32, day: u32) -> i64 {
    let year = if month <= 2 { year - 1 } else { year };
    let era = if year >= 0 { year } else { year - 399 } / 400;
    let year_of_era = (year - era * 400) as u64;
    let shifted_month = if month > 2 { month - 3 } else { month + 9 } as u64;
    let day_of_year = (153 * shifted_month + 2) / 5 + day as u64 - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    era * 146_097 + day_of_era as i64 - 719_468
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_known_timestamps() {
        assert_eq!(format_utc(0), "1970-01-01T00:00:00Z");
        assert_eq!(format_utc(1_000_000_000), "2001-09-09T01:46:40Z");
        assert_eq!(format_utc(-1), "1969-12-31T23:59:59Z");
        assert_eq!(format_utc(86_399), "1970-01-01T23:59:59Z");
        assert_eq!(format_utc(86_400), "1970-01-02T00:00:00Z");
        assert_eq!(format_utc(1_709_164_800), "2024-02-29T00:00:00Z");
    }

    #[test]
    fn parses_what_it_formats() {
        for seconds in [0i64, 1, -1, 86_399, 1_000_000_000, 1_709_164_800, 2_000_000_000] {
            let text = format_utc(seconds);
            assert_eq!(parse_utc(&text), Some(seconds), "{text}");
        }
    }

    #[test]
    fn parsing_is_tolerant_but_strict_about_shape() {
        assert_eq!(parse_utc("2026-10-27T01:02:03"), Some(parse_utc("2026-10-27T01:02:03Z").unwrap()));
        assert_eq!(parse_utc(" 2026-10-27T01:02:03.999Z "), parse_utc("2026-10-27T01:02:03Z"));
        assert_eq!(parse_utc("not a date"), None);
        assert_eq!(parse_utc("2026-13-01T00:00:00Z"), None);
        assert_eq!(parse_utc("2026-10-27T25:00:00Z"), None);
        assert_eq!(parse_utc("2026-10-27"), None);
    }

    #[test]
    fn calendar_conversion_round_trips() {
        for days in -80_000..80_000i64 {
            let (year, month, day) = civil_from_days(days);
            assert_eq!(days_from_civil(year, month, day), days, "{days} -> {year}-{month}-{day}");
            assert!((1..=12).contains(&month) && (1..=31).contains(&day));
        }
    }

    #[test]
    fn unix_now_is_plausible() {
        assert!(unix_now() > 1_700_000_000);
    }
}