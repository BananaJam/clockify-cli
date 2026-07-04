use anyhow::{Result, bail};

use crate::config::Ctx;
use crate::models::{Project, Task};

fn looks_like_id(s: &str) -> bool {
    s.len() == 24 && s.chars().all(|c| c.is_ascii_hexdigit())
}

fn pick<'a, T>(
    kind: &str,
    needle: &str,
    items: &'a [T],
    id: impl Fn(&T) -> &str,
    name: impl Fn(&T) -> &str,
) -> Result<&'a T> {
    if looks_like_id(needle)
        && let Some(item) = items.iter().find(|i| id(i) == needle)
    {
        return Ok(item);
    }
    let lower = needle.to_lowercase();
    if let Some(item) = items.iter().find(|i| name(i).to_lowercase() == lower) {
        return Ok(item);
    }
    let matches: Vec<&T> = items
        .iter()
        .filter(|i| name(i).to_lowercase().contains(&lower))
        .collect();
    match matches.as_slice() {
        [one] => Ok(one),
        [] => bail!("no {kind} matches '{needle}' — run `clockify {kind}s` to see what's available"),
        many => {
            let names: Vec<&str> = many.iter().map(|i| name(i)).collect();
            bail!("'{needle}' is ambiguous — matching {kind}s: {}", names.join(", "))
        }
    }
}

pub fn project(ctx: &Ctx, needle: &str) -> Result<Project> {
    let projects = ctx.client.projects(&ctx.workspace_id)?;
    let active: Vec<Project> = projects.into_iter().filter(|p| !p.archived).collect();
    pick("project", needle, &active, |p| &p.id, |p| &p.name).cloned()
}

pub fn task(ctx: &Ctx, project_id: &str, needle: &str) -> Result<Task> {
    let tasks = ctx.client.tasks(&ctx.workspace_id, project_id)?;
    pick("task", needle, &tasks, |t| &t.id, |t| &t.name).cloned()
}
