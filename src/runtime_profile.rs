use crate::domain::AgentRuntime;
use crate::store::TaskStore;
use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeProfile {
    #[serde(default)]
    pub runtimes: BTreeMap<String, RuntimeProfileEntry>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeProfileEntry {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resume: Option<String>,
}

impl RuntimeProfile {
    pub fn entry(&self, runtime: AgentRuntime) -> Option<&RuntimeProfileEntry> {
        self.runtimes.get(runtime.as_str())
    }

    pub fn set(
        &mut self,
        runtime: AgentRuntime,
        command: Option<String>,
        resume: Option<String>,
    ) -> Result<()> {
        if runtime == AgentRuntime::Acp {
            bail!("runtime profile does not apply to ACP agents");
        }
        let command = normalize_profile_value(command);
        let resume = normalize_profile_value(resume);
        if command.is_none() && resume.is_none() {
            bail!("runtime profile set requires a non-empty command or resume value");
        }

        let entry = self
            .runtimes
            .entry(runtime.as_str().to_string())
            .or_default();

        if let Some(command) = command {
            entry.command = Some(command);
        }
        if let Some(resume) = resume {
            entry.resume = Some(resume);
        }
        Ok(())
    }
}

pub fn runtime_profile_path(store: &TaskStore) -> PathBuf {
    store.root().join("runtime").join("profile.yaml")
}

pub fn load_runtime_profile(store: &TaskStore) -> Result<RuntimeProfile> {
    let path = runtime_profile_path(store);
    if !path.exists() {
        return Ok(RuntimeProfile::default());
    }

    let content = fs::read_to_string(&path)
        .with_context(|| format!("read runtime profile {}", path.display()))?;
    serde_yaml::from_str(&content)
        .with_context(|| format!("parse runtime profile {}", path.display()))
}

pub fn save_runtime_profile(store: &TaskStore, profile: &RuntimeProfile) -> Result<PathBuf> {
    let path = runtime_profile_path(store);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("create runtime profile directory {}", parent.display()))?;
    }

    let yaml = serde_yaml::to_string(profile).context("serialize runtime profile")?;
    fs::write(&path, yaml).with_context(|| format!("write runtime profile {}", path.display()))?;
    Ok(path)
}

fn normalize_profile_value(value: Option<String>) -> Option<String> {
    value.and_then(|value| {
        let value = value.trim();
        if value.is_empty() {
            None
        } else {
            Some(value.to_string())
        }
    })
}
