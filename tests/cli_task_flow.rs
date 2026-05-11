use assert_cmd::Command;
use helm_agent::domain::{AgentRuntime, ReviewState, RiskLevel, TaskStatus};
use helm_agent::store::TaskStore;
use predicates::prelude::PredicateBooleanExt;
use predicates::str::{contains, is_empty};
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use tempfile::tempdir;
use time::{Duration, OffsetDateTime};

fn helm_agent_with_home(home: &std::path::Path) -> Command {
    let mut cmd = Command::cargo_bin("helm-agent").unwrap();
    cmd.env("HELM_AGENT_HOME", home);
    cmd
}

fn fake_tmux_script(path: &Path, record_path: &Path) {
    let record_path = record_path.display().to_string().replace('\'', "'\\''");
    fs::write(
        path,
        format!(
            "#!/bin/sh\nfor arg in \"$@\"; do\n  printf '%s\\n' \"$arg\"\ndone > '{record_path}'\n"
        ),
    )
    .unwrap();
    let mut permissions = fs::metadata(path).unwrap().permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(path, permissions).unwrap();
}

#[test]
fn create_status_event_and_resume_task() {
    let home = tempdir().unwrap();

    helm_agent_with_home(home.path())
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

    helm_agent_with_home(home.path())
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

    helm_agent_with_home(home.path())
        .args(["task", "status", "PM-20260509-001"])
        .assert()
        .success()
        .stdout(contains("PM-20260509-001"))
        .stdout(contains("[inbox]"))
        .stdout(contains("Fix login redirect bug"))
        .stdout(contains("Found redirect handler"));

    helm_agent_with_home(home.path())
        .args(["task", "resume", "PM-20260509-001"])
        .assert()
        .success()
        .stdout(contains("No tmux session recorded"))
        .stdout(contains("No native resume command recorded"));
}

#[test]
fn duplicate_create_fails_without_overwriting_task() {
    let home = tempdir().unwrap();

    helm_agent_with_home(home.path())
        .args([
            "task",
            "create",
            "--id",
            "PM-20260509-002",
            "--title",
            "Original title",
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
            "PM-20260509-002",
            "--title",
            "Replacement title",
            "--project",
            "/other",
        ])
        .assert()
        .failure()
        .stdout(is_empty())
        .stderr(contains("task PM-20260509-002 already exists"));

    helm_agent_with_home(home.path())
        .args(["task", "status", "PM-20260509-002"])
        .assert()
        .success()
        .stdout(contains("Original title"))
        .stdout(contains("/repo"))
        .stdout(predicates::str::contains("Replacement title").not());
}

#[test]
fn review_accept_and_request_changes_update_status() {
    let home = tempdir().unwrap();

    helm_agent_with_home(home.path())
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

    helm_agent_with_home(home.path())
        .args([
            "task",
            "mark",
            "PM-20260509-002",
            "--ready-for-review",
            "--message",
            "Ready for review",
        ])
        .assert()
        .success();

    helm_agent_with_home(home.path())
        .args(["task", "review", "PM-20260509-002", "--accept"])
        .assert()
        .success()
        .stdout(contains("Accepted PM-20260509-002"));

    helm_agent_with_home(home.path())
        .args(["task", "status", "PM-20260509-002"])
        .assert()
        .success()
        .stdout(contains("[done]"));

    helm_agent_with_home(home.path())
        .args([
            "task",
            "create",
            "--id",
            "PM-20260509-012",
            "--title",
            "Review follow-up patch",
            "--project",
            "/repo",
        ])
        .assert()
        .success();
    helm_agent_with_home(home.path())
        .args([
            "task",
            "mark",
            "PM-20260509-012",
            "--ready-for-review",
            "--message",
            "Ready for review",
        ])
        .assert()
        .success();

    helm_agent_with_home(home.path())
        .args([
            "task",
            "review",
            "PM-20260509-012",
            "--request-changes",
            "Add regression test",
        ])
        .assert()
        .success()
        .stdout(contains("Requested changes for PM-20260509-012"));

    helm_agent_with_home(home.path())
        .args(["task", "status", "PM-20260509-012"])
        .assert()
        .success()
        .stdout(contains("[needs_changes]"))
        .stdout(contains("Add regression test"));
}

