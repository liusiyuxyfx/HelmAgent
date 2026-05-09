# Agent Ops Center Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the V1 `aoc` Rust CLI core for local task tracking, policy decisions, tmux-based dispatch records, and human recovery commands.

**Architecture:** The first implementation is CLI-first and file-backed. Rust owns the task domain model, YAML task records, JSONL events, policy decisions, command output, and launcher command construction; agent-specific integrations remain thin wrappers or documentation that call the shared CLI.

**Tech Stack:** Rust, `clap`, `serde`, `serde_yaml`, `serde_json`, `anyhow`, `thiserror`, `tracing`, `directories`, `assert_cmd`, `predicates`, `tempfile`.

---

## Scope

This plan implements the first working core:

- `aoc task create`
- `aoc task status`
- `aoc task resume`
- `aoc task review --accept`
- `aoc task review --request-changes <message>`
- `aoc task event`
- `aoc task dispatch --dry-run`
- File-backed task records and event logs
- Default semi-automatic policy
- tmux command construction and adapter capability records
- Main-agent usage documentation

This plan does not implement a Web Board, container sandboxing, external issue imports, recursive agent delegation, or full ACP session transport.

## File Structure

- `Cargo.toml`: package metadata, binary name, dependencies, dev dependencies.
- `src/main.rs`: binary entrypoint and error reporting.
- `src/lib.rs`: module exports used by integration tests.
- `src/adapter.rs`: runtime adapter command and resume capability metadata.
- `src/cli.rs`: `clap` command definitions and top-level command handling.
- `src/domain.rs`: task IDs, statuses, risk, runtime, records, events, and review decisions.
- `src/paths.rs`: Agent Ops Center home directory resolution and test override.
- `src/store.rs`: YAML task persistence and JSONL event append/read.
- `src/policy.rs`: default policy and task triage decision helpers.
- `src/launcher.rs`: tmux session naming, command construction, dry-run dispatch, and executable dispatch.
- `src/output.rs`: human-readable CLI formatting.
- `docs/agent-integrations/main-agent.md`: instructions for Claude Code/Codex main agents.
- `tests/cli_task_flow.rs`: end-to-end CLI tests using a temporary AOC home.
- `tests/store_tests.rs`: store persistence tests.
- `tests/policy_tests.rs`: policy decision tests.
- `tests/launcher_tests.rs`: launcher command tests.

## Task 1: Rust Project Scaffold

**Files:**
- Create: `Cargo.toml`
- Create: `src/main.rs`
- Create: `src/lib.rs`
- Create: `src/adapter.rs`
- Create: `src/cli.rs`
- Create: `src/domain.rs`
- Create: `src/launcher.rs`
- Create: `src/output.rs`
- Create: `src/paths.rs`
- Create: `src/policy.rs`
- Create: `src/store.rs`

- [ ] **Step 1: Create the Rust package files**

Create `Cargo.toml` with this content:

```toml
[package]
name = "helm-agent"
version = "0.1.0"
edition = "2021"

[[bin]]
name = "aoc"
path = "src/main.rs"

[dependencies]
anyhow = "1"
clap = { version = "4", features = ["derive"] }
directories = "5"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
serde_yaml = "0.9"
thiserror = "1"
time = { version = "0.3", features = ["formatting", "macros", "parsing", "serde"] }
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }

[dev-dependencies]
assert_cmd = "2"
predicates = "3"
tempfile = "3"
```

Create `src/main.rs` with this content:

```rust
use anyhow::Result;

fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "warn".into()),
        )
        .without_time()
        .init();

    helm_agent::cli::run()
}
```

Create `src/lib.rs` with this content:

```rust
pub mod adapter;
pub mod cli;
pub mod domain;
pub mod launcher;
pub mod output;
pub mod paths;
pub mod policy;
pub mod store;
```

Create `src/cli.rs` with this content:

```rust
use anyhow::Result;

pub fn run() -> Result<()> {
    Ok(())
}
```

Create these stub module files with empty content:

```text
src/adapter.rs
src/domain.rs
src/launcher.rs
src/output.rs
src/paths.rs
src/policy.rs
src/store.rs
```

- [ ] **Step 2: Run formatting**

Run:

```bash
cargo fmt
```

Expected: command exits 0.

- [ ] **Step 3: Run check**

Run:

```bash
cargo check
```

Expected: PASS.

- [ ] **Step 4: Commit scaffold**

Run:

```bash
git add Cargo.toml src/main.rs src/lib.rs src/adapter.rs src/cli.rs src/domain.rs src/launcher.rs src/output.rs src/paths.rs src/policy.rs src/store.rs
git commit -m "feat: scaffold aoc rust cli"
```

Expected: commit succeeds.

## Task 2: Domain Model

**Files:**
- Modify: `src/domain.rs`
- Create: `tests/domain_tests.rs`
- Modify: `src/lib.rs`

- [ ] **Step 1: Write domain serialization tests**

Create `tests/domain_tests.rs` with this content:

```rust
use helm_agent::domain::{
    AgentRuntime, ReviewState, RiskLevel, TaskEvent, TaskRecord, TaskStatus,
};
use time::OffsetDateTime;

#[test]
fn task_status_serializes_as_snake_case() {
    let status = TaskStatus::ReadyForReview;
    let yaml = serde_yaml::to_string(&status).unwrap();
    assert!(yaml.contains("ready_for_review"));
}

#[test]
fn task_record_round_trips_through_yaml() {
    let now = OffsetDateTime::parse(
        "2026-05-09T10:00:00Z",
        &time::format_description::well_known::Rfc3339,
    )
    .unwrap();

    let task = TaskRecord::new(
        "PM-20260509-001".to_string(),
        "Fix login redirect bug".to_string(),
        "/repo".into(),
        now,
    );

    let yaml = serde_yaml::to_string(&task).unwrap();
    let parsed: TaskRecord = serde_yaml::from_str(&yaml).unwrap();

    assert_eq!(parsed.id, "PM-20260509-001");
    assert_eq!(parsed.status, TaskStatus::Inbox);
    assert_eq!(parsed.risk, RiskLevel::Low);
    assert_eq!(parsed.project.path.to_string_lossy(), "/repo");
    assert_eq!(parsed.review.state, ReviewState::NotRequired);
}

#[test]
fn task_event_round_trips_through_json() {
    let event = TaskEvent::progress(
        "PM-20260509-001".to_string(),
        "Found redirect handler".to_string(),
        OffsetDateTime::UNIX_EPOCH,
    );

    let json = serde_json::to_string(&event).unwrap();
    let parsed: TaskEvent = serde_json::from_str(&json).unwrap();

    assert_eq!(parsed.task_id, "PM-20260509-001");
    assert_eq!(parsed.event_type, "progress");
    assert_eq!(parsed.message, "Found redirect handler");
}

#[test]
fn runtime_display_names_match_cli_values() {
    assert_eq!(AgentRuntime::Claude.as_str(), "claude");
    assert_eq!(AgentRuntime::Codex.as_str(), "codex");
    assert_eq!(AgentRuntime::OpenCode.as_str(), "opencode");
}
```

