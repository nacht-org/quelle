use chrono::{NaiveDate, NaiveDateTime, NaiveTime};
use parse_datetime::parse_datetime_at_date;

use crate::modules::time::local_time;

/// Parses a string into a **`NaiveDateTime`**. This function first attempts
/// to interpret the string as a specific date using the provided `fmt` format.
/// If that fails, it then tries to parse it as a relative time expression (e.g., "tomorrow", "last Tuesday").
pub fn parse_date_or_relative_time(s: &str, fmt: &str) -> Result<NaiveDateTime, eyre::Report> {
    let result = NaiveDate::parse_from_str(s, fmt).map(|date| date.and_time(NaiveTime::MIN));
    if let Ok(value) = result {
        return Ok(value);
    }

    let now = local_time()?;
    let result = parse_datetime_at_date(now, s).map(|v| v.naive_local());
    result.map_err(|e| {
        eyre::eyre!("Failed to parse date with format({fmt}) or relative time: {s}: {e}")
    })
}

/// Parses a string into a **`NaiveDateTime`**. It prioritizes parsing the
/// string as a specific date and time using the provided `fmt` format. If
/// that attempt is unsuccessful, it then falls back to interpreting the string
/// as a relative time expression (e.g., "now", "in 5 minutes").
pub fn parse_date_time_or_relative_time(s: &str, fmt: &str) -> Result<NaiveDateTime, eyre::Report> {
    let result = NaiveDateTime::parse_from_str(s, fmt);
    if let Ok(value) = result {
        return Ok(value);
    }

    let now = local_time()?;
    let result = parse_datetime_at_date(now, s).map(|v| v.naive_local());
    result.map_err(|e| {
        eyre::eyre!("Failed to parse date time with format({fmt}) or relative time: {s}: {e}")
    })
}

/// Parses a string as a relative time expression (e.g., "tomorrow", "last Tuesday").
/// This function uses the current local time as a reference point for parsing.
pub fn parse_relative_time(s: &str) -> Result<NaiveDateTime, eyre::Report> {
    let now = local_time()?;
    let result = parse_datetime_at_date(now, s).map(|v| v.naive_local());
    result.map_err(|e| eyre::eyre!("Failed to parse relative time: {s}: {e}"))
}
