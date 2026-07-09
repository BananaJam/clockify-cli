use chrono::{Duration, Local, NaiveDate};
use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{
    Block, BorderType, Borders, Clear, List, ListItem, ListState, Paragraph, Tabs,
};

use super::app::{App, Field, Form, Mode, TABS};
use super::theme::Theme;
use crate::models::{Expense, Project, TimeEntry};
use crate::time::{fmt_duration, fmt_duration_secs, fmt_local_time};

pub fn draw_splash(f: &mut Frame, t: &Theme) {
    f.render_widget(Block::new().style(Style::new().bg(t.bg).fg(t.fg)), f.area());
    let rows = Layout::vertical([
        Constraint::Percentage(45),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Min(0),
    ])
    .split(f.area());
    f.render_widget(
        Paragraph::new(Span::styled(
            "⏱ clockify",
            Style::new().fg(t.accent).add_modifier(Modifier::BOLD),
        ))
        .alignment(Alignment::Center),
        rows[1],
    );
    f.render_widget(
        Paragraph::new(Span::styled(
            "connecting… (your API key may need a 1Password unlock)",
            Style::new().fg(t.dim),
        ))
        .alignment(Alignment::Center),
        rows[2],
    );
}

pub fn draw(f: &mut Frame, app: &App) {
    let t = app.theme;
    f.render_widget(Block::new().style(Style::new().bg(t.bg).fg(t.fg)), f.area());

    let rows = Layout::vertical([
        Constraint::Length(1), // header
        Constraint::Length(1), // spacer
        Constraint::Length(1), // tabs
        Constraint::Length(1), // spacer
        Constraint::Min(0),    // body
        Constraint::Length(1), // status
        Constraint::Length(1), // footer
    ])
    .split(f.area());

    draw_header(f, app, rows[0]);
    draw_tabs(f, app, rows[2]);
    match app.tab {
        0 => draw_log(f, app, rows[4]),
        1 => draw_report(f, app, rows[4]),
        2 => draw_expenses(f, app, rows[4]),
        3 => draw_projects(f, app, rows[4]),
        _ => draw_workspaces(f, app, rows[4]),
    }
    draw_status(f, app, rows[5]);
    draw_footer(f, app, rows[6]);

    match &app.mode {
        Mode::Confirm { message, .. } => draw_confirm(f, t, message),
        Mode::Form(form) => draw_form(f, app, form),
        Mode::Normal => {}
    }
}

fn project_color(t: &Theme, p: Option<&Project>) -> Color {
    p.and_then(Project::rgb)
        .map(|(r, g, b)| Color::Rgb(r, g, b))
        .unwrap_or(t.accent)
}

fn draw_header(f: &mut Frame, app: &App, area: Rect) {
    let t = app.theme;
    let cols = Layout::horizontal([Constraint::Min(0), Constraint::Length(46)]).split(area);
    let left = Line::from(vec![
        Span::styled(
            " ⏱ clockify ",
            Style::new().fg(t.accent).add_modifier(Modifier::BOLD),
        ),
        Span::styled(format!("— {}", app.workspace_name), Style::new().fg(t.dim)),
    ]);
    f.render_widget(Paragraph::new(left), cols[0]);

    let right = match &app.running {
        Some(e) => {
            let desc = if e.description.is_empty() {
                "(no description)"
            } else {
                &e.description
            };
            Line::from(vec![
                Span::styled("▶ ", Style::new().fg(t.green).add_modifier(Modifier::BOLD)),
                Span::styled(truncate(desc, 26), Style::new().fg(t.fg)),
                Span::styled(
                    format!(" · {} ", fmt_duration_secs(e.duration())),
                    Style::new().fg(t.green),
                ),
            ])
        }
        None => Line::from(Span::styled("no timer running ", Style::new().fg(t.dim))),
    };
    f.render_widget(Paragraph::new(right).alignment(Alignment::Right), cols[1]);
}

