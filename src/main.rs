mod api;
mod commands;
mod config;
mod models;
mod output;
mod resolve;
mod status_cache;
mod time;
mod tui;

use std::io::IsTerminal;

use anyhow::Result;
use clap::{CommandFactory, Parser, Subcommand};
use colored::Colorize;

use config::Ctx;

#[derive(Parser)]
#[command(name = "clockify", version, about = "Track your work time in Clockify")]
#[command(after_help = "Running without a command opens the interactive TUI.")]
struct Cli {
    /// Print machine-readable JSON instead of styled output
    #[arg(long, global = true)]
    json: bool,
    #[command(subcommand)]
    cmd: Option<Cmd>,
}

#[derive(Subcommand)]
enum Cmd {
    /// Set up your Clockify credentials (interactive wizard)
    Auth {
        #[command(subcommand)]
        cmd: Option<AuthCmd>,
    },
    /// List workspaces or switch the default one
    Workspaces {
        #[command(subcommand)]
        cmd: Option<WorkspacesCmd>,
    },
    /// List projects in the current workspace
    Projects {
        /// Include archived projects
        #[arg(long)]
        all: bool,
        #[command(subcommand)]
        cmd: Option<ProjectsCmd>,
    },
    /// List tasks in a project
    Tasks {
        /// Project name, ID, or unique id suffix
        project: String,
    },
    /// Start a timer (stops any already-running one)
    Start {
        /// What you're working on
        description: String,
        /// Project name, ID, or unique id suffix (falls back to the default project)
        #[arg(short, long)]
        project: Option<String>,
        /// Create the entry without a project, ignoring the default
        #[arg(long, conflicts_with = "project")]
        no_project: bool,
        /// Task name, ID, or unique id suffix (requires a project)
        #[arg(short, long)]
        task: Option<String>,
        /// Mark the entry billable
        #[arg(long)]
        billable: bool,
        /// Start time, e.g. "09:30" or "yesterday 17:00" (default: now)
        #[arg(long)]
        at: Option<String>,
    },
    /// Stop the running timer
    Stop {
        /// Stop time, e.g. "17:30" (default: now)
        #[arg(long)]
        at: Option<String>,
    },
    /// Discard the running timer without saving the time
    Discard {
        /// Skip the confirmation prompt
        #[arg(short, long)]
        yes: bool,
    },
    /// Show the currently running timer
    Status {
        /// One-line cached output for shell prompts (empty when idle)
        #[arg(long)]
        short: bool,
    },
    /// List time entries (default: today)
    Log {
        /// Only today's entries (the default)
        #[arg(long, conflicts_with_all = ["week", "from", "to"])]
        today: bool,
        /// Entries from Monday through today
        #[arg(long, conflicts_with_all = ["from", "to"])]
        week: bool,
        /// Start date: YYYY-MM-DD, "today", or "yesterday"
        #[arg(long)]
        from: Option<String>,
        /// End date (default: today)
        #[arg(long)]
        to: Option<String>,
        /// Show at most N entries
        #[arg(short = 'n', long)]
        limit: Option<usize>,
    },
    /// Add a completed time entry manually
    Add {
        /// What you worked on
        description: String,
        /// Start time, e.g. "09:00" or "yesterday 14:00"
        #[arg(long)]
        from: String,
        /// End time, e.g. "12:30"
        #[arg(long)]
        to: String,
        /// Project name, ID, or unique id suffix (falls back to the default project)
        #[arg(short, long)]
        project: Option<String>,
        /// Create the entry without a project, ignoring the default
        #[arg(long, conflicts_with = "project")]
        no_project: bool,
        /// Task name, ID, or unique id suffix (requires a project)
        #[arg(short, long)]
        task: Option<String>,
        /// Mark the entry billable
        #[arg(long)]
        billable: bool,
    },
    /// Edit an existing time entry
    Edit {
        /// Entry ID, unique id suffix (see `clockify log`), or '@' for the running timer
        id: String,
        /// New description
        #[arg(short, long)]
        description: Option<String>,
        /// New project name, ID, or unique id suffix
        #[arg(short, long)]
        project: Option<String>,
        /// New start time
        #[arg(long)]
        from: Option<String>,
        /// New end time
        #[arg(long)]
        to: Option<String>,
    },
    /// Delete a time entry
    Delete {
        /// Entry ID, unique id suffix (see `clockify log`), or '@' for the running timer
        id: String,
        /// Skip the confirmation prompt
        #[arg(short, long)]
        yes: bool,
    },
    /// List and manage expenses
    Expenses {
        /// Only today's expenses (the default)
        #[arg(long, conflicts_with_all = ["week", "month", "from", "to"])]
        today: bool,
        /// Expenses from Monday through today
        #[arg(long, conflicts_with_all = ["month", "from", "to"])]
        week: bool,
        /// Expenses from the first of this month through today
        #[arg(long, conflicts_with_all = ["from", "to"])]
        month: bool,
        /// Start date: YYYY-MM-DD, "today", or "yesterday"
        #[arg(long)]
        from: Option<String>,
        /// End date (default: today)
        #[arg(long)]
        to: Option<String>,
        #[command(subcommand)]
        cmd: Option<ExpensesCmd>,
    },
    /// Time-per-project summary (default: this week)
    Report {
        /// This week (Monday through today, the default)
        #[arg(long, conflicts_with_all = ["month", "from", "to"])]
        week: bool,
        /// This month so far
        #[arg(long, conflicts_with_all = ["from", "to"])]
        month: bool,
        /// Start date: YYYY-MM-DD, "today", or "yesterday"
        #[arg(long)]
        from: Option<String>,
        /// End date (default: today)
        #[arg(long)]
        to: Option<String>,
    },
    /// Submit a time approval request (default: this month)
    Submit {
        /// Submit expenses instead of time entries
        #[arg(long)]
        expenses: bool,
        /// Submit this week (Monday through Sunday)
        #[arg(long, conflicts_with_all = ["month", "from", "period"])]
        week: bool,
        /// Submit this month (the default)
        #[arg(long, conflicts_with_all = ["week", "from", "period"])]
        month: bool,
        /// Approval period start date: YYYY-MM-DD, "today", or "yesterday"
        #[arg(long)]
        from: Option<String>,
        /// Approval period used with --from, or to override the default
        #[arg(long, value_enum)]
        period: Option<commands::submit::Period>,
        /// Re-submit rejected or withdrawn entries instead of creating a new request
        #[arg(long)]
        resubmit: bool,
        /// Skip the confirmation prompt
        #[arg(short, long)]
        yes: bool,
    },
    /// Manage the bundled agent skill (Claude Code, Codex)
    Skill {
        #[command(subcommand)]
        cmd: SkillCmd,
    },
}

