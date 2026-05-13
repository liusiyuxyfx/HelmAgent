use crate::store::TaskStore;
use agent_client_protocol::schema::{
    ContentBlock, InitializeRequest, NewSessionRequest, PermissionOption, PermissionOptionKind,
    PromptRequest, ProtocolVersion, RequestPermissionOutcome, RequestPermissionRequest,
    RequestPermissionResponse, SelectedPermissionOutcome, SessionNotification, TextContent,
};
use agent_client_protocol::{Agent, Client, ConnectionTo};
use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;
use tokio::process::Child;
use tokio_util::compat::{TokioAsyncReadCompatExt, TokioAsyncWriteCompatExt};

const DEFAULT_HANDOFF_TIMEOUT: Duration = Duration::from_secs(300);
const EXITED_AGENT_GRACE_PERIOD: Duration = Duration::from_millis(200);
pub const ACP_CHECK_PROMPT: &str =
    "HelmAgent ACP check: reply briefly to confirm this agent can receive a prompt.";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AcpAgentConfig {
    pub command: PathBuf,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub env: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resume_template: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AcpAgentsFile {
    #[serde(default)]
    pub agents: BTreeMap<String, AcpAgentConfig>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AcpPromptResult {
    pub session_id: String,
    pub stop_reason: String,
}

pub fn acp_agents_path(store: &TaskStore) -> PathBuf {
    store.root().join("acp").join("agents.yaml")
}

pub fn load_acp_agents(store: &TaskStore) -> Result<AcpAgentsFile> {
    let path = acp_agents_path(store);
    if !path.exists() {
        return Ok(AcpAgentsFile::default());
    }

    let content = fs::read_to_string(&path).with_context(|| format!("read {}", path.display()))?;
    let agents: AcpAgentsFile =
        serde_yaml::from_str(&content).with_context(|| format!("parse {}", path.display()))?;
    Ok(agents)
}

pub fn save_acp_agents(store: &TaskStore, agents: &AcpAgentsFile) -> Result<()> {
    let path = acp_agents_path(store);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).with_context(|| format!("create {}", parent.display()))?;
    }
    let yaml = serde_yaml::to_string(agents).context("serialize ACP agents")?;
    fs::write(&path, yaml).with_context(|| format!("write {}", path.display()))?;
    Ok(())
}

pub fn add_acp_agent(store: &TaskStore, name: &str, config: AcpAgentConfig) -> Result<()> {
    validate_agent_name(name)?;
    if config.command.as_os_str().is_empty() {
        bail!("ACP agent command cannot be empty");
    }
    if let Some(template) = config.resume_template.as_deref() {
        validate_resume_template(template)?;
    }

    let mut agents = load_acp_agents(store)?;
    agents.agents.insert(name.to_string(), config);
    save_acp_agents(store, &agents)
}

pub fn remove_acp_agent(store: &TaskStore, name: &str) -> Result<()> {
    let mut agents = load_acp_agents(store)?;
    if agents.agents.remove(name).is_none() {
        bail!("ACP agent not found: {name}");
    }
    save_acp_agents(store, &agents)
}

pub fn get_acp_agent(store: &TaskStore, name: &str) -> Result<AcpAgentConfig> {
    let agents = load_acp_agents(store)?;
    agents
        .agents
        .get(name)
        .cloned()
        .with_context(|| format!("ACP agent not found: {name}"))
}

pub fn render_acp_agent_list(agents: &AcpAgentsFile) -> String {
    if agents.agents.is_empty() {
        return "No ACP agents\n".to_string();
    }

    let mut output = String::new();
    for (name, config) in &agents.agents {
        let args = if config.args.is_empty() {
            "-".to_string()
        } else {
            config.args.join(" ")
        };
        output.push_str(&format!("{name}\t{}\t{args}\n", config.command.display()));
    }
    output
}

pub fn format_agent_command(config: &AcpAgentConfig) -> String {
    std::iter::once(shell_quote(&config.command.display().to_string()))
        .chain(config.args.iter().map(|arg| shell_quote(arg)))
        .collect::<Vec<_>>()
        .join(" ")
}

