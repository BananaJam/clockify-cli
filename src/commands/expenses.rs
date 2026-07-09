use std::io::IsTerminal;
use std::path::PathBuf;

use anyhow::{Context, Result, bail};
use chrono::{Datelike, Days, Local, NaiveDate};
use colored::Colorize;
use dialoguer::Confirm;
use dialoguer::theme::ColorfulTheme;
use serde_json::json;

use super::{short_id, styled_id};
use crate::commands::submit::{self, Period, PeriodWindow};
use crate::config::Ctx;
use crate::models::{ApprovalRequest, Expense, ExpenseCategory, ExpenseChangeField, ExpenseDraft};
use crate::output;
use crate::resolve;
use crate::time::{day_range, parse_date};

pub struct ListArgs {
    pub week: bool,
    pub month: bool,
    pub from: Option<String>,
    pub to: Option<String>,
    pub json: bool,
}

pub struct AddArgs {
    pub amount: f64,
    pub category: String,
    pub date: String,
    pub project: String,
    pub task: Option<String>,
    pub notes: Option<String>,
    pub billable: bool,
    pub file: Option<PathBuf>,
    pub json: bool,
}

pub struct EditArgs {
    pub id: String,
    pub amount: Option<f64>,
    pub category: Option<String>,
    pub date: Option<String>,
    pub project: Option<String>,
    pub task: Option<String>,
    pub notes: Option<String>,
    pub billable: Option<bool>,
    pub file: Option<PathBuf>,
    pub json: bool,
}

pub struct SubmitArgs {
    pub week: bool,
    pub month: bool,
    pub from: Option<String>,
    pub period: Option<Period>,
    pub resubmit: bool,
    pub yes: bool,
    pub json: bool,
}

#[derive(Debug, Clone)]
pub struct ExpenseSubmissionSummary {
    pub window: PeriodWindow,
    pub expense_count: usize,
    pub total: f64,
    pub period_start: chrono::DateTime<chrono::Utc>,
}

pub fn list(ctx: &Ctx, args: ListArgs) -> Result<()> {
    let (from, to) = list_range(
        args.week,
        args.month,
        args.from.as_deref(),
        args.to.as_deref(),
    )?;
    let mut expenses = expenses_in_range(ctx, from, to)?;
    expenses.sort_by_key(|expense| expense.date);

    if args.json {
        output::print(&serde_json::Value::Array(
            expenses.iter().map(output::expense_json).collect(),
        ));
        return Ok(());
    }

    if expenses.is_empty() {
        println!("No expenses between {from} and {to}.");
        return Ok(());
    }

    print_expenses(&expenses, from, to);
    Ok(())
}

pub fn categories(ctx: &Ctx, all: bool, as_json: bool) -> Result<()> {
    let mut categories = ctx.client.expense_categories(&ctx.workspace_id, false)?;
    if all {
        categories.extend(ctx.client.expense_categories(&ctx.workspace_id, true)?);
        categories.sort_by_key(|category| category.id.clone());
        categories.dedup_by(|a, b| a.id == b.id);
        categories.sort_by_key(|category| category.name.to_lowercase());
    }
    if as_json {
        output::print(&serde_json::Value::Array(
            categories
                .iter()
                .map(|category| {
                    json!({
                        "id": category.id,
                        "name": category.name,
                        "archived": category.archived,
                        "has_unit_price": category.has_unit_price,
                        "price_in_cents": category.price_in_cents,
                        "unit": category.unit,
                    })
                })
                .collect(),
        ));
        return Ok(());
    }
    if categories.is_empty() {
        println!("No expense categories found.");
        return Ok(());
    }
    for category in categories {
        let flags = if category.archived {
            " (archived)".dimmed().to_string()
        } else if category.has_unit_price {
            match category.unit.as_deref() {
                Some(unit) if !unit.is_empty() => format!(" ({unit})").dimmed().to_string(),
                _ => " (unit price)".dimmed().to_string(),
            }
        } else {
            String::new()
        };
        println!("{}  {}{}", short_id(&category.id), category.name, flags);
    }
    Ok(())
}

