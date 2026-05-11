# HelmAgent Phase 2 Review Queue Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add task listing, real review/blocker state transitions, and triage inputs so HelmAgent can act as a usable review queue.

**Architecture:** Keep the CLI-first Rust architecture. Extend the domain with parsing helpers, extend `TaskStore` with task enumeration, keep output formatting in `src/output.rs`, and add focused CLI commands in `src/cli.rs`.

**Tech Stack:** Rust 2021, clap derive, serde YAML/JSONL store, assert_cmd integration tests, tempfile-backed CLI flows.

---

## File Map

- `src/domain.rs`: add parsing helpers and display helpers for `TaskStatus` and `RiskLevel`.
- `src/store.rs`: add `list_tasks()` for enumerating persisted task YAML records.
- `src/output.rs`: add compact list output.
- `src/cli.rs`: add `task list`, `task mark`, and `task triage`.
- `tests/store_tests.rs`: cover list enumeration and parse errors.
- `tests/cli_task_flow.rs`: cover list, mark, triage, and policy interaction.
- `README.md`: mention Phase 2 review queue commands.
- `docs/agent-integrations/main-agent.md`: update main-agent flow to use real mark/triage commands.

---

## Task 1: Store Enumeration

**Files:**
- Modify: `src/store.rs`
- Modify: `tests/store_tests.rs`

- [ ] **Step 1: Write failing store enumeration tests**

Append to `tests/store_tests.rs`:

```rust
#[test]
fn list_tasks_returns_all_saved_tasks() {
    let temp = tempdir().unwrap();
    let store = TaskStore::new(temp.path().to_path_buf());
    let now = OffsetDateTime::UNIX_EPOCH;
    let first = TaskRecord::new(
        "PM-20260511-001".to_string(),
        "First task".to_string(),
        "/repo".into(),
        now,
    );
    let second = TaskRecord::new(
        "A/B".to_string(),
        "Unsafe id task".to_string(),
        "/repo".into(),
        now,
    );

    store.save_task(&first).unwrap();
    store.save_task(&second).unwrap();

    let mut tasks = store.list_tasks().unwrap();
    tasks.sort_by(|left, right| left.id.cmp(&right.id));

    assert_eq!(tasks.len(), 2);
    assert_eq!(tasks[0].id, "A/B");
    assert_eq!(tasks[1].id, "PM-20260511-001");
}
```

- [ ] **Step 2: Verify store tests fail**

Run:

```bash
rtk cargo test --test store_tests list_tasks_returns_all_saved_tasks
```

Expected: fail because `TaskStore::list_tasks` does not exist.

- [ ] **Step 3: Implement `TaskStore::list_tasks`**

Add to `impl TaskStore` in `src/store.rs`:

```rust
pub fn list_tasks(&self) -> Result<Vec<TaskRecord>> {
    let tasks_dir = self.root.join("tasks");
    if !tasks_dir.exists() {
        return Ok(Vec::new());
    }

    let mut tasks = Vec::new();
    for year_entry in fs::read_dir(&tasks_dir)
        .with_context(|| format!("read tasks directory {}", tasks_dir.display()))?
    {
        let year_entry =
            year_entry.with_context(|| format!("read entry from {}", tasks_dir.display()))?;
        let year_path = year_entry.path();
        if !year_path.is_dir() {
            continue;
        }

        for task_entry in fs::read_dir(&year_path)
            .with_context(|| format!("read task year directory {}", year_path.display()))?
        {
            let task_entry =
                task_entry.with_context(|| format!("read entry from {}", year_path.display()))?;
            let path = task_entry.path();
            if path.extension().and_then(|value| value.to_str()) != Some("yaml") {
                continue;
            }

            let content = fs::read_to_string(&path)
                .with_context(|| format!("read task {}", path.display()))?;
            let task: TaskRecord = serde_yaml::from_str(&content)
                .with_context(|| format!("parse task {}", path.display()))?;
            if self.task_path(&task.id) != path {
                bail!(
                    "task id mismatch: loaded {} from unexpected path {}",
                    task.id,
                    path.display()
                );
            }
            tasks.push(task);
        }
    }

    Ok(tasks)
}
```

- [ ] **Step 4: Verify store tests pass**

Run:

```bash
rtk cargo test --test store_tests
```

Expected: pass.

- [ ] **Step 5: Commit**

Run:

```bash
rtk git add src/store.rs tests/store_tests.rs
rtk git commit -m "feat: list stored tasks"
```

---

## Task 2: List Output and CLI

**Files:**
- Modify: `src/output.rs`
- Modify: `src/cli.rs`
- Modify: `tests/cli_task_flow.rs`

- [ ] **Step 1: Write failing CLI list tests**