#[derive(Subcommand)]
enum SkillCmd {
    /// Install the skill for every agent found (or the ones you pick)
    Install {
        /// Install into the current project instead of user-level
        #[arg(long)]
        project: bool,
        /// Install for Claude Code (~/.claude/skills)
        #[arg(long)]
        claude: bool,
        /// Install for Codex (~/.codex/skills, or .agents/skills with --project)
        #[arg(long)]
        codex: bool,
    },
    /// Print the skill file to stdout
    Show,
}

#[derive(Subcommand)]
enum AuthCmd {
    /// Show who you're authenticated as
    Status,
}

#[derive(Subcommand)]
enum ProjectsCmd {
    /// Show or set the default project for new entries
    Default {
        /// Project name, ID, or unique id suffix (omit to show the current default)
        project: Option<String>,
        /// Remove the default project
        #[arg(long, conflicts_with = "project")]
        clear: bool,
    },
}

#[derive(Subcommand)]
enum WorkspacesCmd {
    /// Switch the default workspace
    Switch {
        /// Workspace name, ID, or unique id suffix
        workspace: String,
    },
}

#[derive(Subcommand)]
enum ExpensesCmd {
    /// List expense categories
    Categories {
        /// Include archived categories
        #[arg(long)]
        all: bool,
    },
    /// Add an expense
    Add {
        /// Expense amount in USD, e.g. 12.50
        #[arg(long)]
        amount: f64,
        /// Expense category name, ID, or unique id suffix
        #[arg(long)]
        category: String,
        /// Expense date: YYYY-MM-DD, "today", or "yesterday"
        #[arg(long)]
        date: String,
        /// Project name, ID, or unique id suffix
        #[arg(short, long)]
        project: String,
        /// Task name, ID, or unique id suffix
        #[arg(short, long)]
        task: Option<String>,
        /// Expense notes
        #[arg(long)]
        notes: Option<String>,
        /// Mark the expense billable
        #[arg(long)]
        billable: bool,
        /// Receipt file path
        #[arg(long)]
        file: Option<std::path::PathBuf>,
    },
    /// Show one expense
    Show {
        /// Expense ID or unique id suffix
        id: String,
    },
    /// Edit an expense
    Edit {
        /// Expense ID or unique id suffix
        id: String,
        /// New amount in USD, e.g. 12.50
        #[arg(long)]
        amount: Option<f64>,
        /// New expense category name, ID, or unique id suffix
        #[arg(long)]
        category: Option<String>,
        /// New expense date: YYYY-MM-DD, "today", or "yesterday"
        #[arg(long)]
        date: Option<String>,
        /// New project name, ID, or unique id suffix
        #[arg(short, long)]
        project: Option<String>,
        /// New task name, ID, or unique id suffix
        #[arg(short, long)]
        task: Option<String>,
        /// New notes; pass an empty string to clear
        #[arg(long)]
        notes: Option<String>,
        /// Mark the expense billable
        #[arg(long, conflicts_with = "non_billable")]
        billable: bool,
        /// Mark the expense non-billable
        #[arg(long)]
        non_billable: bool,
        /// Replacement receipt file path
        #[arg(long)]
        file: Option<std::path::PathBuf>,
    },
    /// Delete an expense
    Delete {
        /// Expense ID or unique id suffix
        id: String,
        /// Skip the confirmation prompt
        #[arg(short, long)]
        yes: bool,
    },
    /// Submit an expense approval request
    Submit {
        /// Submit this week (Monday through Sunday)
        #[arg(long, conflicts_with_all = ["month", "from", "period"])]
        week: bool,
        /// Submit this month (the default)
        #[arg(long, conflicts_with_all = ["week", "from", "period"])]
        month: bool,
        /// Approval period start date: YYYY-MM-DD, "today", or "yesterday"
        #[arg(long)]
        from: Option<String>,
        /// Approval period used with --from, or to override the default
        #[arg(long, value_enum)]
        period: Option<commands::submit::Period>,
        /// Re-submit rejected or withdrawn expenses instead of creating a new request
        #[arg(long)]
        resubmit: bool,
        /// Skip the confirmation prompt
        #[arg(short, long)]
        yes: bool,
    },
}

