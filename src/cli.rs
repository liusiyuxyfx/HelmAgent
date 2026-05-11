use crate::domain::{AgentRuntime, ReviewState, RiskLevel, TaskEvent, TaskRecord, TaskStatus};
use crate::guidance::{self, GuidanceFile, GuidanceRuntime};
use crate::launcher::{DispatchPlan, Launcher, TmuxSessionState};
use crate::output;
use crate::paths::canonical_helm_agent_home;
use crate::policy::{DispatchDecision, PolicyInput};
use crate::store::TaskStore;
use crate::web_board;
use anyhow::{bail, Result};
use clap::{Args, Parser, Subcommand, ValueEnum};
use std::path::PathBuf;
use time::OffsetDateTime;

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

#[derive(Debug, Subcommand)]
enum TaskSubcommand {
    List(ListArgs),
    Board,
    Create(CreateArgs),
    Status(StatusArgs),
    Resume(ResumeArgs),
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
    id: String,
    #[arg(long)]
    runtime: RuntimeArg,
    #[arg(long = "dry-run")]
    dry_run: bool,
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
    }
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

fn handle_task_sync(args: SyncArgs, store: &TaskStore) -> Result<()> {
    let launcher = Launcher::new();
    match (args.id, args.all) {
        (Some(id), false) => {
            let task = store.load_task(&id)?;
            println!("{}", sync_task(task, store, &launcher)?);
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
                println!("{}", sync_task(task, store, &launcher)?);
            }
            Ok(())
        }
        (None, false) => bail!("sync requires exactly one target: <id> or --all"),
        (Some(_), true) => bail!("sync accepts either <id> or --all"),
    }
}