#[test]
fn dry_run_dispatch_records_recovery_commands() {
    let home = tempdir().unwrap();

    helm_agent_with_home(home.path())
        .args([
            "task",
            "create",
            "--id",
            "PM-20260509-004",
            "--title",
            "Dispatch task to child agent",
            "--project",
            "/repo/project",
        ])
        .assert()
        .success();

    helm_agent_with_home(home.path())
        .args([
            "task",
            "dispatch",
            "PM-20260509-004",
            "--runtime",
            "codex",
            "--dry-run",
        ])
        .assert()
        .success()
        .stdout(contains("Dry-run dispatch PM-20260509-004"))
        .stdout(contains(
            "Start: tmux new-session -d -s helm-agent-PM-20260509-004-codex -c /repo/project codex",
        ))
        .stdout(contains(
            "Attach: tmux attach -t helm-agent-PM-20260509-004-codex",
        ))
        .stdout(contains("Resume: codex resume <session-id> --all"));

    let store = TaskStore::new(home.path().to_path_buf());
    let task = store.load_task("PM-20260509-004").unwrap();
    assert_eq!(task.status, TaskStatus::Queued);
    assert_eq!(task.assignment.runtime, Some(AgentRuntime::Codex));
    assert_eq!(
        task.assignment.tmux_session.as_deref(),
        Some("helm-agent-PM-20260509-004-codex")
    );
    assert_eq!(task.progress.last_event, "Dry-run dispatch recorded");
    assert_eq!(
        task.progress.next_action,
        "Start or inspect child agent session"
    );
    assert_eq!(
        task.recovery.resume_command.as_deref(),
        Some("codex resume <session-id> --all")
    );
    let events = store.read_events("PM-20260509-004").unwrap();
    let event = events.last().unwrap();
    assert_eq!(event.event_type, "dispatch_planned");
    assert_eq!(
        event.message,
        "tmux new-session -d -s helm-agent-PM-20260509-004-codex -c /repo/project codex"
    );

    helm_agent_with_home(home.path())
        .args(["task", "resume", "PM-20260509-004"])
        .assert()
        .success()
        .stdout(contains(
            "Attach: tmux attach -t helm-agent-PM-20260509-004-codex",
        ))
        .stdout(contains("Resume: codex resume <session-id> --all"));
}

#[test]
fn dry_run_dispatch_omits_unavailable_native_resume_command() {
    let home = tempdir().unwrap();

    helm_agent_with_home(home.path())
        .args([
            "task",
            "create",
            "--id",
            "PM-20260509-005",
            "--title",
            "Dispatch task to OpenCode",
            "--project",
            "/repo/project",
        ])
        .assert()
        .success();

    helm_agent_with_home(home.path())
        .args([
            "task",
            "dispatch",
            "PM-20260509-005",
            "--runtime",
            "opencode",
            "--dry-run",
        ])
        .assert()
        .success()
        .stdout(contains("Dry-run dispatch PM-20260509-005"))
        .stdout(contains(
            "Start: tmux new-session -d -s helm-agent-PM-20260509-005-opencode -c /repo/project opencode",
        ))
        .stdout(contains(
            "Attach: tmux attach -t helm-agent-PM-20260509-005-opencode",
        ))
        .stdout(contains("Resume: No native resume command recorded"));

    let store = TaskStore::new(home.path().to_path_buf());
    let task = store.load_task("PM-20260509-005").unwrap();
    assert_eq!(task.status, TaskStatus::Queued);
    assert_eq!(task.assignment.runtime, Some(AgentRuntime::OpenCode));
    assert_eq!(
        task.assignment.tmux_session.as_deref(),
        Some("helm-agent-PM-20260509-005-opencode")
    );
    assert_eq!(task.recovery.resume_command, None);

    helm_agent_with_home(home.path())
        .args(["task", "resume", "PM-20260509-005"])
        .assert()
        .success()
        .stdout(contains(
            "Attach: tmux attach -t helm-agent-PM-20260509-005-opencode",
        ))
        .stdout(contains("Resume: No native resume command recorded"));
}

