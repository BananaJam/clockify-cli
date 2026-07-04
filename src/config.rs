use std::collections::HashMap;
use std::ffi::OsString;
use std::fs;
use std::path::PathBuf;
use std::process::Command;

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};

use crate::api;

/// A project used for new entries when none is given, keyed per workspace.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DefaultProject {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Config {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_key: Option<String>,
    /// 1Password secret reference (op://Vault/Item/field); resolved via `op read`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_key_ref: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub workspace_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub workspace_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_name: Option<String>,
    /// TUI color theme (see src/tui/theme.rs).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub theme: Option<String>,
    /// Default project per workspace id.
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub default_projects: HashMap<String, DefaultProject>,
}

fn config_file(xdg_config_home: Option<OsString>, home: Option<PathBuf>) -> Result<PathBuf> {
    let base = match xdg_config_home.filter(|d| !d.is_empty()) {
        Some(dir) => PathBuf::from(dir),
        None => home.context("could not determine your home directory")?.join(".config"),
    };
    Ok(base.join("clockify").join("config.toml"))
}

impl Config {
    pub fn path() -> Result<PathBuf> {
        config_file(std::env::var_os("XDG_CONFIG_HOME"), dirs::home_dir())
    }

    /// Where configs were stored before v0.0.2 (macOS: ~/Library/Application Support).
    fn legacy_path() -> Option<PathBuf> {
        Some(dirs::config_dir()?.join("clockify").join("config.toml"))
    }

    pub fn load() -> Result<Config> {
        let path = Self::path()?;
        if !path.exists() {
            match Self::legacy_path().filter(|p| p.exists() && *p != path) {
                Some(old) => migrate(&old, &path)?,
                None => return Ok(Config::default()),
            }
        }
        let raw = fs::read_to_string(&path)
            .with_context(|| format!("failed to read {}", path.display()))?;
        toml::from_str(&raw).with_context(|| format!("invalid config file {}", path.display()))
    }

    pub fn save(&self) -> Result<()> {
        let path = Self::path()?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("failed to create {}", parent.display()))?;
        }
        let raw = toml::to_string_pretty(self)?;
        fs::write(&path, raw).with_context(|| format!("failed to write {}", path.display()))?;
        restrict_permissions(&path)?;
        Ok(())
    }

    /// Resolve the API key: CLOCKIFY_API_KEY env var, then the 1Password
    /// reference, then the plaintext key from the config file.
    pub fn resolve_api_key(&self) -> Result<Option<String>> {
        if let Some(key) = api_key_from_env() {
            return Ok(Some(key));
        }
        if let Some(reference) = &self.api_key_ref {
            let key = op_read(reference)
                .with_context(|| format!("could not get the API key from 1Password ({reference})"))?;
            return Ok(Some(key));
        }
        Ok(self.api_key.clone())
    }
}

pub fn api_key_from_env() -> Option<String> {
    std::env::var("CLOCKIFY_API_KEY").ok().filter(|k| !k.trim().is_empty())
}

fn restrict_permissions(path: &std::path::Path) -> Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o600))?;
    }
    #[cfg(not(unix))]
    let _ = path;
    Ok(())
}

fn migrate(old: &std::path::Path, new: &std::path::Path) -> Result<()> {
    if let Some(parent) = new.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    if fs::rename(old, new).is_err() {
        fs::copy(old, new)
            .with_context(|| format!("failed to migrate config to {}", new.display()))?;
        fs::remove_file(old).ok();
    }
    restrict_permissions(new)?;
    eprintln!("note: moved config from {} to {}", old.display(), new.display());
    Ok(())
}

/// Fetch a secret from 1Password via the `op` CLI.
pub fn op_read(reference: &str) -> Result<String> {
    let output = match Command::new("op").args(["read", "--no-newline", reference]).output() {
        Ok(out) => out,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            bail!("the 1Password CLI (`op`) is not installed — get it with `brew install 1password-cli`")
        }
        Err(e) => return Err(e).context("failed to run the 1Password CLI"),
    };
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!("`op read {reference}` failed: {}", stderr.trim());
    }
    let key = String::from_utf8(output.stdout)
        .context("the 1Password CLI returned non-UTF-8 output")?
        .trim()
        .to_string();
    if key.is_empty() {
        bail!("1Password returned an empty value for {reference}");
    }
    Ok(key)
}

/// Check that the `op` CLI is available, returning its version.
pub fn op_available() -> Result<String> {
    let output = match Command::new("op").arg("--version").output() {
        Ok(out) => out,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            bail!("the 1Password CLI (`op`) is not installed — get it with `brew install 1password-cli`")
        }
        Err(e) => return Err(e).context("failed to run the 1Password CLI"),
    };
    if !output.status.success() {
        bail!("`op --version` failed — is the 1Password CLI set up correctly?");
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

/// Everything an authenticated command needs.
pub struct Ctx {
    pub client: api::Client,
    pub workspace_id: String,
    pub user_id: String,
    /// The configured default project for the current workspace, if any.
    pub default_project: Option<DefaultProject>,
}

impl Ctx {
    pub fn load() -> Result<Ctx> {
        let mut cfg = Config::load()?;
        let Some(key) = cfg.resolve_api_key()? else {
            bail!("not authenticated — run `clockify auth` first");
        };
        let client = api::Client::new(key)?;

        // Fill in workspace/user if missing (e.g. key supplied via env var only).
        if cfg.workspace_id.is_none() || cfg.user_id.is_none() {
            let user = client.current_user()?;
            cfg.user_id = Some(user.id);
            cfg.user_name = Some(user.name);
            if cfg.workspace_id.is_none() {
                cfg.workspace_id = user.active_workspace.or(user.default_workspace);
            }
            cfg.save()?;
        }

        let workspace_id = cfg
            .workspace_id
            .clone()
            .context("no workspace configured — run `clockify auth`")?;
        let user_id = cfg
            .user_id
            .clone()
            .context("no user configured — run `clockify auth`")?;
        let default_project = cfg.default_projects.get(&workspace_id).cloned();

        Ok(Ctx { client, workspace_id, user_id, default_project })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_path_honors_xdg_config_home() {
        let p = config_file(Some("/custom/xdg".into()), Some("/home/u".into())).unwrap();
        assert_eq!(p, PathBuf::from("/custom/xdg/clockify/config.toml"));
    }

    #[test]
    fn config_path_defaults_to_dot_config() {
        let p = config_file(None, Some("/home/u".into())).unwrap();
        assert_eq!(p, PathBuf::from("/home/u/.config/clockify/config.toml"));
        // An empty XDG_CONFIG_HOME counts as unset.
        let p = config_file(Some("".into()), Some("/home/u".into())).unwrap();
        assert_eq!(p, PathBuf::from("/home/u/.config/clockify/config.toml"));
    }
}
