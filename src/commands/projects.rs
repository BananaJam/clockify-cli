use anyhow::Result;
use colored::Colorize;

use super::in_project_color;
use crate::config::Ctx;
use crate::models::Project;

pub fn run(ctx: &Ctx, all: bool) -> Result<()> {
    let mut projects = ctx.client.projects(&ctx.workspace_id)?;
    projects.retain(|p| all || !p.archived);
    if projects.is_empty() {
        println!("No projects in this workspace{}.", if all { "" } else { " (try --all)" });
        return Ok(());
    }
    projects.sort_by_key(|p| p.name.to_lowercase());
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