#[test]
fn non_dry_run_dispatch_invokes_tmux_and_records_running_state() {
    let home = tempdir().unwrap();
    let temp = tempdir().unwrap();
    let tmux_bin = temp.path().join("fake-tmux");
    let record_path = temp.path().join("tmux-args.txt");
    fake_tmux_script(&tmux_bin, &record_path);

    helm_agent_with_home(home.path())
        .args([
            "task",
            "create",
            "--id",
            "PM-20260509-006",
            "--title",
            "Dispatch task to real tmux",
            "--project",
            "/repo/my project",
        ])
        .assert()
        .success();

    helm_agent_with_home(home.path())
        .env("HELM_AGENT_TMUX_BIN", &tmux_bin)
        .args([
            "task",
            "dispatch",
            "PM-20260509-006",
            "--runtime",
            "claude",
        ])
        .assert()
        .success()
        .stdout(contains("Started PM-20260509-006"))
        .stdout(contains(
            "Start: tmux new-session -d -s helm-agent-PM-20260509-006-claude -c '/repo/my project' claude",
        ))
        .stdout(contains(
            "Attach: tmux attach -t helm-agent-PM-20260509-006-claude",
        ))
        .stdout(contains("Resume: claude --resume <session-id>"));

    assert_eq!(
        fs::read_to_string(record_path).unwrap(),
        "new-session\n-d\n-s\nhelm-agent-PM-20260509-006-claude\n-c\n/repo/my project\nclaude\n"
    );

    let store = TaskStore::new(home.path().to_path_buf());
    let task = store.load_task("PM-20260509-006").unwrap();
    assert_eq!(task.status, TaskStatus::Running);
    assert_eq!(task.assignment.runtime, Some(AgentRuntime::Claude));
    assert_eq!(
        task.assignment.tmux_session.as_deref(),
        Some("helm-agent-PM-20260509-006-claude")
    );
    assert_eq!(task.progress.last_event, "Dispatch started");
    assert_eq!(
        task.recovery.attach_command.as_deref(),
        Some("tmux attach -t helm-agent-PM-20260509-006-claude")
    );
    let events = store.read_events("PM-20260509-006").unwrap();
    let event = events.last().unwrap();
    assert_eq!(event.event_type, "dispatch_started");
    assert_eq!(
        event.message,
        "tmux new-session -d -s helm-agent-PM-20260509-006-claude -c '/repo/my project' claude"
    );

    helm_agent_with_home(home.path())
        .args(["task", "status", "PM-20260509-006"])
        .assert()
        .success()
        .stdout(contains("[running]"));
}

#[test]
fn codex_dispatch_requires_confirmation_before_tmux_launch() {
    let home = tempdir().unwrap();
    let temp = tempdir().unwrap();
    let tmux_bin = temp.path().join("fake-tmux");
    let record_path = temp.path().join("tmux-args.txt");
    fake_tmux_script(&tmux_bin, &record_path);

    helm_agent_with_home(home.path())
        .args([
            "task",
            "create",
            "--id",
            "PM-20260509-007",
            "--title",
            "Dispatch task to Codex",
            "--project",
            "/repo/project",
        ])
        .assert()
        .success();

    helm_agent_with_home(home.path())
        .env("HELM_AGENT_TMUX_BIN", &tmux_bin)
        .args(["task", "dispatch", "PM-20260509-007", "--runtime", "codex"])
        .assert()
        .failure()
        .stderr(contains("requires --confirm"));

    assert!(!record_path.exists());

    helm_agent_with_home(home.path())
        .args(["task", "status", "PM-20260509-007"])
        .assert()
        .success()
        .stdout(contains("[inbox]"));

    helm_agent_with_home(home.path())
        .env("HELM_AGENT_TMUX_BIN", &tmux_bin)
        .args([
            "task",
            "dispatch",
            "PM-20260509-007",
            "--runtime",
            "codex",
            "--confirm",
        ])
        .assert()
        .success()
        .stdout(contains("Started PM-20260509-007"));

    assert_eq!(
        fs::read_to_string(record_path).unwrap(),
        "new-session\n-d\n-s\nhelm-agent-PM-20260509-007-codex\n-c\n/repo/project\ncodex\n"
    );
}

