use anyhow::{anyhow, Result};
use directories::BaseDirs;
use std::env;
use std::path::PathBuf;

pub const HELM_AGENT_HOME_ENV: &str = "HELM_AGENT_HOME";

pub fn helm_agent_home() -> Result<PathBuf> {
    if let Some(path) = env::var_os(HELM_AGENT_HOME_ENV) {
        return Ok(PathBuf::from(path));
    }

    let dirs = BaseDirs::new()
        .ok_or_else(|| anyhow!("could not resolve a home directory for HelmAgent"))?;

    Ok(dirs.home_dir().join(".helm-agent"))
}
