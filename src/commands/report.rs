use std::collections::HashMap;

use anyhow::{Result, bail};
use chrono::{Datelike, Days, Duration, Local};
use colored::Colorize;

use super::{in_project_color, project_map};
use crate::config::Ctx;
use crate::output;
use crate::time::{day_range, fmt_duration, parse_date};

pub struct Args {
    pub month: bool,
    pub from: Option<String>,
    pub to: Option<String>,
    pub json: bool,
}

const BAR_WIDTH: usize = 24;

pub fn run(ctx: &Ctx, args: Args) -> Result<()> {
    let today = Local::now().date_naive();
    let (from, to) = match (&args.from, &args.to) {
        (Some(f), t) => (
            parse_date(f)?,
            t.as_deref().map(parse_date).transpose()?.unwrap_or(today),
        ),
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
        if args.json {
            output::print(&serde_json::json!({
                "from": from.to_string(),
                "to": to.to_string(),
                "total_seconds": 0,
                "projects": [],
            }));
        } else {
            println!("No time entries between {from} and {to}.");
        }
        return Ok(());
    }

    let projects = project_map(ctx)?;
    let mut per_project: HashMap<Option<String>, Duration> = HashMap::new();
    let mut total = Duration::zero();
    for e in &entries {
        *per_project
            .entry(e.project_id.clone())
            .or_insert_with(Duration::zero) += e.duration();
        total += e.duration();
    }

    let mut rows: Vec<(Option<String>, Duration)> = per_project.into_iter().collect();
    rows.sort_by_key(|(_, d)| -d.num_seconds());

    if args.json {
        let list: Vec<_> = rows
            .iter()
            .map(|(id, dur)| {
                let project = id.as_deref().and_then(|pid| projects.get(pid));
                serde_json::json!({
                    "id": id,
                    "name": project.map(|p| p.name.clone()),
                    "duration_seconds": dur.num_seconds(),
                    "percent": 100.0 * dur.num_seconds() as f64
                        / total.num_seconds().max(1) as f64,
                })
            })
            .collect();
        output::print(&serde_json::json!({
            "from": from.to_string(),
            "to": to.to_string(),
            "total_seconds": total.num_seconds(),
            "projects": list,
        }));
        return Ok(());
    }

    let max_secs = rows.first().map_or(0, |(_, d)| d.num_seconds()).max(1);

    let name_of = |id: &Option<String>| -> String {
        id.as_deref()
            .map(|id| {
                projects
                    .get(id)
                    .map_or_else(|| id.to_string(), |p| p.name.clone())
            })
            .unwrap_or_else(|| "(no project)".to_string())
    };
    let name_w = rows
        .iter()
        .map(|(id, _)| name_of(id).chars().count())
        .max()
        .unwrap_or(0);
    let dur_w = rows
        .iter()
        .map(|(_, d)| fmt_duration(*d).len())
        .max()
        .unwrap_or(0);

    println!(
        "{}  {}",
        format!("Report {from} – {to}").bold(),
        format!("· {}", fmt_duration(total)).yellow()
    );
    for (id, dur) in &rows {
        let project = id.as_deref().and_then(|id| projects.get(id));
        let name = format!("{:<name_w$}", name_of(id));
        let name = if project.is_some() {
            in_project_color(&name, project)
        } else {
            name.dimmed()
        };
        let share = 100.0 * dur.num_seconds() as f64 / total.num_seconds().max(1) as f64;
        let bar_len = ((dur.num_seconds() as f64 / max_secs as f64) * BAR_WIDTH as f64)
            .round()
            .max(1.0) as usize;
        let bar = "█".repeat(bar_len);
        let bar = if project.is_some() {
            in_project_color(&bar, project)
        } else {
            bar.dimmed()
        };
        println!(
            "  {name}  {:>dur_w$}  {:>4}  {bar}",
            fmt_duration(*dur).bold(),
            format!("{share:.0}%").yellow(),
        );
    }
    println!();
    println!(
        "{} entries, total {}",
        entries.len(),
        fmt_duration(total).bold()
    );
    Ok(())
}
