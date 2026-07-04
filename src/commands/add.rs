use anyhow::{Result, bail};
use colored::Colorize;
use serde_json::json;

use crate::config::Ctx;
use crate::resolve;
use crate::time::{fmt_duration, fmt_local_date, fmt_local_time, parse_time, to_api};

pub struct Args {
    pub description: String,
    pub from: String,
    pub to: String,
    pub project: Option<String>,
    pub no_project: bool,
    pub task: Option<String>,
    pub billable: bool,
}

pub fn run(ctx: &Ctx, args: Args) -> Result<()> {
    let start = parse_time(&args.from)?;
    let end = parse_time(&args.to)?;
    if end <= start {
        bail!("--to must be after --from");
    }

    let project = match (&args.project, args.no_project) {
        (Some(p), _) => Some(resolve::project(ctx, p)?),
        (None, true) => None,
        (None, false) => resolve::default_project(ctx)?,
    };
    let task = match (&args.task, &project) {
        (Some(t), Some(p)) => Some(resolve::task(ctx, &p.id, t)?),
        (Some(_), None) => bail!("--task requires --project"),
        _ => None,
    };

    let mut body = json!({
        "start": to_api(start),
        "end": to_api(end),
        "description": args.description,
        "billable": args.billable,
    });
    if let Some(p) = &project {
        body["projectId"] = json!(p.id);
    }
    if let Some(t) = &task {
        body["taskId"] = json!(t.id);
    }

    let entry = ctx.client.create_time_entry(&ctx.workspace_id, &body)?;
    let mut what = entry.description.bold().to_string();
    if let Some(p) = &project {
        what.push_str(&format!(" [{}]", p.name.blue()));
        if let Some(t) = &task {
            what.push_str(&format!(" / {}", t.name));
        }
    }
    println!(
        "{} Added {} on {} ({} – {}, {})",
        "✓".green().bold(),
        what,
        fmt_local_date(entry.time_interval.start),
        fmt_local_time(entry.time_interval.start),
        fmt_local_time(entry.time_interval.end.unwrap_or(end)),
        fmt_duration(entry.duration()).bold()
    );
    Ok(())
}
