use anyhow::{Context, Result, bail};
use chrono::{
    DateTime, Days, Duration, Local, LocalResult, NaiveDate, NaiveDateTime, NaiveTime, TimeZone,
    Utc,
};

/// Format a timestamp the way the Clockify API expects it.
pub fn to_api(dt: DateTime<Utc>) -> String {
    dt.format("%Y-%m-%dT%H:%M:%SZ").to_string()
}

fn local_to_utc(naive: NaiveDateTime) -> Result<DateTime<Utc>> {
    match Local.from_local_datetime(&naive) {
        LocalResult::Single(dt) | LocalResult::Ambiguous(dt, _) => Ok(dt.with_timezone(&Utc)),
        LocalResult::None => bail!("{naive} does not exist in your local timezone (DST gap)"),
    }
}

/// Parse a point in time. Accepts `HH:MM` (today), `yesterday HH:MM`,
/// `YYYY-MM-DD HH:MM`, and full RFC 3339 timestamps. Times are interpreted
/// in the local timezone.
pub fn parse_time(s: &str) -> Result<DateTime<Utc>> {
    let s = s.trim();
    if let Ok(dt) = DateTime::parse_from_rfc3339(s) {
        return Ok(dt.with_timezone(&Utc));
    }
    let today = Local::now().date_naive();
    if let Ok(t) = NaiveTime::parse_from_str(s, "%H:%M") {
        return local_to_utc(today.and_time(t));
    }
    if let Some(rest) = s.strip_prefix("yesterday ")
        && let Ok(t) = NaiveTime::parse_from_str(rest.trim(), "%H:%M")
    {
        let yesterday = today.checked_sub_days(Days::new(1)).context("date out of range")?;
        return local_to_utc(yesterday.and_time(t));
    }
    if let Ok(dt) = NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M") {
        return local_to_utc(dt);
    }
    bail!(
        "could not parse time '{s}' — try HH:MM, 'yesterday HH:MM', 'YYYY-MM-DD HH:MM', or RFC 3339"
    )
}

/// Parse a date: `today`, `yesterday`, or `YYYY-MM-DD`.
pub fn parse_date(s: &str) -> Result<NaiveDate> {
    let s = s.trim();
    let today = Local::now().date_naive();
    match s {
        "today" => Ok(today),
        "yesterday" => today.checked_sub_days(Days::new(1)).context("date out of range"),
        _ => NaiveDate::parse_from_str(s, "%Y-%m-%d")
            .with_context(|| format!("could not parse date '{s}' — try YYYY-MM-DD, 'today', or 'yesterday'")),
    }
}

/// UTC range covering the local days `from`..=`to` (inclusive).
pub fn day_range(from: NaiveDate, to: NaiveDate) -> Result<(DateTime<Utc>, DateTime<Utc>)> {
    let start = local_to_utc(from.and_time(NaiveTime::MIN))?;
    let end_day = to.checked_add_days(Days::new(1)).context("date out of range")?;
    let end = local_to_utc(end_day.and_time(NaiveTime::MIN))?;
    Ok((start, end))
}

/// e.g. "1h 23m"; sub-minute durations show as "0m".
pub fn fmt_duration(d: Duration) -> String {
    let mins = d.num_minutes().max(0);
    let (h, m) = (mins / 60, mins % 60);
    if h > 0 { format!("{h}h {m:02}m") } else { format!("{m}m") }
}

/// e.g. "1h 23m 45s" — used for live elapsed time in `status`.
pub fn fmt_duration_secs(d: Duration) -> String {
    let secs = d.num_seconds().max(0);
    let (h, m, s) = (secs / 3600, (secs % 3600) / 60, secs % 60);
    if h > 0 {
        format!("{h}h {m:02}m {s:02}s")
    } else if m > 0 {
        format!("{m}m {s:02}s")
    } else {
        format!("{s}s")
    }
}

/// Local wall-clock "HH:MM" for display.
pub fn fmt_local_time(dt: DateTime<Utc>) -> String {
    dt.with_timezone(&Local).format("%H:%M").to_string()
}

/// Local date "YYYY-MM-DD" for display.
pub fn fmt_local_date(dt: DateTime<Utc>) -> String {
    dt.with_timezone(&Local).format("%Y-%m-%d").to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_rfc3339() {
        let dt = parse_time("2026-07-04T10:30:00Z").unwrap();
        assert_eq!(to_api(dt), "2026-07-04T10:30:00Z");
    }

    #[test]
    fn parses_hh_mm_as_today_local() {
        let dt = parse_time("10:30").unwrap();
        let local = dt.with_timezone(&Local);
        assert_eq!(local.date_naive(), Local::now().date_naive());
        assert_eq!(local.format("%H:%M").to_string(), "10:30");
    }

    #[test]
    fn parses_yesterday() {
        let dt = parse_time("yesterday 09:15").unwrap();
        let local = dt.with_timezone(&Local);
        let expected = Local::now().date_naive().checked_sub_days(Days::new(1)).unwrap();
        assert_eq!(local.date_naive(), expected);
        assert_eq!(local.format("%H:%M").to_string(), "09:15");
    }

    #[test]
    fn parses_full_date_time() {
        let dt = parse_time("2026-01-15 14:00").unwrap();
        let local = dt.with_timezone(&Local);
        assert_eq!(local.format("%Y-%m-%d %H:%M").to_string(), "2026-01-15 14:00");
    }

    #[test]
    fn rejects_garbage_time() {
        assert!(parse_time("lunchtime").is_err());
    }

    #[test]
    fn parses_dates() {
        assert_eq!(parse_date("today").unwrap(), Local::now().date_naive());
        assert_eq!(
            parse_date("2026-07-01").unwrap(),
            NaiveDate::from_ymd_opt(2026, 7, 1).unwrap()
        );
        assert!(parse_date("someday").is_err());
    }

    #[test]
    fn formats_durations() {
        assert_eq!(fmt_duration(Duration::minutes(83)), "1h 23m");
        assert_eq!(fmt_duration(Duration::minutes(5)), "5m");
        assert_eq!(fmt_duration(Duration::seconds(59)), "0m");
        assert_eq!(fmt_duration_secs(Duration::seconds(3723)), "1h 02m 03s");
        assert_eq!(fmt_duration_secs(Duration::seconds(45)), "45s");
    }

    #[test]
    fn day_range_covers_full_days() {
        let d = NaiveDate::from_ymd_opt(2026, 7, 4).unwrap();
        let (start, end) = day_range(d, d).unwrap();
        assert_eq!(end - start, Duration::days(1));
    }
}
