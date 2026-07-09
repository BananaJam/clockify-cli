use chrono::{DateTime, NaiveDate, Utc};
use serde::Deserialize;
use std::path::PathBuf;

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
#[serde(rename_all = "camelCase")]
pub struct Expense {
    pub id: String,
    #[serde(default)]
    pub user_id: Option<String>,
    #[serde(default)]
    pub workspace_id: Option<String>,
    #[serde(default)]
    pub category_id: Option<String>,
    #[serde(default)]
    pub project_id: Option<String>,
    #[serde(default)]
    pub task_id: Option<String>,
    #[serde(default)]
    pub category: Option<ExpenseCategory>,
    #[serde(default)]
    pub project: Option<NamedRef>,
    #[serde(default)]
    pub task: Option<NamedRef>,
    #[serde(deserialize_with = "deserialize_expense_date")]
    pub date: NaiveDate,
    #[serde(default)]
    pub file_id: Option<String>,
    #[serde(default)]
    pub file_name: Option<String>,
    #[serde(default)]
    pub notes: Option<String>,
    #[serde(default)]
    pub quantity: Option<f64>,
    #[serde(default)]
    pub total: f64,
    #[serde(default)]
    pub billable: bool,
    #[serde(default)]
    pub locked: bool,
}

impl Expense {
    pub fn category_id(&self) -> Option<&str> {
        self.category_id
            .as_deref()
            .or_else(|| self.category.as_ref().map(|c| c.id.as_str()))
    }

    pub fn category_name(&self) -> Option<&str> {
        self.category.as_ref().map(|c| c.name.as_str())
    }

    pub fn project_id(&self) -> Option<&str> {
        self.project_id
            .as_deref()
            .or_else(|| self.project.as_ref().map(|p| p.id.as_str()))
    }

    pub fn project_name(&self) -> Option<&str> {
        self.project.as_ref().and_then(|p| p.name.as_deref())
    }

    pub fn task_id(&self) -> Option<&str> {
        self.task_id
            .as_deref()
            .or_else(|| self.task.as_ref().map(|t| t.id.as_str()))
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExpenseCategory {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub archived: bool,
    #[serde(default)]
    pub has_unit_price: bool,
    #[serde(default)]
    pub price_in_cents: Option<i64>,
    #[serde(default)]
    pub unit: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NamedRef {
    pub id: String,
    #[serde(default)]
    pub name: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExpensesAndTotals {
    #[serde(default)]
    pub expenses: ExpensesWithCount,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct ExpensesWithCount {
    #[serde(default)]
    pub count: usize,
    #[serde(default)]
    pub expenses: Vec<Expense>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ExpenseCategoriesWithCount {
    #[serde(default)]
    pub count: usize,
    #[serde(default)]
    pub categories: Vec<ExpenseCategory>,
}

#[derive(Debug, Clone)]
pub struct ExpenseDraft {
    pub amount: f64,
    pub category_id: String,
    pub date: DateTime<Utc>,
    pub user_id: String,
    pub project_id: Option<String>,
    pub task_id: Option<String>,
    pub notes: Option<String>,
    pub billable: bool,
    pub file: Option<PathBuf>,
    pub change_fields: Vec<ExpenseChangeField>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExpenseChangeField {
    Date,
    Project,
    Task,
    Category,
    Notes,
    Amount,
    Billable,
    File,
}

impl ExpenseChangeField {
    pub fn as_api_str(self) -> &'static str {
        match self {
            ExpenseChangeField::Date => "DATE",
            ExpenseChangeField::Project => "PROJECT",
            ExpenseChangeField::Task => "TASK",
            ExpenseChangeField::Category => "CATEGORY",
            ExpenseChangeField::Notes => "NOTES",
            ExpenseChangeField::Amount => "AMOUNT",
            ExpenseChangeField::Billable => "BILLABLE",
            ExpenseChangeField::File => "FILE",
        }
    }
}

fn deserialize_expense_date<'de, D>(deserializer: D) -> Result<NaiveDate, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value = String::deserialize(deserializer)?;
    NaiveDate::parse_from_str(&value, "%Y-%m-%d")
        .or_else(|_| {
            DateTime::parse_from_rfc3339(&value).map(|dt| dt.with_timezone(&Utc).date_naive())
        })
        .map_err(serde::de::Error::custom)
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
    #[serde(alias = "entries", alias = "timeEntries")]
    pub time_entries: Vec<serde_json::Value>,
    #[serde(default)]
    pub expenses: Vec<Expense>,
    #[serde(default)]
    pub expense_total: Option<f64>,
}