fn sync_task(mut task: TaskRecord, store: &TaskStore, launcher: &Launcher) -> Result<String> {
    if matches!(task.status, TaskStatus::Done | TaskStatus::Archived) {
        return Ok(format!("{} skipped {}", task.id, task.status.as_str()));
    }

    let now = OffsetDateTime::now_utc();
    let Some(session) = task.assignment.tmux_session.clone() else {
        return Ok(format!("{} no_session", task.id));
    };

    match launcher.session_state(&session)? {
        TmuxSessionState::Alive => {
            if matches!(
                task.status,
                TaskStatus::Queued | TaskStatus::Running | TaskStatus::Blocked
            ) {
                task.status = TaskStatus::Running;
                if task
                    .progress
                    .blocker
                    .as_deref()
                    .is_some_and(|blocker| blocker.starts_with("tmux session missing:"))
                {
                    task.progress.blocker = None;
                }
                task.progress.last_event = format!("tmux session alive: {session}");
                task.progress.next_action =
                    "Inspect child agent session or wait for review handoff".to_string();
                task.touch(now);
                store.save_task(&task)?;
                store.append_event(&TaskEvent::new(
                    task.id.clone(),
                    "sync_alive",
                    format!("tmux session alive: {session}"),
                    now,
                ))?;
            }
            Ok(format!("{} alive {}", task.id, session))
        }
        TmuxSessionState::Missing => {
            if matches!(task.status, TaskStatus::Running | TaskStatus::Blocked) {
                let message = format!("tmux session missing: {session}");
                task.status = TaskStatus::Blocked;
                task.progress.blocker = Some(message.clone());
                task.progress.last_event = message.clone();
                task.progress.next_action =
                    "Restart dispatch or inspect the task manually".to_string();
                task.touch(now);
                store.save_task(&task)?;
                store.append_event(&TaskEvent::new(
                    task.id.clone(),
                    "sync_missing",
                    format!("tmux session missing: {session}"),
                    now,
                ))?;
            }
            Ok(format!("{} missing {}", task.id, session))
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

            let runtime = AgentRuntime::from(args.runtime);
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

            let launcher = Launcher::new();
            let launch = if args.dry_run {
                launcher.dry_run(&dispatch)
            } else {
                launcher.launch(&dispatch)?
            };

            task.status = if args.dry_run {
                TaskStatus::Queued
            } else {
                TaskStatus::Running
            };
            task.assignment.runtime = Some(runtime);
            task.assignment.tmux_session = Some(launch.tmux_session.clone());
            task.recovery.attach_command = Some(launch.attach_command.clone());
            task.recovery.resume_command = launch.resume_command.clone();
            task.progress.last_event = if args.dry_run {
                "Dry-run dispatch recorded".to_string()
            } else {
                "Dispatch started".to_string()
            };
            task.progress.next_action = "Start or inspect child agent session".to_string();
            task.touch(now);

            store.save_task(&task)?;
            store.append_event(&TaskEvent::new(
                args.id.clone(),
                if args.dry_run {
                    "dispatch_planned"
                } else {
                    "dispatch_started"
                },
                launch.start_command.clone(),
                now,
            ))?;

            if args.dry_run {
                println!("Dry-run dispatch {}", args.id);
            } else {
                println!("Started {}", args.id);
            }
            println!("Start: {}", launch.start_command);
            println!("Attach: {}", launch.attach_command);
            println!(
                "Resume: {}",
                launch
                    .resume_command
                    .as_deref()
                    .unwrap_or("No native resume command recorded")
            );
            Ok(())
        }
        TaskSubcommand::Mark(args) => {
            let now = OffsetDateTime::now_utc();
            let mut task = store.load_task(&args.id)?;
            let (status, event_type, next_action) = if args.ready_for_review {
                task.review.state = ReviewState::Required;
                task.progress.blocker = None;
                (
                    TaskStatus::ReadyForReview,
                    "ready_for_review",
                    "Human review required",
                )
            } else if args.blocked {
                task.progress.blocker = Some(args.message.clone());
                (TaskStatus::Blocked, "blocked", "Resolve blocker")
            } else if args.triaged {
                task.progress.blocker = None;
                (TaskStatus::Triaged, "triaged", "Dispatch or defer task")
            } else {
                bail!("mark requires --ready-for-review, --blocked, or --triaged");
            };

            task.status = status;
            task.progress.last_event = args.message.clone();
            task.progress.next_action = next_action.to_string();
            task.touch(now);
            store.save_task(&task)?;
            store.append_event(&TaskEvent::new(
                args.id.clone(),
                event_type,
                args.message,
                now,
            ))?;
            println!("Marked {} {}", args.id, status.as_str());
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
            let mut task = store.load_task(&args.id)?;
            task.progress.last_event = args.message.clone();
            task.touch(now);
            store.save_task(&task)?;
            store.append_event(&TaskEvent::new(
                args.id.clone(),
                args.event_type.as_str(),
                args.message,
                now,
            ))?;
            println!("Recorded {} for {}", args.event_type.as_str(), args.id);
            Ok(())
        }
        TaskSubcommand::Review(args) => {
            let now = OffsetDateTime::now_utc();
            let mut task = store.load_task(&args.id)?;
            if !args.accept && args.request_changes.is_none() {
                bail!("review requires --accept or --request-changes <message>");
            }
            if !matches!(
                task.status,
                TaskStatus::ReadyForReview | TaskStatus::Reviewing
            ) {
                bail!(
                    "cannot review {} with status {}",
                    args.id,
                    task.status.as_str()
                );
            }

            if args.accept {
                task.status = TaskStatus::Done;
                task.review.state = ReviewState::Accepted;
                task.progress.last_event = "Review accepted".to_string();
                task.progress.next_action = "Archive task when ready".to_string();
                task.touch(now);
                store.save_task(&task)?;
                store.append_event(&TaskEvent::new(
                    args.id.clone(),
                    "review_accepted",
                    "Review accepted".to_string(),
                    now,
                ))?;
                println!("Accepted {}", args.id);
                return Ok(());
            }

            if let Some(message) = args.request_changes {
                task.status = TaskStatus::NeedsChanges;
                task.review.state = ReviewState::ChangesRequested;
                task.progress.last_event = message.clone();
                task.progress.next_action = "Dispatch follow-up changes".to_string();
                task.touch(now);
                store.save_task(&task)?;
                store.append_event(&TaskEvent::new(
                    args.id.clone(),
                    "changes_requested",
                    message,
                    now,
                ))?;
                println!("Requested changes for {}", args.id);
                return Ok(());
            }

            Ok(())
        }
    }
}
