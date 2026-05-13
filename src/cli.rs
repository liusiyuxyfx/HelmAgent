use crate::acp_adapter::{self, AcpAgentConfig};
use crate::adapter::RuntimeAdapter;
use crate::brief;
use crate::domain::{AgentRuntime, ReviewState, RiskLevel, TaskEvent, TaskRecord, TaskStatus};
use crate::guidance::{self, GuidanceFile, GuidanceRuntime};
use crate::launcher::{DispatchPlan, Launcher};
use crate::output;
use crate::paths::canonical_helm_agent_home;
use crate::policy::{DispatchDecision, PolicyInput};
use crate::runtime_profile;
use crate::store::TaskStore;
use crate::task_actions::{self, MarkAction, ReviewAction};
use crate::web_board;
use anyhow::{bail, Context, Result};
use clap::{Args, Parser, Subcommand, ValueEnum};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command as ProcessCommand, Stdio};
use time::OffsetDateTime;

const CLAUDE_CODE_ACP_PRESET: &str = "claude-code";
const CLAUDE_CODE_ACP_RESUME_TEMPLATE: &str = "cd {cwd} && claude --resume {session_id}";

#[derive(Debug, Parser)]
#[command(name = "helm-agent")]
#[command(about = "HelmAgent local coordinator")]
pub struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    Task(TaskCommand),
    Project(ProjectCommand),
    Agent(AgentCommand),
    Board(BoardCommand),
    Acp(AcpCommand),
    Runtime(RuntimeCommand),
}

#[derive(Debug, Args)]
struct AcpCommand {
    #[command(subcommand)]
    command: AcpSubcommand,
}

#[derive(Debug, Subcommand)]
enum AcpSubcommand {
    Agent(AcpAgentCommand),
    Preset(AcpPresetCommand),
}

#[derive(Debug, Args)]
struct AcpAgentCommand {
    #[command(subcommand)]
    command: AcpAgentSubcommand,
}

#[derive(Debug, Subcommand)]
enum AcpAgentSubcommand {
    Add(AcpAgentAddArgs),
    Check(AcpAgentCheckArgs),
    List,
    Remove(AcpAgentRemoveArgs),
}

#[derive(Debug, Args)]
struct AcpAgentAddArgs {
    name: String,
    #[arg(long)]
    command: PathBuf,
    #[arg(long = "arg", allow_hyphen_values = true)]
    args: Vec<String>,
    #[arg(long = "env")]
    env: Vec<String>,
    #[arg(long = "resume-template")]
    resume_template: Option<String>,
}

#[derive(Debug, Args)]
struct AcpAgentCheckArgs {
    name: String,
}

#[derive(Debug, Args)]
struct AcpAgentRemoveArgs {
    name: String,
}

#[derive(Debug, Args)]
struct AcpPresetCommand {
    #[command(subcommand)]
    command: AcpPresetSubcommand,
}

#[derive(Debug, Subcommand)]
enum AcpPresetSubcommand {
    Install(AcpPresetInstallArgs),
}

#[derive(Debug, Args)]
struct AcpPresetInstallArgs {
    preset: String,
}

#[derive(Debug, Args)]
struct TaskCommand {
    #[command(subcommand)]
    command: TaskSubcommand,
}

#[derive(Debug, Args)]
struct ProjectCommand {
    #[command(subcommand)]
    command: ProjectSubcommand,
}

#[derive(Debug, Subcommand)]
enum ProjectSubcommand {
    Init(ProjectInitArgs),
}

#[derive(Debug, Args)]
struct ProjectInitArgs {
    #[arg(long)]
    path: PathBuf,
    #[arg(long)]
    agent: ProjectAgentArg,
}

#[derive(Debug, Args)]
struct AgentCommand {
    #[command(subcommand)]
    command: AgentSubcommand,
}

#[derive(Debug, Subcommand)]
enum AgentSubcommand {
    Prompt(AgentPromptArgs),
}

#[derive(Debug, Args)]
struct AgentPromptArgs {
    #[arg(long)]
    runtime: GuidanceRuntimeArg,
}

#[derive(Debug, Args)]
struct BoardCommand {
    #[command(subcommand)]
    command: BoardSubcommand,
}

#[derive(Debug, Subcommand)]
enum BoardSubcommand {
    Html,
    Serve(BoardServeArgs),
}

#[derive(Debug, Args)]
struct BoardServeArgs {
    #[arg(long, default_value = "127.0.0.1")]
    host: String,
    #[arg(long, default_value_t = 8765)]
    port: u16,
}

#[derive(Debug, Args)]
struct RuntimeCommand {
    #[command(subcommand)]
    command: RuntimeSubcommand,
}

#[derive(Debug, Subcommand)]
enum RuntimeSubcommand {
    Doctor,
    Profile(RuntimeProfileCommand),
}

#[derive(Debug, Args)]
struct RuntimeProfileCommand {
    #[command(subcommand)]
    command: RuntimeProfileSubcommand,
}

#[derive(Debug, Subcommand)]
enum RuntimeProfileSubcommand {
    Doctor,
    Set(RuntimeProfileSetArgs),
}

#[derive(Debug, Args)]
struct RuntimeProfileSetArgs {
    runtime: RuntimeArg,
    #[arg(long)]
    command: Option<String>,
    #[arg(long)]
    resume: Option<String>,
}

#[derive(Debug, Subcommand)]
enum TaskSubcommand {
    List(ListArgs),
    Board,
    Create(CreateArgs),
    Status(StatusArgs),
    Resume(ResumeArgs),
    Brief(BriefArgs),
    Sync(SyncArgs),
    Dispatch(DispatchArgs),
    Mark(MarkArgs),
    Triage(TriageArgs),
    Event(EventArgs),
    Review(ReviewArgs),
}

#[derive(Debug, Args)]
struct ListArgs {
    #[arg(long = "status")]
    status: Vec<StatusArg>,
    #[arg(long)]
    review: bool,
}

#[derive(Debug, Args)]
struct CreateArgs {
    #[arg(long)]
    id: String,
    #[arg(long)]
    title: String,
    #[arg(long)]
    project: PathBuf,
}

