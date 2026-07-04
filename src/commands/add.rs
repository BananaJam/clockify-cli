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
    pub task: Option<String>,
    pub billable: bool,
}

pub fn run(ctx: &Ctx, args: Args) -> Result<()> {
    let start = parse_time(&args.from)?;
    let end = parse_time(&args.to)?;
    if end <= start {
        bail!("--to must be after --from");
    }

    let project = args.project.as_deref().map(|p| resolve::project(ctx, p)).transpose()?;
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
    println!(
        "{} Added {} on {} ({} – {}, {})",
        "✓".green().bold(),
        entry.description.bold(),
        fmt_local_date(entry.time_interval.start),
        fmt_local_time(entry.time_interval.start),
        fmt_local_time(entry.time_interval.end.unwrap_or(end)),
        fmt_duration(entry.duration()).bold()
    );
    Ok(())
}
