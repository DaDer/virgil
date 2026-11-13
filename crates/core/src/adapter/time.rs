//! ISO-8601 timestamps to epoch milliseconds, without a date library.

use crate::contract::EpochMs;

/// Parses `2026-11-13T09:29:27.993Z`, also with a `+hh:mm` / `-hh:mm` offset, a space instead of
/// `T`, or no fraction. Returns `None` for anything else.
pub fn parse_iso(s: &str) -> Option<EpochMs> {
    let b = s.trim().as_bytes();
    if b.len() < 19 {
        return None;
    }
    let num = |from: usize, len: usize| -> Option<i64> {
        let part = b.get(from..from + len)?;
        let mut v = 0i64;
        for &c in part {
            if !c.is_ascii_digit() {
                return None;
            }
            v = v * 10 + (c - b'0') as i64;
        }
        Some(v)
    };
    if b[4] != b'-' || b[7] != b'-' || !(b[10] == b'T' || b[10] == b't' || b[10] == b' ') || b[13] != b':' || b[16] != b':' {
        return None;
    }
    let (year, month, day) = (num(0, 4)?, num(5, 2)?, num(8, 2)?);
    let (hour, min, sec) = (num(11, 2)?, num(14, 2)?, num(17, 2)?);
    if !(1..=12).contains(&month) || !(1..=31).contains(&day) || hour > 23 || min > 59 || sec > 60 {
        return None;
    }
    let mut i = 19;
    let mut millis = 0i64;
    if b.get(i) == Some(&b'.') {
        i += 1;
        let mut digits = 0;
        while let Some(c) = b.get(i).filter(|c| c.is_ascii_digit()) {
            if digits < 3 {
                millis = millis * 10 + (c - b'0') as i64;
            }
            digits += 1;
            i += 1;
        }
        if digits == 0 {
            return None;
        }
        for _ in digits..3 {
            millis *= 10;
        }
    }
    let offset_min = match b.get(i) {
        None => 0,
        Some(b'Z') | Some(b'z') if i + 1 == b.len() => 0,
        Some(&sign @ (b'+' | b'-')) => {
            let h = num(i + 1, 2)?;
            let m = if b.get(i + 3) == Some(&b':') { num(i + 4, 2)? } else { num(i + 3, 2)? };
            let v = h * 60 + m;
            if sign == b'+' { v } else { -v }
        }
        _ => return None,
    };
    let days = days_from_civil(year, month, day);
    let secs = days * 86_400 + hour * 3600 + min * 60 + sec - offset_min * 60;
    Some(secs * 1000 + millis)
}

/// Howard Hinnant's days_from_civil.
fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_common_shapes() {
        assert_eq!(parse_iso("2026-11-14T09:53:20.000Z"), Some(1794650000000));
        assert_eq!(parse_iso("2026-11-14T09:53:20Z"), Some(1794650000000));
        assert_eq!(parse_iso("2026-11-14T10:53:20.5+01:00"), Some(1794650000500));
        assert_eq!(parse_iso("1970-01-01T00:00:00.001Z"), Some(1));
        assert_eq!(parse_iso("not a date"), None);
        assert_eq!(parse_iso("2026-13-14T09:53:20Z"), None);
    }
}