pub fn render_resume_command(
    config: &AcpAgentConfig,
    cwd: &Path,
    session_id: &str,
) -> Option<String> {
    let template = config.resume_template.as_deref()?;
    Some(
        template
            .replace("{cwd}", &shell_quote(&cwd.display().to_string()))
            .replace("{session_id}", &shell_quote(session_id)),
    )
}

pub fn is_successful_stop_reason(stop_reason: &str) -> bool {
    stop_reason == "EndTurn"
}

pub fn dispatch_prompt(
    config: &AcpAgentConfig,
    cwd: &Path,
    prompt: String,
) -> Result<AcpPromptResult> {
    if tokio::runtime::Handle::try_current().is_ok() {
        bail!("ACP sync dispatch cannot run inside an existing Tokio runtime");
    }

    let runtime = tokio::runtime::Runtime::new().context("create ACP tokio runtime")?;
    runtime.block_on(dispatch_prompt_async(
        config.clone(),
        cwd.to_path_buf(),
        prompt,
    ))
}

pub async fn dispatch_prompt_async(
    config: AcpAgentConfig,
    cwd: PathBuf,
    prompt: String,
) -> Result<AcpPromptResult> {
    let (child_stdin, child_stdout, mut child) = spawn_agent_process(&config, &cwd).await?;
    let transport =
        agent_client_protocol::ByteStreams::new(child_stdin.compat_write(), child_stdout.compat());

    let client = Client
        .builder()
        .name("helm-agent")
        .on_receive_notification(
            async move |_notification: SessionNotification, _cx| Ok(()),
            agent_client_protocol::on_receive_notification!(),
        )
        .on_receive_request(
            async move |request: RequestPermissionRequest, responder, _connection| {
                responder.respond(RequestPermissionResponse::new(reject_permission_outcome(
                    &request.options,
                )))
            },
            agent_client_protocol::on_receive_request!(),
        )
        .connect_with(transport, |connection: ConnectionTo<Agent>| async move {
            let init_response = connection
                .send_request(InitializeRequest::new(ProtocolVersion::V1))
                .block_task()
                .await?;
            if init_response.protocol_version != ProtocolVersion::V1 {
                return Err(
                    agent_client_protocol::Error::invalid_request().data(format!(
                        "ACP agent negotiated unsupported protocol version {:?}",
                        init_response.protocol_version
                    )),
                );
            }

            let session = connection
                .send_request(NewSessionRequest::new(&cwd))
                .block_task()
                .await?;
            let session_id = session.session_id;

            let prompt_response = connection
                .send_request(PromptRequest::new(
                    session_id.clone(),
                    vec![ContentBlock::Text(TextContent::new(prompt))],
                ))
                .block_task()
                .await?;

            Ok(AcpPromptResult {
                session_id: session_id.to_string(),
                stop_reason: format!("{:?}", prompt_response.stop_reason),
            })
        });

    tokio::pin!(client);
    let handoff_timeout = acp_handoff_timeout();
    let timeout = tokio::time::sleep(handoff_timeout);
    tokio::pin!(timeout);
    let result = tokio::select! {
        result = &mut client => {
            result.map_err(|error| anyhow::anyhow!("ACP protocol failed: {error:?}"))
        }
        status = child.wait() => {
            match status {
                Ok(status) => match tokio::time::timeout(EXITED_AGENT_GRACE_PERIOD, &mut client).await {
                    Ok(result) => result.map_err(|error| anyhow::anyhow!("ACP protocol failed: {error:?}")),
                    Err(_) => Err(anyhow::anyhow!("ACP agent exited before handoff: {status}")),
                },
                Err(error) => Err(anyhow::anyhow!("wait for ACP agent process: {error}")),
            }
        }
        _ = &mut timeout => {
            Err(anyhow::anyhow!(
                "ACP handoff timed out after {}ms",
                handoff_timeout.as_millis()
            ))
        }
    };

    terminate_child(&mut child).await;
    result
}

