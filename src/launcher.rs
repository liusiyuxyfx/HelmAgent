use crate::adapter::RuntimeAdapter;
use crate::domain::AgentRuntime;
use anyhow::{bail, Context, Result};
use std::env;
use std::path::PathBuf;
use std::process::Command;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DispatchPlan {
    pub task_id: String,
    pub runtime: AgentRuntime,
    pub cwd: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LaunchPreview {
    pub tmux_session: String,
    pub start_command: String,
    pub attach_command: String,
    pub resume_command: Option<String>,
}

#[derive(Debug, Clone)]
pub struct Launcher {
    tmux_bin: PathBuf,
}

impl Launcher {
    pub fn new() -> Self {
        Self {
            tmux_bin: env::var_os("HELM_AGENT_TMUX_BIN")
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from("tmux")),
        }
    }

    pub fn with_tmux_bin(tmux_bin: PathBuf) -> Self {
        Self { tmux_bin }
    }

    pub fn dry_run(&self, dispatch: &DispatchPlan) -> LaunchPreview {
        let adapter = RuntimeAdapter::for_runtime(dispatch.runtime);
        let tmux_session = format!(
            "helm-agent-{task_id}-{runtime}",
            task_id = dispatch.task_id,
            runtime = dispatch.runtime.as_str()
        );
        let resume_command = adapter
            .native_resume_available
            .then(|| adapter.native_resume_template.to_string());

        LaunchPreview {
            start_command: format!(
                "tmux new-session -d -s {tmux_session} -c {cwd} {command}",
                cwd = shell_quote(&dispatch.cwd.display().to_string()),
                command = shell_quote(adapter.command)
            ),
            attach_command: format!("tmux attach -t {tmux_session}"),
            resume_command,
            tmux_session,
        }
    }

    pub fn launch(&self, dispatch: &DispatchPlan) -> Result<LaunchPreview> {
        let preview = self.dry_run(dispatch);
        let adapter = RuntimeAdapter::for_runtime(dispatch.runtime);
        let status = Command::new(&self.tmux_bin)
            .arg("new-session")
            .arg("-d")
            .arg("-s")
            .arg(&preview.tmux_session)
            .arg("-c")
            .arg(&dispatch.cwd)
            .arg(adapter.command)
            .status()
            .with_context(|| {
                format!(
                    "failed to start tmux session {} using {}",
                    preview.tmux_session,
                    self.tmux_bin.display()
                )
            })?;

        if !status.success() {
            bail!(
                "failed to start tmux session {}: tmux exited with status {}",
                preview.tmux_session,
                status
            );
        }

        Ok(preview)
    }
}

impl Default for Launcher {
    fn default() -> Self {
        Self::new()
    }
}

fn shell_quote(value: &str) -> String {
    if value.chars().all(is_shell_safe_char) {
        return value.to_string();
    }

    format!("'{}'", value.replace('\'', "'\\''"))
}

fn is_shell_safe_char(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || matches!(ch, '/' | '.' | '_' | '-' | ':' | '+' | '=' | ',')
}
