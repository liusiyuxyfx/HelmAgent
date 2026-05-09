use crate::domain::{TaskEvent, TaskRecord};
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
    Event(EventArgs),
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
struct EventArgs {
    id: String,
    #[arg(long = "type")]
    event_type: EventTypeArg,
    #[arg(long)]
    message: String,
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
    }
}
