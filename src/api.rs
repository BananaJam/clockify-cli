use anyhow::{Context, Result, bail};
use chrono::{DateTime, Utc};
use reqwest::StatusCode;
use reqwest::blocking::{Client as HttpClient, RequestBuilder, Response};
use serde::de::DeserializeOwned;
use serde_json::{Value, json};

use crate::commands::submit::Period;
use crate::models::{
    ApprovalRequest, ApprovalRequestRow, Project, Task, TimeEntry, User, Workspace,
};
use crate::time::{to_api, to_api_query};

const BASE: &str = "https://api.clockify.me/api/v1";
const PAGE_SIZE: usize = 200;

#[derive(Clone)]
pub struct Client {
    http: HttpClient,
    api_key: String,
}

fn check(resp: Response) -> Result<Response> {
    let status = resp.status();
    if status.is_success() {
        return Ok(resp);
    }
    let body = resp.text().unwrap_or_default();
    let msg = serde_json::from_str::<Value>(&body)
        .ok()
        .and_then(|v| v.get("message").and_then(|m| m.as_str()).map(String::from))
        .unwrap_or(body);
    match status {
        StatusCode::UNAUTHORIZED => {
            bail!("invalid API key — run `clockify auth` to set up your credentials")
        }
        StatusCode::FORBIDDEN => bail!("access denied by Clockify (403): {msg}"),
        StatusCode::NOT_FOUND => bail!("not found (404): {msg}"),
        _ => bail!("Clockify API error ({status}): {msg}"),
    }
}

impl Client {
    pub fn new(api_key: String) -> Result<Client> {
        let http = HttpClient::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .context("failed to build HTTP client")?;
        Ok(Client { http, api_key })
    }

    fn request(&self, method: reqwest::Method, path: &str) -> RequestBuilder {
        self.http
            .request(method, format!("{BASE}{path}"))
            .header("X-Api-Key", &self.api_key)
    }

    fn send(&self, req: RequestBuilder) -> Result<Response> {
        check(req.send().context("request to the Clockify API failed")?)
    }

    fn get_json<T: DeserializeOwned>(&self, path: &str, query: &[(&str, String)]) -> Result<T> {
        let req = self.request(reqwest::Method::GET, path).query(query);
        self.send(req)?
            .json()
            .context("failed to parse the Clockify API response")
    }

    /// GET a paginated list endpoint, following pages until exhausted.
    fn get_paged<T: DeserializeOwned>(
        &self,
        path: &str,
        query: &[(&str, String)],
        limit: Option<usize>,
    ) -> Result<Vec<T>> {
        let mut all = Vec::new();
        let mut page = 1usize;
        loop {
            let page_size = PAGE_SIZE.min(limit.unwrap_or(usize::MAX) - all.len());
            let mut q: Vec<(&str, String)> = query.to_vec();
            q.push(("page", page.to_string()));
            q.push(("page-size", page_size.to_string()));
            let batch: Vec<T> = self.get_json(path, &q)?;
            let n = batch.len();
            all.extend(batch);
            let done = n < page_size || limit.is_some_and(|l| all.len() >= l);
            if done {
                return Ok(all);
            }
            page += 1;
        }
    }

    pub fn current_user(&self) -> Result<User> {
        self.get_json("/user", &[])
    }

    pub fn workspaces(&self) -> Result<Vec<Workspace>> {
        self.get_json("/workspaces", &[])
    }

    pub fn projects(&self, ws: &str) -> Result<Vec<Project>> {
        self.get_paged(&format!("/workspaces/{ws}/projects"), &[], None)
    }

    pub fn project(&self, ws: &str, id: &str) -> Result<Project> {
        self.get_json(&format!("/workspaces/{ws}/projects/{id}"), &[])
    }

    pub fn tasks(&self, ws: &str, project: &str) -> Result<Vec<Task>> {
        self.get_paged(
            &format!("/workspaces/{ws}/projects/{project}/tasks"),
            &[],
            None,
        )
    }

    pub fn create_time_entry(&self, ws: &str, body: &Value) -> Result<TimeEntry> {
        let req = self
            .request(
                reqwest::Method::POST,
                &format!("/workspaces/{ws}/time-entries"),
            )
            .json(body);
        let entry = self
            .send(req)?
            .json()
            .context("failed to parse the created time entry")?;
        crate::status_cache::invalidate();
        Ok(entry)
    }

