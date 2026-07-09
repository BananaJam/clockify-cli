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

impl Project {
    pub fn rgb(&self) -> Option<(u8, u8, u8)> {
        let hex = self.color.as_deref()?.strip_prefix('#')?;
        if hex.len() != 6 {
            return None;
        }
        let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
        let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
        let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
        Some((r, g, b))
    }
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

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApprovalRequest {
    pub id: String,
    #[serde(default)]
    pub date_range: Option<ApprovalDateRange>,
    #[serde(default)]
    pub status: Option<ApprovalStatus>,
    #[serde(default)]
    pub workspace_id: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ApprovalDateRange {
    pub start: DateTime<Utc>,
    pub end: DateTime<Utc>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ApprovalStatus {
    pub state: String,
    #[serde(default)]
    pub note: Option<String>,
    #[serde(default)]
    pub updated_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApprovalRequestRow {
    pub approval_request: ApprovalRequest,
    #[serde(default)]
    pub time_entries: Vec<TimeEntry>,
}