pub fn add(ctx: &Ctx, args: AddArgs) -> Result<()> {
    if args.amount <= 0.0 {
        bail!("--amount must be greater than zero");
    }
    let category = resolve_category(ctx, &args.category)?;
    let project = resolve::project(ctx, &args.project)?;
    let task = args
        .task
        .as_deref()
        .map(|task| resolve::task(ctx, &project.id, task))
        .transpose()?;
    let date = parse_date(&args.date)?;
    let draft = ExpenseDraft {
        amount: args.amount,
        category_id: category.id,
        date: expense_date(date)?,
        user_id: ctx.user_id.clone(),
        project_id: Some(project.id),
        task_id: task.map(|task| task.id),
        notes: args.notes.filter(|notes| !notes.is_empty()),
        billable: args.billable,
        file: args.file,
        change_fields: Vec::new(),
    };
    let expense = friendly_expense_result(ctx.client.create_expense(&ctx.workspace_id, &draft))?;
    print_expense_result("Added", &expense, args.json);
    Ok(())
}

pub fn show(ctx: &Ctx, id: &str, as_json: bool) -> Result<()> {
    let expense = resolve_expense(ctx, id)?;
    if as_json {
        output::print(&output::expense_json(&expense));
    } else {
        let date = expense.date;
        print_expenses(&[expense], date, date);
    }
    Ok(())
}

pub fn edit(ctx: &Ctx, args: EditArgs) -> Result<()> {
    let existing = resolve_expense(ctx, &args.id)?;
    if existing.locked {
        bail!("expense is locked and cannot be edited");
    }

    let mut change_fields = Vec::new();
    let amount = match args.amount {
        Some(amount) if amount < 0.0 => bail!("--amount must not be negative"),
        Some(amount) => {
            change_fields.push(ExpenseChangeField::Amount);
            amount
        }
        None => existing.total,
    };
    let category_id = match args.category.as_deref() {
        Some(category) => {
            change_fields.push(ExpenseChangeField::Category);
            resolve_category(ctx, category)?.id
        }
        None => existing
            .category_id()
            .context("existing expense has no category id; pass --category")?
            .to_string(),
    };
    let date = match args.date.as_deref() {
        Some(date) => {
            change_fields.push(ExpenseChangeField::Date);
            parse_date(date)?
        }
        None => existing.date,
    };
    let project_id = match args.project.as_deref() {
        Some(project) => {
            change_fields.push(ExpenseChangeField::Project);
            Some(resolve::project(ctx, project)?.id)
        }
        None => existing.project_id().map(ToString::to_string),
    };
    let task_id = match args.task.as_deref() {
        Some(task) => {
            let project_id = project_id
                .as_deref()
                .context("--task requires the expense to have a project or --project")?;
            change_fields.push(ExpenseChangeField::Task);
            Some(resolve::task(ctx, project_id, task)?.id)
        }
        None if args.project.is_some() => None,
        None => existing.task_id().map(ToString::to_string),
    };
    let notes = match args.notes {
        Some(notes) => {
            change_fields.push(ExpenseChangeField::Notes);
            Some(notes)
        }
        None => existing.notes.clone(),
    };
    let billable = match args.billable {
        Some(billable) => {
            change_fields.push(ExpenseChangeField::Billable);
            billable
        }
        None => existing.billable,
    };
    if args.file.is_some() {
        change_fields.push(ExpenseChangeField::File);
    }
    if change_fields.is_empty() {
        bail!("nothing to update");
    }

    let draft = ExpenseDraft {
        amount,
        category_id,
        date: expense_date(date)?,
        user_id: ctx.user_id.clone(),
        project_id,
        task_id,
        notes,
        billable,
        file: args.file,
        change_fields,
    };
    let expense = friendly_expense_result(ctx.client.update_expense(
        &ctx.workspace_id,
        &existing.id,
        &draft,
    ))?;
    print_expense_result("Updated", &expense, args.json);
    Ok(())
}

