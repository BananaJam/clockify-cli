use anyhow::Result;
use colored::Colorize;
use dialoguer::Confirm;
use dialoguer::theme::ColorfulTheme;

use crate::config::Ctx;
use crate::resolve;
use crate::time::{fmt_duration, fmt_local_date};

pub fn run(ctx: &Ctx, id: &str, yes: bool) -> Result<()> {
    let entry = resolve::entry(ctx, id)?;
    let desc = if entry.description.is_empty() {
        "(no description)".to_string()
    } else {
        entry.description.clone()
    };

    if !yes {
        let confirmed = Confirm::with_theme(&ColorfulTheme::default())
            .with_prompt(format!(
                "Delete \"{desc}\" from {} ({})?",
                fmt_local_date(entry.time_interval.start),
                fmt_duration(entry.duration())
            ))
            .default(false)
            .interact()?;
        if !confirmed {
            println!("Aborted.");
            return Ok(());
        }
    }

    ctx.client.delete_time_entry(&ctx.workspace_id, &entry.id)?;
    println!("{} Deleted \"{desc}\"", "✓".green().bold());
    Ok(())
}
