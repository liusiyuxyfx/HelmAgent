use crate::adapter::RuntimeAdapter;
use crate::domain::AgentRuntime;
use crate::paths::HELM_AGENT_HOME_ENV;
use crate::runtime_profile::RuntimeProfile;
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
    runtime_commands: RuntimeCommandOverrides,
    helm_agent_home: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct RuntimeCommandOverrides {
    claude: Option<String>,
    codex: Option<String>,
    opencode: Option<String>,
    claude_resume: Option<String>,
    codex_resume: Option<String>,
    opencode_resume: Option<String>,
}

impl Launcher {
    pub fn new() -> Self {
        Self {
            tmux_bin: env::var_os("HELM_AGENT_TMUX_BIN")
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from("tmux")),
            runtime_commands: RuntimeCommandOverrides::from_env(),
            helm_agent_home: env::var_os(HELM_AGENT_HOME_ENV)
                .map(|home| home.to_string_lossy().to_string()),
        }
    }

    pub fn with_tmux_bin(tmux_bin: PathBuf) -> Self {
        Self {
            tmux_bin,
            runtime_commands: RuntimeCommandOverrides::default(),
            helm_agent_home: None,
        }
    }

    pub fn with_runtime_command_override(
        tmux_bin: PathBuf,
        runtime: AgentRuntime,
        command: String,
    ) -> Self {
        let mut runtime_commands = RuntimeCommandOverrides::default();
        runtime_commands.set(runtime, command);
        Self {
            tmux_bin,
            runtime_commands,
            helm_agent_home: None,
        }
    }

    pub fn with_tmux_bin_and_helm_agent_home(tmux_bin: PathBuf, helm_agent_home: String) -> Self {
        Self {
            tmux_bin,
            runtime_commands: RuntimeCommandOverrides::default(),
            helm_agent_home: Some(helm_agent_home),
        }
    }

    pub fn with_helm_agent_home(mut self, helm_agent_home: String) -> Self {
        self.helm_agent_home = Some(helm_agent_home);
        self
    }

    pub fn with_runtime_profile(mut self, profile: &RuntimeProfile) -> Self {
        self.runtime_commands = RuntimeCommandOverrides::from_profile_and_env(profile);
        self
    }

    pub fn dry_run(&self, dispatch: &DispatchPlan) -> LaunchPreview {
        let adapter = RuntimeAdapter::for_runtime(dispatch.runtime);
        let runtime_command = self.runtime_command(&adapter);
        let tmux_session = format!(
            "helm-agent-{task_id}-{runtime}",
            task_id = dispatch.task_id,
            runtime = dispatch.runtime.as_str()
        );
        let resume_command = self.runtime_resume_command(&adapter, runtime_command);

        let env_args = self
            .helm_agent_home
            .as_ref()
            .map(|home| {
                format!(
                    " -e {}",
                    shell_quote(&format!("{HELM_AGENT_HOME_ENV}={home}"))
                )
            })
            .unwrap_or_default();

        LaunchPreview {
            start_command: format!(
                "tmux new-session -d{env_args} -s {tmux_session} -c {cwd} {command}",
                tmux_session = shell_quote(&tmux_session),
                cwd = shell_quote(&dispatch.cwd.display().to_string()),
                command = shell_quote(runtime_command)
            ),
            attach_command: format!("tmux attach -t {}", shell_quote(&tmux_session)),
            resume_command,
            tmux_session,
        }
    }

    pub fn launch(&self, dispatch: &DispatchPlan) -> Result<LaunchPreview> {
        let preview = self.dry_run(dispatch);
        let adapter = RuntimeAdapter::for_runtime(dispatch.runtime);
        let runtime_command = self.runtime_command(&adapter);
        let mut command = Command::new(&self.tmux_bin);
        command.arg("new-session").arg("-d");
        if let Some(home) = &self.helm_agent_home {
            command
                .arg("-e")
                .arg(format!("{HELM_AGENT_HOME_ENV}={home}"));
        }
        let output = command
            .arg("-s")
            .arg(&preview.tmux_session)
            .arg("-c")
            .arg(&dispatch.cwd)
            .arg(runtime_command)
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

    fn runtime_command<'a>(&'a self, adapter: &'a RuntimeAdapter) -> &'a str {
        self.runtime_commands
            .get(adapter.runtime)
            .unwrap_or(adapter.command)
    }

    fn runtime_resume_command(
        &self,
        adapter: &RuntimeAdapter,
        runtime_command: &str,
    ) -> Option<String> {
        if let Some(resume) = self.runtime_commands.get_resume(adapter.runtime) {
            return Some(resume.to_string());
        }
        if !adapter.native_resume_available {
            return None;
        }
        if runtime_command == adapter.command {
            return Some(adapter.native_resume_template.to_string());
        }

        let suffix = adapter
            .native_resume_template
            .strip_prefix(adapter.command)
            .unwrap_or("");
        Some(format!("{runtime_command}{suffix}"))
    }

    pub fn send_keys(&self, session: &str, message: &str) -> Result<()> {
        let output = Command::new(&self.tmux_bin)
            .arg("send-keys")
            .arg("-t")
            .arg(format!("={session}:"))
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

impl RuntimeCommandOverrides {
    fn from_env() -> Self {
        Self {
            claude: read_command_override("HELM_AGENT_CLAUDE_COMMAND"),
            codex: read_command_override("HELM_AGENT_CODEX_COMMAND"),
            opencode: read_command_override("HELM_AGENT_OPENCODE_COMMAND"),
            claude_resume: read_command_override("HELM_AGENT_CLAUDE_RESUME_COMMAND"),
            codex_resume: read_command_override("HELM_AGENT_CODEX_RESUME_COMMAND"),
            opencode_resume: read_command_override("HELM_AGENT_OPENCODE_RESUME_COMMAND"),
        }
    }

    fn from_profile_and_env(profile: &RuntimeProfile) -> Self {
        let mut runtime_commands = Self::from_profile(profile);
        runtime_commands.apply_env();
        runtime_commands
    }

    fn from_profile(profile: &RuntimeProfile) -> Self {
        let mut runtime_commands = Self::default();
        for runtime in [
            AgentRuntime::Claude,
            AgentRuntime::Codex,
            AgentRuntime::OpenCode,
        ] {
            if let Some(entry) = profile.entry(runtime) {
                if let Some(command) = entry.command.clone().and_then(normalize_command_override) {
                    runtime_commands.set(runtime, command);
                }
                if let Some(resume) = entry.resume.clone().and_then(normalize_command_override) {
                    runtime_commands.set_resume(runtime, resume);
                }
            }
        }
        runtime_commands
    }

    fn apply_env(&mut self) {
        self.overlay(Self::from_env());
    }

    fn overlay(&mut self, other: Self) {
        if other.claude.is_some() {
            self.claude = other.claude;
        }
        if other.codex.is_some() {
            self.codex = other.codex;
        }
        if other.opencode.is_some() {
            self.opencode = other.opencode;
        }
        if other.claude_resume.is_some() {
            self.claude_resume = other.claude_resume;
        }
        if other.codex_resume.is_some() {
            self.codex_resume = other.codex_resume;
        }
        if other.opencode_resume.is_some() {
            self.opencode_resume = other.opencode_resume;
        }
    }

    fn get(&self, runtime: AgentRuntime) -> Option<&str> {
        match runtime {
            AgentRuntime::Claude => self.claude.as_deref(),
            AgentRuntime::Codex => self.codex.as_deref(),
            AgentRuntime::OpenCode => self.opencode.as_deref(),
            AgentRuntime::Acp => None,
        }
    }

    fn get_resume(&self, runtime: AgentRuntime) -> Option<&str> {
        match runtime {
            AgentRuntime::Claude => self.claude_resume.as_deref(),
            AgentRuntime::Codex => self.codex_resume.as_deref(),
            AgentRuntime::OpenCode => self.opencode_resume.as_deref(),
            AgentRuntime::Acp => None,
        }
    }

    fn set(&mut self, runtime: AgentRuntime, command: String) {
        let command = normalize_command_override(command);
        match runtime {
            AgentRuntime::Claude => self.claude = command,
            AgentRuntime::Codex => self.codex = command,
            AgentRuntime::OpenCode => self.opencode = command,
            AgentRuntime::Acp => {}
        }
    }

    fn set_resume(&mut self, runtime: AgentRuntime, command: String) {
        let command = normalize_command_override(command);
        match runtime {
            AgentRuntime::Claude => self.claude_resume = command,
            AgentRuntime::Codex => self.codex_resume = command,
            AgentRuntime::OpenCode => self.opencode_resume = command,
            AgentRuntime::Acp => {}
        }
    }
}

fn read_command_override(name: &str) -> Option<String> {
    env::var(name).ok().and_then(normalize_command_override)
}

fn normalize_command_override(command: String) -> Option<String> {
    let command = command.trim();
    if command.is_empty() {
        None
    } else {
        Some(command.to_string())
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
