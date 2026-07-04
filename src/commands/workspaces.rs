use anyhow::{Result, bail};
use colored::Colorize;

use crate::config::{Config, Ctx};
use crate::output;

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

    let name_w = workspaces.iter().map(|w| w.name.chars().count()).max().unwrap_or(0);
    for w in &workspaces {
        let current = w.id == ctx.workspace_id;
        let marker = if current { "✓".green().bold() } else { " ".normal() };
        let name = format!("{:<name_w$}", w.name);
        let name = if current { name.bold() } else { name.normal() };
        println!("{marker} {name}  {}", w.id.dimmed());
    }
    Ok(())
}

pub fn switch(ctx: &Ctx, needle: &str) -> Result<()> {
    let workspaces = ctx.client.workspaces()?;
    let lower = needle.to_lowercase();
    let matches: Vec<_> = workspaces
        .iter()
        .filter(|w| w.id == needle || w.name.to_lowercase().contains(&lower))
        .collect();
    let workspace = match matches.as_slice() {
        [one] => (*one).clone(),
        [] => bail!("no workspace matches '{needle}' — run `clockify workspaces` to list them"),
        many => {
            let names: Vec<&str> = many.iter().map(|w| w.name.as_str()).collect();
            bail!("'{needle}' is ambiguous — matching workspaces: {}", names.join(", "))
        }
    };

    let mut cfg = Config::load()?;
    cfg.workspace_id = Some(workspace.id);
    cfg.workspace_name = Some(workspace.name.clone());
    cfg.save()?;
    println!("{} Switched to workspace {}", "✓".green().bold(), workspace.name.bold());
    Ok(())
}