async fn spawn_agent_process(
    config: &AcpAgentConfig,
    cwd: &Path,
) -> Result<(
    tokio::process::ChildStdin,
    tokio::process::ChildStdout,
    Child,
)> {
    let mut command = tokio::process::Command::new(&config.command);
    command
        .args(&config.args)
        .envs(&config.env)
        .current_dir(cwd)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    #[cfg(unix)]
    command.process_group(0);

    let mut child = command
        .spawn()
        .with_context(|| format!("start ACP agent {}", config.command.display()))?;
    let stdin = child
        .stdin
        .take()
        .context("ACP agent stdin was not available")?;
    let stdout = child
        .stdout
        .take()
        .context("ACP agent stdout was not available")?;

    Ok((stdin, stdout, child))
}

fn acp_handoff_timeout() -> Duration {
    std::env::var("HELM_AGENT_ACP_TIMEOUT_MS")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .filter(|milliseconds| *milliseconds > 0)
        .map(Duration::from_millis)
        .unwrap_or(DEFAULT_HANDOFF_TIMEOUT)
}

async fn terminate_child(child: &mut Child) {
    #[cfg(unix)]
    {
        if let Some(pid) = child.id() {
            let process_group = -(pid as i32);
            // The child is started in a fresh process group, so this only targets
            // the configured ACP command and descendants it spawned.
            unsafe {
                libc::kill(process_group, libc::SIGTERM);
            }
            if tokio::time::timeout(Duration::from_millis(250), child.wait())
                .await
                .is_err()
            {
                unsafe {
                    libc::kill(process_group, libc::SIGKILL);
                }
                let _ = child.wait().await;
            }
            return;
        }
    }

    let _ = child.kill().await;
    let _ = child.wait().await;
}

fn reject_permission_outcome(options: &[PermissionOption]) -> RequestPermissionOutcome {
    options
        .iter()
        .find(|option| {
            matches!(
                option.kind,
                PermissionOptionKind::RejectOnce | PermissionOptionKind::RejectAlways
            )
        })
        .map(|option| {
            RequestPermissionOutcome::Selected(SelectedPermissionOutcome::new(
                option.option_id.clone(),
            ))
        })
        .unwrap_or(RequestPermissionOutcome::Cancelled)
}

pub fn parse_env_pair(pair: &str) -> Result<(String, String)> {
    let Some((key, value)) = pair.split_once('=') else {
        bail!("env must be KEY=VALUE");
    };
    if key.is_empty() {
        bail!("env key cannot be empty");
    }
    Ok((key.to_string(), value.to_string()))
}

fn validate_agent_name(name: &str) -> Result<()> {
    if name.is_empty() {
        bail!("ACP agent name cannot be empty");
    }
    if !name
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.'))
    {
        bail!("ACP agent name may only contain ASCII letters, digits, '-', '_', or '.'");
    }
    Ok(())
}

fn validate_resume_template(template: &str) -> Result<()> {
    if !template.contains("{session_id}") {
        bail!("ACP resume template must contain {{session_id}}");
    }
    if !template.contains("{cwd}") {
        bail!("ACP resume template must contain {{cwd}}");
    }
    Ok(())
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn permission_outcome_selects_reject_option_when_available() {
        let outcome = reject_permission_outcome(&[
            PermissionOption::new("allow", "Allow", PermissionOptionKind::AllowOnce),
            PermissionOption::new("reject", "Reject", PermissionOptionKind::RejectOnce),
        ]);

        match outcome {
            RequestPermissionOutcome::Selected(selected) => {
                assert_eq!(selected.option_id.to_string(), "reject");
            }
            _ => panic!("expected selected reject option"),
        }
    }

    #[test]
    fn permission_outcome_cancels_when_no_reject_option_exists() {
        let outcome = reject_permission_outcome(&[PermissionOption::new(
            "allow",
            "Allow",
            PermissionOptionKind::AllowOnce,
        )]);

        assert!(matches!(outcome, RequestPermissionOutcome::Cancelled));
    }
}
