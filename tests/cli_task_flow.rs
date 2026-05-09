use assert_cmd::Command;
use predicates::prelude::PredicateBooleanExt;
use predicates::str::{contains, is_empty};
use tempfile::tempdir;

fn helm_agent_with_home(home: &std::path::Path) -> Command {
    let mut cmd = Command::cargo_bin("helm-agent").unwrap();
    cmd.env("HELM_AGENT_HOME", home);
    cmd
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