#[derive(Debug, Args)]
struct StatusArgs {
    id: String,
}

#[derive(Debug, Args)]
struct ResumeArgs {
    id: String,
}

#[derive(Debug, Args)]
#[command(about = "Render or write a child-agent task brief")]
struct BriefArgs {
    /// Task id to render a child-agent brief for.
    id: String,
    /// Write the brief to this task's session directory and record the path.
    #[arg(long)]
    write: bool,
}

#[derive(Debug, Args)]
#[command(
    about = "Sync recorded tmux session health for one task or all active session-backed tasks"
)]
struct SyncArgs {
    /// Task id to sync. Use --all instead to sync every active task with a recorded tmux session.
    id: Option<String>,
    /// Sync every active task that has a recorded tmux session.
    #[arg(long, conflicts_with = "id")]
    all: bool,
}

#[derive(Debug, Args)]
struct DispatchArgs {
    /// Task id to dispatch.
    id: String,
    /// Child-agent runtime to start.
    #[arg(long)]
    runtime: RuntimeArg,
    /// Named ACP agent config to use when --runtime acp.
    #[arg(long)]
    agent: Option<String>,
    /// Record the planned tmux dispatch without launching a child agent.
    #[arg(long = "dry-run")]
    dry_run: bool,
    /// Send the generated brief path into the tmux child-agent session after real dispatch.
    #[arg(long = "send-brief")]
    send_brief: bool,
    /// Confirm paid or elevated-risk real dispatch.
    #[arg(long)]
    confirm: bool,
}

#[derive(Debug, Args)]
struct MarkArgs {
    id: String,
    #[arg(long = "ready-for-review", conflicts_with_all = ["blocked", "triaged"])]
    ready_for_review: bool,
    #[arg(long, conflicts_with_all = ["ready_for_review", "triaged"])]
    blocked: bool,
    #[arg(long, conflicts_with_all = ["ready_for_review", "blocked"])]
    triaged: bool,
    #[arg(long)]
    message: String,
}

#[derive(Debug, Args)]
struct TriageArgs {
    id: String,
    #[arg(long)]
    risk: Option<RiskArg>,
    #[arg(long)]
    priority: Option<PriorityArg>,
    #[arg(long)]
    runtime: Option<RuntimeArg>,
    #[arg(long = "review-reason")]
    review_reason: Option<String>,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
#[clap(rename_all = "snake_case")]
enum StatusArg {
    Inbox,
    Triaged,
    WaitingUser,
    Queued,
    Running,
    Blocked,
    ReadyForReview,
    Reviewing,
    NeedsChanges,
    Done,
    Archived,
}

impl From<StatusArg> for TaskStatus {
    fn from(value: StatusArg) -> Self {
        match value {
            StatusArg::Inbox => TaskStatus::Inbox,
            StatusArg::Triaged => TaskStatus::Triaged,
            StatusArg::WaitingUser => TaskStatus::WaitingUser,
            StatusArg::Queued => TaskStatus::Queued,
            StatusArg::Running => TaskStatus::Running,
            StatusArg::Blocked => TaskStatus::Blocked,
            StatusArg::ReadyForReview => TaskStatus::ReadyForReview,
            StatusArg::Reviewing => TaskStatus::Reviewing,
            StatusArg::NeedsChanges => TaskStatus::NeedsChanges,
            StatusArg::Done => TaskStatus::Done,
            StatusArg::Archived => TaskStatus::Archived,
        }
    }
}

#[derive(Debug, Clone, Copy, ValueEnum)]
#[clap(rename_all = "snake_case")]
enum RiskArg {
    Low,
    Medium,
    High,
}

impl From<RiskArg> for RiskLevel {
    fn from(value: RiskArg) -> Self {
        match value {
            RiskArg::Low => RiskLevel::Low,
            RiskArg::Medium => RiskLevel::Medium,
            RiskArg::High => RiskLevel::High,
        }
    }
}

#[derive(Debug, Clone, Copy, ValueEnum)]
#[clap(rename_all = "snake_case")]
enum PriorityArg {
    Low,
    Normal,
    High,
}

impl PriorityArg {
    fn as_str(self) -> &'static str {
        match self {
            PriorityArg::Low => "low",
            PriorityArg::Normal => "normal",
            PriorityArg::High => "high",
        }
    }
}

