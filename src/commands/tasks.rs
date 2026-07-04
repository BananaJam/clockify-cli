use anyhow::Result;

use super::table;
use crate::config::Ctx;
use crate::resolve;

pub fn run(ctx: &Ctx, project: &str) -> Result<()> {
    let project = resolve::project(ctx, project)?;
    let tasks = ctx.client.tasks(&ctx.workspace_id, &project.id)?;
    if tasks.is_empty() {
        println!("Project {} has no tasks.", project.name);
        return Ok(());
    }
    let mut t = table(&["Name", "Status", "ID"]);
    for task in &tasks {
        t.add_row(vec![
            task.name.clone(),
            task.status.clone().unwrap_or_default(),
            task.id.clone(),
        ]);
    }
    println!("Tasks in {}:", project.name);
    println!("{t}");
    Ok(())
}
