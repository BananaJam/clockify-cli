use std::io::IsTerminal;

use anyhow::{Result, bail};
use colored::Colorize;
use dialoguer::Confirm;
use dialoguer::theme::ColorfulTheme;

use crate::config::Ctx;
use crate::output;
use crate::resolve;
use crate::time::{fmt_duration, fmt_local_date};

pub fn run(ctx: &Ctx, id: &str, yes: bool, json: bool) -> Result<()> {
    let entry = resolve::entry(ctx, id)?;
    let desc = if entry.description.is_empty() {
        "(no description)".to_string()
    } else {
        entry.description.clone()
    };

    if !yes {
        if !std::io::stdin().is_terminal() {
            bail!("refusing to prompt for confirmation without a terminal — pass -y/--yes");
        }
        let confirmed = Confirm::with_theme(&ColorfulTheme::default())
            .with_prompt(format!(
                "Delete \"{desc}\" from {} ({})?",
                fmt_local_date(entry.time_interval.start),
                fmt_duration(entry.duration())
            ))
            .default(false)
            .interact()?;
        if !confirmed {
            if json {
                output::print(&serde_json::Value::Null);
            } else {
                println!("Aborted.");
            }
            return Ok(());
        }
    }

    ctx.client.delete_time_entry(&ctx.workspace_id, &entry.id)?;
    if json {
        output::print(&serde_json::json!({ "deleted": entry.id }));
    } else {
        println!("{} Deleted \"{desc}\"", "✓".green().bold());
    }
    Ok(())
}