fn draw_tabs(f: &mut Frame, app: &App, area: Rect) {
    let t = app.theme;
    let area = Rect {
        x: area.x + 1,
        width: area.width.saturating_sub(1),
        ..area
    };
    let tabs = Tabs::new(
        TABS.iter()
            .map(|s| Line::from(format!(" {s} ")))
            .collect::<Vec<_>>(),
    )
    .select(app.tab)
    .style(Style::new().fg(t.dim))
    .highlight_style(
        Style::new()
            .bg(t.selection_bg)
            .fg(t.fg)
            .add_modifier(Modifier::BOLD),
    )
    .divider(Span::styled(" · ", Style::new().fg(t.dim)));
    f.render_widget(tabs, area);
}

fn week_label(app: &App) -> String {
    let (from, to) = app.week_bounds();
    let label = match app.week_offset {
        0 => "this week",
        -1 => "last week",
        n => {
            return format!(
                "{} – {}  ({} weeks ago)",
                from.format("%-d %b"),
                to.format("%-d %b %Y"),
                -n
            );
        }
    };
    format!(
        "{} – {}  ({label})",
        from.format("%-d %b"),
        to.format("%-d %b %Y")
    )
}

fn report_label(app: &App) -> String {
    let (from, to) = app.report_bounds();
    match app.report_period {
        crate::commands::submit::Period::Weekly => week_label(app),
        crate::commands::submit::Period::Monthly => match app.month_offset {
            0 => format!(
                "{} – {}  (this month)",
                from.format("%-d %b"),
                to.format("%-d %b %Y")
            ),
            -1 => format!(
                "{} – {}  (last month)",
                from.format("%-d %b"),
                to.format("%-d %b %Y")
            ),
            n => format!(
                "{} – {}  ({} months ago)",
                from.format("%-d %b"),
                to.format("%-d %b %Y"),
                -n
            ),
        },
        crate::commands::submit::Period::SemiMonthly => {
            format!("{} – {}", from.format("%-d %b"), to.format("%-d %b %Y"))
        }
    }
}

fn draw_log(f: &mut Frame, app: &App, area: Rect) {
    let t = app.theme;
    let rows = Layout::vertical([Constraint::Length(1), Constraint::Min(0)]).split(area);
    f.render_widget(
        Paragraph::new(Span::styled(
            format!(" {}", week_label(app)),
            Style::new().fg(t.dim),
        )),
        rows[0],
    );

    if app.entries.is_empty() {
        let text = if app.loading {
            "loading…"
        } else {
            "no entries this week — press s to start a timer or a to add one"
        };
        let msg =
            Paragraph::new(Span::styled(text, Style::new().fg(t.dim))).alignment(Alignment::Center);
        f.render_widget(msg, rows[1]);
        return;
    }

    let dur_w = app
        .entries
        .iter()
        .map(|e| fmt_duration(e.duration()).len())
        .max()
        .unwrap_or(0);
    let proj_w = app
        .entries
        .iter()
        .map(|e| app.project_of(e).map_or(0, |p| p.name.chars().count()))
        .max()
        .unwrap_or(0);

    let today = Local::now().date_naive();
    let mut items: Vec<ListItem> = Vec::new();
    let mut selected_row = None;
    let mut current_day: Option<NaiveDate> = None;
    for (i, e) in app.entries.iter().enumerate() {
        let date = e.time_interval.start.with_timezone(&Local).date_naive();
        if current_day != Some(date) {
            current_day = Some(date);
            if !items.is_empty() {
                items.push(ListItem::new(Line::default()));
            }
            let day_total: Duration = app
                .entries
                .iter()
                .filter(|e| e.time_interval.start.with_timezone(&Local).date_naive() == date)
                .map(|e| e.duration())
                .sum();
            items.push(ListItem::new(Line::from(vec![
                Span::styled(
                    format!(" {}", day_name(date, today)),
                    Style::new().fg(t.accent).add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    format!("  · {}", fmt_duration(day_total)),
                    Style::new().fg(t.yellow),
                ),
            ])));
        }
        if i == app.sel_log {
            selected_row = Some(items.len());
        }
        items.push(ListItem::new(entry_line(app, e, dur_w, proj_w)));
    }

    let mut state = ListState::default();
    state.select(selected_row);
    let list = List::new(items).highlight_style(Style::new().bg(t.selection_bg));
    f.render_stateful_widget(list, rows[1], &mut state);
}

