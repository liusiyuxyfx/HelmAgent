use assert_cmd::Command;
use predicates::str::contains;
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
        .stdout(contains("Fix login redirect bug"))
        .stdout(contains("Found redirect handler"));

    helm_agent_with_home(home.path())
        .args(["task", "resume", "PM-20260509-001"])
        .assert()
        .success()
        .stdout(contains("No tmux session recorded"))
        .stdout(contains("No native resume command recorded"));
}
