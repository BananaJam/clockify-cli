use anyhow::Result;
use colored::Colorize;

use super::{in_project_color, styled_id};
use crate::config::Ctx;
use crate::output;
use crate::resolve;

pub fn run(ctx: &Ctx, project: &str, json: bool) -> Result<()> {
    let project = resolve::project(ctx, project)?;
    let tasks = ctx.client.tasks(&ctx.workspace_id, &project.id)?;

    if json {
        let list: Vec<_> = tasks
            .iter()
            .map(|t| serde_json::json!({ "id": t.id, "name": t.name, "status": t.status }))
            .collect();
        output::print(&serde_json::Value::Array(list));
        return Ok(());
    }

    if tasks.is_empty() {
        println!(
            "Project {} has no tasks.",
            in_project_color(&project.name, Some(&project))
        );
        return Ok(());
    }
    let name_w = tasks
        .iter()
        .map(|t| t.name.chars().count())
        .max()
        .unwrap_or(0);
    let id_lens = resolve::unique_suffix_lens(tasks.iter().map(|t| t.id.as_str()));
    println!(
        "{}  {}",
        in_project_color(&project.name, Some(&project)).bold(),
        format!(
            "· {} task{}",
            tasks.len(),
            if tasks.len() == 1 { "" } else { "s" }
        )
        .yellow()
    );
    for task in &tasks {
        let status = match task.status.as_deref() {
            Some("ACTIVE") => "active".green(),
            Some("DONE") => "done".dimmed(),
            Some(other) => other.to_lowercase().normal(),
            None => "".normal(),
        };
        println!(
            "  {:<name_w$}  {:<8}  {}",
            task.name,
            status,
            styled_id(&task.id, id_lens.get(&task.id).copied().unwrap_or(6))
        );
    }
    Ok(())
}