fn entry_line(app: &App, e: &TimeEntry, dur_w: usize, proj_w: usize) -> Line<'static> {
    let t = app.theme;
    let project = app.project_of(e);
    let running = e.time_interval.end.is_none();
    let end_span = match e.time_interval.end {
        Some(end) => Span::styled(fmt_local_time(end), Style::new().fg(t.fg)),
        None => Span::styled(
            "now  ".to_string(),
            Style::new().fg(t.green).add_modifier(Modifier::BOLD),
        ),
    };
    let dur_style = if running {
        Style::new().fg(t.green).add_modifier(Modifier::BOLD)
    } else {
        Style::new().fg(t.fg).add_modifier(Modifier::BOLD)
    };
    let desc = if e.description.is_empty() {
        Span::styled("(no description)".to_string(), Style::new().fg(t.dim))
    } else {
        Span::styled(e.description.clone(), Style::new().fg(t.fg))
    };
    Line::from(vec![
        Span::styled(
            format!("   {}–", fmt_local_time(e.time_interval.start)),
            Style::new().fg(t.fg),
        ),
        end_span,
        Span::styled(
            format!("  {:>dur_w$}", fmt_duration(e.duration())),
            dur_style,
        ),
        Span::styled(
            format!(
                "  {:<proj_w$}",
                project.map(|p| p.name.as_str()).unwrap_or("")
            ),
            Style::new().fg(project_color(t, project)),
        ),
        Span::raw("  "),
        desc,
    ])
}

fn day_name(date: NaiveDate, today: NaiveDate) -> String {
    let prefix = match today.signed_duration_since(date).num_days() {
        0 => "Today · ",
        1 => "Yesterday · ",
        _ => "",
    };
    format!("{prefix}{}", date.format("%A, %-d %B"))
}

