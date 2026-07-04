use anyhow::Result;
use chrono::Utc;
use colored::Colorize;

use super::short_id;
use crate::config::{Config, Ctx};
use crate::status_cache::{self, CachedEntry, CachedStatus, TTL_SECS};
use crate::time::{fmt_duration_secs, fmt_local_time};

/// One compact line for shell prompts (starship): "▶ 1h23m description".
/// Prints nothing when no timer runs and swallows every error — a broken
/// setup must never break the prompt. Served from the status cache when
/// fresh so prompts don't pay for 1Password/API round-trips.
pub fn short() {
    let _ = short_inner();
}

fn short_inner() -> Result<()> {
    let cfg = Config::load()?;
    let Some(workspace_id) = cfg.workspace_id.clone() else { return Ok(()) };

    let now = status_cache::now_unix();
    if let Some(cached) = status_cache::load()
        && cached.workspace_id == workspace_id
        && now.saturating_sub(cached.fetched_at) <= TTL_SECS
    {
        print_short(cached.entry.as_ref());
        return Ok(());
    }

    let ctx = Ctx::load()?;
    let running = ctx.client.running_entry(&ctx.workspace_id, &ctx.user_id)?;
    let entry = running.map(|e| CachedEntry {
        description: e.description.clone(),
        start: e.time_interval.start,
        project_name: e
            .project_id
            .as_deref()
            .and_then(|id| ctx.client.project(&ctx.workspace_id, id).ok())
            .map(|p| p.name),
    });
    status_cache::save(&CachedStatus { fetched_at: now, workspace_id, entry: entry.clone() });
    print_short(entry.as_ref());
    Ok(())
}

fn print_short(entry: Option<&CachedEntry>) {
    let Some(entry) = entry else { return };
    let mins = (Utc::now() - entry.start).num_minutes().max(0);
    let elapsed =
        if mins >= 60 { format!("{}h{:02}m", mins / 60, mins % 60) } else { format!("{mins}m") };
    let what = if !entry.description.is_empty() {
        entry.description.clone()
    } else {
        entry.project_name.clone().unwrap_or_default()
    };
    let what: String = if what.chars().count() > 24 {
        format!("{}…", what.chars().take(23).collect::<String>())
    } else {
        what
    };
    if what.is_empty() {
        println!("▶ {elapsed}");
    } else {
        println!("▶ {elapsed} {what}");
    }
}

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