#[derive(Debug, Clone, Copy, ValueEnum)]
#[clap(rename_all = "kebab_case")]
enum RuntimeArg {
    Claude,
    Codex,
    #[clap(name = "opencode")]
    OpenCode,
    Acp,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
#[clap(rename_all = "kebab_case")]
enum ProjectAgentArg {
    Claude,
    Codex,
    #[clap(name = "opencode")]
    OpenCode,
    All,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
#[clap(rename_all = "kebab_case")]
enum GuidanceRuntimeArg {
    Claude,
    Codex,
    #[clap(name = "opencode")]
    OpenCode,
    All,
}

impl From<RuntimeArg> for AgentRuntime {
    fn from(value: RuntimeArg) -> Self {
        match value {
            RuntimeArg::Claude => AgentRuntime::Claude,
            RuntimeArg::Codex => AgentRuntime::Codex,
            RuntimeArg::OpenCode => AgentRuntime::OpenCode,
            RuntimeArg::Acp => AgentRuntime::Acp,
        }
    }
}

impl From<GuidanceRuntimeArg> for GuidanceRuntime {
    fn from(value: GuidanceRuntimeArg) -> Self {
        match value {
            GuidanceRuntimeArg::Claude => GuidanceRuntime::Claude,
            GuidanceRuntimeArg::Codex => GuidanceRuntime::Codex,
            GuidanceRuntimeArg::OpenCode => GuidanceRuntime::OpenCode,
            GuidanceRuntimeArg::All => GuidanceRuntime::All,
        }
    }
}

#[derive(Debug, Args)]
struct EventArgs {
    id: String,
    #[arg(long = "type")]
    event_type: EventTypeArg,
    #[arg(long)]
    message: String,
}

#[derive(Debug, Args)]
struct ReviewArgs {
    id: String,
    #[arg(long, conflicts_with = "request_changes")]
    accept: bool,
    #[arg(long = "request-changes")]
    request_changes: Option<String>,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
#[clap(rename_all = "snake_case")]
enum EventTypeArg {
    Progress,
    Blocked,
    ReadyForReview,
}

impl EventTypeArg {
    fn as_str(self) -> &'static str {
        match self {
            EventTypeArg::Progress => "progress",
            EventTypeArg::Blocked => "blocked",
            EventTypeArg::ReadyForReview => "ready_for_review",
        }
    }
}

pub fn run() -> Result<()> {
    let cli = Cli::parse();
    let store = TaskStore::new(canonical_helm_agent_home()?);

    match cli.command {
        Command::Task(task) => handle_task(task, &store),
        Command::Project(project) => handle_project(project),
        Command::Agent(agent) => handle_agent(agent),
        Command::Board(board) => handle_board(board, &store),
        Command::Acp(acp) => handle_acp(acp, &store),
        Command::Runtime(runtime) => handle_runtime(runtime, &store),
    }
}

fn handle_runtime(runtime: RuntimeCommand, store: &TaskStore) -> Result<()> {
    match runtime.command {
        RuntimeSubcommand::Doctor => print_runtime_doctor(store),
        RuntimeSubcommand::Profile(profile) => handle_runtime_profile(profile, store),
    }
}

fn handle_runtime_profile(profile: RuntimeProfileCommand, store: &TaskStore) -> Result<()> {
    match profile.command {
        RuntimeProfileSubcommand::Doctor => print_runtime_doctor(store),
        RuntimeProfileSubcommand::Set(args) => {
            if args.command.is_none() && args.resume.is_none() {
                bail!("runtime profile set requires --command, --resume, or both");
            }
            let runtime = AgentRuntime::from(args.runtime);
            let mut profile = runtime_profile::load_runtime_profile(store)?;
            profile.set(runtime, args.command, args.resume)?;
            let path = runtime_profile::save_runtime_profile(store, &profile)?;
            println!("Saved runtime profile {}", path.display());

            let saved = profile
                .entry(runtime)
                .expect("runtime profile entry exists");
            match saved.command.as_deref() {
                Some(command) => println!("{} command: {}", runtime.as_str(), command),
                None => println!("{} command: unchanged", runtime.as_str()),
            }
            match saved.resume.as_deref() {
                Some(resume) => println!("{} resume: {}", runtime.as_str(), resume),
                None => println!("{} resume: unchanged", runtime.as_str()),
            }
            Ok(())
        }
    }
}

fn print_runtime_doctor(store: &TaskStore) -> Result<()> {
    let profile = runtime_profile::load_runtime_profile(store)?;
    let profile_path = runtime_profile::runtime_profile_path(store);
    let tmux_bin = tmux_bin_from_env();
    println!("Runtime profile: {}", profile_path.display());
    println!(
        "Runtime profile exists: {}",
        if profile_path.exists() { "yes" } else { "no" }
    );
    println!("tmux: {}", tmux_status(&tmux_bin));
    println!(
        "tmux new-session -e: {}",
        tmux_env_support_status(&tmux_bin)
    );

    for runtime in [
        AgentRuntime::Claude,
        AgentRuntime::Codex,
        AgentRuntime::OpenCode,
    ] {
        let (command, source) = effective_runtime_command(runtime, &profile);
        let resume = effective_runtime_resume(runtime, &profile, &command);
        println!(
            "{} command: {} ({}) [{}]",
            runtime.as_str(),
            command,
            source,
            command_status(&command)
        );
        match resume {
            Some((resume, source)) => {
                println!("{} resume: {} ({})", runtime.as_str(), resume, source);
            }
            None => {
                println!("{} resume: none (default)", runtime.as_str());
            }
        }
    }
    Ok(())
}

fn effective_runtime_command(
    runtime: AgentRuntime,
    profile: &runtime_profile::RuntimeProfile,
) -> (String, &'static str) {
    let adapter = RuntimeAdapter::for_runtime(runtime);
    if let Some(value) = env_runtime_value(command_env_name(runtime)) {
        return (value, "env");
    }
    if let Some(value) = profile
        .entry(runtime)
        .and_then(|entry| entry.command.as_deref())
        .and_then(normalize_runtime_value)
    {
        return (value.to_string(), "profile");
    }
    (adapter.command.to_string(), "default")
}

fn effective_runtime_resume(
    runtime: AgentRuntime,
    profile: &runtime_profile::RuntimeProfile,
    runtime_command: &str,
) -> Option<(String, &'static str)> {
    let adapter = RuntimeAdapter::for_runtime(runtime);
    if let Some(value) = env_runtime_value(resume_env_name(runtime)) {
        return Some((value, "env"));
    }
    if let Some(value) = profile
        .entry(runtime)
        .and_then(|entry| entry.resume.as_deref())
        .and_then(normalize_runtime_value)
    {
        return Some((value.to_string(), "profile"));
    }
    if !adapter.native_resume_available {
        return None;
    }
    if runtime_command == adapter.command {
        return Some((adapter.native_resume_template.to_string(), "default"));
    }

    let suffix = adapter
        .native_resume_template
        .strip_prefix(adapter.command)
        .unwrap_or("");
    Some((format!("{runtime_command}{suffix}"), "derived"))
}

fn command_env_name(runtime: AgentRuntime) -> &'static str {
    match runtime {
        AgentRuntime::Claude => "HELM_AGENT_CLAUDE_COMMAND",
        AgentRuntime::Codex => "HELM_AGENT_CODEX_COMMAND",
        AgentRuntime::OpenCode => "HELM_AGENT_OPENCODE_COMMAND",
        AgentRuntime::Acp => "",
    }
}

fn resume_env_name(runtime: AgentRuntime) -> &'static str {
    match runtime {
        AgentRuntime::Claude => "HELM_AGENT_CLAUDE_RESUME_COMMAND",
        AgentRuntime::Codex => "HELM_AGENT_CODEX_RESUME_COMMAND",
        AgentRuntime::OpenCode => "HELM_AGENT_OPENCODE_RESUME_COMMAND",
        AgentRuntime::Acp => "",
    }
}

