use anyhow::Result;
use colored::Colorize;

use super::short_id;
use crate::config::Ctx;
use crate::time::{fmt_duration_secs, fmt_local_time};

pub fn run(ctx: &Ctx) -> Result<()> {
    let Some(entry) = ctx.client.running_entry(&ctx.workspace_id, &ctx.user_id)? else {
        println!("No timer is running. Start one with {}.", "clockify start".cyan());
        return Ok(());
    };

    let desc = if entry.description.is_empty() {
        "(no description)".dimmed().to_string()
    } else {
        entry.description.bold().to_string()
    };
    let project = entry
        .project_id
        .as_deref()
        .map(|id| ctx.client.project(&ctx.workspace_id, id))
        .transpose()?;

    println!("{} {}", "▶ Running:".green().bold(), desc);
    if let Some(p) = project {
        println!("  Project:  {}", p.name.blue());
    }
    println!("  Started:  {}", fmt_local_time(entry.time_interval.start));
    println!("  Elapsed:  {}", fmt_duration_secs(entry.duration()).bold());
    println!("  Entry ID: {}", short_id(&entry.id));
    Ok(())
}