fn run() -> Result<()> {
    let cli = Cli::parse();
    let json = cli.json;
    let Some(cmd) = cli.cmd else {
        // The TUI needs a terminal; an agent or script running bare
        // `clockify` gets usage instead of a ratatui panic.
        if !std::io::stdout().is_terminal() {
            Cli::command().print_help()?;
            return Ok(());
        }
        return tui::run();
    };
    match cmd {
        Cmd::Auth { cmd: None } => commands::auth::wizard(),
        Cmd::Auth {
            cmd: Some(AuthCmd::Status),
        } => commands::auth::status(),
        Cmd::Workspaces { cmd: None } => commands::workspaces::list(&Ctx::load()?, json),
        Cmd::Workspaces {
            cmd: Some(WorkspacesCmd::Switch { workspace }),
        } => commands::workspaces::switch(&Ctx::load()?, &workspace),
        Cmd::Projects { all, cmd: None } => commands::projects::run(&Ctx::load()?, all, json),
        Cmd::Projects {
            cmd: Some(ProjectsCmd::Default { project, clear }),
            ..
        } => commands::projects::default(&Ctx::load()?, project.as_deref(), clear),
        Cmd::Tasks { project } => commands::tasks::run(&Ctx::load()?, &project, json),
        Cmd::Start {
            description,
            project,
            no_project,
            task,
            billable,
            at,
        } => commands::start::run(
            &Ctx::load()?,
            commands::start::Args {
                description,
                project,
                no_project,
                task,
                billable,
                at,
                json,
            },
        ),
        Cmd::Stop { at } => commands::stop::run(&Ctx::load()?, at, json),
        Cmd::Discard { yes } => commands::discard::run(&Ctx::load()?, yes, json),
        Cmd::Status { short: true } => {
            commands::status::short();
            Ok(())
        }
        Cmd::Status { short: false } => commands::status::run(&Ctx::load()?, json),
        // `today` is the default range; the flag exists only for explicitness.
        Cmd::Log {
            today: _,
            week,
            from,
            to,
            limit,
        } => commands::log::run(
            &Ctx::load()?,
            commands::log::Args {
                week,
                from,
                to,
                limit,
                json,
            },
        ),
        Cmd::Add {
            description,
            from,
            to,
            project,
            no_project,
            task,
            billable,
        } => commands::add::run(
            &Ctx::load()?,
            commands::add::Args {
                description,
                from,
                to,
                project,
                no_project,
                task,
                billable,
                json,
            },
        ),
        Cmd::Edit {
            id,
            description,
            project,
            from,
            to,
        } => commands::edit::run(
            &Ctx::load()?,
            commands::edit::Args {
                id,
                description,
                project,
                from,
                to,
                json,
            },
        ),
        Cmd::Delete { id, yes } => commands::delete::run(&Ctx::load()?, &id, yes, json),
        // `today` is the default range; the flag exists only for explicitness.
        Cmd::Expenses {
            today: _,
            week,
            month,
            from,
            to,
            cmd: None,
        } => commands::expenses::list(
            &Ctx::load()?,
            commands::expenses::ListArgs {
                week,
                month,
                from,
                to,
                json,
            },
        ),
        Cmd::Expenses {
            cmd: Some(ExpensesCmd::Categories { all }),
            ..
        } => commands::expenses::categories(&Ctx::load()?, all, json),
        Cmd::Expenses {
            cmd:
                Some(ExpensesCmd::Add {
                    amount,
                    category,
                    date,
                    project,
                    task,
                    notes,
                    billable,
                    file,
                }),
            ..
        } => commands::expenses::add(
            &Ctx::load()?,
            commands::expenses::AddArgs {
                amount,
                category,
                date,
                project,
                task,
                notes,
                billable,
                file,
                json,
            },
        ),
        Cmd::Expenses {
            cmd: Some(ExpensesCmd::Show { id }),
            ..
        } => commands::expenses::show(&Ctx::load()?, &id, json),
        Cmd::Expenses {
            cmd:
                Some(ExpensesCmd::Edit {
                    id,
                    amount,
                    category,
                    date,
                    project,
                    task,
                    notes,
                    billable,
                    non_billable,
                    file,
                }),
            ..
        } => commands::expenses::edit(
            &Ctx::load()?,
            commands::expenses::EditArgs {
                id,
                amount,
                category,
                date,
                project,
                task,
                notes,
                billable: if billable {
                    Some(true)
                } else if non_billable {
                    Some(false)
                } else {
                    None
                },
                file,
                json,
            },
        ),
        Cmd::Expenses {
            cmd: Some(ExpensesCmd::Delete { id, yes }),
            ..
        } => commands::expenses::delete(&Ctx::load()?, &id, yes, json),
        Cmd::Expenses {
            cmd:
                Some(ExpensesCmd::Submit {
                    week,
                    month,
                    from,
                    period,
                    resubmit,
                    yes,
                }),
            ..
        } => commands::expenses::submit(
            &Ctx::load()?,
            commands::expenses::SubmitArgs {
                week,
                month,
                from,
                period,
                resubmit,
                yes,
                json,
            },
        ),
        // `week` is the default range; the flag exists only for explicitness.
        Cmd::Report {
            week: _,
            month,
            from,
            to,
        } => commands::report::run(
            &Ctx::load()?,
            commands::report::Args {
                month,
                from,
                to,
                json,
            },
        ),
        Cmd::Submit {
            expenses,
            week,
            month,
            from,
            period,
            resubmit,
            yes,
        } => {
            if expenses {
                commands::expenses::submit(
                    &Ctx::load()?,
                    commands::expenses::SubmitArgs {
                        week,
                        month,
                        from,
                        period,
                        resubmit,
                        yes,
                        json,
                    },
                )
            } else {
                commands::submit::run(
                    &Ctx::load()?,
                    commands::submit::Args {
                        week,
                        month,
                        from,
                        period,
                        resubmit,
                        yes,
                        json,
                    },
                )
            }
        }
        Cmd::Skill {
            cmd:
                SkillCmd::Install {
                    project,
                    claude,
                    codex,
                },
        } => commands::skill::install(project, claude, codex),
        Cmd::Skill {
            cmd: SkillCmd::Show,
        } => {
            commands::skill::show();
            Ok(())
        }
    }
}

fn main() {
    if let Err(e) = run() {
        eprintln!("{} {e:#}", "error:".red().bold());
        std::process::exit(1);
    }
}
