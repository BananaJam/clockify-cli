use chrono::SecondsFormat;
use serde_json::{Value, json};

use crate::models::{Project, TimeEntry};

/// The one JSON shape for a time entry, shared by every `--json` output so
/// agents and scripts can rely on it.
pub fn entry_json(e: &TimeEntry, project: Option<&Project>) -> Value {
    json!({
        "id": e.id,
        "description": e.description,
        "project": project.map(|p| json!({ "id": p.id, "name": p.name })),
        "start": e.time_interval.start.to_rfc3339_opts(SecondsFormat::Secs, true),
        "end": e.time_interval.end.map(|t| t.to_rfc3339_opts(SecondsFormat::Secs, true)),
        "duration_seconds": e.duration().num_seconds(),
        "running": e.time_interval.end.is_none(),
    })
}

pub fn print(value: &Value) {
    println!("{}", serde_json::to_string_pretty(value).expect("serializing json values cannot fail"));
}

#[cfg(test)]
mod tests {
    use chrono::{Duration, TimeZone, Utc};

    use super::*;
    use crate::models::TimeInterval;

    #[test]
    fn entry_json_completed_and_running() {
        let start = Utc.with_ymd_and_hms(2026, 7, 4, 9, 0, 0).unwrap();
        let mut entry = TimeEntry {
            id: "68b3a1d2e4f5a6b7c8d9e0f1".into(),
            description: "fixing the parser".into(),
            project_id: Some("p1".into()),
            task_id: None,
            time_interval: TimeInterval { start, end: Some(start + Duration::minutes(90)) },
        };
        let project = Project {
            id: "p1".into(),
            name: "Backend".into(),
            client_name: None,
            billable: true,
            archived: false,
            color: None,
        };

        let v = entry_json(&entry, Some(&project));
        assert_eq!(v["id"], "68b3a1d2e4f5a6b7c8d9e0f1");
        assert_eq!(v["project"]["name"], "Backend");
        assert_eq!(v["start"], "2026-07-04T09:00:00Z");
        assert_eq!(v["end"], "2026-07-04T10:30:00Z");
        assert_eq!(v["duration_seconds"], 5400);
        assert_eq!(v["running"], false);

        entry.time_interval.end = None;
        let v = entry_json(&entry, None);
        assert!(v["project"].is_null());
        assert!(v["end"].is_null());
        assert_eq!(v["running"], true);
    }
}
