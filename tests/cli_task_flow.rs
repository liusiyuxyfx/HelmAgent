use assert_cmd::Command;
use helm_agent::domain::{AgentRuntime, RiskLevel, TaskStatus};
use helm_agent::store::TaskStore;
use predicates::prelude::PredicateBooleanExt;
use predicates::str::{contains, is_empty};
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use tempfile::tempdir;

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
            "review",
            "PM-20260509-002",
            "--request-changes",
            "Add regression test",
        ])
        .assert()
        .success()
        .stdout(contains("Requested changes for PM-20260509-002"));

    helm_agent_with_home(home.path())
        .args(["task", "status", "PM-20260509-002"])
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

    let store = TaskStore::new(home.path().to_path_buf());
    let mut task = store.load_task("PM-20260509-008").unwrap();
    task.risk = RiskLevel::Medium;
    store.save_task(&task).unwrap();

    helm_agent_with_home(home.path())
        .env("HELM_AGENT_TMUX_BIN", &tmux_bin)
        .args(["task", "dispatch", "PM-20260509-008", "--runtime", "claude"])
        .assert()
        .failure()
        .stderr(contains("requires --confirm"));

    assert!(!record_path.exists());
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