- [ ] **Step 2: Run test to verify it fails**

Run:

```bash
cargo test --test domain_tests
```

Expected: FAIL with unresolved imports from `helm_agent::domain`.

- [ ] **Step 3: Implement domain types**

Create `src/domain.rs` with this content:

```rust
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use time::OffsetDateTime;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RiskLevel {
    Low,
    Medium,
    High,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentRuntime {
    Claude,
    Codex,
    OpenCode,
}

impl AgentRuntime {
    pub fn as_str(self) -> &'static str {
        match self {
            AgentRuntime::Claude => "claude",
            AgentRuntime::Codex => "codex",
            AgentRuntime::OpenCode => "opencode",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReviewState {
    NotRequired,
    Required,
    Accepted,
    ChangesRequested,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectRef {
    pub path: PathBuf,
    pub branch: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Assignment {
    pub runtime: Option<AgentRuntime>,
    pub workflow: Option<String>,
    pub tmux_session: Option<String>,
    pub native_session_id: Option<String>,
    pub acp_session_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Recovery {
    pub attach_command: Option<String>,
    pub resume_command: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Progress {
    pub summary: String,
    pub last_event: String,
    pub next_action: String,
    pub blocker: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Review {
    pub state: ReviewState,
    pub reason: Option<String>,
    pub artifacts: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskRecord {
    pub id: String,
    pub title: String,
    pub status: TaskStatus,
    pub priority: String,
    pub risk: RiskLevel,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
    pub project: ProjectRef,
    pub assignment: Assignment,
    pub recovery: Recovery,
    pub progress: Progress,
    pub review: Review,
}

impl TaskRecord {
    pub fn new(id: String, title: String, project_path: PathBuf, now: OffsetDateTime) -> Self {
        Self {
            id,
            title,
            status: TaskStatus::Inbox,
            priority: "normal".to_string(),
            risk: RiskLevel::Low,
            created_at: now,
            updated_at: now,
            project: ProjectRef {
                path: project_path,
                branch: None,
            },
            assignment: Assignment {
                runtime: None,
                workflow: None,
                tmux_session: None,
                native_session_id: None,
                acp_session_id: None,
            },
            recovery: Recovery {
                attach_command: None,
                resume_command: None,
            },
            progress: Progress {
                summary: "Created task".to_string(),
                last_event: "Task created".to_string(),
                next_action: "Triage task".to_string(),
                blocker: None,
            },
            review: Review {
                state: ReviewState::NotRequired,
                reason: None,
                artifacts: Vec::new(),
            },
        }
    }

    pub fn touch(&mut self, now: OffsetDateTime) {
        self.updated_at = now;
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskEvent {
    pub task_id: String,
    pub event_type: String,
    pub message: String,
    pub created_at: OffsetDateTime,
}

impl TaskEvent {
    pub fn new(
        task_id: String,
        event_type: impl Into<String>,
        message: String,
        created_at: OffsetDateTime,
    ) -> Self {
        Self {
            task_id,
            event_type: event_type.into(),
            message,
            created_at,
        }
    }

    pub fn progress(task_id: String, message: String, created_at: OffsetDateTime) -> Self {
        Self::new(task_id, "progress", message, created_at)
    }
}
```

- [ ] **Step 4: Run domain tests**

Run:

```bash
cargo test --test domain_tests
```

Expected: PASS.

- [ ] **Step 5: Commit domain model**

Run:

```bash
git add src/domain.rs tests/domain_tests.rs
git commit -m "feat: add task domain model"
```

Expected: commit succeeds.

## Task 3: Paths and File Store

**Files:**
- Modify: `src/paths.rs`
- Modify: `src/store.rs`
- Create: `tests/store_tests.rs`

- [ ] **Step 1: Write store tests**

Create `tests/store_tests.rs` with this content:

```rust
use helm_agent::domain::{TaskEvent, TaskRecord};
use helm_agent::store::TaskStore;
use tempfile::tempdir;
use time::OffsetDateTime;

#[test]
fn save_and_load_task_record() {
    let temp = tempdir().unwrap();
    let store = TaskStore::new(temp.path().to_path_buf());
    let now = OffsetDateTime::UNIX_EPOCH;
    let task = TaskRecord::new(
        "PM-20260509-001".to_string(),
        "Fix login redirect bug".to_string(),
        "/repo".into(),
        now,
    );

    store.save_task(&task).unwrap();
    let loaded = store.load_task("PM-20260509-001").unwrap();

    assert_eq!(loaded.title, "Fix login redirect bug");
    assert_eq!(
        store.task_path("PM-20260509-001"),
        temp.path()
            .join("tasks")
            .join("2026")
            .join("PM-20260509-001.yaml")
    );
}

#[test]
fn append_and_read_events() {
    let temp = tempdir().unwrap();
    let store = TaskStore::new(temp.path().to_path_buf());
    let event = TaskEvent::progress(
        "PM-20260509-001".to_string(),
        "Found redirect handler".to_string(),
        OffsetDateTime::UNIX_EPOCH,
    );

    store.append_event(&event).unwrap();
    let events = store.read_events("PM-20260509-001").unwrap();

    assert_eq!(events.len(), 1);
    assert_eq!(events[0].message, "Found redirect handler");
}
```

