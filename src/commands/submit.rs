use std::fmt;
use std::io::IsTerminal;

use anyhow::{Context, Result, bail};
use chrono::{DateTime, Datelike, Days, Duration, Local, NaiveDate, Utc};
use clap::ValueEnum;
use colored::Colorize;
use dialoguer::Confirm;
use dialoguer::theme::ColorfulTheme;
use serde_json::json;

use crate::config::Ctx;
use crate::models::{ApprovalRequest, TimeEntry};
use crate::output;
use crate::time::{day_range, fmt_duration, parse_date};

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
#[clap(rename_all = "kebab-case")]
pub enum Period {
    Weekly,
    SemiMonthly,
    Monthly,
}

impl Period {
    pub fn as_api_str(self) -> &'static str {
        match self {
            Period::Weekly => "WEEKLY",
            Period::SemiMonthly => "SEMI_MONTHLY",
            Period::Monthly => "MONTHLY",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Period::Weekly => "weekly",
            Period::SemiMonthly => "semi-monthly",
            Period::Monthly => "monthly",
        }
    }
}

impl fmt::Display for Period {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

pub struct Args {
    pub week: bool,
    pub month: bool,
    pub from: Option<String>,
    pub period: Option<Period>,
    pub resubmit: bool,
    pub yes: bool,
    pub json: bool,
}

#[derive(Debug, Clone)]
pub struct PeriodWindow {
    pub period: Period,
    pub from: NaiveDate,
    pub to: NaiveDate,
}

#[derive(Debug, Clone)]
pub struct SubmissionSummary {
    pub window: PeriodWindow,
    pub entry_count: usize,
    pub total: Duration,
    pub period_start: DateTime<Utc>,
}

pub fn run(ctx: &Ctx, args: Args) -> Result<()> {
    let window = select_window(args.week, args.month, args.from.as_deref(), args.period)?;
    let summary = summarize(ctx, window)?;

    if !args.yes {
        if !std::io::stdin().is_terminal() {
            bail!("refusing to prompt for confirmation without a terminal — pass -y/--yes");
        }
        let action = if args.resubmit { "Resubmit" } else { "Submit" };
        let confirmed = Confirm::with_theme(&ColorfulTheme::default())
            .with_prompt(format!(
                "{action} {} approval for {} – {} ({} entries, {})?",
                summary.window.period,
                summary.window.from,
                summary.window.to,
                summary.entry_count,
                fmt_duration(summary.total)
            ))
            .default(false)
            .interact()?;
        if !confirmed {
            if args.json {
                output::print(&serde_json::Value::Null);
            } else {
                println!("Aborted.");
            }
            return Ok(());
        }
    }

    let approval = submit_summary(ctx, &summary, args.resubmit)?;
    print_result(&summary, &approval, args.resubmit, args.json);
    Ok(())
}

pub fn select_window(
    week: bool,
    month: bool,
    from: Option<&str>,
    period: Option<Period>,
) -> Result<PeriodWindow> {
    let today = Local::now().date_naive();
    match (week, month, from, period) {
        (true, false, None, None) => Ok(window_for(Period::Weekly, week_start(today)?)),
        (false, true, None, None) => Ok(window_for(Period::Monthly, month_start(today)?)),
        (false, false, None, None) => Ok(window_for(Period::Monthly, month_start(today)?)),
        (false, false, Some(from), Some(period)) => Ok(window_for(period, parse_date(from)?)),
        (false, false, None, Some(period)) => {
            let start = match period {
                Period::Weekly => week_start(today)?,
                Period::SemiMonthly => semi_month_start(today)?,
                Period::Monthly => month_start(today)?,
            };
            Ok(window_for(period, start))
        }
        (false, false, Some(_), None) => bail!("--from requires --period"),
        (true, false, None, Some(Period::Weekly)) => {
            Ok(window_for(Period::Weekly, week_start(today)?))
        }
        (false, true, None, Some(Period::Monthly)) => {
            Ok(window_for(Period::Monthly, month_start(today)?))
        }
        _ => bail!("choose only one period: --week, --month, or --from with --period"),
    }
}

pub fn window_for(period: Period, from: NaiveDate) -> PeriodWindow {
    let to = match period {
        Period::Weekly => from + Duration::days(6),
        Period::SemiMonthly if from.day() <= 15 => from.with_day(15).unwrap_or(from),
        Period::SemiMonthly => last_day_of_month(from),
        Period::Monthly => last_day_of_month(from),
    };
    PeriodWindow { period, from, to }
}

pub fn summarize(ctx: &Ctx, window: PeriodWindow) -> Result<SubmissionSummary> {
    let (start, end) = day_range(window.from, window.to)?;
    let entries = ctx
        .client
        .time_entries(&ctx.workspace_id, &ctx.user_id, start, end, None)?;
    summarize_entries(window, start, &entries)
}

pub fn summarize_entries(
    window: PeriodWindow,
    period_start: DateTime<Utc>,
    entries: &[TimeEntry],
) -> Result<SubmissionSummary> {
    if entries.is_empty() {
        bail!(
            "no time entries to submit for {} – {}",
            window.from,
            window.to
        );
    }
    if entries
        .iter()
        .any(|entry| entry.time_interval.end.is_none())
    {
        bail!("stop the running timer before submitting this period");
    }
    let total = entries.iter().map(TimeEntry::duration).sum();
    Ok(SubmissionSummary {
        window,
        entry_count: entries.len(),
        total,
        period_start,
    })
}

pub fn submit_summary(
    ctx: &Ctx,
    summary: &SubmissionSummary,
    resubmit: bool,
) -> Result<ApprovalRequest> {
    if resubmit {
        ctx.client.resubmit_approval_request(
            &ctx.workspace_id,
            summary.window.period,
            summary.period_start,
        )
    } else {
        ctx.client.submit_approval_request(
            &ctx.workspace_id,
            summary.window.period,
            summary.period_start,
        )
    }
}

fn print_result(
    summary: &SubmissionSummary,
    approval: &ApprovalRequest,
    resubmit: bool,
    as_json: bool,
) {
    let state = approval
        .status
        .as_ref()
        .map(|s| s.state.as_str())
        .unwrap_or("submitted");
    if as_json {
        let status = approval.status.as_ref();
        output::print(&json!({
            "id": approval.id,
            "state": state,
            "note": status.and_then(|s| s.note.as_deref()),
            "updated_at": status.and_then(|s| s.updated_at.map(|dt| dt.to_rfc3339())),
            "period": summary.window.period.as_api_str(),
            "from": summary.window.from.to_string(),
            "to": summary.window.to.to_string(),
            "entry_count": summary.entry_count,
            "total_seconds": summary.total.num_seconds(),
            "resubmitted": resubmit,
            "workspace_id": approval.workspace_id,
            "date_range": approval.date_range.as_ref().map(|range| json!({
                "start": range.start.to_rfc3339(),
                "end": range.end.to_rfc3339(),
            })),
        }));
        return;
    }

    let verb = if resubmit { "Resubmitted" } else { "Submitted" };
    println!(
        "{} {verb} {} approval for {} – {} ({} entries, {})",
        "✓".green().bold(),
        summary.window.period,
        summary.window.from,
        summary.window.to,
        summary.entry_count,
        fmt_duration(summary.total).bold(),
    );
    println!("  request {} · {}", approval.id.yellow(), state);
}

pub fn week_start(date: NaiveDate) -> Result<NaiveDate> {
    date.checked_sub_days(Days::new(date.weekday().num_days_from_monday() as u64))
        .context("date out of range")
}

pub fn month_start(date: NaiveDate) -> Result<NaiveDate> {
    date.with_day(1).context("date out of range")
}

pub fn semi_month_start(date: NaiveDate) -> Result<NaiveDate> {
    if date.day() <= 15 {
        month_start(date)
    } else {
        date.with_day(16).context("date out of range")
    }
}

fn last_day_of_month(date: NaiveDate) -> NaiveDate {
    let (year, month) = if date.month() == 12 {
        (date.year() + 1, 1)
    } else {
        (date.year(), date.month() + 1)
    };
    NaiveDate::from_ymd_opt(year, month, 1)
        .unwrap()
        .checked_sub_days(Days::new(1))
        .unwrap_or(date)
}

#[cfg(test)]
mod tests {
    use chrono::{TimeZone, Utc};

