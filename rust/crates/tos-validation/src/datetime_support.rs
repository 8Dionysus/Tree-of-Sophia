//! Portable implementation of the observed Python timestamp comparison
//! profile shared by source-agnostic validation kernels and native adapters.

pub(crate) const MAX_EVENT_BYTES: usize = 1024 * 1024;

/// Frozen Python 3.14 `datetime.fromisoformat(s.replace('Z', '+00:00'))`
/// comparison. Calendar/basic/ISO-week dates, date-only values, a single
/// Unicode separator, reduced clock precision and normalized offset fields
/// retain their observed source behavior. Fractions truncate to microseconds.
/// A mixed naive/aware comparison is invalid, as Python's TypeError is invalid
/// source for this operation. This helper grants no clock/current authority.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ObservedDateTimeError {
    Invalid,
    Budget,
}

/// Source knowledge-assessment `_instant`: aware inputs only, then UTC
/// normalization. Naive values are invalid even when both operands are naive;
/// UTC normalization outside Python's supported years is also invalid.
/// This compares observations and supplies no trusted clock or current grant.
pub fn observed_instant_order(
    start: &str,
    end: &str,
) -> Result<std::cmp::Ordering, ObservedDateTimeError> {
    let start = observed_datetime(start, true)?;
    let end = observed_datetime(end, true)?;
    let upper = year_days(10000) * 86_400_000_000;
    if !start.1 || !end.1 || !(0..upper).contains(&start.0) || !(0..upper).contains(&end.0) {
        return Err(ObservedDateTimeError::Invalid);
    }
    Ok(start.0.cmp(&end.0))
}

/// Elapsed aware UTC microseconds using the same observed source parser.
/// This supplies no trusted clock or authorization.
pub fn observed_instant_elapsed_micros(
    start: &str,
    end: &str,
) -> Result<i128, ObservedDateTimeError> {
    let start = observed_datetime(start, true)?;
    let end = observed_datetime(end, true)?;
    let upper = year_days(10000) * 86_400_000_000;
    if !start.1 || !end.1 || !(0..upper).contains(&start.0) || !(0..upper).contains(&end.0) {
        return Err(ObservedDateTimeError::Invalid);
    }
    Ok(i128::from(end.0) - i128::from(start.0))
}

/// Queue receipts historically interpret naive timestamps as UTC.
/// The returned microsecond key is source ordering evidence, not a trusted clock.
pub fn observed_utc_or_naive_timestamp_micros(value: &str) -> Result<i64, ObservedDateTimeError> {
    let (micros, _) = observed_datetime(value, true)?;
    let upper = year_days(10000) * 86_400_000_000;
    if !(0..upper).contains(&micros) {
        return Err(ObservedDateTimeError::Invalid);
    }
    Ok(micros)
}

pub fn observed_datetime_order(
    start: &str,
    end: &str,
) -> Result<std::cmp::Ordering, ObservedDateTimeError> {
    observed_datetime_compare(start, end, true)
}

/// Provenance uses direct Python fromisoformat, preserving Z as a possible
/// single date/time separator. Retirement alone applies the global replacement.
pub(crate) fn observed_datetime_raw_order(
    start: &str,
    end: &str,
) -> Result<std::cmp::Ordering, ObservedDateTimeError> {
    observed_datetime_compare(start, end, false)
}

fn observed_datetime_compare(
    start: &str,
    end: &str,
    replace_z: bool,
) -> Result<std::cmp::Ordering, ObservedDateTimeError> {
    let start = observed_datetime(start, replace_z)?;
    let end = observed_datetime(end, replace_z)?;
    if start.1 != end.1 {
        return Err(ObservedDateTimeError::Invalid);
    }
    Ok(start.0.cmp(&end.0))
}