fn draw_report(f: &mut Frame, app: &App, area: Rect) {
    let t = app.theme;
    let rows = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Min(0),
    ])
    .split(area);
    f.render_widget(
        Paragraph::new(Span::styled(
            format!(" {}", report_label(app)),
            Style::new().fg(t.dim),
        )),
        rows[0],
    );

    let entries = app.report_entries();
    if entries.is_empty() {
        let noun = if app.report_period == crate::commands::submit::Period::Monthly {
            "month"
        } else {
            "week"
        };
        let text = if app.report_loading() {
            "loading…".to_string()
        } else {
            format!("no entries this {noun}")
        };
        f.render_widget(
            Paragraph::new(Span::styled(text, Style::new().fg(t.dim))).alignment(Alignment::Center),
            rows[2],
        );
        return;
    }

    // Aggregate per project.
    let mut agg: Vec<(Option<&Project>, Duration)> = Vec::new();
    let mut total = Duration::zero();
    for e in entries {
        let project = app.project_of(e);
        total += e.duration();
        match agg
            .iter_mut()
            .find(|(p, _)| p.map(|p| &p.id) == project.map(|p| &p.id))
        {
            Some((_, d)) => *d += e.duration(),
            None => agg.push((project, e.duration())),
        }
    }
    agg.sort_by_key(|(_, d)| -d.num_seconds());
    let max_secs = agg.first().map_or(1, |(_, d)| d.num_seconds()).max(1);
    let name_w = agg
        .iter()
        .map(|(p, _)| {
            p.map(|p| p.name.chars().count())
                .unwrap_or("(no project)".len())
        })
        .max()
        .unwrap_or(0);
    let dur_w = agg
        .iter()
        .map(|(_, d)| fmt_duration(*d).len())
        .max()
        .unwrap_or(0);

    let mut lines = vec![Line::from(vec![
        Span::styled(" total ".to_string(), Style::new().fg(t.dim)),
        Span::styled(
            fmt_duration(total),
            Style::new().fg(t.fg).add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!("  across {} entries", entries.len()),
            Style::new().fg(t.dim),
        ),
    ])];
    if let Some(row) = app.report_approval() {
        let request = &row.approval_request;
        let state = request
            .status
            .as_ref()
            .map(|s| s.state.as_str())
            .unwrap_or("submitted");
        let note = request
            .status
            .as_ref()
            .and_then(|s| s.note.as_deref())
            .filter(|note| !note.is_empty())
            .map(|note| format!(" · {note}"))
            .unwrap_or_default();
        let updated = request
            .status
            .as_ref()
            .and_then(|s| s.updated_at)
            .map(|dt| {
                format!(
                    " · updated {}",
                    dt.with_timezone(&Local).format("%-d %b %H:%M")
                )
            })
            .unwrap_or_default();
        lines.push(Line::from(vec![
            Span::styled(" approval ".to_string(), Style::new().fg(t.dim)),
            Span::styled(
                state.to_string(),
                Style::new().fg(t.yellow).add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!(" · {} entries{note}{updated}", row.time_entries.len()),
                Style::new().fg(t.dim),
            ),
        ]));
    }
    lines.push(Line::default());
    for (project, dur) in &agg {
        let name = project
            .map(|p| p.name.clone())
            .unwrap_or_else(|| "(no project)".to_string());
        let share = 100.0 * dur.num_seconds() as f64 / total.num_seconds().max(1) as f64;
        let bar_len = ((dur.num_seconds() as f64 / max_secs as f64) * 30.0)
            .round()
            .max(1.0) as usize;
        let color = project_color(t, *project);
        lines.push(Line::from(vec![
            Span::styled(format!("  {name:<name_w$}"), Style::new().fg(color)),
            Span::styled(
                format!("  {:>dur_w$}", fmt_duration(*dur)),
                Style::new().fg(t.fg).add_modifier(Modifier::BOLD),
            ),
            Span::styled(format!("  {share:>3.0}%  "), Style::new().fg(t.yellow)),
            Span::styled("█".repeat(bar_len), Style::new().fg(color)),
        ]));
    }
    f.render_widget(Paragraph::new(lines), rows[2]);
}

fn draw_expenses(f: &mut Frame, app: &App, area: Rect) {
    let t = app.theme;
    let rows = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(2),
        Constraint::Min(0),
    ])
    .split(area);
    f.render_widget(
        Paragraph::new(Span::styled(
            format!(" {}", report_label(app)),
            Style::new().fg(t.dim),
        )),
        rows[0],
    );

    let total: f64 = app.expenses.iter().map(|expense| expense.total).sum();
    let mut summary = vec![Line::from(vec![
        Span::styled(" total ".to_string(), Style::new().fg(t.dim)),
        Span::styled(
            crate::commands::expenses::format_amount(total),
            Style::new().fg(t.fg).add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!("  across {} expenses", app.expenses.len()),
            Style::new().fg(t.dim),
        ),
    ])];
    if let Some(row) = app.expense_approval() {
        let state = row
            .approval_request
            .status
            .as_ref()
            .map(|status| status.state.as_str())
            .unwrap_or("submitted");
        let total = row.expense_total.unwrap_or_else(|| {
            row.expenses
                .iter()
                .map(|expense| expense.total)
                .sum::<f64>()
        });
        summary.push(Line::from(vec![
            Span::styled(" approval ".to_string(), Style::new().fg(t.dim)),
            Span::styled(
                state.to_string(),
                Style::new().fg(t.yellow).add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!(
                    " · {} expenses · {}",
                    row.expenses.len(),
                    crate::commands::expenses::format_amount(total)
                ),
                Style::new().fg(t.dim),
            ),
        ]));
    }
    f.render_widget(Paragraph::new(summary), rows[1]);

    if app.expenses.is_empty() {
        let text = if app.expenses_loading {
            "loading…"
        } else {
            "no expenses in this period"
        };
        f.render_widget(
            Paragraph::new(Span::styled(text, Style::new().fg(t.dim))).alignment(Alignment::Center),
            rows[2],
        );
        return;
    }

    let amount_w = app
        .expenses
        .iter()
        .map(|expense| crate::commands::expenses::format_amount(expense.total).len())
        .max()
        .unwrap_or(0);
    let category_w = app
        .expenses
        .iter()
        .map(|expense| {
            expense
                .category_name()
                .unwrap_or("(no category)")
                .chars()
                .count()
        })
        .max()
        .unwrap_or(0)
        .min(24);
    let project_w = app
        .expenses
        .iter()
        .map(|expense| expense.project_name().unwrap_or("").chars().count())
        .max()
        .unwrap_or(0)
        .min(24);

    let items: Vec<ListItem> = app
        .expenses
        .iter()
        .map(|expense| ListItem::new(expense_line(app, expense, amount_w, category_w, project_w)))
        .collect();
    let mut state = ListState::default();
    state.select(Some(app.sel_expense));
    let list = List::new(items).highlight_style(Style::new().bg(t.selection_bg));
    f.render_stateful_widget(list, rows[2], &mut state);
}

