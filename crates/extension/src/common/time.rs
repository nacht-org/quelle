use chrono::{Local, NaiveDate, NaiveDateTime, NaiveTime};
use parse_datetime::parse_datetime_at_date;

pub fn parse_date_or_relative_time(s: &str, fmt: &str) -> Result<NaiveDateTime, eyre::Report> {
    let result = NaiveDate::parse_from_str(s, fmt).map(|date| date.and_time(NaiveTime::MIN));
    if let Ok(value) = result {
        return Ok(value);
    }

    let now = Local::now();
    let result = parse_datetime_at_date(now, s).map(|v| v.naive_local());
    result.map_err(|e| {
        eyre::eyre!("Failed to parse date with format({fmt}) or relative time: {s}: {e}")
    })
}

pub fn parse_date_time_or_relative_time(s: &str, fmt: &str) -> Result<NaiveDateTime, eyre::Report> {
    let result = NaiveDateTime::parse_from_str(s, fmt);
    if let Ok(value) = result {
        return Ok(value);
    }

    let now = Local::now();
    let result = parse_datetime_at_date(now, s).map(|v| v.naive_local());
    result.map_err(|e| {
        eyre::eyre!("Failed to parse date time with format({fmt}) or relative time: {s}: {e}")
    })
}

pub fn parse_relative_time(s: &str) -> Result<NaiveDateTime, eyre::Report> {
    let now = Local::now();
    let result = parse_datetime_at_date(now, s).map(|v| v.naive_local());
    result.map_err(|e| eyre::eyre!("Failed to parse relative time: {s}: {e}"))
}
