use anyhow::{Context, Result, bail};
use colored::Colorize;
use dialoguer::theme::ColorfulTheme;
use dialoguer::{Confirm, Password, Select};

use crate::api;
use crate::config::Config;
use crate::models::User;

fn mask(key: &str) -> String {
    if key.len() <= 8 {
        "****".to_string()
    } else {
        format!("{}…{}", &key[..4], &key[key.len() - 4..])
    }
}

pub fn wizard() -> Result<()> {
    let theme = ColorfulTheme::default();
    let mut cfg = Config::load()?;

    println!("{}", "Welcome to the Clockify CLI setup!".bold());
    println!();
    println!("You'll need a Clockify API key. To get one:");
    println!("  1. Open {}", "https://app.clockify.me/user/preferences#advanced".cyan());
    println!("  2. Scroll to the {} section", "API".bold());
    println!("  3. Click {} and copy the key", "Generate".bold());
    println!();

    if cfg.api_key.is_some() {
        let overwrite = Confirm::with_theme(&theme)
            .with_prompt(format!(
                "You're already set up as {} — replace the existing credentials?",
                cfg.user_name.as_deref().unwrap_or("an existing user")
            ))
            .default(false)
            .interact()?;
        if !overwrite {
            println!("Keeping the existing setup.");
            return Ok(());
        }
    }

    let (client, user) = prompt_for_key(&theme)?;
    println!("{} Hi, {} ({})!", "✓".green().bold(), user.name.bold(), user.email);

    let workspaces = client.workspaces()?;
    let workspace = match workspaces.as_slice() {
        [] => bail!("your Clockify account has no workspaces"),
        [only] => {
            println!("Using your only workspace: {}", only.name.bold());
            only.clone()
        }
        many => {
            let names: Vec<&str> = many.iter().map(|w| w.name.as_str()).collect();
            let default = user
                .active_workspace
                .as_deref()
                .and_then(|id| many.iter().position(|w| w.id == id))
                .unwrap_or(0);
            let idx = Select::with_theme(&theme)
                .with_prompt("Which workspace do you want to track time in by default?")
                .items(&names)
                .default(default)
                .interact()?;
            many[idx].clone()
        }
    };

    cfg.user_id = Some(user.id);
    cfg.user_name = Some(user.name);
    cfg.workspace_id = Some(workspace.id);
    cfg.workspace_name = Some(workspace.name);
    cfg.save()?;

    println!();
    println!("{} You're all set! Config saved to {}", "✓".green().bold(), Config::path()?.display());
    println!();
    println!("Try these next:");
    println!("  {}   list your projects", "clockify projects".cyan());
    println!("  {}   start a timer", "clockify start \"writing code\" -p <project>".cyan());
    println!("  {}     see what's running", "clockify status".cyan());
    Ok(())
}

fn prompt_for_key(theme: &ColorfulTheme) -> Result<(api::Client, User)> {
    for attempt in 1..=3 {
        let key: String = Password::with_theme(theme)
            .with_prompt("Paste your API key (input is hidden)")
            .interact()
            .context("failed to read input — are you running in a real terminal?")?;
        let key = key.trim().to_string();
        if key.is_empty() {
            eprintln!("{} The key is empty, try again.", "✗".red());
            continue;
        }
        let client = api::Client::new(key.clone())?;
        match client.current_user() {
            Ok(user) => {
                let mut cfg = Config::load()?;
                cfg.api_key = Some(key);
                cfg.save()?;
                return Ok((client, user));
            }
            Err(e) if attempt < 3 => {
                eprintln!("{} That key didn't work ({e}). Let's try again.", "✗".red());
            }
            Err(e) => return Err(e.context("could not validate the API key after 3 attempts")),
        }
    }
    unreachable!("loop either returns or errors on the last attempt")
}

pub fn status() -> Result<()> {
    let cfg = Config::load()?;
    let from_env = std::env::var("CLOCKIFY_API_KEY").is_ok_and(|k| !k.trim().is_empty());
    let Some(key) = cfg.resolve_api_key() else {
        println!("Not authenticated — run {} to get started.", "clockify auth".cyan());
        return Ok(());
    };
    println!(
        "API key:   {}{}",
        mask(&key),
        if from_env { " (from CLOCKIFY_API_KEY)" } else { "" }
    );
    if let Some(name) = &cfg.user_name {
        println!("User:      {name}");
    }
    match (&cfg.workspace_name, &cfg.workspace_id) {
        (Some(name), Some(id)) => println!("Workspace: {name} ({id})"),
        (None, Some(id)) => println!("Workspace: {id}"),
        _ => println!("Workspace: not set — run {}", "clockify auth".cyan()),
    }
    Ok(())
}
