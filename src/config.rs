use std::fs;
use std::path::PathBuf;

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};

use crate::api;

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Config {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub workspace_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub workspace_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_name: Option<String>,
}

impl Config {
    pub fn path() -> Result<PathBuf> {
        let dir = dirs::config_dir().context("could not determine the config directory")?;
        Ok(dir.join("clockify").join("config.toml"))
    }

    pub fn load() -> Result<Config> {
        let path = Self::path()?;
        if !path.exists() {
            return Ok(Config::default());
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
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&path, fs::Permissions::from_mode(0o600))?;
        }
        Ok(())
    }

    /// API key with the CLOCKIFY_API_KEY env var taking precedence.
    pub fn resolve_api_key(&self) -> Option<String> {
        std::env::var("CLOCKIFY_API_KEY")
            .ok()
            .filter(|k| !k.trim().is_empty())
            .or_else(|| self.api_key.clone())
    }
}

/// Everything an authenticated command needs.
pub struct Ctx {
    pub client: api::Client,
    pub workspace_id: String,
    pub user_id: String,
}

impl Ctx {
    pub fn load() -> Result<Ctx> {
        let mut cfg = Config::load()?;
        let Some(key) = cfg.resolve_api_key() else {
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

        Ok(Ctx { client, workspace_id, user_id })
    }
}