fn env_runtime_value(name: &str) -> Option<String> {
    if name.is_empty() {
        return None;
    }
    std::env::var(name)
        .ok()
        .as_deref()
        .and_then(normalize_runtime_value)
        .map(ToString::to_string)
}

fn normalize_runtime_value(value: &str) -> Option<&str> {
    let value = value.trim();
    if value.is_empty() {
        None
    } else {
        Some(value)
    }
}

fn command_status(command: &str) -> String {
    let Some(executable) = command.split_whitespace().next() else {
        return "missing command".to_string();
    };
    if command_exists(executable) {
        format!("ok: {executable}")
    } else {
        format!("missing: {executable}")
    }
}

fn tmux_bin_from_env() -> PathBuf {
    std::env::var_os("HELM_AGENT_TMUX_BIN")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("tmux"))
}

fn tmux_status(tmux_bin: &Path) -> String {
    match tmux_version(tmux_bin) {
        Ok(version) if version.is_empty() => format!("ok: {}", tmux_bin.display()),
        Ok(version) => format!("ok: {} ({version})", tmux_bin.display()),
        Err(message) => message,
    }
}

fn tmux_version(tmux_bin: &Path) -> std::result::Result<String, String> {
    let output = ProcessCommand::new(tmux_bin)
        .arg("-V")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .map_err(|error| format!("missing: {} ({error})", tmux_bin.display()))?;

    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        if stderr.is_empty() {
            Err(format!(
                "failed: {} ({})",
                tmux_bin.display(),
                output.status
            ))
        } else {
            Err(format!("failed: {} ({stderr})", tmux_bin.display()))
        }
    }
}

fn command_exists(executable: &str) -> bool {
    ProcessCommand::new("sh")
        .arg("-c")
        .arg(format!(
            "command -v {}",
            shell_quote_for_process(executable)
        ))
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

fn tmux_env_support_status(tmux_bin: &Path) -> String {
    let session = format!(
        "helm-agent-doctor-{}-{}",
        std::process::id(),
        OffsetDateTime::now_utc().unix_timestamp_nanos()
    );
    let output = ProcessCommand::new(tmux_bin)
        .arg("new-session")
        .arg("-d")
        .arg("-e")
        .arg("HELM_AGENT_DOCTOR=1")
        .arg("-s")
        .arg(&session)
        .arg("true")
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .output();

    match output {
        Ok(output) if output.status.success() => {
            let _ = ProcessCommand::new(tmux_bin)
                .arg("kill-session")
                .arg("-t")
                .arg(&session)
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status();
            "ok".to_string()
        }
        Ok(output) => {
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
            if stderr.is_empty() {
                format!("failed ({})", output.status)
            } else {
                format!("failed ({stderr})")
            }
        }
        Err(error) => format!("missing: {} ({error})", tmux_bin.display()),
    }
}

fn shell_quote_for_process(value: &str) -> String {
    if !value.is_empty()
        && value
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '/' | '.' | '_' | '-' | ':'))
    {
        return value.to_string();
    }

    format!("'{}'", value.replace('\'', "'\\''"))
}

fn handle_acp(acp: AcpCommand, store: &TaskStore) -> Result<()> {
    match acp.command {
        AcpSubcommand::Agent(agent) => match agent.command {
            AcpAgentSubcommand::Add(args) => {
                let mut env = std::collections::BTreeMap::new();
                for pair in args.env {
                    let (key, value) = acp_adapter::parse_env_pair(&pair)?;
                    env.insert(key, value);
                }
                acp_adapter::add_acp_agent(
                    store,
                    &args.name,
                    AcpAgentConfig {
                        command: args.command,
                        args: args.args,
                        env,
                        resume_template: args.resume_template,
                    },
                )?;
                println!("Added ACP agent {}", args.name);
                Ok(())
            }
            AcpAgentSubcommand::List => {
                let agents = acp_adapter::load_acp_agents(store)?;
                print!("{}", acp_adapter::render_acp_agent_list(&agents));
                Ok(())
            }
            AcpAgentSubcommand::Check(args) => {
                let agent = acp_adapter::get_acp_agent(store, &args.name)?;
                let check_dir = store.root().join("acp").join(format!(
                    ".check-{}-{}",
                    std::process::id(),
                    OffsetDateTime::now_utc().unix_timestamp_nanos()
                ));
                fs::create_dir_all(&check_dir).with_context(|| {
                    format!("create ACP check directory {}", check_dir.display())
                })?;
                let check_result = acp_adapter::dispatch_prompt(
                    &agent,
                    &check_dir,
                    acp_adapter::ACP_CHECK_PROMPT.to_string(),
                );
                let cleanup_result = fs::remove_dir_all(&check_dir);
                let result = match check_result {
                    Ok(result) => result,
                    Err(error) => {
                        if let Err(cleanup_error) = cleanup_result {
                            eprintln!(
                                "Warning: ACP check cleanup failed for {}: {cleanup_error}",
                                check_dir.display()
                            );
                        }
                        return Err(error);
                    }
                };
                if let Err(cleanup_error) = cleanup_result {
                    eprintln!(
                        "Warning: ACP check cleanup failed for {}: {cleanup_error}",
                        check_dir.display()
                    );
                }
                if !acp_adapter::is_successful_stop_reason(&result.stop_reason) {
                    bail!(
                        "ACP agent {} check failed: stop reason {}",
                        args.name,
                        result.stop_reason
                    );
                }
                println!("ACP agent {} ok", args.name);
                println!("Session: {}", result.session_id);
                println!("Stop: {}", result.stop_reason);
                Ok(())
            }
            AcpAgentSubcommand::Remove(args) => {
                acp_adapter::remove_acp_agent(store, &args.name)?;
                println!("Removed ACP agent {}", args.name);
                Ok(())
            }
        },
        AcpSubcommand::Preset(preset) => handle_acp_preset(preset, store),
    }
}

