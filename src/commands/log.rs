use anyhow::{Result, bail};
use chrono::{Datelike, Days, Duration, Local, NaiveDate};
use colored::Colorize;

use super::{in_project_color, project_map, styled_id};
use crate::config::Ctx;
use crate::models::TimeEntry;
use crate::output;
use crate::resolve;
use crate::time::{day_range, fmt_duration, fmt_local_time, parse_date};

pub struct Args {
    pub week: bool,
    pub from: Option<String>,
    pub to: Option<String>,
    pub limit: Option<usize>,
    pub json: bool,
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
        if args.json {
            output::print(&serde_json::json!([]));
        } else {
            println!("No time entries between {from} and {to}.");
        }
        return Ok(());
    }
    // The API returns newest first; show oldest first.
    entries.reverse();

    let projects = project_map(ctx)?;
    let project_of = |e: &TimeEntry| e.project_id.as_deref().and_then(|id| projects.get(id));

    if args.json {
        let list: Vec<_> = entries.iter().map(|e| output::entry_json(e, project_of(e))).collect();
        output::print(&serde_json::Value::Array(list));
        return Ok(());
    }

    // Shortest-unique-suffix lengths, computed against the same 90-day set
    // that suffix resolution searches, so a highlighted suffix always works.
    // Displayed entries are included too in case the range is older.
    let mut candidate_ids: Vec<String> =
        resolve::lookback_entries(ctx)?.into_iter().map(|e| e.id).collect();
    candidate_ids.extend(entries.iter().map(|e| e.id.clone()));
    candidate_ids.sort();
    candidate_ids.dedup();
    let suffix_lens = resolve::unique_suffix_lens(candidate_ids.iter().map(String::as_str));
    let id_of = |e: &TimeEntry| styled_id(&e.id, suffix_lens.get(&e.id).copied().unwrap_or(6));

    // Column widths (plain text lengths, before coloring).
    let dur_w = entries.iter().map(|e| fmt_duration(e.duration()).len()).max().unwrap_or(0);
    let proj_w = entries
        .iter()
        .map(|e| project_of(e).map_or(0, |p| p.name.chars().count()))
        .max()
        .unwrap_or(0);
    let desc_w = entries
        .iter()
        .map(|e| e.description.chars().count().max("(no description)".len()))
        .max()
        .unwrap_or(0)
        .min(50);

    let mut days: Vec<(NaiveDate, Vec<&TimeEntry>)> = Vec::new();
    for e in &entries {
        let date = e.time_interval.start.with_timezone(&Local).date_naive();
        match days.last_mut() {
            Some((d, group)) if *d == date => group.push(e),
            _ => days.push((date, vec![e])),
        }
    }

    let mut total = Duration::zero();
    for (date, group) in &days {
        let day_total: Duration = group.iter().map(|e| e.duration()).sum();
        total += day_total;
        println!(
            "{}  {}",
            fmt_day(*date, today).bold(),
            format!("· {}", fmt_duration(day_total)).yellow()
        );
        for e in group {
            let times = format!(
                "{}–{}",
                fmt_local_time(e.time_interval.start),
                match e.time_interval.end {
                    Some(end) => fmt_local_time(end).normal(),
                    None => "now  ".green().bold(),
                }
            );
            let duration = format!("{:>dur_w$}", fmt_duration(e.duration()));
            let duration =
                if e.time_interval.end.is_none() { duration.green().bold() } else { duration.bold() };
            let project_name = project_of(e).map(|p| p.name.as_str()).unwrap_or("");
            let project = in_project_color(&format!("{project_name:<proj_w$}"), project_of(e));
            let desc = if e.description.is_empty() {
                format!("{:<desc_w$}", "(no description)").dimmed()
            } else {
                format!("{:<desc_w$}", e.description).normal()
            };
            println!("  {times}  {duration}  {project}  {desc}  {}", id_of(e));
        }
        println!();
    }
    println!("{} entries, total {}", entries.len(), fmt_duration(total).bold());
    Ok(())
}

fn fmt_day(date: NaiveDate, today: NaiveDate) -> String {
    let label = match today.signed_duration_since(date).num_days() {
        0 => "Today · ",
        1 => "Yesterday · ",
        _ => "",
    };
    let year = if date.year() == today.year() {
        String::new()
    } else {
        format!(" {}", date.year())
    };
    format!("{label}{}{year}", date.format("%A, %-d %B"))
}
