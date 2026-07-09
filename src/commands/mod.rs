pub mod add;
pub mod auth;
pub mod delete;
pub mod discard;
pub mod edit;
pub mod log;
pub mod projects;
pub mod report;
pub mod skill;
pub mod start;
pub mod status;
pub mod stop;
pub mod submit;
pub mod tasks;
pub mod workspaces;

use std::collections::HashMap;

use anyhow::Result;
use colored::{ColoredString, Colorize};

use crate::config::Ctx;
use crate::models::Project;

/// Map of project id -> project for the current workspace.
pub fn project_map(ctx: &Ctx) -> Result<HashMap<String, Project>> {
    Ok(ctx
        .client
        .projects(&ctx.workspace_id)?
        .into_iter()
        .map(|p| (p.id.clone(), p))
        .collect())
}

/// Render text in a project's Clockify color (falls back to blue).
pub fn in_project_color(text: &str, project: Option<&Project>) -> ColoredString {
    match project.and_then(Project::rgb) {
        Some((r, g, b)) => text.truecolor(r, g, b),
        None => text.blue(),
    }
}

/// Short display form of an entry id: the last characters, with the
/// shortest suffix that uniquely identifies the entry highlighted —
/// typing just the highlighted part is enough to address it.
pub fn styled_id(id: &str, unique_len: usize) -> String {
    let unique_len = unique_len.clamp(crate::resolve::MIN_SUFFIX, id.len());
    let shown = unique_len.max(6).min(id.len());
    let tail = &id[id.len() - shown..];
    let (dim, bright) = tail.split_at(shown - unique_len);
    format!("{}{}{}", "…".dimmed(), dim.dimmed(), bright.yellow().bold())
}

/// Like styled_id when the unique length is unknown (no lookback fetched):
/// shows the last 6 characters without claiming any part is sufficient.
pub fn short_id(id: &str) -> String {
    let tail: String = id
        .chars()
        .rev()
        .take(6)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();
    format!("{}{}", "…".dimmed(), tail.yellow().dimmed())
}