fn handle_acp_preset(preset: AcpPresetCommand, store: &TaskStore) -> Result<()> {
    match preset.command {
        AcpPresetSubcommand::Install(args) => match args.preset.as_str() {
            CLAUDE_CODE_ACP_PRESET => install_claude_code_acp_preset(store),
            other => bail!("unknown ACP preset: {other}"),
        },
    }
}

fn install_claude_code_acp_preset(store: &TaskStore) -> Result<()> {
    acp_adapter::add_acp_agent(
        store,
        CLAUDE_CODE_ACP_PRESET,
        AcpAgentConfig {
            command: PathBuf::from("npx"),
            args: vec![
                "-y".to_string(),
                "@zed-industries/claude-agent-acp".to_string(),
            ],
            env: std::collections::BTreeMap::new(),
            resume_template: Some(CLAUDE_CODE_ACP_RESUME_TEMPLATE.to_string()),
        },
    )?;

    println!("Installed ACP preset {CLAUDE_CODE_ACP_PRESET}");
    println!("Agent: {CLAUDE_CODE_ACP_PRESET}");
    println!("Command: npx -y @zed-industries/claude-agent-acp");
    println!("Resume: {CLAUDE_CODE_ACP_RESUME_TEMPLATE}");
    Ok(())
}

fn handle_project(project: ProjectCommand) -> Result<()> {
    match project.command {
        ProjectSubcommand::Init(args) => {
            let files = project_agent_files(args.agent);
            for file in files {
                guidance::add_installed_project_guidance_include(&args.path, *file)?;
                println!("Updated {}", file.file_name());
            }
            Ok(())
        }
    }
}

fn handle_agent(agent: AgentCommand) -> Result<()> {
    match agent.command {
        AgentSubcommand::Prompt(args) => {
            print!(
                "{}\n",
                guidance::render_main_agent_prompt(GuidanceRuntime::from(args.runtime))?
            );
            Ok(())
        }
    }
}

fn handle_board(board: BoardCommand, store: &TaskStore) -> Result<()> {
    match board.command {
        BoardSubcommand::Html => {
            let tasks = web_board::load_task_board_tasks(store)?;
            print!("{}", web_board::render_task_board_html(&tasks));
            Ok(())
        }
        BoardSubcommand::Serve(args) => web_board::serve_task_board(store, &args.host, args.port),
    }
}

fn project_agent_files(agent: ProjectAgentArg) -> &'static [GuidanceFile] {
    match agent {
        ProjectAgentArg::Codex | ProjectAgentArg::OpenCode => &[GuidanceFile::Agents],
        ProjectAgentArg::Claude => &[GuidanceFile::Claude],
        ProjectAgentArg::All => &[GuidanceFile::Agents, GuidanceFile::Claude],
    }
}

fn launcher_for_store(store: &TaskStore) -> Result<Launcher> {
    let profile = runtime_profile::load_runtime_profile(store)?;
    Ok(Launcher::new()
        .with_runtime_profile(&profile)
        .with_helm_agent_home(store.root().display().to_string()))
}

fn handle_task_sync(args: SyncArgs, store: &TaskStore) -> Result<()> {
    let launcher = Launcher::new();
    match (args.id, args.all) {
        (Some(id), false) => {
            let task = store.load_task(&id)?;
            println!("{}", task_actions::sync_task(task, store, &launcher)?);
            Ok(())
        }
        (None, true) => {
            let mut tasks = store.list_tasks()?;
            tasks.retain(|task| {
                !matches!(task.status, TaskStatus::Done | TaskStatus::Archived)
                    && task.assignment.tmux_session.is_some()
            });
            tasks.sort_by(|left, right| right.updated_at.cmp(&left.updated_at));

            if tasks.is_empty() {
                println!("No syncable tasks");
                return Ok(());
            }

            for task in tasks {
                println!("{}", task_actions::sync_task(task, store, &launcher)?);
            }
            Ok(())
        }
        (None, false) => bail!("sync requires exactly one target: <id> or --all"),
        (Some(_), true) => bail!("sync accepts either <id> or --all"),
    }
}

fn write_task_brief(
    store: &TaskStore,
    task: &mut TaskRecord,
    events: &[TaskEvent],
) -> Result<PathBuf> {
    task.recovery.brief_path = Some(store.brief_path(&task.id));
    let markdown = brief::render_task_brief(task, events);
    store.write_brief(&task.id, &markdown)
}

fn persist_task_brief(
    store: &TaskStore,
    task: &mut TaskRecord,
    events: &[TaskEvent],
) -> Result<PathBuf> {
    let path = write_task_brief(store, task, events)?;
    if let Err(error) = store.save_task(task) {
        let _ = fs::remove_file(&path);
        return Err(error);
    }
    Ok(path)
}

fn handle_task_brief(args: BriefArgs, store: &TaskStore) -> Result<()> {
    let mut task = store.load_task(&args.id)?;
    let events = store.read_events(&args.id)?;

    if args.write {
        let now = OffsetDateTime::now_utc();
        task.touch(now);
        let event = TaskEvent::new(
            args.id.clone(),
            "brief_written",
            format!("Brief written {}", store.brief_path(&args.id).display()),
            now,
        );
        let mut events_with_write = events;
        events_with_write.push(event.clone());
        let path = persist_task_brief(store, &mut task, &events_with_write)?;
        store.append_event(&event)?;
        println!("Wrote brief: {}", path.display());
        return Ok(());
    }

    print!("{}", brief::render_task_brief(&task, &events));
    Ok(())
}

