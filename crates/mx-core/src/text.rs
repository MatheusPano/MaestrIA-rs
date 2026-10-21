use chrono::{DateTime, Datelike, Local, NaiveDateTime, TimeZone, Timelike};

pub type Time = DateTime<Local>;

pub fn utf16_len(s: &str) -> usize {
    s.chars().map(char::len_utf16).sum()
}

pub fn clip_utf16(s: &str, max_units: usize) -> &str {
    let mut units = 0;
    for (i, c) in s.char_indices() {
        units += c.len_utf16();
        if units > max_units {
            return &s[..i];
        }
    }
    s
}

pub fn clip_utf16_ellipsis(s: &str, max_units: usize, ellipsis: &str) -> String {
    if utf16_len(s) <= max_units {
        return s.to_owned();
    }
    let room = max_units.saturating_sub(utf16_len(ellipsis));
    format!("{}{}", clip_utf16(s, room), ellipsis)
}

pub fn clip_or_ellipsis(s: &str, max_units: usize) -> String {
    if utf16_len(s) <= max_units {
        s.to_owned()
    } else {
        format!("{}…", clip_utf16(s, max_units))
    }
}

pub fn utf16_sum(s: &str) -> u64 {
    s.encode_utf16().map(u64::from).sum()
}

pub fn base36(mut n: u64) -> String {
    const DIGITS: &[u8; 36] = b"0123456789abcdefghijklmnopqrstuvwxyz";
    if n == 0 {
        return "0".into();
    }
    let mut out = Vec::new();
    while n > 0 {
        out.push(DIGITS[(n % 36) as usize]);
        n /= 36;
    }
    out.reverse();
    String::from_utf8(out).unwrap_or_default()
}

pub fn last_segment(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}

pub fn iso8601(at: &Time) -> String {
    let micros = at.timestamp_subsec_micros() % 1000;
    let millis = at.timestamp_subsec_millis();
    let base = format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}.{:03}",
        at.year(),
        at.month(),
        at.day(),
        at.hour(),
        at.minute(),
        at.second(),
        millis
    );
    if micros == 0 {
        base
    } else {
        format!("{base}{micros:03}")
    }
}

pub fn parse_iso8601(raw: &str) -> Option<Time> {
    let raw = raw.trim();
    if raw.is_empty() {
        return None;
    }
    if let Ok(dt) = DateTime::parse_from_rfc3339(raw) {
        return Some(dt.with_timezone(&Local));
    }
    if let Some(stripped) = raw.strip_suffix('Z').or_else(|| raw.strip_suffix('z')) {
        let naive = parse_naive(stripped)?;
        return Some(chrono::Utc.from_utc_datetime(&naive).with_timezone(&Local));
    }
    for fmt in [
        "%Y-%m-%dT%H:%M:%S%.f%:z",
        "%Y-%m-%dT%H:%M:%S%.f%z",
        "%Y-%m-%d %H:%M:%S%.f%:z",
    ] {
        if let Ok(dt) = DateTime::parse_from_str(raw, fmt) {
            return Some(dt.with_timezone(&Local));
        }
    }
    let naive = parse_naive(raw)?;
    Local.from_local_datetime(&naive).earliest()
}

fn parse_naive(raw: &str) -> Option<NaiveDateTime> {
    for fmt in [
        "%Y-%m-%dT%H:%M:%S%.f",
        "%Y-%m-%d %H:%M:%S%.f",
        "%Y-%m-%dT%H:%M",
        "%Y-%m-%d %H:%M",
    ] {
        if let Ok(n) = NaiveDateTime::parse_from_str(raw, fmt) {
            return Some(n);
        }
    }
    chrono::NaiveDate::parse_from_str(raw, "%Y-%m-%d")
        .ok()
        .and_then(|d| d.and_hms_opt(0, 0, 0))
}

pub fn parse_iso8601_or(raw: Option<&str>, fallback: Time) -> Time {
    raw.and_then(parse_iso8601).unwrap_or(fallback)
}

fn day_month(at: &Time) -> String {
    format!("{}/{}", at.day(), at.month())
}

pub fn short_ago(at: &Time, now: &Time) -> String {
    let since = now.signed_duration_since(*at);
    if since.num_seconds() < 60 {
        "agora".into()
    } else if since.num_minutes() < 60 {
        format!("{}min", since.num_minutes())
    } else if since.num_hours() < 24 {
        format!("{}h", since.num_hours())
    } else if since.num_days() < 7 {
        format!("{}d", since.num_days())
    } else {
        day_month(at)
    }
}

pub fn short_until(at: &Time, now: &Time) -> String {
    let left = at.signed_duration_since(*now);
    if left
        .num_microseconds()
        .map_or(left < chrono::Duration::zero(), |m| m < 0)
    {
        "agora".into()
    } else if left.num_minutes() < 1 {
        "<1min".into()
    } else if left.num_minutes() < 60 {
        format!("{}min", left.num_minutes())
    } else if left.num_hours() < 24 {
        format!("{}h", left.num_hours())
    } else if left.num_days() < 7 {
        format!("{}d", left.num_days())
    } else {
        day_month(at)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_astral_chars_as_two_units() {
        assert_eq!(utf16_len("a😀"), 3);
        assert_eq!(clip_utf16("a😀b", 2), "a");
        assert_eq!(clip_utf16("a😀b", 3), "a😀");
    }

    #[test]
    fn ellipsis_fits_inside_limit() {
        assert_eq!(clip_utf16_ellipsis("abcdef", 4, "…"), "abc…");
        assert_eq!(clip_utf16_ellipsis("abc", 4, "…"), "abc");
    }

    #[test]
    fn base36_is_lowercase() {
        assert_eq!(base36(0), "0");
        assert_eq!(base36(35), "z");
        assert_eq!(base36(36), "10");
        assert_eq!(base36(1_729_500_000_000_000), "h121bxmsqo");
    }
}
