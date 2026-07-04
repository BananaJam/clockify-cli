use std::collections::HashMap;

use anyhow::{Result, bail};
use chrono::{Datelike, Days, Duration, Local};
use colored::Colorize;

use super::{project_map, table};
use crate::config::Ctx;
use crate::time::{day_range, fmt_duration, parse_date};

pub struct Args {
    pub month: bool,
    pub from: Option<String>,
    pub to: Option<String>,
}

pub fn run(ctx: &Ctx, args: Args) -> Result<()> {
    let today = Local::now().date_naive();
    let (from, to) = match (&args.from, &args.to) {
        (Some(f), t) => (parse_date(f)?, t.as_deref().map(parse_date).transpose()?.unwrap_or(today)),
        (None, Some(_)) => bail!("--to requires --from"),
        (None, None) if args.month => (today.with_day(1).unwrap(), today),
        // Default to the current week.
        (None, None) => {
            let monday = today - Days::new(today.weekday().num_days_from_monday() as u64);
            (monday, today)
        }
    };
    if from > to {
        bail!("--from must not be after --to");
    }

    let (start, end) = day_range(from, to)?;
    let entries = ctx
        .client
        .time_entries(&ctx.workspace_id, &ctx.user_id, start, end, None)?;
    if entries.is_empty() {
        println!("No time entries between {from} and {to}.");
        return Ok(());
    }

    let projects = project_map(ctx)?;
    let mut per_project: HashMap<String, Duration> = HashMap::new();
    let mut total = Duration::zero();
    for e in &entries {
        let key = e
            .project_id
            .as_deref()
            .map(|id| projects.get(id).map_or_else(|| id.to_string(), |p| p.name.clone()))
            .unwrap_or_else(|| "(no project)".to_string());
        *per_project.entry(key).or_insert_with(Duration::zero) += e.duration();
        total += e.duration();
    }

    let mut rows: Vec<(String, Duration)> = per_project.into_iter().collect();
    rows.sort_by_key(|(_, d)| -d.num_seconds());

    println!("Report {from} – {to}");
    let mut t = table(&["Project", "Time", "Share"]);
    for (project, dur) in &rows {
        let share = if total.num_seconds() > 0 {
            format!("{:.0}%", 100.0 * dur.num_seconds() as f64 / total.num_seconds() as f64)
        } else {
            String::new()
        };
        t.add_row(vec![project.clone(), fmt_duration(*dur), share]);
    }
    println!("{t}");
    println!("Total: {} across {} entries", fmt_duration(total).bold(), entries.len());
    Ok(())
}