fn handle_acp_task_dispatch(
    args: DispatchArgs,
    store: &TaskStore,
    mut task: TaskRecord,
    now: OffsetDateTime,
) -> Result<()> {
    if args.send_brief {
        bail!("--send-brief is only supported for tmux runtimes");
    }
    let Some(agent_name) = args.agent.as_deref() else {
        bail!("ACP dispatch requires --agent <name>");
    };

    let agent = acp_adapter::get_acp_agent(store, agent_name)?;
    let command = acp_adapter::format_agent_command(&agent);
    if !args.dry_run && !args.confirm {
        bail!("dispatch {} with runtime acp requires --confirm", args.id);
    }

    task.status = TaskStatus::Queued;
    task.assignment.runtime = Some(AgentRuntime::Acp);
    task.assignment.tmux_session = None;
    task.assignment.acp_session_id = None;
    task.recovery.attach_command = None;
    task.recovery.resume_command = Some(format!(
        "helm-agent task dispatch {} --runtime acp --agent {} --confirm",
        args.id, agent_name
    ));
    task.progress.last_event = if args.dry_run {
        "Dry-run ACP dispatch recorded".to_string()
    } else {
        "ACP dispatch prepared".to_string()
    };
    task.progress.next_action = "Start or inspect ACP agent handoff".to_string();
    task.touch(now);

    let event = TaskEvent::new(
        args.id.clone(),
        if args.dry_run {
            "acp_dispatch_planned"
        } else {
            "acp_dispatch_prepared"
        },
        format!("{agent_name}: {command}"),
        now,
    );
    let mut events = store.read_events(&args.id)?;
    events.push(event.clone());
    let prepared_brief_path = persist_task_brief(store, &mut task, &events)?;
    store.append_event(&event)?;

    if args.dry_run {
        println!("Dry-run ACP dispatch {}", args.id);
        println!("Agent: {agent_name}");
        println!("Command: {command}");
        println!(
            "Resume: {}",
            task.recovery
                .resume_command
                .as_deref()
                .unwrap_or("No ACP resume command recorded")
        );
        println!("Brief: {}", prepared_brief_path.display());
        return Ok(());
    }

    let prompt = fs::read_to_string(&prepared_brief_path)
        .with_context(|| format!("read brief {}", prepared_brief_path.display()))?;
    match acp_adapter::dispatch_prompt(&agent, &task.project.path, prompt) {
        Ok(result) => {
            let mut completed_task = store.load_task(&args.id)?;
            let mut final_events = store.read_events(&args.id)?;
            completed_task.assignment.runtime = Some(AgentRuntime::Acp);
            completed_task.assignment.tmux_session = None;
            completed_task.assignment.acp_session_id = Some(result.session_id.clone());
            completed_task.recovery.attach_command = None;
            completed_task.recovery.resume_command = acp_adapter::render_resume_command(
                &agent,
                &completed_task.project.path,
                &result.session_id,
            )
            .or_else(|| task.recovery.resume_command.clone());
            completed_task.recovery.brief_path = Some(store.brief_path(&completed_task.id));
            if !matches!(
                completed_task.status,
                TaskStatus::Blocked | TaskStatus::NeedsChanges | TaskStatus::WaitingUser
            ) {
                completed_task.status = TaskStatus::ReadyForReview;
                completed_task.progress.blocker = None;
                completed_task.progress.last_event =
                    format!("ACP dispatch completed: {}", result.stop_reason);
                completed_task.progress.next_action = "Review ACP agent output".to_string();
                completed_task.review.state = ReviewState::Required;
                completed_task.review.reason =
                    Some("ACP agent completed a one-shot handoff".to_string());
            }
            completed_task.touch(now);

            let completed_event = TaskEvent::new(
                args.id.clone(),
                "acp_dispatch_completed",
                format!("{agent_name}: session {}", result.session_id),
                now,
            );
            final_events.push(completed_event.clone());
            let final_markdown = brief::render_task_brief(&completed_task, &final_events);
            if let Err(error) = store.save_task(&completed_task) {
                let warning =
                    format!("ACP completion state update failed after handoff: {error:#}");
                if let Ok(mut retry_task) = store.load_task(&args.id) {
                    retry_task.status = TaskStatus::NeedsChanges;
                    retry_task.assignment.runtime = Some(AgentRuntime::Acp);
                    retry_task.assignment.acp_session_id = Some(result.session_id.clone());
                    retry_task.recovery.resume_command =
                        completed_task.recovery.resume_command.clone();
                    retry_task.review.state = ReviewState::ChangesRequested;
                    retry_task.review.reason = Some(warning.clone());
                    retry_task.progress.last_event = warning.clone();
                    retry_task.progress.next_action =
                        "Fix local HelmAgent state persistence and retry dispatch".to_string();
                    retry_task.touch(now);
                    let _ = store.save_task(&retry_task);
                }
                let _ = store.append_event(&TaskEvent::new(
                    args.id.clone(),
                    "acp_dispatch_state_warning",
                    warning,
                    now,
                ));
                return Err(error).context("ACP completion state update failed after handoff");
            }
            if let Err(error) = store.write_brief(&completed_task.id, &final_markdown) {
                let warning =
                    format!("ACP completion state update failed after handoff: {error:#}");
                if let Ok(mut retry_task) = store.load_task(&args.id) {
                    retry_task.status = TaskStatus::NeedsChanges;
                    retry_task.assignment.runtime = Some(AgentRuntime::Acp);
                    retry_task.assignment.acp_session_id = Some(result.session_id.clone());
                    retry_task.recovery.resume_command =
                        completed_task.recovery.resume_command.clone();
                    retry_task.review.state = ReviewState::ChangesRequested;
                    retry_task.review.reason = Some(warning.clone());
                    retry_task.progress.last_event = warning.clone();
                    retry_task.progress.next_action =
                        "Fix local HelmAgent state persistence and retry dispatch".to_string();
                    retry_task.touch(now);
                    let _ = store.save_task(&retry_task);
                }
                let _ = store.append_event(&TaskEvent::new(
                    args.id.clone(),
                    "acp_dispatch_state_warning",
                    warning,
                    now,
                ));
                return Err(error).context("ACP completion state update failed after handoff");
            }
            if let Err(error) = store.append_event(&completed_event) {
                eprintln!("Warning: ACP completed but event record failed: {error:#}");
            }

            println!("Completed ACP {}", args.id);
            println!("Agent: {agent_name}");
            println!("Command: {command}");
            println!("Session: {}", result.session_id);
            println!(
                "Resume: {}",
                completed_task
                    .recovery
                    .resume_command
                    .as_deref()
                    .unwrap_or("No ACP resume command recorded")
            );
            println!("Brief: {}", store.brief_path(&args.id).display());
            Ok(())
        }
        Err(error) => {
            let message = format!("ACP dispatch failed: {error:#}");
            task.status = TaskStatus::NeedsChanges;
            task.progress.blocker = None;
            task.progress.last_event = message.clone();
            task.progress.next_action = "Fix ACP agent config and retry dispatch".to_string();
            task.review.state = ReviewState::ChangesRequested;
            task.review.reason = Some(message.clone());
            task.touch(now);
            let failed_event = TaskEvent::new(args.id.clone(), "acp_dispatch_failed", message, now);
            events.push(failed_event.clone());
            task.recovery.brief_path = Some(store.brief_path(&task.id));
            let final_markdown = brief::render_task_brief(&task, &events);
            store.save_task(&task)?;
            store.write_brief(&task.id, &final_markdown)?;
            store.append_event(&failed_event)?;
            Err(error)
        }
    }
}