    use super::*;
    use crate::models::TimeInterval;

    #[test]
    fn monthly_window_starts_on_first_and_ends_on_last_day() {
        let from = NaiveDate::from_ymd_opt(2026, 2, 1).unwrap();
        let window = window_for(Period::Monthly, from);

        assert_eq!(window.from, from);
        assert_eq!(window.to, NaiveDate::from_ymd_opt(2026, 2, 28).unwrap());
    }

    #[test]
    fn weekly_window_covers_seven_days() {
        let from = NaiveDate::from_ymd_opt(2026, 7, 6).unwrap();
        let window = window_for(Period::Weekly, from);

        assert_eq!(window.to, NaiveDate::from_ymd_opt(2026, 7, 12).unwrap());
    }

    #[test]
    fn custom_from_requires_explicit_period() {
        assert!(select_window(false, false, Some("2026-07-01"), None).is_err());
    }

    #[test]
    fn custom_from_with_period_is_accepted() {
        let window =
            select_window(false, false, Some("2026-07-01"), Some(Period::Monthly)).unwrap();

        assert_eq!(window.period, Period::Monthly);
        assert_eq!(window.from, NaiveDate::from_ymd_opt(2026, 7, 1).unwrap());
    }

    #[test]
    fn summary_rejects_running_entries() {
        let start = Utc.with_ymd_and_hms(2026, 7, 1, 9, 0, 0).unwrap();
        let entries = vec![TimeEntry {
            id: "entry".into(),
            description: "work".into(),
            project_id: None,
            task_id: None,
            time_interval: TimeInterval { start, end: None },
        }];
        let window = window_for(
            Period::Monthly,
            NaiveDate::from_ymd_opt(2026, 7, 1).unwrap(),
        );

        assert!(summarize_entries(window, start, &entries).is_err());
    }
}