fn expense_line(
    app: &App,
    expense: &Expense,
    amount_w: usize,
    category_w: usize,
    project_w: usize,
) -> Line<'static> {
    let t = app.theme;
    let category = expense.category_name().unwrap_or("(no category)");
    let project = expense.project_name().unwrap_or("");
    let notes = expense
        .notes
        .as_deref()
        .filter(|notes| !notes.is_empty())
        .unwrap_or("");
    let file = expense
        .file_name
        .as_deref()
        .or(expense.file_id.as_deref())
        .unwrap_or("");
    let billable = if expense.billable { "billable" } else { "" };
    Line::from(vec![
        Span::styled(format!("   {}", expense.date), Style::new().fg(t.fg)),
        Span::styled(
            format!(
                "  {:>amount_w$}",
                crate::commands::expenses::format_amount(expense.total)
            ),
            Style::new().fg(t.fg).add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!("  {:<category_w$}", truncate(category, category_w.max(1))),
            Style::new().fg(t.accent),
        ),
        Span::styled(
            format!("  {:<project_w$}", truncate(project, project_w.max(1))),
            Style::new().fg(t.yellow),
        ),
        Span::styled(format!("  {:<8}", billable), Style::new().fg(t.dim)),
        Span::styled(
            format!("  {:<12}", truncate(file, 12)),
            Style::new().fg(t.dim),
        ),
        Span::styled(format!("  {}", truncate(notes, 36)), Style::new().fg(t.fg)),
    ])
}

