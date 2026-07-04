use anyhow::Result;
use colored::Colorize;

use super::in_project_color;
use crate::config::{Config, Ctx, DefaultProject};
use crate::models::Project;
use crate::output;
use crate::resolve;

pub fn run(ctx: &Ctx, all: bool, json: bool) -> Result<()> {
    let mut projects = ctx.client.projects(&ctx.workspace_id)?;
    projects.retain(|p| all || !p.archived);
    projects.sort_by_key(|p| p.name.to_lowercase());

    if json {
        let list: Vec<_> = projects
            .iter()
            .map(|p| {
                serde_json::json!({
                    "id": p.id,
                    "name": p.name,
                    "client": p.client_name.as_deref().filter(|c| !c.is_empty()),
                    "billable": p.billable,
                    "archived": p.archived,
                    "default": ctx.default_project.as_ref().is_some_and(|d| d.id == p.id),
                })
            })
            .collect();
        output::print(&serde_json::Value::Array(list));
        return Ok(());
    }

    if projects.is_empty() {
        println!("No projects in this workspace{}.", if all { "" } else { " (try --all)" });
        return Ok(());
    }
    let name_w = projects.iter().map(|p| p.name.chars().count()).max().unwrap_or(0);

    // Group by client, like log groups by day; clientless projects last.
    let mut groups: Vec<(String, Vec<&Project>)> = Vec::new();
    for p in &projects {
        let client = p.client_name.clone().filter(|c| !c.is_empty());
        let key = client.unwrap_or_else(|| "(no client)".to_string());
        match groups.iter_mut().find(|(name, _)| *name == key) {
            Some((_, group)) => group.push(p),
            None => groups.push((key, vec![p])),
        }
    }
    groups.sort_by(|(a, _), (b, _)| {
        (a == "(no client)").cmp(&(b == "(no client)")).then(a.to_lowercase().cmp(&b.to_lowercase()))
    });

    for (client, group) in &groups {
        println!(
            "{}  {}",
            client.bold(),
            format!("· {} project{}", group.len(), if group.len() == 1 { "" } else { "s" })
                .yellow()
        );
        for p in group {
            let mut flags = Vec::new();
            if ctx.default_project.as_ref().is_some_and(|d| d.id == p.id) {
                flags.push("default");
            }
            if !p.billable {
                flags.push("not billable");
            }
            if p.archived {
                flags.push("archived");
            }
            let flags = if flags.is_empty() {
                String::new()
            } else {
                format!(" ({})", flags.join(", "))
            };
            println!(
                "  {}{}  {}",
                in_project_color(&format!("{:<name_w$}", p.name), Some(p)),
                flags.dimmed(),
                p.id.dimmed()
            );
        }
        println!();
    }
    Ok(())
}

/// Show, set, or clear the workspace's default project.
pub fn default(ctx: &Ctx, project: Option<&str>, clear: bool) -> Result<()> {
    if clear {
        let mut cfg = Config::load()?;
        match cfg.default_projects.remove(&ctx.workspace_id) {
            Some(old) => {
                cfg.save()?;
                println!("{} Cleared the default project (was {}).", "✓".green().bold(), old.name.bold());
            }
            None => println!("No default project was set."),
        }
        return Ok(());
    }

    let Some(needle) = project else {
        match &ctx.default_project {
            Some(d) => println!(
                "Default project: {}  {}",
                d.name.bold(),
                d.id.dimmed()
            ),
            None => println!(
                "No default project set — set one with {}.",
                "clockify projects default <project>".bold()
            ),
        }
        return Ok(());
    };

    let p = resolve::project(ctx, needle)?;
    let mut cfg = Config::load()?;
    cfg.default_projects
        .insert(ctx.workspace_id.clone(), DefaultProject { id: p.id.clone(), name: p.name.clone() });
    cfg.save()?;
    println!(
        "{} New entries now default to {}.",
        "✓".green().bold(),
        in_project_color(&p.name, Some(&p))
    );
    Ok(())
}