pub fn delete(ctx: &Ctx, id: &str, yes: bool, as_json: bool) -> Result<()> {
    let expense = resolve_expense(ctx, id)?;
    if !yes {
        if !std::io::stdin().is_terminal() {
            bail!("refusing to prompt for confirmation without a terminal — pass -y/--yes");
        }
        let confirmed = Confirm::with_theme(&ColorfulTheme::default())
            .with_prompt(format!(
                "Delete expense {} for {}?",
                format_amount(expense.total),
                expense.date
            ))
            .default(false)
            .interact()?;
        if !confirmed {
            if as_json {
                output::print(&serde_json::Value::Null);
            } else {
                println!("Aborted.");
            }
            return Ok(());
        }
    }
    ctx.client.delete_expense(&ctx.workspace_id, &expense.id)?;
    if as_json {
        output::print(&json!({ "deleted": expense.id }));
    } else {
        println!(
            "{} Deleted expense {}",
            "✓".green().bold(),
            short_id(&expense.id)
        );
    }
    Ok(())
}

pub fn submit(ctx: &Ctx, args: SubmitArgs) -> Result<()> {
    let window = submit::select_window(args.week, args.month, args.from.as_deref(), args.period)?;
    let summary = summarize(ctx, window)?;
    if !args.yes {
        if !std::io::stdin().is_terminal() {
            bail!("refusing to prompt for confirmation without a terminal — pass -y/--yes");
        }
        let action = if args.resubmit { "Resubmit" } else { "Submit" };
        let confirmed = Confirm::with_theme(&ColorfulTheme::default())
            .with_prompt(format!(
                "{action} {} expense approval for {} – {} ({} expenses, {})?",
                summary.window.period,
                summary.window.from,
                summary.window.to,
                summary.expense_count,
                format_amount(summary.total)
            ))
            .default(false)
            .interact()?;
        if !confirmed {
            if args.json {
                output::print(&serde_json::Value::Null);
            } else {
                println!("Aborted.");
            }
            return Ok(());
        }
    }

    let approval = submit_summary(ctx, &summary, args.resubmit)?;
    print_submit_result(&summary, &approval, args.resubmit, args.json);
    Ok(())
}

pub fn summarize(ctx: &Ctx, window: PeriodWindow) -> Result<ExpenseSubmissionSummary> {
    let expenses = expenses_in_range(ctx, window.from, window.to)?;
    let (period_start, _) = day_range(window.from, window.to)?;
    summarize_expenses(window, period_start, &expenses)
}

pub fn summarize_expenses(
    window: PeriodWindow,
    period_start: chrono::DateTime<chrono::Utc>,
    expenses: &[Expense],
) -> Result<ExpenseSubmissionSummary> {
    if expenses.is_empty() {
        bail!("no expenses to submit for {} – {}", window.from, window.to);
    }
    Ok(ExpenseSubmissionSummary {
        window,
        expense_count: expenses.len(),
        total: expenses.iter().map(|expense| expense.total).sum(),
        period_start,
    })
}

pub fn submit_summary(
    ctx: &Ctx,
    summary: &ExpenseSubmissionSummary,
    resubmit: bool,
) -> Result<ApprovalRequest> {
    if resubmit {
        ctx.client.resubmit_approval_request(
            &ctx.workspace_id,
            summary.window.period,
            summary.period_start,
        )
    } else {
        ctx.client.submit_approval_request(
            &ctx.workspace_id,
            summary.window.period,
            summary.period_start,
        )
    }
}

pub fn resolve_category_from_slice(
    categories: &[ExpenseCategory],
    reference: &str,
) -> Result<ExpenseCategory> {
    pick_category(categories, reference).cloned()
}

/// Format a USD amount for display, e.g. `$12.50`.
pub fn format_amount(amount: f64) -> String {
    format!("${amount:.2}")
}

