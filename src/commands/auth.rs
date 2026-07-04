use std::io::IsTerminal;

use anyhow::{Context, Result, bail};
use colored::Colorize;
use dialoguer::theme::ColorfulTheme;
use dialoguer::{Confirm, Input, Password, Select};

use crate::api;
use crate::config::{Config, api_key_from_env, op_available, op_read};
use crate::models::User;

fn mask(key: &str) -> String {
    if key.len() <= 8 {
        "****".to_string()
    } else {
        format!("{}…{}", &key[..4], &key[key.len() - 4..])
    }
}

enum KeySource {
    Plain(String),
    OnePassword(String),
}

pub fn wizard() -> Result<()> {
    if !std::io::stdin().is_terminal() {
        bail!(
            "`clockify auth` is an interactive wizard and needs a terminal — \
             run it yourself, or set the CLOCKIFY_API_KEY environment variable"
        );
    }
    let theme = ColorfulTheme::default();
    let mut cfg = Config::load()?;

    println!("{}", "Welcome to the Clockify CLI setup!".bold());
    println!();
    println!("You'll need a Clockify API key. To get one:");
    println!("  1. Open {}", "https://app.clockify.me/user/preferences#advanced".cyan());
    println!("  2. Scroll to the {} section", "API".bold());
    println!("  3. Click {} and copy the key", "Generate".bold());
    println!();

    if cfg.api_key.is_some() || cfg.api_key_ref.is_some() {
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

    let choice = Select::with_theme(&theme)
        .with_prompt("How do you want to provide the API key?")
        .items(["Paste it manually", "Read it from 1Password (via the op CLI)"])
        .default(0)
        .interact()?;
    let (client, user, source) = match choice {
        0 => prompt_for_key(&theme)?,
        _ => prompt_for_op_ref(&theme)?,
    };
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

    match source {
        KeySource::Plain(key) => {
            cfg.api_key = Some(key);
            cfg.api_key_ref = None;
        }
        KeySource::OnePassword(reference) => {
            cfg.api_key_ref = Some(reference);
            cfg.api_key = None;
        }
    }
    cfg.user_id = Some(user.id);
    cfg.user_name = Some(user.name);
    cfg.workspace_id = Some(workspace.id);
    cfg.workspace_name = Some(workspace.name);
    cfg.save()?;

    println!();
    println!("{} You're all set! Config saved to {}", "✓".green().bold(), Config::path()?.display());
    if cfg.api_key_ref.is_some() {
        println!("  Your API key stays in 1Password — only the reference is stored on disk.");
    }
    println!();
    println!("Try these next:");
    println!("  {}   list your projects", "clockify projects".cyan());
    println!("  {}   start a timer", "clockify start \"writing code\" -p <project>".cyan());
    println!("  {}     see what's running", "clockify status".cyan());
    Ok(())
}

fn prompt_for_key(theme: &ColorfulTheme) -> Result<(api::Client, User, KeySource)> {
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
            Ok(user) => return Ok((client, user, KeySource::Plain(key))),
            Err(e) if attempt < 3 => {
                eprintln!("{} That key didn't work ({e}). Let's try again.", "✗".red());
            }
            Err(e) => return Err(e.context("could not validate the API key after 3 attempts")),
        }
    }
    unreachable!("loop either returns or errors on the last attempt")
}

/// `op` copies secret references to the clipboard wrapped in quotes —
/// accept them quoted or bare.
fn unquote(s: &str) -> &str {
    let s = s.trim();
    s.strip_prefix('"')
        .and_then(|s| s.strip_suffix('"'))
        .or_else(|| s.strip_prefix('\'').and_then(|s| s.strip_suffix('\'')))
        .map(str::trim)
        .unwrap_or(s)
}

fn prompt_for_op_ref(theme: &ColorfulTheme) -> Result<(api::Client, User, KeySource)> {
    let version = op_available()?;
    println!("Found the 1Password CLI (v{version}).");
    println!(
        "Save your API key in 1Password first, then copy its {}:",
        "secret reference".bold()
    );
    println!("  in the 1Password app, click the field's dropdown → {}", "Copy Secret Reference".bold());
    for attempt in 1..=3 {
        let reference: String = Input::with_theme(theme)
            .with_prompt("Secret reference (op://Vault/Item/field)")
            .validate_with(|s: &String| {
                if unquote(s).starts_with("op://") {
                    Ok(())
                } else {
                    Err("a secret reference starts with op://")
                }
            })
            .interact_text()
            .context("failed to read input — are you running in a real terminal?")?;
        let reference = unquote(&reference).to_string();
        let key = match op_read(&reference) {
            Ok(key) => key,
            Err(e) if attempt < 3 => {
                eprintln!("{} {e:#}. Let's try again.", "✗".red());
                continue;
            }
            Err(e) => return Err(e.context("could not read the key from 1Password after 3 attempts")),
        };
        let client = api::Client::new(key)?;
        match client.current_user() {
            Ok(user) => return Ok((client, user, KeySource::OnePassword(reference))),
            Err(e) if attempt < 3 => {
                eprintln!(
                    "{} 1Password gave us a key, but Clockify rejected it ({e}). Let's try again.",
                    "✗".red()
                );
            }
            Err(e) => return Err(e.context("could not validate the API key after 3 attempts")),
        }
    }
    unreachable!("loop either returns or errors on the last attempt")
}

pub fn status() -> Result<()> {
    let cfg = Config::load()?;
    if let Some(key) = api_key_from_env() {
        println!("API key:   {} (from CLOCKIFY_API_KEY)", mask(&key));
    } else if let Some(reference) = &cfg.api_key_ref {
        println!("API key:   from 1Password ({reference})");
    } else if let Some(key) = &cfg.api_key {
        println!("API key:   {} (stored in the config file)", mask(key));
    } else {
        println!("Not authenticated — run {} to get started.", "clockify auth".cyan());
        return Ok(());
    }
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

#[cfg(test)]
mod tests {
    use super::unquote;

    #[test]
    fn unquotes_references() {
        assert_eq!(unquote("op://V/I/f"), "op://V/I/f");
        assert_eq!(unquote("\"op://V/I/f\""), "op://V/I/f");
        assert_eq!(unquote("'op://V/I/f'"), "op://V/I/f");
        assert_eq!(unquote("  \"op://V/I/f\"  "), "op://V/I/f");
        // Unbalanced quotes are left alone rather than mangled.
        assert_eq!(unquote("\"op://V/I/f"), "\"op://V/I/f");
    }
}