Append to `tests/cli_task_flow.rs`:

```rust
#[test]
fn list_tasks_shows_active_tasks_newest_first() {
    let home = tempdir().unwrap();

    helm_agent_with_home(home.path())
        .args([
            "task",
            "create",
            "--id",
            "PM-20260511-001",
            "--title",
            "Older task",
            "--project",
            "/repo",
        ])
        .assert()
        .success();

    helm_agent_with_home(home.path())
        .args([
            "task",
            "create",
            "--id",
            "PM-20260511-002",
            "--title",
            "Newer task",
            "--project",
            "/repo",
        ])
        .assert()
        .success();

    helm_agent_with_home(home.path())
        .args(["task", "list"])
        .assert()
        .success()
        .stdout(contains("PM-20260511-002"))
        .stdout(contains("PM-20260511-001"))
        .stdout(contains("Newer task"))
        .stdout(contains("Older task"));
}

#[test]
fn list_tasks_filters_by_status_and_review_queue() {
    let home = tempdir().unwrap();

    helm_agent_with_home(home.path())
        .args([
            "task",
            "create",
            "--id",
            "PM-20260511-003",
            "--title",
            "Running task",
            "--project",
            "/repo",
        ])
        .assert()
        .success();
    helm_agent_with_home(home.path())
        .args([
            "task",
            "create",
            "--id",
            "PM-20260511-004",
            "--title",
            "Review task",
            "--project",
            "/repo",
        ])
        .assert()
        .success();
    helm_agent_with_home(home.path())
        .args([
            "task",
            "dispatch",
            "PM-20260511-003",
            "--runtime",
            "claude",
            "--dry-run",
        ])
        .assert()
        .success();
    helm_agent_with_home(home.path())
        .args([
            "task",
            "mark",
            "PM-20260511-004",
            "--ready-for-review",
            "--message",
            "Ready",
        ])
        .assert()
        .success();

    helm_agent_with_home(home.path())
        .args(["task", "list", "--status", "queued"])
        .assert()
        .success()
        .stdout(contains("PM-20260511-003"))
        .stdout(predicates::str::contains("PM-20260511-004").not());

    helm_agent_with_home(home.path())
        .args(["task", "list", "--review"])
        .assert()
        .success()
        .stdout(contains("PM-20260511-004"))
        .stdout(predicates::str::contains("PM-20260511-003").not());
}
```

- [ ] **Step 2: Verify CLI list tests fail**

Run:

```bash
rtk cargo test --test cli_task_flow list_tasks
```

Expected: fail because `task list` and `task mark` do not exist.

- [ ] **Step 3: Add list output helper**

Add to `src/output.rs`:

```rust
pub fn task_list(tasks: &[TaskRecord]) -> String {
    let mut output = String::new();
    for task in tasks {
        let runtime = task
            .assignment
            .runtime
            .map(|runtime| runtime.as_str())
            .unwrap_or("-");
        output.push_str(&format!(
            "{id}\t{status}\t{risk:?}\t{runtime}\t{title}\t{last}\t{next}\n",
            id = task.id,
            status = task.status.as_str(),
            risk = task.risk,
            runtime = runtime,
            title = task.title,
            last = task.progress.last_event,
            next = task.progress.next_action,
        ));
    }
    output
}
```

- [ ] **Step 4: Add list CLI args and handler**

In `src/cli.rs`, add `List(ListArgs)` to `TaskSubcommand`.

Add:

```rust
#[derive(Debug, Args)]
struct ListArgs {
    #[arg(long = "status")]
    status: Vec<StatusArg>,
    #[arg(long)]
    review: bool,
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
```

Add match arm:

```rust
TaskSubcommand::List(args) => {
    let mut tasks = store.list_tasks()?;
    tasks.retain(|task| task.status != TaskStatus::Archived);
    if !args.status.is_empty() {
        let statuses: Vec<TaskStatus> = args.status.into_iter().map(TaskStatus::from).collect();
        tasks.retain(|task| statuses.contains(&task.status));
    }
    if args.review {
        tasks.retain(|task| {
            matches!(
                task.status,
                TaskStatus::ReadyForReview | TaskStatus::Reviewing | TaskStatus::NeedsChanges
            )
        });
    }
    tasks.sort_by(|left, right| right.updated_at.cmp(&left.updated_at));
    print!("{}", output::task_list(&tasks));
    Ok(())
}
```

- [ ] **Step 5: Verify list tests after Task 3 mark implementation**

Run after Task 3 is implemented:

```bash
rtk cargo test --test cli_task_flow list_tasks
```

Expected: pass.

- [ ] **Step 6: Commit after Task 3**

Commit list and mark together if Task 2 tests need Task 3.