fn list_range(
    week: bool,
    month: bool,
    from: Option<&str>,
    to: Option<&str>,
) -> Result<(NaiveDate, NaiveDate)> {
    let today = Local::now().date_naive();
    let (from, to) = match (week, month, from, to) {
        (true, false, None, None) => {
            let monday = today - Days::new(today.weekday().num_days_from_monday() as u64);
            (monday, today)
        }
        (false, true, None, None) => (today.with_day(1).context("date out of range")?, today),
        (false, false, Some(from), to) => (
            parse_date(from)?,
            to.map(parse_date).transpose()?.unwrap_or(today),
        ),
        (false, false, None, None) => (today, today),
        (false, false, None, Some(_)) => bail!("--to requires --from"),
        _ => bail!("choose only one range: --week, --month, or --from/--to"),
    };
    if from > to {
        bail!("--from must not be after --to");
    }
    Ok((from, to))
}

pub fn expenses_in_range(ctx: &Ctx, from: NaiveDate, to: NaiveDate) -> Result<Vec<Expense>> {
    Ok(ctx
        .client
        .expenses(&ctx.workspace_id, &ctx.user_id)?
        .into_iter()
        .filter(|expense| expense.date >= from && expense.date <= to)
        .collect())
}

fn resolve_category(ctx: &Ctx, reference: &str) -> Result<ExpenseCategory> {
    let categories = ctx.client.expense_categories(&ctx.workspace_id, false)?;
    resolve_category_from_slice(&categories, reference)
}

fn pick_category<'a>(
    categories: &'a [ExpenseCategory],
    reference: &str,
) -> Result<&'a ExpenseCategory> {
    let needle = reference.trim();
    if needle.is_empty() {
        bail!("category is required");
    }
    if looks_like_id(needle)
        && let Some(category) = categories.iter().find(|category| category.id == needle)
    {
        return Ok(category);
    }
    let lower = needle.to_lowercase();
    if let Some(category) = categories
        .iter()
        .find(|category| category.name.to_lowercase() == lower)
    {
        return Ok(category);
    }
    let matches: Vec<&ExpenseCategory> = categories
        .iter()
        .filter(|category| category.name.to_lowercase().contains(&lower))
        .collect();
    match matches.as_slice() {
        [one] => Ok(one),
        [] => {
            bail!("no expense category matches '{reference}' — run `clockify expenses categories`")
        }
        many => bail!(
            "'{reference}' is ambiguous — matching expense categories: {}",
            many.iter()
                .map(|category| category.name.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        ),
    }
}

fn resolve_expense(ctx: &Ctx, reference: &str) -> Result<Expense> {
    let needle = reference.trim().to_lowercase();
    if looks_like_id(&needle) {
        return ctx.client.expense(&ctx.workspace_id, &needle);
    }
    if needle.is_empty() || !needle.chars().all(|c| c.is_ascii_hexdigit()) {
        bail!(
            "'{reference}' is not an expense id — use the full 24-character id or a hex id suffix \
             from `clockify expenses`"
        );
    }
    let matches: Vec<Expense> = ctx
        .client
        .expenses(&ctx.workspace_id, &ctx.user_id)?
        .into_iter()
        .filter(|expense| expense.id.ends_with(&needle))
        .collect();
    match matches.len() {
        1 => Ok(matches.into_iter().next().unwrap()),
        0 => bail!("no expense has an id ending in '{needle}' — check `clockify expenses`"),
        _ => bail!("'{needle}' matches several expenses; use the full id"),
    }
}

fn expense_date(date: NaiveDate) -> Result<chrono::DateTime<chrono::Utc>> {
    day_range(date, date).map(|(start, _)| start)
}

fn friendly_expense_result<T>(result: Result<T>) -> Result<T> {
    result.map_err(|err| {
        let msg = format!("{err:#}");
        let lower = msg.to_lowercase();
        if lower.contains("file") || lower.contains("receipt") {
            anyhow::anyhow!(
                "Clockify requires a receipt file for this expense; pass --file <path>"
            )
        } else if lower.contains("category") {
            anyhow::anyhow!("Clockify rejected the expense category; run `clockify expenses categories` and pick a valid category")
        } else if lower.contains("project") {
            anyhow::anyhow!("Clockify rejected the expense project; pass a valid active project")
        } else {
            err
        }
    })
}

