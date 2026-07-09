use anyhow::{Result, bail};
use colored::Colorize;
use serde_json::json;

use crate::config::Ctx;
use crate::output;
use crate::resolve;
use crate::time::{fmt_duration, parse_time, to_api};

pub struct Args {
    pub id: String,
    pub description: Option<String>,
    pub project: Option<String>,
    pub from: Option<String>,
    pub to: Option<String>,
    pub json: bool,
}

pub fn run(ctx: &Ctx, args: Args) -> Result<()> {
    if args.description.is_none()
        && args.project.is_none()
        && args.from.is_none()
        && args.to.is_none()
    {
        bail!("nothing to change — pass at least one of --description, --project, --from, --to");
    }

    let existing = resolve::entry(ctx, &args.id)?;

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

    let new_project = args
        .project
        .as_deref()
        .map(|p| resolve::project(ctx, p))
        .transpose()?;
    let project_id = match &new_project {
        Some(p) => Some(p.id.clone()),
        None => existing.project_id.clone(),
    };
    let description = args.description.as_ref().unwrap_or(&existing.description);

    // PUT replaces the entry, so send the merged state of every field —
    // except `billable`: Clockify resets it to the project default on a project
    // change and rejects any other value when the user can't override
    // billability, so send the new project's default then and omit it
    // otherwise (omitted, it stays as-is).
    let mut body = json!({
        "start": to_api(start),
        "description": description,
    });
    if let Some(p) = &new_project
        && Some(&p.id) != existing.project_id.as_ref()
    {
        body["billable"] = json!(p.billable);
    }
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

    let updated = ctx
        .client
        .update_time_entry(&ctx.workspace_id, &existing.id, &body)?;

    if args.json {
        let project = match &updated.project_id {
            Some(id) => match &new_project {
                Some(p) if p.id == *id => Some(p.clone()),
                _ => Some(ctx.client.project(&ctx.workspace_id, id)?),
            },
            None => None,
        };
        output::print(&output::entry_json(&updated, project.as_ref()));
        return Ok(());
    }

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