#[test]
fn medium_risk_dispatch_requires_confirmation_before_tmux_launch() {
    let home = tempdir().unwrap();
    let temp = tempdir().unwrap();
    let tmux_bin = temp.path().join("fake-tmux");
    let record_path = temp.path().join("tmux-args.txt");
    fake_tmux_script(&tmux_bin, &record_path);

    helm_agent_with_home(home.path())
        .args([
            "task",
            "create",
            "--id",
            "PM-20260509-008",
            "--title",
            "Dispatch medium risk task",
            "--project",
            "/repo/project",
        ])
        .assert()
        .success();

    helm_agent_with_home(home.path())
        .args(["task", "triage", "PM-20260509-008", "--risk", "medium"])
        .assert()
        .success();

    helm_agent_with_home(home.path())
        .env("HELM_AGENT_TMUX_BIN", &tmux_bin)
        .args(["task", "dispatch", "PM-20260509-008", "--runtime", "claude"])
        .assert()
        .failure()
        .stderr(contains("requires --confirm"));

    assert!(!record_path.exists());
}

#[test]
fn dispatch_rejects_done_and_archived_tasks() {
    let home = tempdir().unwrap();

    helm_agent_with_home(home.path())
        .args([
            "task",
            "create",
            "--id",
            "PM-20260509-013",
            "--title",
            "Completed task",
            "--project",
            "/repo/project",
        ])
        .assert()
        .success();
    helm_agent_with_home(home.path())
        .args([
            "task",
            "mark",
            "PM-20260509-013",
            "--ready-for-review",
            "--message",
            "Ready",
        ])
        .assert()
        .success();
    helm_agent_with_home(home.path())
        .args(["task", "review", "PM-20260509-013", "--accept"])
        .assert()
        .success();

    helm_agent_with_home(home.path())
        .args([
            "task",
            "dispatch",
            "PM-20260509-013",
            "--runtime",
            "claude",
            "--dry-run",
        ])
        .assert()
        .failure()
        .stderr(contains("cannot dispatch PM-20260509-013 with status done"));

    helm_agent_with_home(home.path())
        .args([
            "task",
            "create",
            "--id",
            "PM-20260509-014",
            "--title",
            "Archived task",
            "--project",
            "/repo/project",
        ])
        .assert()
        .success();
    let store = TaskStore::new(home.path().to_path_buf());
    let mut archived = store.load_task("PM-20260509-014").unwrap();
    archived.status = TaskStatus::Archived;
    store.save_task(&archived).unwrap();

    helm_agent_with_home(home.path())
        .args([
            "task",
            "dispatch",
            "PM-20260509-014",
            "--runtime",
            "claude",
            "--dry-run",
        ])
        .assert()
        .failure()
        .stderr(contains(
            "cannot dispatch PM-20260509-014 with status archived",
        ));
}

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

    let store = TaskStore::new(home.path().to_path_buf());
    let mut older = store.load_task("PM-20260511-001").unwrap();
    older.updated_at = OffsetDateTime::UNIX_EPOCH;
    store.save_task(&older).unwrap();
    let mut newer = store.load_task("PM-20260511-002").unwrap();
    newer.updated_at = OffsetDateTime::UNIX_EPOCH + Duration::seconds(1);
    store.save_task(&newer).unwrap();

    let output = helm_agent_with_home(home.path())
        .args(["task", "list"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let stdout = String::from_utf8(output).unwrap();
    let lines: Vec<&str> = stdout.lines().collect();

    assert!(lines[0].starts_with("PM-20260511-002\t"), "{stdout}");
    assert!(lines[1].starts_with("PM-20260511-001\t"), "{stdout}");
    assert!(stdout.contains("Newer task"), "{stdout}");
    assert!(stdout.contains("Older task"), "{stdout}");
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
            "Queued task",
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

#[test]
fn review_queue_includes_triaged_tasks_that_require_human_attention() {
    let home = tempdir().unwrap();

    helm_agent_with_home(home.path())
        .args([
            "task",
            "create",
            "--id",
            "PM-20260511-009",
            "--title",
            "Risky triage",
            "--project",
            "/repo",
        ])
        .assert()
        .success();
    helm_agent_with_home(home.path())
        .args([
            "task",
            "triage",
            "PM-20260511-009",
            "--risk",
            "medium",
            "--review-reason",
            "Touches auth flow",
        ])
        .assert()
        .success();

    helm_agent_with_home(home.path())
        .args(["task", "list", "--review"])
        .assert()
        .success()
        .stdout(contains("PM-20260511-009"))
        .stdout(contains("triaged"))
        .stdout(contains("Touches auth flow"));

    helm_agent_with_home(home.path())
        .args(["task", "status", "PM-20260511-009"])
        .assert()
        .success()
        .stdout(contains("Review: Touches auth flow"));
}

#[test]
fn list_tasks_hides_archived_by_default_but_allows_explicit_archived_filter() {
    let home = tempdir().unwrap();

    helm_agent_with_home(home.path())
        .args([
            "task",
            "create",
            "--id",
            "PM-20260511-010",
            "--title",
            "Active task",
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
            "PM-20260511-011",
            "--title",
            "Archived task",
            "--project",
            "/repo",
        ])
        .assert()
        .success();

    let store = TaskStore::new(home.path().to_path_buf());
    let mut archived = store.load_task("PM-20260511-011").unwrap();
    archived.status = TaskStatus::Archived;
    store.save_task(&archived).unwrap();

    helm_agent_with_home(home.path())
        .args(["task", "list"])
        .assert()
        .success()
        .stdout(contains("PM-20260511-010"))
        .stdout(predicates::str::contains("PM-20260511-011").not());

    helm_agent_with_home(home.path())
        .args(["task", "list", "--status", "archived"])
        .assert()
        .success()
        .stdout(contains("PM-20260511-011"))
        .stdout(predicates::str::contains("PM-20260511-010").not());
}

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
fn triage_low_clears_review_requirement_only_without_review_reason() {
    let home = tempdir().unwrap();

    helm_agent_with_home(home.path())
        .args([
            "task",
            "create",
            "--id",
            "PM-20260511-015",
            "--title",
            "Downgrade risk",
            "--project",
            "/repo",
        ])
        .assert()
        .success();
    helm_agent_with_home(home.path())
        .args(["task", "triage", "PM-20260511-015", "--risk", "medium"])
        .assert()
        .success();
    helm_agent_with_home(home.path())
        .args(["task", "triage", "PM-20260511-015", "--risk", "low"])
        .assert()
        .success();

    let store = TaskStore::new(home.path().to_path_buf());
    let task = store.load_task("PM-20260511-015").unwrap();
    assert_eq!(task.risk, RiskLevel::Low);
    assert_eq!(task.review.state, ReviewState::NotRequired);

    helm_agent_with_home(home.path())
        .args(["task", "list", "--review"])
        .assert()
        .success()
        .stdout(predicates::str::contains("PM-20260511-015").not());

    helm_agent_with_home(home.path())
        .args([
            "task",
            "create",
            "--id",
            "PM-20260511-016",
            "--title",
            "Keep review reason",
            "--project",
            "/repo",
        ])
        .assert()
        .success();
    helm_agent_with_home(home.path())
        .args([
            "task",
            "triage",
            "PM-20260511-016",
            "--risk",
            "medium",
            "--review-reason",
            "Touches auth flow",
        ])
        .assert()
        .success();
    helm_agent_with_home(home.path())
        .args(["task", "triage", "PM-20260511-016", "--risk", "low"])
        .assert()
        .success();

    let task = store.load_task("PM-20260511-016").unwrap();
    assert_eq!(task.risk, RiskLevel::Low);
    assert_eq!(task.review.state, ReviewState::Required);
    assert_eq!(task.review.reason.as_deref(), Some("Touches auth flow"));
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

#[test]
fn review_requires_accept_or_request_changes() {
    let home = tempdir().unwrap();

    helm_agent_with_home(home.path())
        .args([
            "task",
            "create",
            "--id",
            "PM-20260509-003",
            "--title",
            "Review redirect patch",
            "--project",
            "/repo",
        ])
        .assert()
        .success();

    helm_agent_with_home(home.path())
        .args(["task", "review", "PM-20260509-003"])
        .assert()
        .failure()
        .stderr(contains(
            "review requires --accept or --request-changes <message>",
        ));
}

#[test]
fn review_rejects_tasks_that_are_not_ready_for_review() {
    let home = tempdir().unwrap();

    helm_agent_with_home(home.path())
        .args([
            "task",
            "create",
            "--id",
            "PM-20260509-015",
            "--title",
            "Not ready",
            "--project",
            "/repo",
        ])
        .assert()
        .success();

    helm_agent_with_home(home.path())
        .args(["task", "review", "PM-20260509-015", "--accept"])
        .assert()
        .failure()
        .stderr(contains("cannot review PM-20260509-015 with status inbox"));

    helm_agent_with_home(home.path())
        .args([
            "task",
            "triage",
            "PM-20260509-015",
            "--risk",
            "medium",
            "--review-reason",
            "Needs human attention before dispatch",
        ])
        .assert()
        .success();
    helm_agent_with_home(home.path())
        .args([
            "task",
            "review",
            "PM-20260509-015",
            "--request-changes",
            "No implementation exists yet",
        ])
        .assert()
        .failure()
        .stderr(contains(
            "cannot review PM-20260509-015 with status triaged",
        ));
}

#[test]
fn review_rejects_accept_and_request_changes_together() {
    let home = tempdir().unwrap();

    helm_agent_with_home(home.path())
        .args([
            "task",
            "review",
            "PM-20260509-003",
            "--accept",
            "--request-changes",
            "Add regression test",
        ])
        .assert()
        .failure()
        .stderr(contains("cannot be used with"));
}

#[test]
fn missing_task_commands_fail_with_context() {
    let home = tempdir().unwrap();

    helm_agent_with_home(home.path())
        .args(["task", "status", "PM-20260509-404"])
        .assert()
        .failure()
        .stderr(contains("read task"));

    helm_agent_with_home(home.path())
        .args([
            "task",
            "event",
            "PM-20260509-404",
            "--type",
            "progress",
            "--message",
            "No task",
        ])
        .assert()
        .failure()
        .stderr(contains("read task"));
}

#[test]
fn event_requires_message_argument() {
    let home = tempdir().unwrap();

    helm_agent_with_home(home.path())
        .args(["task", "event", "PM-20260509-001", "--type", "progress"])
        .assert()
        .failure()
        .stderr(contains("required"));
}

#[test]
fn event_rejects_invalid_type() {
    let home = tempdir().unwrap();

    helm_agent_with_home(home.path())
        .args([
            "task",
            "event",
            "PM-20260509-001",
            "--type",
            "unknown",
            "--message",
            "No task",
        ])
        .assert()
        .failure()
        .stderr(contains("invalid value"));
}
