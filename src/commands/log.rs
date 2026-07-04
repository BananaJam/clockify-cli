use anyhow::{Result, bail};
use chrono::{Datelike, Days, Duration, Local};
use colored::Colorize;

use super::{project_names, table};
use crate::config::Ctx;
use crate::time::{day_range, fmt_duration, fmt_local_date, fmt_local_time, parse_date};

pub struct Args {
    pub week: bool,
    pub from: Option<String>,
    pub to: Option<String>,
    pub limit: Option<usize>,
}

pub fn run(ctx: &Ctx, args: Args) -> Result<()> {
    let today = Local::now().date_naive();
    let (from, to) = match (&args.from, &args.to) {
        (Some(f), t) => (parse_date(f)?, t.as_deref().map(parse_date).transpose()?.unwrap_or(today)),
        (None, Some(_)) => bail!("--to requires --from"),
        (None, None) if args.week => {
            let monday = today - Days::new(today.weekday().num_days_from_monday() as u64);
            (monday, today)
        }
        (None, None) => (today, today),
    };
    if from > to {
        bail!("--from must not be after --to");
    }

    let (start, end) = day_range(from, to)?;
    let mut entries = ctx
        .client
        .time_entries(&ctx.workspace_id, &ctx.user_id, start, end, args.limit)?;
    if entries.is_empty() {
        println!("No time entries between {from} and {to}.");
        return Ok(());
    }
    // The API returns newest first; show oldest first.
    entries.reverse();

    let names = project_names(ctx)?;
    let mut t = table(&["Date", "Start", "End", "Duration", "Project", "Description", "ID"]);
    let mut total = Duration::zero();
    for e in &entries {
        total += e.duration();
        let end_str = match e.time_interval.end {
            Some(end) => fmt_local_time(end),
            None => "▶ running".to_string(),
        };
        let project = e
            .project_id
            .as_deref()
            .map(|id| names.get(id).cloned().unwrap_or_else(|| id.to_string()))
            .unwrap_or_default();
        t.add_row(vec![
            fmt_local_date(e.time_interval.start),
            fmt_local_time(e.time_interval.start),
            end_str,
            fmt_duration(e.duration()),
            project,
            e.description.clone(),
            e.id.clone(),
        ]);
    }
    println!("{t}");
    println!(
        "{} entries, total {}",
        entries.len(),
        fmt_duration(total).bold()
    );
    Ok(())
}
