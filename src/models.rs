use chrono::{DateTime, Utc};
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct User {
    pub id: String,
    pub name: String,
    pub email: String,
    pub active_workspace: Option<String>,
    pub default_workspace: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Workspace {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Project {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub client_name: Option<String>,
    #[serde(default)]
    pub billable: bool,
    #[serde(default)]
    pub archived: bool,
    /// Hex color like "#4CAF50", as configured in Clockify.
    #[serde(default)]
    pub color: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Task {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub status: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TimeEntry {
    pub id: String,
    #[serde(default)]
    pub description: String,
    pub project_id: Option<String>,
    pub task_id: Option<String>,
    #[serde(default)]
    pub billable: bool,
    pub time_interval: TimeInterval,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TimeInterval {
    pub start: DateTime<Utc>,
    pub end: Option<DateTime<Utc>>,
}

impl TimeEntry {
    /// Duration of the entry; running entries are measured up to now.
    pub fn duration(&self) -> chrono::Duration {
        let end = self.time_interval.end.unwrap_or_else(Utc::now);
        end - self.time_interval.start
    }
}