fn handle_task(task: TaskCommand, store: &TaskStore) -> Result<()> {
    match task.command {
        TaskSubcommand::List(args) => {
            let mut tasks = store.list_tasks()?;
            let statuses: Vec<TaskStatus> = args.status.into_iter().map(TaskStatus::from).collect();
            let includes_archived = statuses.contains(&TaskStatus::Archived);

            if !includes_archived {
                tasks.retain(|task| task.status != TaskStatus::Archived);
            }

            if !statuses.is_empty() {
                tasks.retain(|task| statuses.contains(&task.status));
            }

            if args.review {
                tasks.retain(|task| {
                    task.review.state == ReviewState::Required
                        || matches!(
                            task.status,
                            TaskStatus::ReadyForReview
                                | TaskStatus::Reviewing
                                | TaskStatus::NeedsChanges
                        )
                });
            }

            tasks.sort_by(|left, right| right.updated_at.cmp(&left.updated_at));
            print!("{}", output::task_list(&tasks));
            Ok(())
        }
        TaskSubcommand::Board => {
            let tasks = web_board::load_task_board_tasks(store)?;
            print!("{}", output::task_board(&tasks));
            Ok(())
        }
        TaskSubcommand::Create(args) => {
            if store.task_path(&args.id).exists() {
                bail!("task {} already exists", args.id);
            }

            let now = OffsetDateTime::now_utc();
            let record = TaskRecord::new(args.id.clone(), args.title, args.project, now);
            store.save_task(&record)?;
            store.append_event(&TaskEvent::new(
                args.id.clone(),
                "created",
                "Task created".to_string(),
                now,
            ))?;
            println!("Created {}", args.id);
            Ok(())
        }
        TaskSubcommand::Status(args) => {
            let task = store.load_task(&args.id)?;
            let events = store.read_events(&args.id)?;
            print!("{}", output::task_status(&task, &events));
            Ok(())
        }
        TaskSubcommand::Resume(args) => {
            let task = store.load_task(&args.id)?;
            print!("{}", output::resume_text(&task));
            Ok(())
        }
        TaskSubcommand::Brief(args) => handle_task_brief(args, store),
        TaskSubcommand::Sync(args) => handle_task_sync(args, store),
        TaskSubcommand::Dispatch(args) => {
            let now = OffsetDateTime::now_utc();
            let mut task = store.load_task(&args.id)?;
            if !matches!(
                task.status,
                TaskStatus::Inbox
                    | TaskStatus::Triaged
                    | TaskStatus::Queued
                    | TaskStatus::NeedsChanges
            ) {
                bail!(
                    "cannot dispatch {} with status {}",
                    args.id,
                    task.status.as_str()
                );
            }
            if args.dry_run && args.send_brief {
                bail!("--send-brief cannot be used with --dry-run");
            }

            let runtime = AgentRuntime::from(args.runtime);
            if runtime == AgentRuntime::Acp {
                return handle_acp_task_dispatch(args, store, task, now);
            }
            if args.agent.is_some() {
                bail!("--agent can only be used with --runtime acp");
            }

            let dispatch = DispatchPlan {
                task_id: args.id.clone(),
                runtime,
                cwd: task.project.path.clone(),
            };
            let policy = PolicyInput {
                risk: task.risk,
                runtime,
                writes_files: true,
                paid_runtime: runtime == AgentRuntime::Codex,
                cross_project: false,
                network_sensitive: false,
            };
            if !args.dry_run
                && policy.evaluate() == DispatchDecision::ConfirmRequired
                && !args.confirm
            {
                bail!(
                    "dispatch {} with runtime {} requires --confirm",
                    args.id,
                    runtime.as_str()
                );
            }

            let launcher = launcher_for_store(store)?;
            let launch = launcher.dry_run(&dispatch);

            task.status = TaskStatus::Queued;
            task.assignment.runtime = Some(runtime);
            task.assignment.tmux_session = Some(launch.tmux_session.clone());
            task.recovery.attach_command = Some(launch.attach_command.clone());
            task.recovery.resume_command = launch.resume_command.clone();
            task.progress.last_event = if args.dry_run {
                "Dry-run dispatch recorded".to_string()
            } else {
                "Dispatch prepared".to_string()
            };
            task.progress.next_action = "Start or inspect child agent session".to_string();
            task.touch(now);

            let event = TaskEvent::new(
                args.id.clone(),
                if args.dry_run {
                    "dispatch_planned"
                } else {
                    "dispatch_prepared"
                },
                launch.start_command.clone(),
                now,
            );
            let mut events = store.read_events(&args.id)?;
            events.push(event.clone());
            let prepared_brief_path = persist_task_brief(store, &mut task, &events)?;
            store.append_event(&event)?;

            if args.dry_run {
                println!("Dry-run dispatch {}", args.id);
                println!("Start: {}", launch.start_command);
                println!("Attach: {}", launch.attach_command);
                println!(
                    "Resume: {}",
                    launch
                        .resume_command
                        .as_deref()
                        .unwrap_or("No native resume command recorded")
                );
                println!("Brief: {}", prepared_brief_path.display());
                return Ok(());
            }

            launcher.launch(&dispatch)?;
            task.status = TaskStatus::Running;
            task.progress.last_event = "Dispatch started".to_string();
            task.touch(now);
            let started_event = TaskEvent::new(
                args.id.clone(),
                "dispatch_started",
                launch.start_command.clone(),
                now,
            );
            let dispatch_started_recorded = store.append_event(&started_event);
            events.push(started_event.clone());
            let final_brief_path = store.brief_path(&task.id);
            task.recovery.brief_path = Some(final_brief_path.clone());
            let final_markdown = brief::render_task_brief(&task, &events);
            let final_result = store
                .save_task(&task)
                .and_then(|()| store.write_brief(&task.id, &final_markdown))
                .map(|path| (path, dispatch_started_recorded));

            println!("Started {}", args.id);
            println!("Start: {}", launch.start_command);
            println!("Attach: {}", launch.attach_command);
            println!(
                "Resume: {}",
                launch
                    .resume_command
                    .as_deref()
                    .unwrap_or("No native resume command recorded")
            );
            let active_brief_path = match final_result {
                Ok((brief_path, dispatch_started_recorded)) => {
                    if let Err(error) = dispatch_started_recorded {
                        eprintln!("Warning: Dispatch started but event record failed: {error:#}");
                    }
                    println!("Brief: {}", brief_path.display());
                    brief_path
                }
                Err(error) => {
                    let message =
                        format!("Dispatch state update failed after tmux start: {error:#}");
                    let _ = store.append_event(&TaskEvent::new(
                        args.id.clone(),
                        "dispatch_state_warning",
                        message.clone(),
                        now,
                    ));
                    println!("Brief: {}", prepared_brief_path.display());
                    eprintln!("Warning: {message}");
                    prepared_brief_path
                }
            };
            if args.send_brief {
                let handoff = format!(
                    "Use this HelmAgent child-agent brief before starting work:\n{}",
                    active_brief_path.display()
                );
                match launcher.send_keys(&launch.tmux_session, &handoff) {
                    Ok(()) => {
                        if let Err(error) = store.append_event(&TaskEvent::new(
                            args.id.clone(),
                            "brief_sent",
                            format!("Brief sent {}", active_brief_path.display()),
                            now,
                        )) {
                            eprintln!("Warning: Brief sent but event record failed: {error:#}");
                        }
                        println!("Brief sent: yes");
                    }
                    Err(error) => {
                        let message = format!("Brief send failed after tmux start: {error:#}");
                        let _ = store.append_event(&TaskEvent::new(
                            args.id.clone(),
                            "brief_send_warning",
                            message.clone(),
                            now,
                        ));
                        println!("Brief sent: no");
                        eprintln!("Warning: {message}");
                    }
                }
            }
            Ok(())
        }
        TaskSubcommand::Mark(args) => {
            let now = OffsetDateTime::now_utc();
            let action = if args.ready_for_review {
                MarkAction::ReadyForReview
            } else if args.blocked {
                MarkAction::Blocked
            } else if args.triaged {
                MarkAction::Triaged
            } else {
                bail!("mark requires --ready-for-review, --blocked, or --triaged");
            };

            let task = task_actions::mark_task(store, &args.id, action, args.message, now)?;
            println!("Marked {} {}", args.id, task.status.as_str());
            Ok(())
        }
        TaskSubcommand::Triage(args) => {
            if args.risk.is_none()
                && args.priority.is_none()
                && args.runtime.is_none()
                && args.review_reason.is_none()
            {
                bail!("triage requires at least one option");
            }

            let now = OffsetDateTime::now_utc();
            let mut task = store.load_task(&args.id)?;
            let mut changed = Vec::new();

            if let Some(risk) = args.risk {
                task.risk = RiskLevel::from(risk);
                changed.push(format!("risk={}", task.risk.as_str()));
                if task.risk != RiskLevel::Low {
                    task.review.state = ReviewState::Required;
                } else if task.review.reason.is_none() && args.review_reason.is_none() {
                    task.review.state = ReviewState::NotRequired;
                }
            }

            if let Some(priority) = args.priority {
                task.priority = priority.as_str().to_string();
                changed.push(format!("priority={}", priority.as_str()));
            }

            if let Some(runtime) = args.runtime {
                let runtime = AgentRuntime::from(runtime);
                task.assignment.runtime = Some(runtime);
                changed.push(format!("runtime={}", runtime.as_str()));
            }

            if let Some(reason) = args.review_reason {
                task.review.reason = Some(reason);
                task.review.state = ReviewState::Required;
                changed.push("review_reason=set".to_string());
            }

            task.status = TaskStatus::Triaged;
            let message = format!("Triaged {}", changed.join(", "));
            task.progress.last_event = message.clone();
            task.progress.next_action = "Dispatch or defer task".to_string();
            task.touch(now);
            store.save_task(&task)?;
            store.append_event(&TaskEvent::new(args.id.clone(), "triaged", message, now))?;
            println!("Triaged {}", args.id);
            Ok(())
        }
        TaskSubcommand::Event(args) => {
            let now = OffsetDateTime::now_utc();
            let event_type = args.event_type.as_str();
            task_actions::record_event(store, &args.id, event_type, args.message, now)?;
            println!("Recorded {} for {}", args.event_type.as_str(), args.id);
            Ok(())
        }
        TaskSubcommand::Review(args) => {
            let now = OffsetDateTime::now_utc();
            let (action, accepted) = if args.accept {
                (ReviewAction::Accept, true)
            } else if let Some(message) = args.request_changes {
                (ReviewAction::RequestChanges(message), false)
            } else {
                bail!("review requires --accept or --request-changes <message>");
            };

            task_actions::review_task(store, &args.id, action, now)?;
            if accepted {
                println!("Accepted {}", args.id);
            } else {
                println!("Requested changes for {}", args.id);
            }
            Ok(())
        }
    }
}
