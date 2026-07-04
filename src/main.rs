mod api;
mod commands;
mod config;
mod models;
mod resolve;
mod time;

use anyhow::Result;
use clap::{Parser, Subcommand};
use colored::Colorize;

use config::Ctx;

#[derive(Parser)]
#[command(name = "clockify", version, about = "Track your work time in Clockify")]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
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
    },
    /// List tasks in a project
    Tasks {
        /// Project name or ID
        project: String,
    },
    /// Start a timer (stops any already-running one)
    Start {
        /// What you're working on
        description: Option<String>,
        /// Project name or ID
        #[arg(short, long)]
        project: Option<String>,
        /// Task name or ID (requires --project)
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
    Status,
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
        /// Project name or ID
        #[arg(short, long)]
        project: Option<String>,
        /// Task name or ID (requires --project)
        #[arg(short, long)]
        task: Option<String>,
        /// Mark the entry billable
        #[arg(long)]
        billable: bool,
    },
    /// Edit an existing time entry
    Edit {
        /// Entry ID or unique id suffix (see `clockify log`)
        id: String,
        /// New description
        #[arg(short, long)]
        description: Option<String>,
        /// New project name or ID
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
        /// Entry ID or unique id suffix (see `clockify log`)
        id: String,
        /// Skip the confirmation prompt
        #[arg(short, long)]
        yes: bool,
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
}

#[derive(Subcommand)]
enum AuthCmd {
    /// Show who you're authenticated as
    Status,
}

#[derive(Subcommand)]
enum WorkspacesCmd {
    /// Switch the default workspace
    Switch {
        /// Workspace name or ID
        workspace: String,
    },
}

fn run() -> Result<()> {
    let cli = Cli::parse();
    match cli.cmd {
        Cmd::Auth { cmd: None } => commands::auth::wizard(),
        Cmd::Auth { cmd: Some(AuthCmd::Status) } => commands::auth::status(),
        Cmd::Workspaces { cmd: None } => commands::workspaces::list(&Ctx::load()?),
        Cmd::Workspaces { cmd: Some(WorkspacesCmd::Switch { workspace }) } => {
            commands::workspaces::switch(&Ctx::load()?, &workspace)
        }
        Cmd::Projects { all } => commands::projects::run(&Ctx::load()?, all),
        Cmd::Tasks { project } => commands::tasks::run(&Ctx::load()?, &project),
        Cmd::Start { description, project, task, billable, at } => commands::start::run(
            &Ctx::load()?,
            commands::start::Args { description, project, task, billable, at },
        ),
        Cmd::Stop { at } => commands::stop::run(&Ctx::load()?, at),
        Cmd::Discard { yes } => commands::discard::run(&Ctx::load()?, yes),
        Cmd::Status => commands::status::run(&Ctx::load()?),
        // `today` is the default range; the flag exists only for explicitness.
        Cmd::Log { today: _, week, from, to, limit } => commands::log::run(
            &Ctx::load()?,
            commands::log::Args { week, from, to, limit },
        ),
        Cmd::Add { description, from, to, project, task, billable } => commands::add::run(
            &Ctx::load()?,
            commands::add::Args { description, from, to, project, task, billable },
        ),
        Cmd::Edit { id, description, project, from, to } => commands::edit::run(
            &Ctx::load()?,
            commands::edit::Args { id, description, project, from, to },
        ),
        Cmd::Delete { id, yes } => commands::delete::run(&Ctx::load()?, &id, yes),
        // `week` is the default range; the flag exists only for explicitness.
        Cmd::Report { week: _, month, from, to } => commands::report::run(
            &Ctx::load()?,
            commands::report::Args { month, from, to },
        ),
    }
}

fn main() {
    if let Err(e) = run() {
        eprintln!("{} {e:#}", "error:".red().bold());
        std::process::exit(1);
    }
}