- [ ] **Step 2: Run store tests to verify failure**

Run:

```bash
cargo test --test store_tests
```

Expected: FAIL with unresolved `TaskStore`.

- [ ] **Step 3: Implement path resolution**

Create `src/paths.rs` with this content:

```rust
use anyhow::{anyhow, Result};
use directories::ProjectDirs;
use std::env;
use std::path::PathBuf;

pub const AOC_HOME_ENV: &str = "AOC_HOME";

pub fn aoc_home() -> Result<PathBuf> {
    if let Some(path) = env::var_os(AOC_HOME_ENV) {
        return Ok(PathBuf::from(path));
    }

    let dirs = ProjectDirs::from("dev", "helm-agent", "agent-ops-center")
        .ok_or_else(|| anyhow!("could not resolve a data directory for Agent Ops Center"))?;

    Ok(dirs.data_dir().to_path_buf())
}
```

- [ ] **Step 4: Implement file-backed task store**

Create `src/store.rs` with this content:

```rust
use crate::domain::{TaskEvent, TaskRecord};
use anyhow::{Context, Result};
use std::fs::{self, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct TaskStore {
    root: PathBuf,
}

impl TaskStore {
    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }

    pub fn root(&self) -> &PathBuf {
        &self.root
    }

    pub fn task_path(&self, task_id: &str) -> PathBuf {
        let year = task_id
            .split('-')
            .nth(1)
            .and_then(|date| date.get(0..4))
            .unwrap_or("unknown");

        self.root
            .join("tasks")
            .join(year)
            .join(format!("{task_id}.yaml"))
    }

    pub fn session_dir(&self, task_id: &str) -> PathBuf {
        self.root.join("sessions").join(task_id)
    }

    pub fn events_path(&self, task_id: &str) -> PathBuf {
        self.session_dir(task_id).join("events.jsonl")
    }

    pub fn save_task(&self, task: &TaskRecord) -> Result<()> {
        let path = self.task_path(&task.id);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("create task directory {}", parent.display()))?;
        }

        let yaml = serde_yaml::to_string(task).context("serialize task record")?;
        fs::write(&path, yaml).with_context(|| format!("write task {}", path.display()))?;
        Ok(())
    }

    pub fn load_task(&self, task_id: &str) -> Result<TaskRecord> {
        let path = self.task_path(task_id);
        let content =
            fs::read_to_string(&path).with_context(|| format!("read task {}", path.display()))?;
        serde_yaml::from_str(&content).with_context(|| format!("parse task {}", path.display()))
    }

    pub fn append_event(&self, event: &TaskEvent) -> Result<()> {
        let path = self.events_path(&event.task_id);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("create session directory {}", parent.display()))?;
        }

        let line = serde_json::to_string(event).context("serialize task event")?;
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .with_context(|| format!("open events log {}", path.display()))?;
        writeln!(file, "{line}").with_context(|| format!("append event {}", path.display()))?;
        Ok(())
    }

    pub fn read_events(&self, task_id: &str) -> Result<Vec<TaskEvent>> {
        let path = self.events_path(task_id);
        if !path.exists() {
            return Ok(Vec::new());
        }

        let file = fs::File::open(&path).with_context(|| format!("open {}", path.display()))?;
        let reader = BufReader::new(file);
        let mut events = Vec::new();

        for line in reader.lines() {
            let line = line.with_context(|| format!("read line from {}", path.display()))?;
            if line.trim().is_empty() {
                continue;
            }
            events.push(
                serde_json::from_str(&line)
                    .with_context(|| format!("parse event from {}", path.display()))?,
            );
        }

        Ok(events)
    }
}
```

- [ ] **Step 5: Run store tests**

Run:

```bash
cargo test --test store_tests
```

Expected: PASS.

- [ ] **Step 6: Commit store**

Run:

```bash
git add src/paths.rs src/store.rs tests/store_tests.rs
git commit -m "feat: add file-backed task store"
```

Expected: commit succeeds.

## Task 4: CLI Create, Status, Resume, and Event Commands

**Files:**
- Modify: `src/cli.rs`
- Modify: `src/output.rs`
- Create: `tests/cli_task_flow.rs`

- [ ] **Step 1: Write CLI flow tests**

Create `tests/cli_task_flow.rs` with this content:

```rust
use assert_cmd::Command;
use predicates::str::contains;
use tempfile::tempdir;

fn aoc_with_home(home: &std::path::Path) -> Command {
    let mut cmd = Command::cargo_bin("aoc").unwrap();
    cmd.env("AOC_HOME", home);
    cmd
}

#[test]
fn create_status_event_and_resume_task() {
    let home = tempdir().unwrap();

    aoc_with_home(home.path())
        .args([
            "task",
            "create",
            "--id",
            "PM-20260509-001",
            "--title",
            "Fix login redirect bug",
            "--project",
            "/repo",
        ])
        .assert()
        .success()
        .stdout(contains("Created PM-20260509-001"));

    aoc_with_home(home.path())
        .args([
            "task",
            "event",
            "PM-20260509-001",
            "--type",
            "progress",
            "--message",
            "Found redirect handler",
        ])
        .assert()
        .success()
        .stdout(contains("Recorded progress for PM-20260509-001"));

    aoc_with_home(home.path())
        .args(["task", "status", "PM-20260509-001"])
        .assert()
        .success()
        .stdout(contains("PM-20260509-001"))
        .stdout(contains("Fix login redirect bug"))
        .stdout(contains("Found redirect handler"));

    aoc_with_home(home.path())
        .args(["task", "resume", "PM-20260509-001"])
        .assert()
        .success()
        .stdout(contains("No tmux session recorded"))
        .stdout(contains("No native resume command recorded"));
}
```

- [ ] **Step 2: Run CLI test to verify failure**

Run:

```bash
cargo test --test cli_task_flow
```

Expected: FAIL with unresolved `helm_agent::cli::run`.

- [ ] **Step 3: Implement output formatting**

Create `src/output.rs` with this content:

```rust
use crate::domain::{TaskEvent, TaskRecord};

pub fn task_status(task: &TaskRecord, events: &[TaskEvent]) -> String {
    let last_event = events
        .last()
        .map(|event| event.message.as_str())
        .unwrap_or(task.progress.last_event.as_str());

    format!(
        "{id} [{status:?}]\nTitle: {title}\nProject: {project}\nProgress: {progress}\nNext: {next}\n",
        id = task.id,
        status = task.status,
        title = task.title,
        project = task.project.path.display(),
        progress = last_event,
        next = task.progress.next_action,
    )
}

pub fn resume_text(task: &TaskRecord) -> String {
    let attach = task
        .recovery
        .attach_command
        .as_deref()
        .unwrap_or("No tmux session recorded");
    let resume = task
        .recovery
        .resume_command
        .as_deref()
        .unwrap_or("No native resume command recorded");

    format!(
        "{id}\nAttach: {attach}\nResume: {resume}\n",
        id = task.id
    )
}
```

- [ ] **Step 4: Implement CLI commands**

Create `src/cli.rs` with this content:

```rust
use crate::domain::{TaskEvent, TaskRecord};
use crate::output;
use crate::paths::aoc_home;
use crate::store::TaskStore;
use anyhow::Result;
use clap::{Args, Parser, Subcommand};
use std::path::PathBuf;
use time::OffsetDateTime;

#[derive(Debug, Parser)]
#[command(name = "aoc")]
#[command(about = "Agent Ops Center local coordinator")]
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
    event_type: String,
    #[arg(long)]
    message: String,
}

pub fn run() -> Result<()> {
    let cli = Cli::parse();
    let store = TaskStore::new(aoc_home()?);

    match cli.command {
        Command::Task(task) => handle_task(task, &store),
    }
}

fn handle_task(task: TaskCommand, store: &TaskStore) -> Result<()> {
    match task.command {
        TaskSubcommand::Create(args) => {
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
                args.event_type.clone(),
                args.message,
                now,
            ))?;
            println!("Recorded {} for {}", args.event_type, args.id);
            Ok(())
        }
    }
}
```

- [ ] **Step 5: Run CLI flow test**

Run:

```bash
cargo test --test cli_task_flow
```

Expected: PASS.

- [ ] **Step 6: Run full test suite**

Run:

```bash
cargo test
```

Expected: PASS.

- [ ] **Step 7: Commit CLI basics**

Run:

```bash
git add src/cli.rs src/output.rs tests/cli_task_flow.rs
git commit -m "feat: add task create and status cli"
```

Expected: commit succeeds.

## Task 5: Review State Transitions

**Files:**
- Modify: `src/cli.rs`
- Modify: `src/output.rs`
- Modify: `tests/cli_task_flow.rs`

- [ ] **Step 1: Add review CLI test**

Append this test to `tests/cli_task_flow.rs`:

```rust
#[test]
fn review_accept_and_request_changes_update_status() {
    let home = tempdir().unwrap();

    aoc_with_home(home.path())
        .args([
            "task",
            "create",
            "--id",
            "PM-20260509-002",
            "--title",
            "Review redirect patch",
            "--project",
            "/repo",
        ])
        .assert()
        .success();

    aoc_with_home(home.path())
        .args(["task", "review", "PM-20260509-002", "--accept"])
        .assert()
        .success()
        .stdout(contains("Accepted PM-20260509-002"));

    aoc_with_home(home.path())
        .args(["task", "status", "PM-20260509-002"])
        .assert()
        .success()
        .stdout(contains("Done"));

    aoc_with_home(home.path())
        .args([
            "task",
            "review",
            "PM-20260509-002",
            "--request-changes",
            "Add regression test",
        ])
        .assert()
        .success()
        .stdout(contains("Requested changes for PM-20260509-002"));

    aoc_with_home(home.path())
        .args(["task", "status", "PM-20260509-002"])
        .assert()
        .success()
        .stdout(contains("NeedsChanges"))
        .stdout(contains("Add regression test"));
}
```

- [ ] **Step 2: Run test to verify failure**

Run:

```bash
cargo test --test cli_task_flow review_accept_and_request_changes_update_status
```

Expected: FAIL because `task review` is not a recognized subcommand.

- [ ] **Step 3: Add review subcommand implementation**

Modify `src/cli.rs` by adding `Review(ReviewArgs)` to `TaskSubcommand`:

```rust
#[derive(Debug, Subcommand)]
enum TaskSubcommand {
    Create(CreateArgs),
    Status(StatusArgs),
    Resume(ResumeArgs),
    Event(EventArgs),
    Review(ReviewArgs),
}
```

Add this args struct after `EventArgs`:

```rust
#[derive(Debug, Args)]
struct ReviewArgs {
    id: String,
    #[arg(long, conflicts_with = "request_changes")]
    accept: bool,
    #[arg(long = "request-changes")]
    request_changes: Option<String>,
}
```

Add imports at the top of `src/cli.rs`:

```rust
use crate::domain::{ReviewState, TaskEvent, TaskRecord, TaskStatus};
```

Replace the existing domain import line with the import above.

Add this match arm inside `handle_task`:

```rust
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

            anyhow::bail!("review requires --accept or --request-changes <message>");
        }
```

- [ ] **Step 4: Run review test**

Run:

```bash
cargo test --test cli_task_flow review_accept_and_request_changes_update_status
```

Expected: PASS.

- [ ] **Step 5: Run full tests**

Run:

```bash
cargo test
```

Expected: PASS.

- [ ] **Step 6: Commit review transitions**

Run:

```bash
git add src/cli.rs tests/cli_task_flow.rs
git commit -m "feat: add task review transitions"
```

Expected: commit succeeds.

## Task 6: Default Policy

**Files:**
- Create: `src/policy.rs`
- Create: `tests/policy_tests.rs`
- Modify: `src/cli.rs`

- [ ] **Step 1: Write policy tests**

Create `tests/policy_tests.rs` with this content:

