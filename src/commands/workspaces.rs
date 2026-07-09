use anyhow::Result;
use colored::Colorize;

use super::styled_id;
use crate::config::{Config, Ctx};
use crate::output;
use crate::resolve;

pub fn list(ctx: &Ctx, json: bool) -> Result<()> {
    let workspaces = ctx.client.workspaces()?;

    if json {
        let list: Vec<_> = workspaces
            .iter()
            .map(|w| {
                serde_json::json!({
                    "id": w.id,
                    "name": w.name,
                    "current": w.id == ctx.workspace_id,
                })
            })
            .collect();
        output::print(&serde_json::Value::Array(list));
        return Ok(());
    }

    let name_w = workspaces
        .iter()
        .map(|w| w.name.chars().count())
        .max()
        .unwrap_or(0);
    let id_lens = resolve::unique_suffix_lens(workspaces.iter().map(|w| w.id.as_str()));
    for w in &workspaces {
        let current = w.id == ctx.workspace_id;
        let marker = if current {
            "✓".green().bold()
        } else {
            " ".normal()
        };
        let name = format!("{:<name_w$}", w.name);
        let name = if current { name.bold() } else { name.normal() };
        println!(
            "{marker} {name}  {}",
            styled_id(&w.id, id_lens.get(&w.id).copied().unwrap_or(6))
        );
    }
    Ok(())
}

pub fn switch(ctx: &Ctx, needle: &str) -> Result<()> {
    let workspaces = ctx.client.workspaces()?;
    let workspace = resolve::pick(
        "workspace",
        "clockify workspaces",
        needle,
        &workspaces,
        |w| &w.id,
        |w| &w.name,
    )?
    .clone();

    let mut cfg = Config::load()?;
    cfg.workspace_id = Some(workspace.id);
    cfg.workspace_name = Some(workspace.name.clone());
    cfg.save()?;
    println!(
        "{} Switched to workspace {}",
        "✓".green().bold(),
        workspace.name.bold()
    );
    Ok(())
}
