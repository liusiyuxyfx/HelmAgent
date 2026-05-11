use anyhow::{anyhow, bail, Context, Result};
use directories::BaseDirs;
use std::env;
use std::fs;
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

pub fn canonical_helm_agent_home() -> Result<PathBuf> {
    let home = helm_agent_home()?;
    if !home.is_absolute() {
        bail!("HELM_AGENT_HOME must be absolute: {}", home.display());
    }

    fs::create_dir_all(&home)
        .with_context(|| format!("create HelmAgent home {}", home.display()))?;
    let canonical = home
        .canonicalize()
        .with_context(|| format!("canonicalize HelmAgent home {}", home.display()))?;
    let metadata = fs::metadata(&canonical)
        .with_context(|| format!("inspect HelmAgent home {}", canonical.display()))?;
    if !metadata.is_dir() {
        bail!(
            "HELM_AGENT_HOME is not a directory: {}",
            canonical.display()
        );
    }

    Ok(canonical)
}
