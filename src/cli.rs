use crate::domain::{AgentRuntime, ReviewState, TaskEvent, TaskRecord, TaskStatus};
use crate::launcher::{DispatchPlan, Launcher};
use crate::output;
use crate::paths::helm_agent_home;
use crate::store::TaskStore;
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
}

#[derive(Debug, Args)]
struct TaskCommand {
    #[command(subcommand)]
    command: TaskSubcommand,
}

#[derive(Debug, Subcommand)]
enum TaskSubcommand {
    Create(CreateArgs),
    Status(StatusArgs),
    Resume(ResumeArgs),
    Dispatch(DispatchArgs),
    Event(EventArgs),
    Review(ReviewArgs),
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
struct DispatchArgs {
    id: String,
    #[arg(long)]
    runtime: RuntimeArg,
    #[arg(long = "dry-run")]
    dry_run: bool,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
#[clap(rename_all = "kebab_case")]
enum RuntimeArg {
    Claude,
    Codex,
    #[clap(name = "opencode")]
    OpenCode,
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
    let store = TaskStore::new(helm_agent_home()?);

    match cli.command {
        Command::Task(task) => handle_task(task, &store),
    }
}

fn handle_task(task: TaskCommand, store: &TaskStore) -> Result<()> {
    match task.command {
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
        TaskSubcommand::Dispatch(args) => {
            let now = OffsetDateTime::now_utc();
            let mut task = store.load_task(&args.id)?;
            let runtime = AgentRuntime::from(args.runtime);
            let dispatch = DispatchPlan {
                task_id: args.id.clone(),
                runtime,
                cwd: task.project.path.clone(),
            };
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

            bail!("review requires --accept or --request-changes <message>");
        }
    }
}