```rust
use helm_agent::domain::{AgentRuntime, RiskLevel};
use helm_agent::policy::{DispatchDecision, PolicyInput};

#[test]
fn low_risk_free_read_task_can_auto_start() {
    let input = PolicyInput {
        risk: RiskLevel::Low,
        runtime: AgentRuntime::OpenCode,
        writes_files: false,
        paid_runtime: false,
        cross_project: false,
        network_sensitive: false,
    };

    let decision = input.evaluate();

    assert_eq!(decision, DispatchDecision::AutoStart);
}

#[test]
fn codex_requires_confirmation() {
    let input = PolicyInput {
        risk: RiskLevel::Low,
        runtime: AgentRuntime::Codex,
        writes_files: false,
        paid_runtime: true,
        cross_project: false,
        network_sensitive: false,
    };

    let decision = input.evaluate();

    assert_eq!(decision, DispatchDecision::ConfirmRequired);
}

#[test]
fn high_risk_requires_confirmation() {
    let input = PolicyInput {
        risk: RiskLevel::High,
        runtime: AgentRuntime::Claude,
        writes_files: false,
        paid_runtime: false,
        cross_project: false,
        network_sensitive: false,
    };

    let decision = input.evaluate();

    assert_eq!(decision, DispatchDecision::ConfirmRequired);
}
```

- [ ] **Step 2: Run policy tests to verify failure**

Run:

```bash
cargo test --test policy_tests
```

Expected: FAIL with unresolved `PolicyInput`.

- [ ] **Step 3: Implement policy module**

Create `src/policy.rs` with this content:

```rust
use crate::domain::{AgentRuntime, RiskLevel};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DispatchDecision {
    AutoStart,
    ConfirmRequired,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PolicyInput {
    pub risk: RiskLevel,
    pub runtime: AgentRuntime,
    pub writes_files: bool,
    pub paid_runtime: bool,
    pub cross_project: bool,
    pub network_sensitive: bool,
}

impl PolicyInput {
    pub fn evaluate(self) -> DispatchDecision {
        if self.risk == RiskLevel::High
            || self.paid_runtime
            || self.runtime == AgentRuntime::Codex
            || self.cross_project
            || self.network_sensitive
        {
            return DispatchDecision::ConfirmRequired;
        }

        DispatchDecision::AutoStart
    }
}
```

- [ ] **Step 4: Run policy tests**

Run:

```bash
cargo test --test policy_tests
```

Expected: PASS.

- [ ] **Step 5: Commit policy**

Run:

```bash
git add src/policy.rs tests/policy_tests.rs
git commit -m "feat: add default dispatch policy"
```

Expected: commit succeeds.

## Task 7: Adapter Capabilities and Dry-Run Dispatch

**Files:**
- Modify: `src/adapter.rs`
- Modify: `src/launcher.rs`
- Create: `tests/launcher_tests.rs`
- Modify: `src/cli.rs`
- Modify: `src/domain.rs`

- [ ] **Step 1: Write launcher tests**

Create `tests/launcher_tests.rs` with this content:

```rust
use helm_agent::adapter::RuntimeAdapter;
use helm_agent::domain::AgentRuntime;
use helm_agent::launcher::{DispatchPlan, Launcher};
use std::path::PathBuf;

#[test]
fn adapters_expose_command_and_resume_templates() {
    let claude = RuntimeAdapter::for_runtime(AgentRuntime::Claude);
    let codex = RuntimeAdapter::for_runtime(AgentRuntime::Codex);

    assert_eq!(claude.command, "claude");
    assert_eq!(claude.native_resume_template, "claude --resume <session-id>");
    assert_eq!(codex.command, "codex");
    assert_eq!(
        codex.native_resume_template,
        "codex resume <session-id> --all"
    );
}

#[test]
fn launcher_builds_tmux_session_and_recovery_commands() {
    let plan = DispatchPlan {
        task_id: "PM-20260509-001".to_string(),
        runtime: AgentRuntime::Claude,
        cwd: PathBuf::from("/repo"),
    };

    let launch = Launcher::new().dry_run(&plan);

    assert_eq!(launch.tmux_session, "aoc-PM-20260509-001-claude");
    assert_eq!(
        launch.attach_command,
        "tmux attach -t aoc-PM-20260509-001-claude"
    );
    assert_eq!(
        launch.start_command,
        "tmux new-session -d -s aoc-PM-20260509-001-claude -c /repo claude"
    );
    assert_eq!(launch.resume_command, "claude --resume <session-id>");
}

#[test]
fn codex_resume_command_uses_codex_resume() {
    let plan = DispatchPlan {
        task_id: "PM-20260509-003".to_string(),
        runtime: AgentRuntime::Codex,
        cwd: PathBuf::from("/repo"),
    };

    let launch = Launcher::new().dry_run(&plan);

    assert_eq!(launch.resume_command, "codex resume <session-id> --all");
}
```

- [ ] **Step 2: Run launcher tests to verify failure**

Run:

```bash
cargo test --test launcher_tests
```

Expected: FAIL with unresolved `Launcher`.

- [ ] **Step 3: Implement runtime adapter metadata**

Replace `src/adapter.rs` with this content:

```rust
use crate::domain::AgentRuntime;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeAdapter {
    pub runtime: AgentRuntime,
    pub command: &'static str,
    pub native_resume_template: &'static str,
    pub native_resume_available: bool,
    pub acp_supported: bool,
}

impl RuntimeAdapter {
    pub fn for_runtime(runtime: AgentRuntime) -> Self {
        match runtime {
            AgentRuntime::Claude => Self {
                runtime,
                command: "claude",
                native_resume_template: "claude --resume <session-id>",
                native_resume_available: true,
                acp_supported: false,
            },
            AgentRuntime::Codex => Self {
                runtime,
                command: "codex",
                native_resume_template: "codex resume <session-id> --all",
                native_resume_available: true,
                acp_supported: false,
            },
            AgentRuntime::OpenCode => Self {
                runtime,
                command: "opencode",
                native_resume_template: "opencode resume <session-id>",
                native_resume_available: false,
                acp_supported: false,
            },
        }
    }
}
```

- [ ] **Step 4: Implement launcher dry-run**

Replace `src/launcher.rs` with this content:

