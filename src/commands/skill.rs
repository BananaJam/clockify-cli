use std::fs;
use std::path::PathBuf;

use anyhow::{Context, Result, bail};
use colored::Colorize;

const SKILL_MD: &str = include_str!("../../skill/SKILL.md");

/// An agent that understands the SKILL.md format and where it looks for it.
struct Agent {
    name: &'static str,
    /// Home directory whose presence means the agent is installed.
    home: &'static str,
    /// Skill directory, relative to $HOME (user-level) …
    user_dir: &'static str,
    /// … or to the current directory (`--project`).
    project_dir: &'static str,
}

const AGENTS: &[Agent] = &[
    Agent {
        name: "Claude Code",
        home: ".claude",
        user_dir: ".claude/skills/clockify",
        project_dir: ".claude/skills/clockify",
    },
    // Codex reads personal skills from ~/.codex/skills and, in repos,
    // `.agents/skills` (the cross-agent standard location).
    Agent {
        name: "Codex",
        home: ".codex",
        user_dir: ".codex/skills/clockify",
        project_dir: ".agents/skills/clockify",
    },
];

/// Write the bundled skill for the selected agents — by default every agent
/// found on this machine — either user-level or into the current project.
pub fn install(project: bool, claude: bool, codex: bool) -> Result<()> {
    let home = dirs::home_dir().context("cannot determine your home directory")?;
    let wanted = |a: &Agent| match (claude, codex) {
        // No flags: whatever is actually installed.
        (false, false) => home.join(a.home).is_dir(),
        _ => (claude && a.home == ".claude") || (codex && a.home == ".codex"),
    };

    let targets: Vec<&Agent> = AGENTS.iter().filter(|a| wanted(a)).collect();
    if targets.is_empty() {
        bail!(
            "no supported agent found (looked for ~/.claude and ~/.codex) — \
             pass --claude or --codex to install anyway"
        );
    }

    for agent in targets {
        let dir = if project {
            PathBuf::from(agent.project_dir)
        } else {
            home.join(agent.user_dir)
        };
        fs::create_dir_all(&dir).with_context(|| format!("creating {}", dir.display()))?;
        let path = dir.join("SKILL.md");
        fs::write(&path, SKILL_MD).with_context(|| format!("writing {}", path.display()))?;
        println!(
            "{} {}: installed the skill to {}",
            "✓".green().bold(),
            agent.name.bold(),
            path.display()
        );
    }
    println!("New agent sessions pick it up automatically — reinstall after upgrades to refresh it.");
    Ok(())
}

pub fn show() {
    print!("{SKILL_MD}");
}