---

## Task 3: Mark State Transitions

**Files:**
- Modify: `src/cli.rs`
- Modify: `tests/cli_task_flow.rs`

- [ ] **Step 1: Write failing mark tests**

Append to `tests/cli_task_flow.rs`:

```rust
#[test]
fn mark_ready_for_review_and_blocked_update_real_status() {
    let home = tempdir().unwrap();

    helm_agent_with_home(home.path())
        .args([
            "task",
            "create",
            "--id",
            "PM-20260511-005",
            "--title",
            "Review me",
            "--project",
            "/repo",
        ])
        .assert()
        .success();

    helm_agent_with_home(home.path())
        .args([
            "task",
            "mark",
            "PM-20260511-005",
            "--ready-for-review",
            "--message",
            "Patch and tests ready",
        ])
        .assert()
        .success()
        .stdout(contains("Marked PM-20260511-005 ready_for_review"));

    helm_agent_with_home(home.path())
        .args(["task", "status", "PM-20260511-005"])
        .assert()
        .success()
        .stdout(contains("[ready_for_review]"))
        .stdout(contains("Patch and tests ready"));

    let store = TaskStore::new(home.path().to_path_buf());
    let task = store.load_task("PM-20260511-005").unwrap();
    assert_eq!(task.review.state, helm_agent::domain::ReviewState::Required);

    helm_agent_with_home(home.path())
        .args([
            "task",
            "mark",
            "PM-20260511-005",
            "--blocked",
            "--message",
            "Waiting for user",
        ])
        .assert()
        .success()
        .stdout(contains("Marked PM-20260511-005 blocked"));

    let task = store.load_task("PM-20260511-005").unwrap();
    assert_eq!(task.status, TaskStatus::Blocked);
    assert_eq!(task.progress.blocker.as_deref(), Some("Waiting for user"));
}

#[test]
fn mark_requires_one_state_and_message() {
    let home = tempdir().unwrap();

    helm_agent_with_home(home.path())
        .args(["task", "mark", "PM-20260511-404", "--ready-for-review"])
        .assert()
        .failure()
        .stderr(contains("required"));

    helm_agent_with_home(home.path())
        .args([
            "task",
            "mark",
            "PM-20260511-404",
            "--ready-for-review",
            "--blocked",
            "--message",
            "bad",
        ])
        .assert()
        .failure()
        .stderr(contains("cannot be used with"));
}
```

- [ ] **Step 2: Verify mark tests fail**

Run:

```bash
rtk cargo test --test cli_task_flow mark_
```

Expected: fail because `task mark` does not exist.

- [ ] **Step 3: Implement mark args and handler**

Add `Mark(MarkArgs)` to `TaskSubcommand`.

Add:

```rust
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
```

Add match arm:

```rust
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
```

- [ ] **Step 4: Verify mark and list tests pass**

Run:

```bash
rtk cargo test --test cli_task_flow mark_
rtk cargo test --test cli_task_flow list_tasks
```

Expected: pass.

- [ ] **Step 5: Commit list and mark**

Run:

```bash
rtk git add src/output.rs src/cli.rs tests/cli_task_flow.rs
rtk git commit -m "feat: add task list and mark"
```

---

## Task 4: Triage Command

**Files:**
- Modify: `src/cli.rs`
- Modify: `tests/cli_task_flow.rs`

- [ ] **Step 1: Write failing triage tests**

Append to `tests/cli_task_flow.rs`:

```rust
#[test]
fn triage_sets_risk_priority_runtime_and_review_reason() {
    let home = tempdir().unwrap();

    helm_agent_with_home(home.path())
        .args([
            "task",
            "create",
            "--id",
            "PM-20260511-006",
            "--title",
            "Classify task",
            "--project",
            "/repo",
        ])
        .assert()
        .success();

    helm_agent_with_home(home.path())
        .args([
            "task",
            "triage",
            "PM-20260511-006",
            "--risk",
            "medium",
            "--priority",
            "high",
            "--runtime",
            "claude",
            "--review-reason",
            "Touches auth flow",
        ])
        .assert()
        .success()
        .stdout(contains("Triaged PM-20260511-006"));

    let store = TaskStore::new(home.path().to_path_buf());
    let task = store.load_task("PM-20260511-006").unwrap();
    assert_eq!(task.status, TaskStatus::Triaged);
    assert_eq!(task.risk, RiskLevel::Medium);
    assert_eq!(task.priority, "high");
    assert_eq!(task.assignment.runtime, Some(AgentRuntime::Claude));
    assert_eq!(task.review.reason.as_deref(), Some("Touches auth flow"));
    assert_eq!(task.review.state, helm_agent::domain::ReviewState::Required);
}

#[test]
fn triage_requires_at_least_one_change() {
    let home = tempdir().unwrap();

    helm_agent_with_home(home.path())
        .args([
            "task",
            "create",
            "--id",
            "PM-20260511-007",
            "--title",
            "No-op triage",
            "--project",
            "/repo",
        ])
        .assert()
        .success();

    helm_agent_with_home(home.path())
        .args(["task", "triage", "PM-20260511-007"])
        .assert()
        .failure()
        .stderr(contains("triage requires at least one option"));
}
```

