use std::collections::HashMap;
use std::sync::mpsc::{Receiver, Sender, channel};

use anyhow::{Context as _, Result, bail};
use chrono::{Datelike, Days, Duration, Local, NaiveDate, Utc};
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use serde_json::json;

use super::input::Input;
use super::theme::{self, Theme};
use crate::config::{Config, Ctx};
use crate::models::{Project, TimeEntry, Workspace};
use crate::time::{day_range, fmt_duration, parse_time, to_api};

pub const TABS: &[&str] = &["Log", "Report", "Projects", "Workspaces"];

pub enum Mode {
    Normal,
    Confirm { message: String, action: ConfirmAction },
    Form(Form),
}

#[derive(Clone)]
pub enum ConfirmAction {
    DeleteEntry(String),
    DiscardTimer(String),
}

#[derive(Clone, Copy, PartialEq)]
pub enum FormKind {
    Start,
    Add,
    Edit,
}

pub enum Field {
    Text { label: &'static str, input: Input },
    Project { label: &'static str, idx: Option<usize> },
    Toggle { label: &'static str, on: bool },
}

pub struct Form {
    pub kind: FormKind,
    pub title: &'static str,
    pub fields: Vec<Field>,
    pub focus: usize,
    pub error: Option<String>,
    pub entry: Option<TimeEntry>,
}

/// Results sent back from background fetch threads. `epoch` guards against
/// stale responses arriving after a workspace switch.
enum Msg {
    Entries { epoch: u64, offset: i64, result: Result<Vec<TimeEntry>> },
    Projects { epoch: u64, result: Result<Vec<Project>> },
    Running { epoch: u64, result: Result<Option<TimeEntry>> },
    Workspaces { result: Result<Vec<Workspace>> },
}

pub struct App {
    pub ctx: Ctx,
    pub theme: &'static Theme,
    pub tab: usize,
    pub week_offset: i64,
    /// Entries of the selected week, oldest first.
    pub entries: Vec<TimeEntry>,
    pub projects: Vec<Project>,
    pub workspaces: Vec<Workspace>,
    pub running: Option<TimeEntry>,
    /// True while the shown week has nothing to display yet.
    pub loading: bool,
    pub sel_log: usize,
    pub sel_ws: usize,
    pub mode: Mode,
    /// (message, is_error)
    pub status: Option<(String, bool)>,
    pub workspace_name: String,
    pub quit: bool,
    week_cache: HashMap<i64, Vec<TimeEntry>>,
    epoch: u64,
    tx: Sender<Msg>,
    rx: Receiver<Msg>,
}

fn week_bounds_for(offset: i64) -> (NaiveDate, NaiveDate) {
    let today = Local::now().date_naive();
    let monday =
        today - Days::new(today.weekday().num_days_from_monday() as u64) + Duration::weeks(offset);
    (monday, monday + Duration::days(6))
}

impl App {
    pub fn new(ctx: Ctx) -> Result<App> {
        let cfg = Config::load()?;
        let (tx, rx) = channel();
        let app = App {
            theme: theme::by_name(cfg.theme.as_deref().unwrap_or("nord")),
            workspace_name: cfg.workspace_name.unwrap_or_else(|| ctx.workspace_id.clone()),
            ctx,
            tab: 0,
            week_offset: 0,
            entries: Vec::new(),
            projects: Vec::new(),
            workspaces: Vec::new(),
            running: None,
            loading: true,
            sel_log: 0,
            sel_ws: 0,
            mode: Mode::Normal,
            status: None,
            quit: false,
            week_cache: HashMap::new(),
            epoch: 0,
            tx,
            rx,
        };
        app.spawn_entries(0);
        app.spawn_projects();
        app.spawn_running();
        app.spawn_workspaces();
        Ok(app)
    }

    pub fn week_bounds(&self) -> (NaiveDate, NaiveDate) {
        week_bounds_for(self.week_offset)
    }

    // --- background fetching ---

    fn spawn_entries(&self, offset: i64) {
        let (tx, epoch) = (self.tx.clone(), self.epoch);
        let (client, ws, uid) =
            (self.ctx.client.clone(), self.ctx.workspace_id.clone(), self.ctx.user_id.clone());
        std::thread::spawn(move || {
            let (from, to) = week_bounds_for(offset);
            let result = day_range(from, to)
                .and_then(|(start, end)| client.time_entries(&ws, &uid, start, end, None))
                .map(|mut entries| {
                    entries.reverse(); // API returns newest first
                    entries
                });
            let _ = tx.send(Msg::Entries { epoch, offset, result });
        });
    }

    fn spawn_projects(&self) {
        let (tx, epoch) = (self.tx.clone(), self.epoch);
        let (client, ws) = (self.ctx.client.clone(), self.ctx.workspace_id.clone());
        std::thread::spawn(move || {
            let result = client.projects(&ws).map(|mut projects| {
                projects.sort_by_key(|p| p.name.to_lowercase());
                projects
            });
            let _ = tx.send(Msg::Projects { epoch, result });
        });
    }

    fn spawn_running(&self) {
        let (tx, epoch) = (self.tx.clone(), self.epoch);
        let (client, ws, uid) =
            (self.ctx.client.clone(), self.ctx.workspace_id.clone(), self.ctx.user_id.clone());
        std::thread::spawn(move || {
            let result = client.running_entry(&ws, &uid);
            let _ = tx.send(Msg::Running { epoch, result });
        });
    }

    fn spawn_workspaces(&self) {
        let tx = self.tx.clone();
        let client = self.ctx.client.clone();
        std::thread::spawn(move || {
            let _ = tx.send(Msg::Workspaces { result: client.workspaces() });
        });
    }

    /// Apply any completed background fetches; called every event-loop tick.
    pub fn pump(&mut self) {
        while let Ok(msg) = self.rx.try_recv() {
            match msg {
                Msg::Entries { epoch, offset, result } => {
                    if epoch != self.epoch {
                        continue;
                    }
                    match result {
                        Ok(entries) => {
                            self.week_cache.insert(offset, entries.clone());
                            if offset == self.week_offset {
                                self.entries = entries;
                                self.loading = false;
                                self.sel_log =
                                    self.sel_log.min(self.entries.len().saturating_sub(1));
                            }
                        }
                        Err(e) => {
                            if offset == self.week_offset {
                                self.loading = false;
                            }
                            self.set_status(format!("{e:#}"), true);
                        }
                    }
                }
                Msg::Projects { epoch, result } => {
                    if epoch != self.epoch {
                        continue;
                    }
                    match result {
                        Ok(projects) => self.projects = projects,
                        Err(e) => self.set_status(format!("{e:#}"), true),
                    }
                }
                Msg::Running { epoch, result } => {
                    if epoch != self.epoch {
                        continue;
                    }
                    match result {
                        Ok(running) => self.running = running,
                        Err(e) => self.set_status(format!("{e:#}"), true),
                    }
                }
                Msg::Workspaces { result } => match result {
                    Ok(workspaces) => self.workspaces = workspaces,
                    Err(e) => self.set_status(format!("{e:#}"), true),
                },
            }
        }
    }

    /// Drop cached weeks and refetch after anything changed entries.
    /// The stale list stays visible until fresh data arrives.
    fn invalidate(&mut self) {
        self.week_cache.clear();
        self.loading = self.entries.is_empty();
        self.spawn_entries(self.week_offset);
        self.spawn_running();
    }

    pub fn active_projects(&self) -> Vec<&Project> {
        self.projects.iter().filter(|p| !p.archived).collect()
    }

    pub fn project_of(&self, entry: &TimeEntry) -> Option<&Project> {
        entry.project_id.as_deref().and_then(|id| self.projects.iter().find(|p| p.id == id))
    }

    pub fn selected_entry(&self) -> Option<&TimeEntry> {
        self.entries.get(self.sel_log)
    }

    fn set_status(&mut self, msg: impl Into<String>, is_error: bool) {
        self.status = Some((msg.into(), is_error));
    }

    pub fn on_key(&mut self, key: KeyEvent) {
        // Don't let ctrl/alt chords fall through as plain characters
        // (e.g. Ctrl+D arriving as 'd' would pop the delete dialog).
        if key.modifiers.intersects(KeyModifiers::CONTROL | KeyModifiers::ALT) {
            if key.code == KeyCode::Char('c') {
                self.quit = true;
            }
            return;
        }
        self.status = None;
        match &self.mode {
            Mode::Normal => self.normal_key(key),
            Mode::Confirm { .. } => self.confirm_key(key),
            Mode::Form(_) => self.form_key(key),
        }
    }

    fn normal_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Char('q') | KeyCode::Esc => self.quit = true,
            KeyCode::Tab => self.tab = (self.tab + 1) % TABS.len(),
            KeyCode::BackTab => self.tab = (self.tab + TABS.len() - 1) % TABS.len(),
            KeyCode::Char(c @ '1'..='4') => self.tab = c as usize - '1' as usize,
            KeyCode::Char('j') | KeyCode::Down => self.move_selection(1),
            KeyCode::Char('k') | KeyCode::Up => self.move_selection(-1),
            KeyCode::Char('h') | KeyCode::Left => self.shift_week(-1),
            KeyCode::Char('l') | KeyCode::Right => self.shift_week(1),
            KeyCode::Char('r') => {
                self.invalidate();
                self.set_status("refreshing…", false);
            }
            KeyCode::Char('t') => self.cycle_theme(),
            KeyCode::Char('s') => self.open_start_form(),
            KeyCode::Char('a') => self.open_add_form(),
            KeyCode::Char('e') => self.open_edit_form(),
            KeyCode::Char('d') => self.confirm_delete(),
            KeyCode::Char('x') => self.stop_timer(),
            KeyCode::Char('X') => self.confirm_discard(),
            KeyCode::Enter if self.tab == 3 => self.switch_workspace(),
            _ => {}
        }
    }

    fn move_selection(&mut self, delta: i64) {
        let (sel, len) = match self.tab {
            0 => (&mut self.sel_log, self.entries.len()),
            3 => (&mut self.sel_ws, self.workspaces.len()),
            _ => return,
        };
        if len == 0 {
            return;
        }
        *sel = (*sel as i64 + delta).clamp(0, len as i64 - 1) as usize;
    }

    fn shift_week(&mut self, delta: i64) {
        if self.tab > 1 {
            return;
        }
        let next = (self.week_offset + delta).min(0);
        if next == self.week_offset {
            return;
        }
        self.week_offset = next;
        self.sel_log = 0;
        match self.week_cache.get(&next) {
            Some(cached) => {
                // Instant from cache; still refetch quietly to stay fresh.
                self.entries = cached.clone();
                self.loading = false;
            }
            None => {
                self.entries.clear();
                self.loading = true;
            }
        }
        self.spawn_entries(next);
    }

    fn cycle_theme(&mut self) {
        self.theme = theme::next(self.theme);
        let saved = Config::load().and_then(|mut c| {
            c.theme = Some(self.theme.name.to_string());
            c.save()
        });
        match saved {
            Ok(()) => self.set_status(format!("theme: {}", self.theme.name), false),
            Err(e) => self.set_status(format!("could not save theme: {e:#}"), true),
        }
    }

    // --- timer actions ---

    fn stop_timer(&mut self) {
        match self.ctx.client.stop_timer(&self.ctx.workspace_id, &self.ctx.user_id, Utc::now()) {
            Ok(Some(e)) => {
                self.set_status(format!("stopped — {}", fmt_duration(e.duration())), false);
                self.invalidate();
            }
            Ok(None) => self.set_status("no timer is running", false),
            Err(e) => self.set_status(format!("{e:#}"), true),
        }
    }

    fn confirm_discard(&mut self) {
        let Some(running) = &self.running else {
            self.set_status("no timer is running", false);
            return;
        };
        let desc = if running.description.is_empty() { "(no description)" } else { &running.description };
        self.mode = Mode::Confirm {
            message: format!("Discard the running timer \"{desc}\"? The time will not be saved."),
            action: ConfirmAction::DiscardTimer(running.id.clone()),
        };
    }

    fn confirm_delete(&mut self) {
        if self.tab != 0 {
            return;
        }
        let Some(entry) = self.selected_entry() else { return };
        let desc = if entry.description.is_empty() { "(no description)" } else { &entry.description };
        self.mode = Mode::Confirm {
            message: format!("Delete \"{desc}\" ({})?", fmt_duration(entry.duration())),
            action: ConfirmAction::DeleteEntry(entry.id.clone()),
        };
    }

    fn confirm_key(&mut self, key: KeyEvent) {
        let confirmed = matches!(key.code, KeyCode::Char('y') | KeyCode::Enter);
        let Mode::Confirm { action, .. } = std::mem::replace(&mut self.mode, Mode::Normal) else {
            return;
        };
        if !confirmed {
            return;
        }
        let (id, verb) = match &action {
            ConfirmAction::DeleteEntry(id) => (id, "deleted"),
            ConfirmAction::DiscardTimer(id) => (id, "discarded"),
        };
        match self.ctx.client.delete_time_entry(&self.ctx.workspace_id, id) {
            Ok(()) => {
                self.set_status(format!("entry {verb}"), false);
                self.invalidate();
            }
            Err(e) => self.set_status(format!("{e:#}"), true),
        }
    }

    // --- workspace switching ---

    fn switch_workspace(&mut self) {
        let Some(w) = self.workspaces.get(self.sel_ws).cloned() else { return };
        if w.id == self.ctx.workspace_id {
            self.set_status("already the current workspace", false);
            return;
        }
        match self.try_switch(&w) {
            Ok(()) => self.set_status(format!("switched to {}", w.name), false),
            Err(e) => self.set_status(format!("{e:#}"), true),
        }
    }

    fn try_switch(&mut self, w: &Workspace) -> Result<()> {
        let mut cfg = Config::load()?;
        cfg.workspace_id = Some(w.id.clone());
        cfg.workspace_name = Some(w.name.clone());
        cfg.save()?;
        self.ctx.workspace_id = w.id.clone();
        self.workspace_name = w.name.clone();
        // New epoch: in-flight responses for the old workspace get dropped.
        self.epoch += 1;
        self.week_cache.clear();
        self.entries.clear();
        self.projects.clear();
        self.running = None;
        self.loading = true;
        self.spawn_entries(self.week_offset);
        self.spawn_projects();
        self.spawn_running();
        Ok(())
    }

    // --- forms ---

    fn open_start_form(&mut self) {
        self.mode = Mode::Form(Form {
            kind: FormKind::Start,
            title: " Start timer ",
            fields: vec![
                Field::Text { label: "Description", input: Input::new("") },
                Field::Project { label: "Project", idx: None },
                Field::Text { label: "At (HH:MM)", input: Input::new("") },
                Field::Toggle { label: "Billable", on: false },
            ],
            focus: 0,
            error: None,
            entry: None,
        });
    }

    fn open_add_form(&mut self) {
        self.mode = Mode::Form(Form {
            kind: FormKind::Add,
            title: " Add entry ",
            fields: vec![
                Field::Text { label: "Description", input: Input::new("") },
                Field::Project { label: "Project", idx: None },
                Field::Text { label: "From", input: Input::new("") },
                Field::Text { label: "To", input: Input::new("") },
                Field::Toggle { label: "Billable", on: false },
            ],
            focus: 0,
            error: None,
            entry: None,
        });
    }

    fn open_edit_form(&mut self) {
        if self.tab != 0 {
            return;
        }
        let Some(entry) = self.selected_entry().cloned() else { return };
        let idx = entry
            .project_id
            .as_deref()
            .and_then(|id| self.active_projects().iter().position(|p| p.id == id));
        let fmt = |dt: chrono::DateTime<Utc>| {
            dt.with_timezone(&Local).format("%Y-%m-%d %H:%M").to_string()
        };
        let from = fmt(entry.time_interval.start);
        let to = entry.time_interval.end.map(fmt).unwrap_or_default();
        self.mode = Mode::Form(Form {
            kind: FormKind::Edit,
            title: " Edit entry ",
            fields: vec![
                Field::Text { label: "Description", input: Input::new(&entry.description) },
                Field::Project { label: "Project", idx },
                Field::Text { label: "From", input: Input::new(&from) },
                Field::Text { label: "To", input: Input::new(&to) },
            ],
            focus: 0,
            error: None,
            entry: Some(entry),
        });
    }

    fn form_key(&mut self, key: KeyEvent) {
        let n_projects = self.active_projects().len();
        let Mode::Form(form) = &mut self.mode else { return };
        match key.code {
            KeyCode::Esc => self.mode = Mode::Normal,
            KeyCode::Enter => self.submit_form(),
            KeyCode::Tab | KeyCode::Down => form.focus = (form.focus + 1) % form.fields.len(),
            KeyCode::BackTab | KeyCode::Up => {
                form.focus = (form.focus + form.fields.len() - 1) % form.fields.len()
            }
            code => match &mut form.fields[form.focus] {
                Field::Text { input, .. } => match code {
                    KeyCode::Char(c) => input.insert(c),
                    KeyCode::Backspace => input.backspace(),
                    KeyCode::Delete => input.delete(),
                    KeyCode::Left => input.left(),
                    KeyCode::Right => input.right(),
                    KeyCode::Home => input.home(),
                    KeyCode::End => input.end(),
                    _ => {}
                },
                Field::Project { idx, .. } => match code {
                    KeyCode::Left => {
                        *idx = match *idx {
                            None => n_projects.checked_sub(1),
                            Some(0) => None,
                            Some(i) => Some(i - 1),
                        }
                    }
                    KeyCode::Right => {
                        *idx = match *idx {
                            None if n_projects > 0 => Some(0),
                            None => None,
                            Some(i) if i + 1 < n_projects => Some(i + 1),
                            Some(_) => None,
                        }
                    }
                    _ => {}
                },
                Field::Toggle { on, .. } => {
                    if matches!(code, KeyCode::Char(' ') | KeyCode::Left | KeyCode::Right) {
                        *on = !*on;
                    }
                }
            },
        }
    }

    fn submit_form(&mut self) {
        let Mode::Form(form) = &self.mode else { return };
        let kind = form.kind;
        let entry = form.entry.clone();
        let mut texts: Vec<String> = Vec::new();
        let mut project_idx = None;
        let mut billable = false;
        for f in &form.fields {
            match f {
                Field::Text { input, .. } => texts.push(input.value().trim().to_string()),
                Field::Project { idx, .. } => project_idx = *idx,
                Field::Toggle { on, .. } => billable = *on,
            }
        }
        let project_id =
            project_idx.and_then(|i| self.active_projects().get(i).map(|p| p.id.clone()));

        let result: Result<String> = (|| {
            match kind {
                FormKind::Start => {
                    if texts[0].is_empty() {
                        bail!("description is required");
                    }
                    let start = if texts[1].is_empty() { Utc::now() } else { parse_time(&texts[1])? };
                    if self.running.is_some() {
                        self.ctx.client.stop_timer(&self.ctx.workspace_id, &self.ctx.user_id, start)?;
                    }
                    let mut body = json!({
                        "start": to_api(start),
                        "description": texts[0],
                        "billable": billable,
                    });
                    if let Some(pid) = &project_id {
                        body["projectId"] = json!(pid);
                    }
                    self.ctx.client.create_time_entry(&self.ctx.workspace_id, &body)?;
                    Ok("timer started".to_string())
                }
                FormKind::Add => {
                    if texts[0].is_empty() {
                        bail!("description is required");
                    }
                    let from = parse_time(&texts[1])?;
                    let to = parse_time(&texts[2])?;
                    if to <= from {
                        bail!("'To' must be after 'From'");
                    }
                    let mut body = json!({
                        "start": to_api(from),
                        "end": to_api(to),
                        "description": texts[0],
                        "billable": billable,
                    });
                    if let Some(pid) = &project_id {
                        body["projectId"] = json!(pid);
                    }
                    self.ctx.client.create_time_entry(&self.ctx.workspace_id, &body)?;
                    Ok("entry added".to_string())
                }
                FormKind::Edit => {
                    let existing = entry.context("the edited entry vanished")?;
                    let from = parse_time(&texts[1])?;
                    let end = if texts[2].is_empty() {
                        existing.time_interval.end
                    } else {
                        Some(parse_time(&texts[2])?)
                    };
                    if let Some(end) = end
                        && end <= from
                    {
                        bail!("'To' must be after 'From'");
                    }
                    // A project picker showing none keeps the original project
                    // (workspaces can require one; archived ones aren't listed).
                    let pid = project_id.or_else(|| existing.project_id.clone());
                    let mut body = json!({
                        "start": to_api(from),
                        "description": texts[0],
                        "billable": existing.billable,
                    });
                    if let Some(end) = end {
                        body["end"] = json!(to_api(end));
                    }
                    if let Some(pid) = &pid {
                        body["projectId"] = json!(pid);
                    }
                    if pid == existing.project_id
                        && let Some(tid) = &existing.task_id
                    {
                        body["taskId"] = json!(tid);
                    }
                    self.ctx.client.update_time_entry(&self.ctx.workspace_id, &existing.id, &body)?;
                    Ok("entry updated".to_string())
                }
            }
        })();

        match result {
            Ok(msg) => {
                self.mode = Mode::Normal;
                self.invalidate();
                self.set_status(msg, false);
            }
            Err(e) => {
                if let Mode::Form(form) = &mut self.mode {
                    form.error = Some(format!("{e:#}"));
                }
            }
        }
    }
}
