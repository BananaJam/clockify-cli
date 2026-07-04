use anyhow::{Result, bail};
use chrono::Utc;
use colored::Colorize;
use serde_json::json;

use crate::config::Ctx;
use crate::resolve;
use crate::time::{fmt_duration, fmt_local_time, parse_time, to_api};

pub struct Args {
    pub description: Option<String>,
    pub project: Option<String>,
    pub task: Option<String>,
    pub billable: bool,
    pub at: Option<String>,
}

pub fn run(ctx: &Ctx, args: Args) -> Result<()> {
    let start = match &args.at {
        Some(s) => parse_time(s)?,
        None => Utc::now(),
    };

    let project = args.project.as_deref().map(|p| resolve::project(ctx, p)).transpose()?;
    let task = match (&args.task, &project) {
        (Some(t), Some(p)) => Some(resolve::task(ctx, &p.id, t)?),
        (Some(_), None) => bail!("--task requires --project"),
        _ => None,
    };

    // Clockify allows overlapping running entries via the API, so mimic the
    // web app: stop anything already running before starting the new timer.
    if let Some(stopped) = ctx.client.stop_timer(&ctx.workspace_id, &ctx.user_id, start)? {
        println!(
            "Stopped previous timer: {} ({})",
            stopped.description.italic(),
            fmt_duration(stopped.duration())
        );
    }

    let mut body = json!({
        "start": to_api(start),
        "billable": args.billable,
    });
    if let Some(d) = &args.description {
        body["description"] = json!(d);
    }
    if let Some(p) = &project {
        body["projectId"] = json!(p.id);
    }
    if let Some(t) = &task {
        body["taskId"] = json!(t.id);
    }

    let entry = ctx.client.create_time_entry(&ctx.workspace_id, &body)?;

    let mut what = if entry.description.is_empty() {
        "(no description)".dimmed().to_string()
    } else {
        entry.description.bold().to_string()
    };
    if let Some(p) = &project {
        what.push_str(&format!(" [{}]", p.name.blue()));
        if let Some(t) = &task {
            what.push_str(&format!(" / {}", t.name));
        }
    }
    println!(
        "{} Timer started at {} — {}",
        "▶".green().bold(),
        fmt_local_time(entry.time_interval.start),
        what
    );
    Ok(())
}
