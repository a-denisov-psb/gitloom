//! Small formatting helpers.

/// Formats a unix timestamp as `YYYY-MM-DD HH:MM` (UTC).
pub fn date_time(timestamp: i64) -> String {
    let days = timestamp.div_euclid(86_400);
    let seconds = timestamp.rem_euclid(86_400);
    let (year, month, day) = civil_from_days(days);
    format!(
        "{year:04}-{month:02}-{day:02} {:02}:{:02}",
        seconds / 3600,
        (seconds % 3600) / 60
    )
}

/// Age of `timestamp` at `now`: "just now", "5 minutes ago", "3 days ago",
/// "2 years ago" (blame gutter).
pub fn relative(timestamp: i64, now: i64) -> String {
    let seconds = (now - timestamp).max(0);
    let (value, unit) = match seconds {
        0..60 => return "just now".to_owned(),
        60..3_600 => (seconds / 60, "minute"),
        3_600..86_400 => (seconds / 3_600, "hour"),
        86_400..2_592_000 => (seconds / 86_400, "day"),
        2_592_000..31_536_000 => (seconds / 2_592_000, "month"),
        _ => (seconds / 31_536_000, "year"),
    };
    let plural = if value == 1 { "" } else { "s" };
    format!("{value} {unit}{plural} ago")
}

/// Relative date of the graph's time markers (M14-06), like GitKraken:
/// "just now", "<n> minutes ago", "<n> hours ago" (same day), "yesterday",
/// "<n> days ago" (< 7), "<n> weeks ago" (< 5), "<n> months ago",
/// "<n> years ago". Days count calendar days (UTC, as the date column).
pub fn time_marker(timestamp: i64, now: i64) -> String {
    let seconds = now - timestamp;
    if seconds < 60 {
        return "just now".to_owned();
    }
    let days = now.div_euclid(86_400) - timestamp.div_euclid(86_400);
    let plural =
        |value: i64, unit: &str| format!("{value} {unit}{} ago", if value == 1 { "" } else { "s" });
    match days {
        0 if seconds < 3_600 => plural(seconds / 60, "minute"),
        0 => plural(seconds / 3_600, "hour"),
        1 => "yesterday".to_owned(),
        2..=6 => plural(days, "day"),
        7..=34 => plural(days / 7, "week"),
        35..=364 => plural((days / 30).max(1), "month"),
        _ => plural(days / 365, "year"),
    }
}

/// Groups digits by thousands with a space: `100000` → `100 000`.
pub fn thousands(value: usize) -> String {
    let digits = value.to_string();
    let mut out = String::with_capacity(digits.len() + digits.len() / 3);
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(' ');
        }
        out.push(c);
    }
    out
}

/// Initials for the avatar placeholder in the graph ("Alice Meyer" -> "AM").
pub fn initials(name: &str) -> String {
    name.split_whitespace()
        .filter_map(|part| part.chars().next())
        .take(2)
        .flat_map(char::to_uppercase)
        .collect()
}

/// Howard Hinnant's days-to-civil algorithm.
fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let month = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let year = yoe + era * 400 + i64::from(month <= 2);
    (year, month, day)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn relative_ages() {
        let now = 1_000_000_000;
        assert_eq!(relative(now - 10, now), "just now");
        assert_eq!(relative(now + 10, now), "just now", "clock skew");
        assert_eq!(relative(now - 60, now), "1 minute ago");
        assert_eq!(relative(now - 7_200, now), "2 hours ago");
        assert_eq!(relative(now - 86_400 * 3, now), "3 days ago");
        assert_eq!(relative(now - 86_400 * 65, now), "2 months ago");
        assert_eq!(relative(now - 31_536_000 * 5, now), "5 years ago");
    }

    #[test]
    fn formats_known_timestamps() {
        assert_eq!(date_time(0), "1970-01-01 00:00");
        assert_eq!(date_time(1_700_000_000), "2023-11-14 22:13");
    }

    #[test]
    fn groups_thousands() {
        assert_eq!(thousands(0), "0");
        assert_eq!(thousands(999), "999");
        assert_eq!(thousands(1000), "1 000");
        assert_eq!(thousands(100_000), "100 000");
        assert_eq!(thousands(1_234_567), "1 234 567");
    }

    #[test]
    fn builds_initials() {
        assert_eq!(initials("Alice Meyer"), "AM");
        assert_eq!(initials("bob"), "B");
    }

    #[test]
    fn time_marker_buckets() {
        // 2026-11-12 12:00:00 UTC.
        let now = 1_794_484_800;
        let day = 86_400;
        let cases = [
            (now, "just now"),
            (now + 300, "just now"),
            (now - 59, "just now"),
            (now - 60, "1 minute ago"),
            (now - 119, "1 minute ago"),
            (now - 3_599, "59 minutes ago"),
            (now - 3_600, "1 hour ago"),
            (now - 12 * 3_600, "12 hours ago"),
            (now - 12 * 3_600 - 1, "yesterday"),
            (now - day - 12 * 3_600, "yesterday"),
            (now - 2 * day, "2 days ago"),
            (now - 6 * day, "6 days ago"),
            (now - 7 * day, "1 week ago"),
            (now - 34 * day, "4 weeks ago"),
            (now - 35 * day, "1 month ago"),
            (now - 364 * day, "12 months ago"),
            (now - 365 * day, "1 year ago"),
            (now - 3 * 365 * day, "3 years ago"),
        ];
        for (timestamp, expected) in cases {
            assert_eq!(time_marker(timestamp, now), expected, "{}", now - timestamp);
        }
    }
}
