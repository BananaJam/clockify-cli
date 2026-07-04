use anyhow::{Result, bail};
use colored::Colorize;
use serde_json::json;

use crate::config::Ctx;
use crate::resolve;
use crate::time::{fmt_duration, parse_time, to_api};

pub struct Args {
    pub id: String,
    pub description: Option<String>,
    pub project: Option<String>,
    pub from: Option<String>,
    pub to: Option<String>,
}

pub fn run(ctx: &Ctx, args: Args) -> Result<()> {
    if args.description.is_none() && args.project.is_none() && args.from.is_none() && args.to.is_none() {
        bail!("nothing to change — pass at least one of --description, --project, --from, --to");
    }

    let existing = ctx.client.time_entry(&ctx.workspace_id, &args.id)?;

    let start = match &args.from {
        Some(s) => parse_time(s)?,
        None => existing.time_interval.start,
    };
    let end = match &args.to {
        Some(s) => Some(parse_time(s)?),
        None => existing.time_interval.end,
    };
    if let Some(end) = end
        && end <= start
    {
        bail!("the entry would end before it starts");
    }

    let project_id = match &args.project {
        Some(p) => Some(resolve::project(ctx, p)?.id),
        None => existing.project_id.clone(),
    };
    let description = args.description.as_ref().unwrap_or(&existing.description);

    // PUT replaces the entry, so send the merged state of every field.
    let mut body = json!({
        "start": to_api(start),
        "description": description,
        "billable": existing.billable,
    });
    if let Some(end) = end {
        body["end"] = json!(to_api(end));
    }
    if let Some(pid) = &project_id {
        body["projectId"] = json!(pid);
    }
    // Keep the task only if the project didn't change.
    if project_id == existing.project_id
        && let Some(tid) = &existing.task_id
    {
        body["taskId"] = json!(tid);
    }

    let updated = ctx.client.update_time_entry(&ctx.workspace_id, &args.id, &body)?;
    println!(
        "{} Updated {} ({})",
        "✓".green().bold(),
        if updated.description.is_empty() {
            "(no description)".dimmed().to_string()
        } else {
            updated.description.bold().to_string()
        },
        fmt_duration(updated.duration())
    );
    Ok(())
}