pub(crate) fn observed_datetime(
    s: &str,
    replace_z: bool,
) -> Result<(i64, bool), ObservedDateTimeError> {
    use ObservedDateTimeError::Invalid;
    if s.len() > MAX_EVENT_BYTES {
        return Err(ObservedDateTimeError::Budget);
    }
    // This global replacement is source behavior, including the surprising
    // date-only "...Z" -> naive midnight case. Expansion is bounded by six
    // times the independently capped event input, and no float is allocated.
    let normalized = if replace_z && s.contains('Z') {
        std::borrow::Cow::Owned(s.replace('Z', "+00:00"))
    } else {
        std::borrow::Cow::Borrowed(s)
    };
    let s = normalized.as_ref();
    let b = s.as_bytes();
    let year = digits(b, 0, 4)?;
    if !(1..=9999).contains(&year) {
        return Err(Invalid);
    }
    let (date_len, days) = if b.get(4) == Some(&b'-') && b.get(5) == Some(&b'W') {
        let week = digits(b, 6, 8)?;
        if b.get(8) == Some(&b'-') {
            (10, week_days(year, week, digits(b, 9, 10)?)?)
        } else {
            (8, week_days(year, week, 1)?)
        }
    } else if b.get(4) == Some(&b'W') {
        let week = digits(b, 5, 7)?;
        // CPython resolves the basic week/day versus numeric separator
        // ambiguity by the next character. Preserve its actual choice.
        let has_day = b.get(7).is_some_and(u8::is_ascii_digit)
            && (b.len() == 8 || !b.get(8).is_some_and(u8::is_ascii_digit));
        if has_day {
            (8, week_days(year, week, digits(b, 7, 8)?)?)
        } else {
            (7, week_days(year, week, 1)?)
        }
    } else if b.get(4) == Some(&b'-') {
        if b.get(7) != Some(&b'-') {
            return Err(Invalid);
        }
        (
            10,
            calendar_days(year, digits(b, 5, 7)?, digits(b, 8, 10)?)?,
        )
    } else {
        (8, calendar_days(year, digits(b, 4, 6)?, digits(b, 6, 8)?)?)
    };
    if b.len() == date_len {
        return Ok((days * 86_400_000_000, false));
    }
    // A separator is exactly one Unicode code point, including numeric and
    // non-ASCII separators. Date bytes have already been checked as ASCII.
    let rest = s.get(date_len..).ok_or(Invalid)?;
    let separator = rest.chars().next().ok_or(Invalid)?;
    let time = rest.get(separator.len_utf8()..).ok_or(Invalid)?;
    if time.is_empty() {
        return Err(Invalid);
    }
    let tz_start = time.bytes().position(|c| matches!(c, b'+' | b'-' | b'Z'));
    let (clock, timezone) = match tz_start {
        Some(i) => (&time[..i], Some(&time[i..])),
        None => (time, None),
    };
    let (h, m, sec, micros) = clock_fields(clock)?;
    if h > 23 || m > 59 || sec > 59 {
        return Err(Invalid);
    }
    let local = days * 86_400_000_000 + (h * 3600 + m * 60 + sec) * 1_000_000 + micros;
    let Some(zone) = timezone else {
        return Ok((local, false));
    };
    if zone == "Z" {
        return Ok((local, true));
    }
    let sign = match zone.as_bytes().first() {
        Some(b'+') => 1,
        Some(b'-') => -1,
        _ => return Err(Invalid),
    };
    let (h, m, sec, micros) = clock_fields(&zone[1..])?;
    // Unlike wall-clock fields, Python normalizes timezone minute/second
    // values up to 99, then requires the total offset to be below one day.
    let offset = (h * 3600 + m * 60 + sec) * 1_000_000 + micros;
    if offset >= 86_400_000_000 {
        return Err(Invalid);
    }
    Ok((local - sign * offset, true))
}

fn digits(b: &[u8], start: usize, end: usize) -> Result<i64, ObservedDateTimeError> {
    let raw = b.get(start..end).ok_or(ObservedDateTimeError::Invalid)?;
    if !raw.iter().all(u8::is_ascii_digit) {
        return Err(ObservedDateTimeError::Invalid);
    }
    Ok(raw.iter().fold(0, |n, c| n * 10 + i64::from(c - b'0')))
}
fn leap(year: i64) -> bool {
    year % 4 == 0 && (year % 100 != 0 || year % 400 == 0)
}
fn year_days(year: i64) -> i64 {
    let y = year - 1;
    y * 365 + y / 4 - y / 100 + y / 400
}
fn calendar_days(y: i64, month: i64, day: i64) -> Result<i64, ObservedDateTimeError> {
    use ObservedDateTimeError::Invalid;
    let days_in = |m| match m {
        2 => {
            if leap(y) {
                29
            } else {
                28
            }
        }
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    };
    if !(1..=12).contains(&month) || day < 1 || day > days_in(month) {
        return Err(Invalid);
    }
    let mut days = year_days(y) + day - 1;
    for m in 1..month {
        days += days_in(m);
    }
    Ok(days)
}
fn week_days(y: i64, week: i64, day: i64) -> Result<i64, ObservedDateTimeError> {
    use ObservedDateTimeError::Invalid;
    let jan4 = year_days(y) + 3;
    let monday = jan4 - jan4 % 7; // Year 1 January 1 was a Monday.
    let next_jan4 = year_days(y + 1) + 3;
    let next_monday = next_jan4 - next_jan4 % 7;
    if !(1..=7).contains(&day) || week < 1 || week > (next_monday - monday) / 7 {
        return Err(Invalid);
    }
    let days = monday + (week - 1) * 7 + day - 1;
    if days < 0 || days >= year_days(10000) {
        return Err(Invalid);
    }
    Ok(days)
}
fn clock_fields(s: &str) -> Result<(i64, i64, i64, i64), ObservedDateTimeError> {
    use ObservedDateTimeError::Invalid;
    let b = s.as_bytes();
    let colon = b.get(2) == Some(&b':');
    let mut pos = 0;
    let mut fields = [0; 3];
    let mut count = 0;
    for (i, field) in fields.iter_mut().enumerate() {
        *field = digits(b, pos, pos + 2)?;
        pos += 2;
        count += 1;
        if pos == b.len() {
            break;
        }
        if matches!(b.get(pos), Some(b'.' | b',')) {
            break;
        }
        if i == 2 {
            return Err(Invalid);
        }
        if colon {
            if b.get(pos) != Some(&b':') {
                return Err(Invalid);
            }
            pos += 1;
        }
    }
    let mut micros = 0;
    if pos != b.len() {
        // Python 3.14 only permits fractional *seconds*, not fractional
        // hour/minute forms. The fraction must contain ASCII digits.
        if count != 3 || !matches!(b.get(pos), Some(b'.' | b',')) {
            return Err(Invalid);
        }
        pos += 1;
        let fraction = b.get(pos..).ok_or(Invalid)?;
        if fraction.is_empty() || !fraction.iter().all(u8::is_ascii_digit) {
            return Err(Invalid);
        }
        for i in 0..6 {
            micros = micros * 10 + fraction.get(i).map_or(0, |c| i64::from(c - b'0'));
        }
    }
    Ok((fields[0], fields[1], fields[2], micros))
}
