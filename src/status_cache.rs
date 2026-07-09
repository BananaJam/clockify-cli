//! A tiny disk cache of the running-timer state so shell-prompt
//! integrations (starship) can render without hitting 1Password or the
//! API on every prompt. Mutating API calls invalidate it (see api.rs).

use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// How long a cached answer is trusted. Local CLI/TUI changes bypass this
/// via invalidation; the TTL only delays visibility of changes made
/// elsewhere (web app, another machine).
pub const TTL_SECS: u64 = 120;

#[derive(Serialize, Deserialize, Clone)]
pub struct CachedEntry {
    pub description: String,
    pub start: DateTime<Utc>,
    pub project_name: Option<String>,
}

#[derive(Serialize, Deserialize)]
pub struct CachedStatus {
    pub fetched_at: u64,
    pub workspace_id: String,
    pub entry: Option<CachedEntry>,
}

fn path() -> Option<PathBuf> {
    let base = match std::env::var_os("XDG_CACHE_HOME").filter(|d| !d.is_empty()) {
        Some(dir) => PathBuf::from(dir),
        None => dirs::home_dir()?.join(".cache"),
    };
    Some(base.join("clockify").join("status.json"))
}

pub fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

pub fn load() -> Option<CachedStatus> {
    let raw = fs::read_to_string(path()?).ok()?;
    serde_json::from_str(&raw).ok()
}

pub fn save(status: &CachedStatus) {
    let Some(path) = path() else { return };
    if let Some(parent) = path.parent()
        && fs::create_dir_all(parent).is_err()
    {
        return;
    }
    if let Ok(raw) = serde_json::to_string(status) {
        let _ = fs::write(path, raw);
    }
}

/// Called after any time-entry mutation so the next prompt refetches.
pub fn invalidate() {
    if let Some(path) = path() {
        let _ = fs::remove_file(path);
    }
}