- [ ] **Step 2: Verify triage tests fail**

Run:

```bash
rtk cargo test --test cli_task_flow triage_
```

Expected: fail because `task triage` does not exist.

- [ ] **Step 3: Implement triage args**

Add `Triage(TriageArgs)` to `TaskSubcommand`.

Add:

```rust
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
```

- [ ] **Step 4: Implement triage handler**

Add match arm:

```rust
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
```

Add `RiskLevel::as_str()` to `src/domain.rs`:

```rust
impl RiskLevel {
    pub fn as_str(self) -> &'static str {
        match self {
            RiskLevel::Low => "low",
            RiskLevel::Medium => "medium",
            RiskLevel::High => "high",
        }
    }
}
```

- [ ] **Step 5: Verify triage tests pass**

Run:

```bash
rtk cargo test --test cli_task_flow triage_
```

Expected: pass.

- [ ] **Step 6: Verify policy interaction**

Run:

```bash
rtk cargo test --test cli_task_flow medium_risk_dispatch_requires_confirmation_before_tmux_launch
```

Expected: pass.

- [ ] **Step 7: Commit**

Run:

```bash
rtk git add src/domain.rs src/cli.rs tests/cli_task_flow.rs
rtk git commit -m "feat: add task triage"
```

---

## Task 5: Documentation and Final Verification

**Files:**
- Modify: `README.md`
- Modify: `docs/agent-integrations/main-agent.md`

- [ ] **Step 1: Update README**

Add Phase 2 commands to README:

```markdown
## Common Commands

```bash
cargo run --bin helm-agent -- task list
cargo run --bin helm-agent -- task list --review
cargo run --bin helm-agent -- task triage PM-20260511-001 --risk medium --priority high --runtime claude --review-reason "Touches auth flow"
cargo run --bin helm-agent -- task mark PM-20260511-001 --ready-for-review --message "Patch and tests ready"
```
```

- [ ] **Step 2: Update main-agent guide**

Replace the V1 note that `ready_for_review` and `blocked` are only event signals with Phase 2 guidance:

```markdown
Use `task mark` for real state transitions:

```bash
helm-agent task mark PM-20260511-101 --blocked --message "Waiting for API contract"
helm-agent task mark PM-20260511-101 --ready-for-review --message "Implementation and tests are ready"
```

Use `task list --review` before asking the human to review delegated work.
```

- [ ] **Step 3: Run all tests**

Run:

```bash
rtk cargo fmt --check
rtk cargo test
```

Expected: all pass.

- [ ] **Step 4: Run CLI smoke flow**

Run:

```bash
rtk env HELM_AGENT_HOME=/private/tmp/helm-agent-phase2-smoke cargo run --bin helm-agent -- task create --id PM-20260511-900 --title "Phase 2 smoke" --project .
rtk env HELM_AGENT_HOME=/private/tmp/helm-agent-phase2-smoke cargo run --bin helm-agent -- task triage PM-20260511-900 --risk medium --priority high --runtime claude --review-reason "Smoke review"
rtk env HELM_AGENT_HOME=/private/tmp/helm-agent-phase2-smoke cargo run --bin helm-agent -- task mark PM-20260511-900 --ready-for-review --message "Smoke ready"
rtk env HELM_AGENT_HOME=/private/tmp/helm-agent-phase2-smoke cargo run --bin helm-agent -- task list --review
```

Expected: final output contains `PM-20260511-900` and `ready_for_review`.

- [ ] **Step 5: Naming scan**

Run:

```bash
rtk rg -n "Agent Ops Center|AOC|agent-ops-center|helmagent|Helm Agent|HELMAGENT|AOC_|AOC-" README.md docs/agent-integrations src tests Cargo.toml
```

Expected: no matches.

- [ ] **Step 6: Commit docs**

Run:

```bash
rtk git add README.md docs/agent-integrations/main-agent.md
rtk git commit -m "docs: update review queue workflow"
```

## Final Review Requirements

After implementation and tests pass, launch multiple review agents in parallel:

- Spec compliance reviewer
- Code quality reviewer
- CLI workflow reviewer
- Docs and safety reviewer

All actionable findings must be fixed before merging.
