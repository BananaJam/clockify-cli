use anyhow::{Result, bail};
use colored::Colorize;

use super::table;
use crate::config::{Config, Ctx};

pub fn list(ctx: &Ctx) -> Result<()> {
    let workspaces = ctx.client.workspaces()?;
    let mut t = table(&["", "Name", "ID"]);
    for w in &workspaces {
        let active = if w.id == ctx.workspace_id { "✓" } else { "" };
        t.add_row(vec![active.to_string(), w.name.clone(), w.id.clone()]);
    }
    println!("{t}");
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
