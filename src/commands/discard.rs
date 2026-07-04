use anyhow::Result;
use colored::Colorize;
use dialoguer::Confirm;
use dialoguer::theme::ColorfulTheme;

use crate::config::Ctx;
use crate::time::fmt_duration_secs;

pub fn run(ctx: &Ctx, yes: bool) -> Result<()> {
    let Some(entry) = ctx.client.running_entry(&ctx.workspace_id, &ctx.user_id)? else {
        println!("No timer is running.");
        return Ok(());
    };

    let desc = if entry.description.is_empty() {
        "(no description)".to_string()
    } else {
        entry.description.clone()
    };

    if !yes {
        let confirmed = Confirm::with_theme(&ColorfulTheme::default())
            .with_prompt(format!(
                "Discard the running timer \"{desc}\" ({} elapsed)? The time will not be saved",
                fmt_duration_secs(entry.duration())
            ))
            .default(false)
            .interact()?;
        if !confirmed {
            println!("Aborted — the timer keeps running.");
            return Ok(());
        }
    }

    ctx.client.delete_time_entry(&ctx.workspace_id, &entry.id)?;
    println!(
        "{} Discarded \"{desc}\" ({} not saved)",
        "✗".red().bold(),
        fmt_duration_secs(entry.duration())
    );
    Ok(())
}