fn draw_projects(f: &mut Frame, app: &App, area: Rect) {
    let t = app.theme;
    if app.projects.is_empty() {
        f.render_widget(
            Paragraph::new(Span::styled(
                "no projects in this workspace",
                Style::new().fg(t.dim),
            ))
            .alignment(Alignment::Center),
            area,
        );
        return;
    }
    let name_w = app
        .projects
        .iter()
        .map(|p| p.name.chars().count())
        .max()
        .unwrap_or(0);
    // Group by client, clientless last (projects are already name-sorted).
    let mut groups: Vec<(String, Vec<&Project>)> = Vec::new();
    for p in &app.projects {
        let key = p
            .client_name
            .clone()
            .filter(|c| !c.is_empty())
            .unwrap_or_else(|| "(no client)".to_string());
        match groups.iter_mut().find(|(name, _)| *name == key) {
            Some((_, g)) => g.push(p),
            None => groups.push((key, vec![p])),
        }
    }
    groups.sort_by(|(a, _), (b, _)| {
        (a == "(no client)")
            .cmp(&(b == "(no client)"))
            .then(a.to_lowercase().cmp(&b.to_lowercase()))
    });

    let mut lines = Vec::new();
    for (client, group) in &groups {
        lines.push(Line::from(vec![
            Span::styled(
                format!(" {client}"),
                Style::new().fg(t.accent).add_modifier(Modifier::BOLD),
            ),
            Span::styled(format!("  · {}", group.len()), Style::new().fg(t.yellow)),
        ]));
        for p in group {
            let mut flags = Vec::new();
            if !p.billable {
                flags.push("not billable");
            }
            if p.archived {
                flags.push("archived");
            }
            let flags = if flags.is_empty() {
                String::new()
            } else {
                format!("  ({})", flags.join(", "))
            };
            lines.push(Line::from(vec![
                Span::styled(
                    format!("   {:<name_w$}", p.name),
                    Style::new().fg(project_color(t, Some(p))),
                ),
                Span::styled(flags, Style::new().fg(t.dim)),
            ]));
        }
        lines.push(Line::default());
    }
    f.render_widget(Paragraph::new(lines), area);
}

fn draw_workspaces(f: &mut Frame, app: &App, area: Rect) {
    let t = app.theme;
    let items: Vec<ListItem> = app
        .workspaces
        .iter()
        .map(|w| {
            let current = w.id == app.ctx.workspace_id;
            let marker = if current {
                Span::styled(" ✓ ", Style::new().fg(t.green).add_modifier(Modifier::BOLD))
            } else {
                Span::raw("   ")
            };
            let name = if current {
                Span::styled(
                    w.name.clone(),
                    Style::new().fg(t.fg).add_modifier(Modifier::BOLD),
                )
            } else {
                Span::styled(w.name.clone(), Style::new().fg(t.fg))
            };
            ListItem::new(Line::from(vec![marker, name]))
        })
        .collect();
    let mut state = ListState::default();
    state.select(Some(app.sel_ws));
    let list = List::new(items).highlight_style(Style::new().bg(t.selection_bg));
    f.render_stateful_widget(list, area, &mut state);
}

fn draw_status(f: &mut Frame, app: &App, area: Rect) {
    let t = app.theme;
    if let Some((msg, is_error)) = &app.status {
        let style = if *is_error {
            Style::new().fg(t.red)
        } else {
            Style::new().fg(t.green)
        };
        f.render_widget(Paragraph::new(Span::styled(format!(" {msg}"), style)), area);
    }
}

fn draw_footer(f: &mut Frame, app: &App, area: Rect) {
    let t = app.theme;
    let keys = match &app.mode {
        Mode::Normal => match app.tab {
            0 => {
                "q quit · tab views · j/k move · h/l week · s start · x stop · X discard · a add · e edit · d delete · t theme"
            }
            1 => {
                "q quit · tab views · h/l period · m month · w week · S submit · R resubmit · s start · x stop · t theme"
            }
            2 => {
                "q quit · tab views · j/k move · h/l period · m month · w week · a add · e edit · d delete · S submit · R resubmit"
            }
            4 => "q quit · tab views · j/k move · enter switch workspace · t theme",
            _ => "q quit · tab views · s start · x stop · a add · t theme",
        },
        Mode::Confirm { .. } => "y confirm · n/esc cancel",
        Mode::Form(form) => match form.fields.get(form.focus) {
            Some(Field::Project { .. }) => {
                "←/→ pick project · tab next field · enter save · esc cancel"
            }
            Some(Field::Toggle { .. }) => "space toggle · tab next field · enter save · esc cancel",
            _ => "tab next field · enter save · esc cancel",
        },
    };
    f.render_widget(
        Paragraph::new(Span::styled(format!(" {keys}"), Style::new().fg(t.dim))),
        area,
    );
}