fn print_expense_result(verb: &str, expense: &Expense, as_json: bool) {
    if as_json {
        output::print(&output::expense_json(expense));
        return;
    }
    println!(
        "{} {verb} expense {} for {} ({})",
        "✓".green().bold(),
        format_amount(expense.total).bold(),
        expense.date,
        short_id(&expense.id)
    );
}

fn print_expenses(expenses: &[Expense], from: NaiveDate, to: NaiveDate) {
    let id_lens =
        crate::resolve::unique_suffix_lens(expenses.iter().map(|expense| expense.id.as_str()));
    let amount_w = expenses
        .iter()
        .map(|expense| format_amount(expense.total).len())
        .max()
        .unwrap_or(0);
    println!("{}", format!("Expenses {from} – {to}").bold());
    for expense in expenses {
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
        let marker = if expense.billable { "billable" } else { "" };
        println!(
            "  {}  {:>amount_w$}  {:<18}  {:<18}  {:<10} {:<12} {}",
            expense.date,
            format_amount(expense.total).bold(),
            category.blue(),
            project,
            marker,
            file.dimmed(),
            notes
        );
        println!(
            "      {}",
            styled_id(&expense.id, id_lens.get(&expense.id).copied().unwrap_or(6))
        );
    }
    let total: f64 = expenses.iter().map(|expense| expense.total).sum();
    println!(
        "{} expenses, total {}",
        expenses.len(),
        format_amount(total).bold()
    );
}

fn print_submit_result(
    summary: &ExpenseSubmissionSummary,
    approval: &ApprovalRequest,
    resubmit: bool,
    as_json: bool,
) {
    let state = approval
        .status
        .as_ref()
        .map(|status| status.state.as_str())
        .unwrap_or("submitted");
    if as_json {
        let status = approval.status.as_ref();
        output::print(&json!({
            "id": approval.id,
            "state": state,
            "note": status.and_then(|s| s.note.as_deref()),
            "updated_at": status.and_then(|s| s.updated_at.map(|dt| dt.to_rfc3339())),
            "period": summary.window.period.as_api_str(),
            "from": summary.window.from.to_string(),
            "to": summary.window.to.to_string(),
            "expense_count": summary.expense_count,
            "total_amount": summary.total,
            "currency": "USD",
            "resubmitted": resubmit,
            "workspace_id": approval.workspace_id,
            "date_range": approval.date_range.as_ref().map(|range| json!({
                "start": range.start.to_rfc3339(),
                "end": range.end.to_rfc3339(),
            })),
        }));
        return;
    }

    let verb = if resubmit { "Resubmitted" } else { "Submitted" };
    println!(
        "{} {verb} {} expense approval for {} – {} ({} expenses, {})",
        "✓".green().bold(),
        summary.window.period,
        summary.window.from,
        summary.window.to,
        summary.expense_count,
        format_amount(summary.total).bold()
    );
    println!("  request {} · {}", approval.id.yellow(), state);
}

fn looks_like_id(s: &str) -> bool {
    s.len() == 24 && s.chars().all(|c| c.is_ascii_hexdigit())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn category(id: &str, name: &str) -> ExpenseCategory {
        ExpenseCategory {
            id: id.to_string(),
            name: name.to_string(),
            archived: false,
            has_unit_price: false,
            price_in_cents: None,
            unit: None,
        }
    }

    #[test]
    fn resolves_category_by_exact_name() {
        let categories = vec![category("abc", "Meals"), category("def", "Taxi")];
        let picked = resolve_category_from_slice(&categories, "meals").unwrap();
        assert_eq!(picked.id, "abc");
    }

    #[test]
    fn rejects_ambiguous_category_substrings() {
        let categories = vec![category("abc", "Meal"), category("def", "Meals client")];
        assert!(resolve_category_from_slice(&categories, "mea").is_err());
    }

    #[test]
    fn list_range_requires_from_for_to() {
        assert!(list_range(false, false, None, Some("2026-07-01")).is_err());
    }

    #[test]
    fn amount_format_shows_usd_with_cents() {
        assert_eq!(format_amount(12.0), "$12.00");
        assert_eq!(format_amount(12.5), "$12.50");
    }
}
