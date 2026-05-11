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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TmuxSessionState {
    Alive,
    Missing,
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
                tmux_session = shell_quote(&tmux_session),
                cwd = shell_quote(&dispatch.cwd.display().to_string()),
                command = shell_quote(adapter.command)
            ),
            attach_command: format!("tmux attach -t {}", shell_quote(&tmux_session)),
            resume_command,
            tmux_session,
        }
    }

    pub fn launch(&self, dispatch: &DispatchPlan) -> Result<LaunchPreview> {
        let preview = self.dry_run(dispatch);
        let adapter = RuntimeAdapter::for_runtime(dispatch.runtime);
        let output = Command::new(&self.tmux_bin)
            .arg("new-session")
            .arg("-d")
            .arg("-s")
            .arg(&preview.tmux_session)
            .arg("-c")
            .arg(&dispatch.cwd)
            .arg(adapter.command)
            .output()
            .with_context(|| {
                format!(
                    "failed to start tmux session {} using {}",
                    preview.tmux_session,
                    self.tmux_bin.display()
                )
            })?;

        if !output.status.success() {
            bail!(
                "failed to start tmux session {}: tmux exited with status {}{}",
                preview.tmux_session,
                output.status,
                tmux_output_context(&output.stdout, &output.stderr)
            );
        }

        Ok(preview)
    }

    pub fn send_keys(&self, session: &str, message: &str) -> Result<()> {
        let output = Command::new(&self.tmux_bin)
            .arg("send-keys")
            .arg("-t")
            .arg(format!("={session}"))
            .arg(message)
            .arg("Enter")
            .output()
            .with_context(|| {
                format!(
                    "failed to send keys to tmux session {session} using {}",
                    self.tmux_bin.display()
                )
            })?;

        if !output.status.success() {
            bail!(
                "failed to send keys to tmux session {session}: tmux exited with status {}{}",
                output.status,
                tmux_output_context(&output.stdout, &output.stderr)
            );
        }

        Ok(())
    }

    pub fn session_state(&self, session: &str) -> Result<TmuxSessionState> {
        let output = Command::new(&self.tmux_bin)
            .arg("has-session")
            .arg("-t")
            .arg(format!("={session}"))
            .output()
            .with_context(|| {
                format!(
                    "failed to inspect tmux session {session} using {}",
                    self.tmux_bin.display()
                )
            })?;

        if output.status.success() {
            Ok(TmuxSessionState::Alive)
        } else {
            Ok(TmuxSessionState::Missing)
        }
    }
}

fn tmux_output_context(stdout: &[u8], stderr: &[u8]) -> String {
    let stdout = String::from_utf8_lossy(stdout).trim().to_string();
    let stderr = String::from_utf8_lossy(stderr).trim().to_string();
    match (stdout.is_empty(), stderr.is_empty()) {
        (true, true) => String::new(),
        (true, false) => format!("; stderr: {stderr}"),
        (false, true) => format!("; stdout: {stdout}"),
        (false, false) => format!("; stderr: {stderr}; stdout: {stdout}"),
    }
}

impl Default for Launcher {
    fn default() -> Self {
        Self::new()
    }
}

fn shell_quote(value: &str) -> String {
    if !value.is_empty() && value.chars().all(is_shell_safe_char) {
        return value.to_string();
    }

    format!("'{}'", value.replace('\'', "'\\''"))
}

fn is_shell_safe_char(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || matches!(ch, '/' | '.' | '_' | '-' | ':' | '+' | '=' | ',')
}