    /// Stop the currently running timer. Returns None when no timer is running.
    pub fn stop_timer(
        &self,
        ws: &str,
        user: &str,
        end: DateTime<Utc>,
    ) -> Result<Option<TimeEntry>> {
        let resp = self
            .request(
                reqwest::Method::PATCH,
                &format!("/workspaces/{ws}/user/{user}/time-entries"),
            )
            .json(&json!({ "end": to_api(end) }))
            .send()
            .context("request to the Clockify API failed")?;
        if resp.status() == StatusCode::NOT_FOUND {
            return Ok(None);
        }
        let entry = check(resp)?
            .json()
            .context("failed to parse the stopped time entry")?;
        crate::status_cache::invalidate();
        Ok(Some(entry))
    }

    pub fn running_entry(&self, ws: &str, user: &str) -> Result<Option<TimeEntry>> {
        let entries: Vec<TimeEntry> = self.get_json(
            &format!("/workspaces/{ws}/user/{user}/time-entries"),
            &[("in-progress", "true".to_string())],
        )?;
        Ok(entries.into_iter().next())
    }

    pub fn time_entries(
        &self,
        ws: &str,
        user: &str,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
        limit: Option<usize>,
    ) -> Result<Vec<TimeEntry>> {
        self.get_paged(
            &format!("/workspaces/{ws}/user/{user}/time-entries"),
            &[("start", to_api_query(start)), ("end", to_api_query(end))],
            limit,
        )
    }

    pub fn time_entry(&self, ws: &str, id: &str) -> Result<TimeEntry> {
        self.get_json(&format!("/workspaces/{ws}/time-entries/{id}"), &[])
    }

    pub fn update_time_entry(&self, ws: &str, id: &str, body: &Value) -> Result<TimeEntry> {
        let req = self
            .request(
                reqwest::Method::PUT,
                &format!("/workspaces/{ws}/time-entries/{id}"),
            )
            .json(body);
        let entry = self
            .send(req)?
            .json()
            .context("failed to parse the updated time entry")?;
        crate::status_cache::invalidate();
        Ok(entry)
    }

    pub fn delete_time_entry(&self, ws: &str, id: &str) -> Result<()> {
        let req = self.request(
            reqwest::Method::DELETE,
            &format!("/workspaces/{ws}/time-entries/{id}"),
        );
        self.send(req)?;
        crate::status_cache::invalidate();
        Ok(())
    }

    pub fn approval_requests(
        &self,
        ws: &str,
        status: Option<&str>,
    ) -> Result<Vec<ApprovalRequestRow>> {
        let mut query = Vec::new();
        if let Some(status) = status {
            query.push(("status", status.to_string()));
        }
        self.get_paged(&format!("/workspaces/{ws}/approval-requests"), &query, None)
    }

    pub fn submit_approval_request(
        &self,
        ws: &str,
        period: Period,
        period_start: DateTime<Utc>,
    ) -> Result<ApprovalRequest> {
        self.send_approval_request(
            ws,
            "/approval-requests",
            period,
            period_start,
            "failed to parse the submitted approval request",
        )
    }

    pub fn resubmit_approval_request(
        &self,
        ws: &str,
        period: Period,
        period_start: DateTime<Utc>,
    ) -> Result<ApprovalRequest> {
        self.send_approval_request(
            ws,
            "/approval-requests/resubmit-entries-for-approval",
            period,
            period_start,
            "failed to parse the resubmitted approval request",
        )
    }

    fn send_approval_request(
        &self,
        ws: &str,
        path: &str,
        period: Period,
        period_start: DateTime<Utc>,
        parse_context: &'static str,
    ) -> Result<ApprovalRequest> {
        let body = approval_payload(period, period_start);
        let req = self
            .request(reqwest::Method::POST, &format!("/workspaces/{ws}{path}"))
            .json(&body);
        self.send(req)?.json().context(parse_context)
    }
}

fn approval_payload(period: Period, period_start: DateTime<Utc>) -> Value {
    json!({
        "period": period.as_api_str(),
        "periodStart": to_api(period_start),
    })
}

#[cfg(test)]
mod tests {
    use chrono::{TimeZone, Utc};

    use super::*;

    #[test]
    fn approval_payload_uses_clockify_wire_names() {
        let start = Utc.with_ymd_and_hms(2026, 7, 1, 0, 0, 0).unwrap();
        let payload = approval_payload(Period::Monthly, start);

        assert_eq!(payload["period"], "MONTHLY");
        assert_eq!(payload["periodStart"], "2026-07-01T00:00:00Z");
    }
}