fn draw_confirm(f: &mut Frame, t: &Theme, message: &str) {
    // max() before min(): on very narrow terminals the available width can
    // drop below the aesthetic minimum, and clamp(min, max) would panic.
    let width = (message.chars().count() as u16 + 6)
        .max(30)
        .min(f.area().width.saturating_sub(2));
    let area = centered_rect(width, 5, f.area());
    f.render_widget(Clear, area);
    let block = Block::new()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::new().fg(t.red))
        .title(Span::styled(
            " Confirm ",
            Style::new().fg(t.red).add_modifier(Modifier::BOLD),
        ))
        .style(Style::new().bg(t.bg).fg(t.fg));
    let inner = block.inner(area);
    f.render_widget(block, area);
    let rows = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
    ])
    .split(inner);
    f.render_widget(
        Paragraph::new(message.to_string()).alignment(Alignment::Center),
        rows[0],
    );
    f.render_widget(
        Paragraph::new(Span::styled("y confirm · n cancel", Style::new().fg(t.dim)))
            .alignment(Alignment::Center),
        rows[2],
    );
}

fn draw_form(f: &mut Frame, app: &App, form: &Form) {
    let t = app.theme;
    let height = form.fields.len() as u16 + 4 + form.error.is_some() as u16;
    let area = centered_rect(56, height, f.area());
    f.render_widget(Clear, area);
    let block = Block::new()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::new().fg(t.accent))
        .title(Span::styled(
            form.title,
            Style::new().fg(t.accent).add_modifier(Modifier::BOLD),
        ))
        .style(Style::new().bg(t.bg).fg(t.fg));
    let inner = block.inner(area);
    f.render_widget(block, area);

    let mut lines = vec![Line::default()];
    let active = app.active_projects();
    for (i, field) in form.fields.iter().enumerate() {
        let focused = i == form.focus;
        let label_style = if focused {
            Style::new().fg(t.accent).add_modifier(Modifier::BOLD)
        } else {
            Style::new().fg(t.dim)
        };
        let mut spans = vec![Span::styled(
            format!(" {:<12} ", field_label(field)),
            label_style,
        )];
        match field {
            Field::Text { input, .. } => {
                if focused {
                    let (before, under, after) = input.split_at_cursor();
                    spans.push(Span::styled(before, Style::new().fg(t.fg)));
                    spans.push(Span::styled(
                        under,
                        Style::new().add_modifier(Modifier::REVERSED),
                    ));
                    spans.push(Span::styled(after, Style::new().fg(t.fg)));
                } else {
                    spans.push(Span::styled(
                        input.value().to_string(),
                        Style::new().fg(t.fg),
                    ));
                }
            }
            Field::Project { idx, .. } => {
                let (text, color) = match idx.and_then(|i| active.get(i)) {
                    Some(p) => (format!("‹ {} ›", p.name), project_color(t, Some(p))),
                    None => ("‹ (no project) ›".to_string(), t.dim),
                };
                spans.push(Span::styled(text, Style::new().fg(color)));
            }
            Field::Toggle { on, .. } => {
                let (text, color) = if *on { ("yes", t.green) } else { ("no", t.dim) };
                spans.push(Span::styled(text, Style::new().fg(color)));
            }
        }
        lines.push(Line::from(spans));
    }
    if let Some(err) = &form.error {
        lines.push(Line::from(Span::styled(
            format!(" {err}"),
            Style::new().fg(t.red),
        )));
    }
    f.render_widget(Paragraph::new(lines), inner);
}

fn field_label(field: &Field) -> &'static str {
    match field {
        Field::Text { label, .. } | Field::Project { label, .. } | Field::Toggle { label, .. } => {
            label
        }
    }
}

fn centered_rect(width: u16, height: u16, r: Rect) -> Rect {
    let width = width.min(r.width);
    let height = height.min(r.height);
    Rect {
        x: r.x + (r.width - width) / 2,
        y: r.y + (r.height - height) / 2,
        width,
        height,
    }
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let cut: String = s.chars().take(max.saturating_sub(1)).collect();
        format!("{cut}…")
    }
}