```rust
use crate::adapter::RuntimeAdapter;
use crate::domain::AgentRuntime;
use std::path::PathBuf;

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
    pub resume_command: String,
}

#[derive(Debug, Default, Clone, Copy)]
pub struct Launcher;

impl Launcher {
    pub fn new() -> Self {
        Self
    }

    pub fn dry_run(&self, plan: &DispatchPlan) -> LaunchPreview {
        let runtime = plan.runtime.as_str();
        let adapter = RuntimeAdapter::for_runtime(plan.runtime);
        let tmux_session = format!("aoc-{}-{runtime}", plan.task_id);
        let attach_command = format!("tmux attach -t {tmux_session}");
        let start_command = format!(
            "tmux new-session -d -s {tmux_session} -c {} {}",
            plan.cwd.display(),
            adapter.command
        );
        let resume_command = adapter.native_resume_template.to_string();

        LaunchPreview {
            tmux_session,
            start_command,
            attach_command,
            resume_command,
        }
    }
}
```

- [ ] **Step 5: Add dispatch CLI test**

Append this test to `tests/cli_task_flow.rs`:

```rust
#[test]
fn dry_run_dispatch_records_recovery_commands() {
    let home = tempdir().unwrap();

    aoc_with_home(home.path())
        .args([
            "task",
            "create",
            "--id",
            "PM-20260509-003",
            "--title",
            "Investigate failing test",
            "--project",
            "/repo",
        ])
        .assert()
        .success();

    aoc_with_home(home.path())
        .args([
            "task",
            "dispatch",
            "PM-20260509-003",
            "--runtime",
            "claude",
            "--dry-run",
        ])
        .assert()
        .success()
        .stdout(contains("Dry-run dispatch PM-20260509-003"))
        .stdout(contains("tmux attach -t aoc-PM-20260509-003-claude"));

    aoc_with_home(home.path())
        .args(["task", "resume", "PM-20260509-003"])
        .assert()
        .success()
        .stdout(contains("tmux attach -t aoc-PM-20260509-003-claude"))
        .stdout(contains("claude --resume <session-id>"));
}
```

- [ ] **Step 6: Add dispatch CLI implementation**

Modify `src/cli.rs` by adding imports:

```rust
use crate::domain::{AgentRuntime, ReviewState, TaskEvent, TaskRecord, TaskStatus};
use crate::launcher::{DispatchPlan, Launcher};
```

Replace the existing domain import with the domain import above.

Add `Dispatch(DispatchArgs)` to `TaskSubcommand`:

```rust
#[derive(Debug, Subcommand)]
enum TaskSubcommand {
    Create(CreateArgs),
    Status(StatusArgs),
    Resume(ResumeArgs),
    Event(EventArgs),
    Review(ReviewArgs),
    Dispatch(DispatchArgs),
}
```

Add this args struct:

```rust
#[derive(Debug, Args)]
struct DispatchArgs {
    id: String,
    #[arg(long)]
    runtime: RuntimeArg,
    #[arg(long)]
    dry_run: bool,
}

#[derive(Debug, Clone, Copy, clap::ValueEnum)]
enum RuntimeArg {
    Claude,
    Codex,
    Opencode,
}

impl From<RuntimeArg> for AgentRuntime {
    fn from(value: RuntimeArg) -> Self {
        match value {
            RuntimeArg::Claude => AgentRuntime::Claude,
            RuntimeArg::Codex => AgentRuntime::Codex,
            RuntimeArg::Opencode => AgentRuntime::OpenCode,
        }
    }
}
```

Add this match arm:

```rust
        TaskSubcommand::Dispatch(args) => {
            let now = OffsetDateTime::now_utc();
            let mut task = store.load_task(&args.id)?;
            let runtime = AgentRuntime::from(args.runtime);
            let launch = Launcher::new().dry_run(&DispatchPlan {
                task_id: args.id.clone(),
                runtime,
                cwd: task.project.path.clone(),
            });

            task.status = TaskStatus::Queued;
            task.assignment.runtime = Some(runtime);
            task.assignment.tmux_session = Some(launch.tmux_session.clone());
            task.recovery.attach_command = Some(launch.attach_command.clone());
            task.recovery.resume_command = Some(launch.resume_command.clone());
            task.progress.last_event = if args.dry_run {
                "Dry-run dispatch recorded".to_string()
            } else {
                "Dispatch requested".to_string()
            };
            task.progress.next_action = "Start or inspect child agent session".to_string();
            task.touch(now);
            store.save_task(&task)?;
            store.append_event(&TaskEvent::new(
                args.id.clone(),
                "dispatch_planned",
                launch.start_command.clone(),
                now,
            ))?;

            if args.dry_run {
                println!("Dry-run dispatch {}", args.id);
            } else {
                println!("Dispatch planned {}", args.id);
            }
            println!("Start: {}", launch.start_command);
            println!("Attach: {}", launch.attach_command);
            println!("Resume: {}", launch.resume_command);
            Ok(())
        }
```

- [ ] **Step 7: Run launcher and CLI tests**

Run:

```bash
cargo test --test launcher_tests
cargo test --test cli_task_flow dry_run_dispatch_records_recovery_commands
```

Expected: both commands PASS.

- [ ] **Step 8: Run full tests**

Run:

```bash
cargo test
```

Expected: PASS.

- [ ] **Step 9: Commit dry-run launcher**

Run:

```bash
git add src/adapter.rs src/launcher.rs src/cli.rs tests/launcher_tests.rs tests/cli_task_flow.rs
git commit -m "feat: add dry-run tmux dispatch"
```

Expected: commit succeeds.

## Task 8: Executable tmux Dispatch

**Files:**
- Modify: `src/launcher.rs`
- Modify: `src/cli.rs`
- Modify: `tests/launcher_tests.rs`
- Modify: `tests/cli_task_flow.rs`

- [ ] **Step 1: Add launcher execution test with fake tmux**

Append this test to `tests/launcher_tests.rs`:

```rust
#[cfg(unix)]
#[test]
fn launch_executes_tmux_with_expected_arguments() {
    use std::fs;
    use std::os::unix::fs::PermissionsExt;
    use tempfile::tempdir;

    let temp = tempdir().unwrap();
    let args_file = temp.path().join("args.txt");
    let tmux_bin = temp.path().join("tmux");
    fs::write(
        &tmux_bin,
        format!("#!/bin/sh\necho \"$@\" > {}\nexit 0\n", args_file.display()),
    )
    .unwrap();
    let mut permissions = fs::metadata(&tmux_bin).unwrap().permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&tmux_bin, permissions).unwrap();

    let plan = DispatchPlan {
        task_id: "PM-20260509-004".to_string(),
        runtime: AgentRuntime::Claude,
        cwd: PathBuf::from("/repo"),
    };

    let launch = Launcher::with_tmux_bin(tmux_bin).launch(&plan).unwrap();
    let args = fs::read_to_string(args_file).unwrap();

    assert_eq!(launch.tmux_session, "aoc-PM-20260509-004-claude");
    assert!(args.contains("new-session -d -s aoc-PM-20260509-004-claude -c /repo claude"));
}
```

- [ ] **Step 2: Run launch test to verify failure**

Run:

```bash
cargo test --test launcher_tests launch_executes_tmux_with_expected_arguments
```

Expected: FAIL because `Launcher::with_tmux_bin` and `Launcher::launch` do not exist.

- [ ] **Step 3: Implement executable launcher**

Replace `src/launcher.rs` with this content:

```rust
use crate::adapter::RuntimeAdapter;
use crate::domain::AgentRuntime;
use anyhow::{bail, Context, Result};
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
    pub resume_command: String,
}

#[derive(Debug, Clone)]
pub struct Launcher {
    tmux_bin: PathBuf,
}

impl Default for Launcher {
    fn default() -> Self {
        Self::new()
    }
}

impl Launcher {
    pub fn new() -> Self {
        let tmux_bin = std::env::var_os("AOC_TMUX_BIN")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("tmux"));
        Self { tmux_bin }
    }

    pub fn with_tmux_bin(tmux_bin: PathBuf) -> Self {
        Self { tmux_bin }
    }

    pub fn dry_run(&self, plan: &DispatchPlan) -> LaunchPreview {
        let runtime = plan.runtime.as_str();
        let adapter = RuntimeAdapter::for_runtime(plan.runtime);
        let tmux_session = format!("aoc-{}-{runtime}", plan.task_id);
        let attach_command = format!("tmux attach -t {tmux_session}");
        let start_command = format!(
            "tmux new-session -d -s {tmux_session} -c {} {}",
            plan.cwd.display(),
            adapter.command
        );
        let resume_command = adapter.native_resume_template.to_string();

        LaunchPreview {
            tmux_session,
            start_command,
            attach_command,
            resume_command,
        }
    }

    pub fn launch(&self, plan: &DispatchPlan) -> Result<LaunchPreview> {
        let preview = self.dry_run(plan);
        let adapter = RuntimeAdapter::for_runtime(plan.runtime);
        let status = Command::new(&self.tmux_bin)
            .arg("new-session")
            .arg("-d")
            .arg("-s")
            .arg(&preview.tmux_session)
            .arg("-c")
            .arg(&plan.cwd)
            .arg(adapter.command)
            .status()
            .with_context(|| format!("start tmux session {}", preview.tmux_session))?;

        if !status.success() {
            bail!(
                "tmux failed to start session {} with status {}",
                preview.tmux_session,
                status
            );
        }

        Ok(preview)
    }
}
```

- [ ] **Step 4: Add CLI execution test with fake tmux**

Append this test to `tests/cli_task_flow.rs`:

```rust
#[cfg(unix)]
#[test]
fn non_dry_run_dispatch_invokes_tmux_and_records_running_state() {
    use std::fs;
    use std::os::unix::fs::PermissionsExt;

    let home = tempdir().unwrap();
    let fake = tempdir().unwrap();
    let args_file = fake.path().join("args.txt");
    let tmux_bin = fake.path().join("tmux");
    fs::write(
        &tmux_bin,
        format!("#!/bin/sh\necho \"$@\" > {}\nexit 0\n", args_file.display()),
    )
    .unwrap();
    let mut permissions = fs::metadata(&tmux_bin).unwrap().permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&tmux_bin, permissions).unwrap();

    aoc_with_home(home.path())
        .args([
            "task",
            "create",
            "--id",
            "PM-20260509-004",
            "--title",
            "Run child agent",
            "--project",
            "/repo",
        ])
        .assert()
        .success();

    let mut dispatch = aoc_with_home(home.path());
    dispatch.env("AOC_TMUX_BIN", &tmux_bin);
    dispatch
        .args([
            "task",
            "dispatch",
            "PM-20260509-004",
            "--runtime",
            "claude",
        ])
        .assert()
        .success()
        .stdout(contains("Started PM-20260509-004"))
        .stdout(contains("tmux attach -t aoc-PM-20260509-004-claude"));

    assert!(fs::read_to_string(args_file)
        .unwrap()
        .contains("new-session -d -s aoc-PM-20260509-004-claude -c /repo claude"));

    aoc_with_home(home.path())
        .args(["task", "status", "PM-20260509-004"])
        .assert()
        .success()
        .stdout(contains("Running"));
}
```

- [ ] **Step 5: Update dispatch CLI to execute tmux when not dry-run**

In `src/cli.rs`, replace this block inside `TaskSubcommand::Dispatch`:

```rust
            let launch = Launcher::new().dry_run(&DispatchPlan {
                task_id: args.id.clone(),
                runtime,
                cwd: task.project.path.clone(),
            });
```

with this block:

```rust
            let launcher = Launcher::new();
            let plan = DispatchPlan {
                task_id: args.id.clone(),
                runtime,
                cwd: task.project.path.clone(),
            };
            let launch = if args.dry_run {
                launcher.dry_run(&plan)
            } else {
                launcher.launch(&plan)?
            };
```

In the same match arm, replace:

```rust
            task.status = TaskStatus::Queued;
```

with:

```rust
            task.status = if args.dry_run {
                TaskStatus::Queued
            } else {
                TaskStatus::Running
            };
```

Replace the final output block:

```rust
            if args.dry_run {
                println!("Dry-run dispatch {}", args.id);
            } else {
                println!("Dispatch planned {}", args.id);
            }
```

with:

```rust
            if args.dry_run {
                println!("Dry-run dispatch {}", args.id);
            } else {
                println!("Started {}", args.id);
            }
```

- [ ] **Step 6: Run launcher execution tests**

Run:

```bash
cargo test --test launcher_tests launch_executes_tmux_with_expected_arguments
cargo test --test cli_task_flow non_dry_run_dispatch_invokes_tmux_and_records_running_state
```

Expected: both commands PASS.

- [ ] **Step 7: Run full tests**

Run:

```bash
cargo test
```

Expected: PASS.

- [ ] **Step 8: Commit executable dispatch**

Run:

```bash
git add src/launcher.rs src/cli.rs tests/launcher_tests.rs tests/cli_task_flow.rs
git commit -m "feat: execute tmux dispatch"
```

Expected: commit succeeds.

## Task 9: Main-Agent Integration Documentation

**Files:**
- Create: `docs/agent-integrations/main-agent.md`
- Modify: `README.md`

- [ ] **Step 1: Add main-agent usage guide**

Create `docs/agent-integrations/main-agent.md` with this content:

```markdown
# Main Agent Usage Guide

Agent Ops Center is controlled through the `aoc` CLI. Claude Code, Codex, and other main agents should treat `aoc` as the source of truth for task state.

## Main Agent Rules

1. Create a task before delegating work.
2. Use `aoc task status <id>` before reporting progress to the user.
3. Use `aoc task dispatch <id> --runtime <runtime> --dry-run` before starting a child agent.
4. Do not claim a code-changing task is complete until it is in `ready_for_review` or the user accepts it.
5. Always show the user the attach and resume commands for delegated work.
6. Ask before using Codex unless the user explicitly approved Codex for this task.

## Common Commands

```bash
aoc task create --id PM-20260509-001 --title "Fix login redirect bug" --project /path/to/repo
aoc task event PM-20260509-001 --type progress --message "Found redirect handler"
aoc task dispatch PM-20260509-001 --runtime claude --dry-run
aoc task status PM-20260509-001
aoc task resume PM-20260509-001
aoc task review PM-20260509-001 --accept
aoc task review PM-20260509-001 --request-changes "Add regression test"
```

## Delegation Summary Template

```text
Task: <task id> <title>
Runtime: <claude|codex|opencode>
Reason: <one sentence>
Status: <status>
Attach: <tmux attach command>
Resume: <native resume command>
Review: <why human review is or is not required>
```
```

- [ ] **Step 2: Update README with V1 usage**

Replace `README.md` with this content:

```markdown
# HelmAgent

HelmAgent provides the `aoc` CLI, a local Agent Ops Center for coordinating coding agents through trackable tasks, tmux sessions, and human review checkpoints.

## V1 Focus

- Create local task records.
- Track progress and review state.
- Record tmux attach and native resume commands.
- Prefer free agents for low-risk work.
- Require confirmation before paid Codex dispatch.

## Development

```bash
cargo test
cargo run --bin aoc -- task create --id PM-20260509-001 --title "Example" --project .
cargo run --bin aoc -- task status PM-20260509-001
```

## Main Agent Integration

See `docs/agent-integrations/main-agent.md`.
```

- [ ] **Step 3: Run docs smoke command**

Run:

```bash
cargo run --bin aoc -- task create --id PM-20260509-900 --title "Docs smoke task" --project .
```

Expected: exits 0 and prints `Created PM-20260509-900`.

- [ ] **Step 4: Commit docs**

Run:

```bash
git add README.md docs/agent-integrations/main-agent.md
git commit -m "docs: add main agent usage guide"
```

Expected: commit succeeds.

## Task 10: Final Verification

**Files:**
- Modify only files needed to fix verification failures.

- [ ] **Step 1: Run formatter check**

Run:

```bash
cargo fmt --check
```

Expected: PASS.

- [ ] **Step 2: Run full tests**

Run:

```bash
cargo test
```

Expected: PASS.

- [ ] **Step 3: Run CLI manual flow with temporary home**

Run:

```bash
AOC_HOME=/tmp/aoc-smoke cargo run --bin aoc -- task create --id PM-20260509-999 --title "Smoke task" --project .
AOC_HOME=/tmp/aoc-smoke cargo run --bin aoc -- task event PM-20260509-999 --type progress --message "Smoke progress"
AOC_HOME=/tmp/aoc-smoke cargo run --bin aoc -- task dispatch PM-20260509-999 --runtime claude --dry-run
AOC_HOME=/tmp/aoc-smoke cargo run --bin aoc -- task status PM-20260509-999
AOC_HOME=/tmp/aoc-smoke cargo run --bin aoc -- task resume PM-20260509-999
```

Expected:

```text
Created PM-20260509-999
Recorded progress for PM-20260509-999
Dry-run dispatch PM-20260509-999
PM-20260509-999 [Queued]
Attach: tmux attach -t aoc-PM-20260509-999-claude
Resume: claude --resume <session-id>
```

- [ ] **Step 4: Check git status**

Run:

```bash
git status --short
```

Expected: no unstaged or untracked implementation files except temporary files outside the repository.

- [ ] **Step 5: Commit verification fixes**

If Step 4 shows tracked implementation files changed because of verification fixes, run:

```bash
git add <changed implementation files>
git commit -m "fix: stabilize aoc verification"
```

Expected: commit succeeds when there are fixes. If there are no changes, skip this command.

## Self-Review Notes

Spec coverage:

- CLI-first Rust core is covered by Tasks 1, 4, 7, 8, and 10.
- File-backed task store is covered by Task 3.
- Task state and review transitions are covered by Tasks 2 and 5.
- Policy engine is covered by Task 6.
- Runtime adapter metadata is covered by Task 7.
- tmux command construction and recovery command recording are covered by Task 7.
- Executable tmux dispatch is covered by Task 8.
- Main-agent integration guidance is covered by Task 9.
- Web Board, heavy isolation, container sandboxing, and ACP transport remain outside this V1 plan by design.

Type consistency:

- Task IDs are passed as `String` and file paths as `PathBuf`.
- `AgentRuntime` CLI values map to `claude`, `codex`, and `opencode`.
- Review transitions use `TaskStatus::Done` and `TaskStatus::NeedsChanges`.
- Dispatch dry-run uses `TaskStatus::Queued` and records recovery commands.
- Executed dispatch uses `TaskStatus::Running` after tmux starts successfully.
