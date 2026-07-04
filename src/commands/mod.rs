pub mod add;
pub mod auth;
pub mod delete;
pub mod edit;
pub mod log;
pub mod projects;
pub mod report;
pub mod start;
pub mod status;
pub mod stop;
pub mod tasks;
pub mod workspaces;

use std::collections::HashMap;

use anyhow::Result;
use comfy_table::{ContentArrangement, Table, presets};

use crate::config::Ctx;

pub fn table(headers: &[&str]) -> Table {
    let mut t = Table::new();
    t.load_preset(presets::UTF8_FULL_CONDENSED)
        .set_content_arrangement(ContentArrangement::Dynamic)
        .set_header(headers.to_vec());
    t
}

/// Map of project id -> project name for the current workspace.
pub fn project_names(ctx: &Ctx) -> Result<HashMap<String, String>> {
    Ok(ctx
        .client
        .projects(&ctx.workspace_id)?
        .into_iter()
        .map(|p| (p.id, p.name))
        .collect())
}
