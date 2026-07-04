use anyhow::Result;

use super::table;
use crate::config::Ctx;

pub fn run(ctx: &Ctx, all: bool) -> Result<()> {
    let projects = ctx.client.projects(&ctx.workspace_id)?;
    let mut shown = 0;
    let mut t = table(&["Name", "Client", "Billable", "ID"]);
    for p in &projects {
        if p.archived && !all {
            continue;
        }
        shown += 1;
        let name = if p.archived { format!("{} (archived)", p.name) } else { p.name.clone() };
        t.add_row(vec![
            name,
            p.client_name.clone().unwrap_or_default(),
            if p.billable { "yes" } else { "no" }.to_string(),
            p.id.clone(),
        ]);
    }
    if shown == 0 {
        println!("No projects in this workspace{}.", if all { "" } else { " (try --all)" });
        return Ok(());
    }
    println!("{t}");
    Ok(())
}
