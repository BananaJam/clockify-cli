use anyhow::Result;
use chrono::Utc;
use colored::Colorize;

use crate::config::Ctx;
use crate::output;
use crate::time::{fmt_duration, fmt_local_time, parse_time};

pub fn run(ctx: &Ctx, at: Option<String>, json: bool) -> Result<()> {
    let end = match &at {
        Some(s) => parse_time(s)?,
        None => Utc::now(),
    };
    match ctx
        .client
        .stop_timer(&ctx.workspace_id, &ctx.user_id, end)?
    {
        Some(entry) => {
            if json {
                let project = entry
                    .project_id
                    .as_deref()
                    .map(|id| ctx.client.project(&ctx.workspace_id, id))
                    .transpose()?;
                output::print(&output::entry_json(&entry, project.as_ref()));
                return Ok(());
            }
            let desc = if entry.description.is_empty() {
                "(no description)".dimmed().to_string()
            } else {
                entry.description.bold().to_string()
            };
            println!(
                "{} Timer stopped — {} ({} – {}, {})",
                "■".red().bold(),
                desc,
                fmt_local_time(entry.time_interval.start),
                fmt_local_time(entry.time_interval.end.unwrap_or(end)),
                fmt_duration(entry.duration()).bold()
            );
        }
        None if json => output::print(&serde_json::Value::Null),
        None => println!("No timer is running."),
    }
    Ok(())
}
