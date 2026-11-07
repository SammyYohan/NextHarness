//! Small, dependency-free time helpers.
//!
//! Orion deliberately does not pull in a date/time crate: it only needs epoch
//! seconds, a civil date in UTC and a human duration, all of which fit in a few
//! lines. Hours are therefore reported in UTC, which the README states plainly.

use std::time::{SystemTime, UNIX_EPOCH};

/// Seconds since the Unix epoch.
pub fn now_epoch() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_secs())
        .unwrap_or_default()
}

/// `42 s`, `12 min 05 s`, `3 h 07 min`, `2 d 04 h`.
pub fn format_duration(seconds: u64) -> String {
    if seconds < 60 {
        return format!("{seconds} s");
    }
    let minutes = seconds / 60;
    if minutes < 60 {
        let rest = seconds % 60;
        return if rest == 0 {
            format!("{minutes} min")
        } else {
            format!("{minutes} min {rest:02} s")
        };
    }
    let hours = minutes / 60;
    if hours < 24 {
        let rest = minutes % 60;
        return if rest == 0 {
            format!("{hours} h")
        } else {
            format!("{hours} h {rest:02} min")
        };
    }
    let days = hours / 24;
    let rest = hours % 24;
    if rest == 0 {
        format!("{days} d")
    } else {
        format!("{days} d {rest:02} h")
    }
}

/// `2026-11-07 14:32 UTC`.
pub fn format_utc(epoch: u64) -> String {
    let days = (epoch / 86_400) as i64;
    let seconds = epoch % 86_400;
    let (year, month, day) = civil_from_days(days);
    format!(
        "{year:04}-{month:02}-{day:02} {:02}:{:02} UTC",
        seconds / 3_600,
        (seconds % 3_600) / 60
    )
}

/// Days since the Unix epoch to a proleptic Gregorian date (Howard Hinnant's
/// `civil_from_days`, the inverse of `days_from_civil`).
fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let shifted = days + 719_468;
    let correction = if shifted >= 0 { 0 } else { -146_096 };
    let era = (shifted + correction) / 146_097;
    let day_of_era = (shifted - era * 146_097) as u64;
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let year = year_of_era as i64 + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_pos = (5 * day_of_year + 2) / 153;
    let day = (day_of_year - (153 * month_pos + 2) / 5 + 1) as u32;
    let month_value = if month_pos < 10 {
        month_pos + 3
    } else {
        month_pos - 9
    };
    let month = month_value as u32;
    (if month <= 2 { year + 1 } else { year }, month, day)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn epoch_formats_as_utc() {
        assert_eq!(format_utc(0), "1970-01-01 00:00 UTC");
        assert_eq!(format_utc(86_399), "1970-01-01 23:59 UTC");
        assert_eq!(format_utc(86_400), "1970-01-02 00:00 UTC");
        // 2026-11-07T14:32:00Z
        assert_eq!(format_utc(1_794_061_920), "2026-11-07 14:32 UTC");
    }

    #[test]
    fn leap_day_is_not_skipped() {
        // 2024-02-29T12:00:00Z
        assert_eq!(format_utc(1_709_208_000), "2024-02-29 12:00 UTC");
    }

    #[test]
    fn durations_read_naturally() {
        assert_eq!(format_duration(0), "0 s");
        assert_eq!(format_duration(59), "59 s");
        assert_eq!(format_duration(60), "1 min");
        assert_eq!(format_duration(725), "12 min 05 s");
        assert_eq!(format_duration(3_600), "1 h");
        assert_eq!(format_duration(11_220), "3 h 07 min");
        assert_eq!(format_duration(187_200), "2 d 04 h");
    }
}
