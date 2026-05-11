use assert_cmd::Command;
use helm_agent::domain::{AgentRuntime, ReviewState, RiskLevel, TaskStatus};
use helm_agent::store::TaskStore;
use predicates::prelude::PredicateBooleanExt;
use predicates::str::{contains, is_empty};
use std::fs;
use std::os::unix::fs::{symlink, PermissionsExt};
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

fn fake_tmux_chmod_script(path: &Path, record_path: &Path, chmod_path: &Path) {
    let record_path = record_path.display().to_string().replace('\'', "'\\''");
    let chmod_path = chmod_path.display().to_string().replace('\'', "'\\''");
    fs::write(
        path,
        format!(
            "#!/bin/sh\nfor arg in \"$@\"; do\n  printf '%s\\n' \"$arg\"\ndone > '{record_path}'\nchmod 444 '{chmod_path}'\n"
        ),
    )
    .unwrap();
    let mut permissions = fs::metadata(path).unwrap().permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(path, permissions).unwrap();
}

fn fake_tmux_has_session_script(path: &Path, record_path: &Path, exit_code: i32) {
    let record_path = record_path.display().to_string().replace('\'', "'\\''");
    fs::write(
        path,
        format!(
            "#!/bin/sh\nfor arg in \"$@\"; do\n  printf '%s\\n' \"$arg\"\ndone > '{record_path}'\nexit {exit_code}\n"
        ),
    )
    .unwrap();
    let mut permissions = fs::metadata(path).unwrap().permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(path, permissions).unwrap();
}

#[test]
fn project_init_all_writes_agent_instruction_files_idempotently() {
    let home = tempdir().unwrap();
    let project = tempdir().unwrap();
    let template = home
        .path()
        .canonicalize()
        .unwrap()
        .join("main-agent-template.md");
    fs::write(&template, "# HelmAgent Main-Agent Operating Template\n").unwrap();

    for _ in 0..2 {
        helm_agent_with_home(home.path())
            .args([
                "project",
                "init",
                "--path",
                project.path().to_str().unwrap(),
                "--agent",
                "all",
            ])
            .assert()
            .success()
            .stdout(contains("Updated AGENTS.md"))
            .stdout(contains("Updated CLAUDE.md"));
    }

    let include = format!("@{}", template.display());
    let agents = fs::read_to_string(project.path().join("AGENTS.md")).unwrap();
    let claude = fs::read_to_string(project.path().join("CLAUDE.md")).unwrap();
    assert_eq!(agents.matches(&include).count(), 1);
    assert_eq!(claude.matches(&include).count(), 1);
}

#[test]
fn project_init_bootstraps_missing_installed_template() {
    let home = tempdir().unwrap();
    let project = tempdir().unwrap();
    let template = home
        .path()
        .canonicalize()
        .unwrap()
        .join("main-agent-template.md");
    assert!(!template.exists());

    helm_agent_with_home(home.path())
        .args([
            "project",
            "init",
            "--path",
            project.path().to_str().unwrap(),
            "--agent",
            "codex",
        ])
        .assert()
        .success()
        .stdout(contains("Updated AGENTS.md"));

    let include = format!("@{}", template.display());
    let agents = fs::read_to_string(project.path().join("AGENTS.md")).unwrap();
    let template_content = fs::read_to_string(template).unwrap();
    assert!(agents.contains(&include), "{agents}");
    assert!(
        template_content.contains("# HelmAgent Main-Agent Operating Template"),
        "{template_content}"
    );
}

#[test]
fn project_init_rejects_relative_helm_agent_home() {
    let project = tempdir().unwrap();

    let mut cmd = Command::cargo_bin("helm-agent").unwrap();
    cmd.env("HELM_AGENT_HOME", "relative-helm-agent-home")
        .args([
            "project",
            "init",
            "--path",
            project.path().to_str().unwrap(),
            "--agent",
            "codex",
        ])
        .assert()
        .failure()
        .stderr(contains("HELM_AGENT_HOME must be absolute"));
}

#[test]
fn board_serve_rejects_relative_helm_agent_home() {
    let mut cmd = Command::cargo_bin("helm-agent").unwrap();
    cmd.env("HELM_AGENT_HOME", "relative-helm-agent-home")
        .args(["board", "serve", "--host", "127.0.0.1", "--port", "0"])
        .assert()
        .failure()
        .stderr(contains("HELM_AGENT_HOME must be absolute"));
}

#[cfg(unix)]
#[test]
fn project_init_rejects_symlink_installed_template() {
    let home = tempdir().unwrap();
    let project = tempdir().unwrap();
    let outside = tempdir().unwrap();
    let outside_template = outside.path().join("main-agent-template.md");
    fs::write(&outside_template, "external template\n").unwrap();
    symlink(
        &outside_template,
        home.path().join("main-agent-template.md"),
    )
    .unwrap();

    helm_agent_with_home(home.path())
        .args([
            "project",
            "init",
            "--path",
            project.path().to_str().unwrap(),
            "--agent",
            "codex",
        ])
        .assert()
        .failure()
        .stderr(contains("refuse to use symlink main-agent template"));

    assert!(!project.path().join("AGENTS.md").exists());
}

#[test]
fn agent_prompt_prints_runtime_bootstrap_and_template() {
    let home = tempdir().unwrap();
    fs::write(
        home.path().join("main-agent-template.md"),
        "# HelmAgent Main-Agent Operating Template\nUse HelmAgent as source of truth.\n",
    )
    .unwrap();

    helm_agent_with_home(home.path())
        .args(["agent", "prompt", "--runtime", "codex"])
        .assert()
        .success()
        .stdout(contains("Runtime: codex"))
        .stdout(contains("helm-agent task board"))
        .stdout(contains("Use HelmAgent as source of truth"));
}

#[test]
fn board_html_renders_read_only_escaped_task_board() {
    let home = tempdir().unwrap();

    helm_agent_with_home(home.path())
        .args([
            "task",
            "create",
            "--id",
            "PM-20260511-HTML",
            "--title",
            "Render <board> safely",
            "--project",
            "/repo/project",
        ])
        .assert()
        .success();

    helm_agent_with_home(home.path())
        .args(["board", "html"])
        .assert()
        .success()
        .stdout(contains("<!doctype html>"))
        .stdout(contains("Render &lt;board&gt; safely"))
        .stdout(contains("No write actions are available"));
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
        .stdout(contains("No native resume command recorded"))
        .stdout(contains("tmux attach is the reliable recovery path"));
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
        .stdout(contains("Resume: codex resume <session-id> --all"))
        .stdout(contains("Brief: "));

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
    let brief_path = task.recovery.brief_path.as_ref().unwrap();
    assert!(brief_path.ends_with("sessions/PM-20260509-004/brief.md"));
    let brief = fs::read_to_string(brief_path).unwrap();
    assert!(brief.contains("# Child Agent Task Brief: PM-20260509-004"));
    assert!(brief.contains("codex resume <session-id> --all"));
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
        .stdout(contains("Resume: codex resume <session-id> --all"))
        .stdout(contains("Brief: "))
        .stdout(contains("tmux attach is the reliable recovery path"));

    helm_agent_with_home(home.path())
        .args(["task", "board"])
        .assert()
        .success()
        .stdout(contains("brief: "));
}

#[test]
fn task_brief_prints_markdown_without_writing() {
    let home = tempdir().unwrap();

    helm_agent_with_home(home.path())
        .args([
            "task",
            "create",
            "--id",
            "PM-20260511-B001",
            "--title",
            "Prepare child agent handoff",
            "--project",
            "/repo/project",
        ])
        .assert()
        .success();

    helm_agent_with_home(home.path())
        .args([
            "task",
            "event",
            "PM-20260511-B001",
            "--type",
            "progress",
            "--message",
            "Defined handoff details",
        ])
        .assert()
        .success();

    helm_agent_with_home(home.path())
        .args(["task", "brief", "PM-20260511-B001"])
        .assert()
        .success()
        .stdout(contains("# Child Agent Task Brief: PM-20260511-B001"))
        .stdout(contains("- Title: Prepare child agent handoff"))
        .stdout(contains("- progress: Defined handoff details"))
        .stdout(contains("## Child Agent Instructions"));

    let store = TaskStore::new(home.path().to_path_buf());
    let task = store.load_task("PM-20260511-B001").unwrap();
    assert!(task.recovery.brief_path.is_none());
    assert!(!store.brief_path("PM-20260511-B001").exists());
}

#[test]
fn task_brief_write_records_path_and_file() {
    let home = tempdir().unwrap();

    helm_agent_with_home(home.path())
        .args([
            "task",
            "create",
            "--id",
            "PM-20260511-B002",
            "--title",
            "Persist child brief",
            "--project",
            "/repo/project",
        ])
        .assert()
        .success();

    helm_agent_with_home(home.path())
        .args(["task", "brief", "PM-20260511-B002", "--write"])
        .assert()
        .success()
        .stdout(contains("Wrote brief: "));

    let store = TaskStore::new(home.path().to_path_buf());
    let task = store.load_task("PM-20260511-B002").unwrap();
    let brief_path = task.recovery.brief_path.as_ref().unwrap();
    assert!(brief_path.ends_with("sessions/PM-20260511-B002/brief.md"));
    assert!(brief_path.exists());
    let brief = fs::read_to_string(brief_path).unwrap();
    assert!(brief.contains("# Child Agent Task Brief: PM-20260511-B002"));
    assert!(brief.contains("brief_written: Brief written"));
    let events = store.read_events("PM-20260511-B002").unwrap();
    let event = events.last().unwrap();
    assert_eq!(event.event_type, "brief_written");
    assert!(event.message.contains("brief.md"));

    helm_agent_with_home(home.path())
        .args(["task", "status", "PM-20260511-B002"])
        .assert()
        .success()
        .stdout(contains("Brief: "));

    helm_agent_with_home(home.path())
        .args(["task", "resume", "PM-20260511-B002"])
        .assert()
        .success()
        .stdout(contains("Brief: "));

    helm_agent_with_home(home.path())
        .args(["task", "board"])
        .assert()
        .success()
        .stdout(contains("brief: "));
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
fn real_dispatch_does_not_launch_when_prelaunch_state_cannot_persist() {
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
            "PM-20260511-PRE",
            "--title",
            "Dispatch with blocked persistence",
            "--project",
            "/repo/project",
        ])
        .assert()
        .success();

    let store = TaskStore::new(home.path().to_path_buf());
    let task_path = store.task_path("PM-20260511-PRE");
    let mut permissions = fs::metadata(&task_path).unwrap().permissions();
    permissions.set_mode(0o444);
    fs::set_permissions(&task_path, permissions).unwrap();

    helm_agent_with_home(home.path())
        .env("HELM_AGENT_TMUX_BIN", &tmux_bin)
        .args(["task", "dispatch", "PM-20260511-PRE", "--runtime", "claude"])
        .assert()
        .failure()
        .stderr(contains("write task"));

    assert!(!record_path.exists());
}

#[test]
fn real_dispatch_prints_recovery_when_postlaunch_state_update_fails() {
    let home = tempdir().unwrap();
    let temp = tempdir().unwrap();
    let tmux_bin = temp.path().join("fake-tmux");
    let record_path = temp.path().join("tmux-args.txt");

    helm_agent_with_home(home.path())
        .args([
            "task",
            "create",
            "--id",
            "PM-20260511-POST",
            "--title",
            "Dispatch with postlaunch warning",
            "--project",
            "/repo/project",
        ])
        .assert()
        .success();

    let store = TaskStore::new(home.path().to_path_buf());
    let task_path = store.task_path("PM-20260511-POST");
    fake_tmux_chmod_script(&tmux_bin, &record_path, &task_path);

    helm_agent_with_home(home.path())
        .env("HELM_AGENT_TMUX_BIN", &tmux_bin)
        .args([
            "task",
            "dispatch",
            "PM-20260511-POST",
            "--runtime",
            "claude",
        ])
        .assert()
        .success()
        .stdout(contains("Started PM-20260511-POST"))
        .stdout(contains(
            "Attach: tmux attach -t helm-agent-PM-20260511-POST-claude",
        ))
        .stdout(contains("Brief: "))
        .stderr(contains(
            "Warning: Dispatch state update failed after tmux start",
        ));

    assert!(record_path.exists());
    let task = store.load_task("PM-20260511-POST").unwrap();
    assert_eq!(task.status, TaskStatus::Queued);
    assert_eq!(task.progress.last_event, "Dispatch prepared");
    assert!(task.recovery.brief_path.is_some());
}

#[test]
fn sync_running_task_blocks_when_tmux_session_is_missing() {
    let home = tempdir().unwrap();
    let temp = tempdir().unwrap();
    let tmux_bin = temp.path().join("fake-tmux");
    let record_path = temp.path().join("tmux-args.txt");
    fake_tmux_has_session_script(&tmux_bin, &record_path, 1);

    helm_agent_with_home(home.path())
        .args([
            "task",
            "create",
            "--id",
            "PM-20260511-S001",
            "--title",
            "Sync missing tmux session",
            "--project",
            "/repo/project",
        ])
        .assert()
        .success();

    let store = TaskStore::new(home.path().to_path_buf());
    let mut task = store.load_task("PM-20260511-S001").unwrap();
    task.status = TaskStatus::Running;
    task.assignment.tmux_session = Some("helm-agent-PM-20260511-S001-claude".to_string());
    store.save_task(&task).unwrap();

    helm_agent_with_home(home.path())
        .env("HELM_AGENT_TMUX_BIN", &tmux_bin)
        .args(["task", "sync", "PM-20260511-S001"])
        .assert()
        .success()
        .stdout(contains(
            "PM-20260511-S001 missing helm-agent-PM-20260511-S001-claude",
        ));

    assert_eq!(
        fs::read_to_string(record_path).unwrap(),
        "has-session\n-t\n=helm-agent-PM-20260511-S001-claude\n"
    );
    let task = store.load_task("PM-20260511-S001").unwrap();
    assert_eq!(task.status, TaskStatus::Blocked);
    assert_eq!(
        task.progress.blocker.as_deref(),
        Some("tmux session missing: helm-agent-PM-20260511-S001-claude")
    );
    let event = store
        .read_events("PM-20260511-S001")
        .unwrap()
        .pop()
        .unwrap();
    assert_eq!(event.event_type, "sync_missing");
}

#[test]
fn sync_queued_dry_run_keeps_missing_session_queued() {
    let home = tempdir().unwrap();
    let temp = tempdir().unwrap();
    let tmux_bin = temp.path().join("fake-tmux");
    let record_path = temp.path().join("tmux-args.txt");
    fake_tmux_has_session_script(&tmux_bin, &record_path, 1);

    helm_agent_with_home(home.path())
        .args([
            "task",
            "create",
            "--id",
            "PM-20260511-S002",
            "--title",
            "Sync dry-run task",
            "--project",
            "/repo/project",
        ])
        .assert()
        .success();
    helm_agent_with_home(home.path())
        .args([
            "task",
            "dispatch",
            "PM-20260511-S002",
            "--runtime",
            "claude",
            "--dry-run",
        ])
        .assert()
        .success();

    helm_agent_with_home(home.path())
        .env("HELM_AGENT_TMUX_BIN", &tmux_bin)
        .args(["task", "sync", "PM-20260511-S002"])
        .assert()
        .success()
        .stdout(contains(
            "PM-20260511-S002 missing helm-agent-PM-20260511-S002-claude",
        ));

    let store = TaskStore::new(home.path().to_path_buf());
    let task = store.load_task("PM-20260511-S002").unwrap();
    assert_eq!(task.status, TaskStatus::Queued);
    assert_eq!(task.progress.blocker, None);
    let events = store.read_events("PM-20260511-S002").unwrap();
    assert!(
        !events
            .iter()
            .any(|event| event.event_type == "sync_missing"),
        "{events:?}"
    );
}

#[test]
fn sync_reports_no_session_without_mutating_task() {
    let home = tempdir().unwrap();

    helm_agent_with_home(home.path())
        .args([
            "task",
            "create",
            "--id",
            "PM-20260511-S003",
            "--title",
            "No tmux session yet",
            "--project",
            "/repo/project",
        ])
        .assert()
        .success();

    helm_agent_with_home(home.path())
        .args(["task", "sync", "PM-20260511-S003"])
        .assert()
        .success()
        .stdout(contains("PM-20260511-S003 no_session"));

    let store = TaskStore::new(home.path().to_path_buf());
    let task = store.load_task("PM-20260511-S003").unwrap();
    assert_eq!(task.status, TaskStatus::Inbox);
    assert_eq!(store.read_events("PM-20260511-S003").unwrap().len(), 1);
}

#[test]
fn sync_all_marks_alive_session_tasks_running() {
    let home = tempdir().unwrap();
    let temp = tempdir().unwrap();
    let tmux_bin = temp.path().join("fake-tmux");
    let record_path = temp.path().join("tmux-args.txt");
    fake_tmux_has_session_script(&tmux_bin, &record_path, 0);

    helm_agent_with_home(home.path())
        .args([
            "task",
            "create",
            "--id",
            "PM-20260511-S004",
            "--title",
            "Alive tmux task",
            "--project",
            "/repo/project",
        ])
        .assert()
        .success();
    helm_agent_with_home(home.path())
        .args([
            "task",
            "dispatch",
            "PM-20260511-S004",
            "--runtime",
            "claude",
            "--dry-run",
        ])
        .assert()
        .success();
    helm_agent_with_home(home.path())
        .args([
            "task",
            "create",
            "--id",
            "PM-20260511-S005",
            "--title",
            "No session task",
            "--project",
            "/repo/project",
        ])
        .assert()
        .success();

    let output = helm_agent_with_home(home.path())
        .env("HELM_AGENT_TMUX_BIN", &tmux_bin)
        .args(["task", "sync", "--all"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let output = String::from_utf8(output).unwrap();

    assert!(
        output.contains("PM-20260511-S004 alive helm-agent-PM-20260511-S004-claude"),
        "{output}"
    );
    assert!(!output.contains("PM-20260511-S005"), "{output}");

    let store = TaskStore::new(home.path().to_path_buf());
    let task = store.load_task("PM-20260511-S004").unwrap();
    assert_eq!(task.status, TaskStatus::Running);
    assert_eq!(
        task.progress.last_event,
        "tmux session alive: helm-agent-PM-20260511-S004-claude"
    );
}

#[test]
fn sync_alive_preserves_non_tmux_blocker() {
    let home = tempdir().unwrap();
    let temp = tempdir().unwrap();
    let tmux_bin = temp.path().join("fake-tmux");
    let record_path = temp.path().join("tmux-args.txt");
    fake_tmux_has_session_script(&tmux_bin, &record_path, 0);

    helm_agent_with_home(home.path())
        .args([
            "task",
            "create",
            "--id",
            "PM-20260511-S006",
            "--title",
            "Blocked for human input",
            "--project",
            "/repo/project",
        ])
        .assert()
        .success();

    let store = TaskStore::new(home.path().to_path_buf());
    let mut task = store.load_task("PM-20260511-S006").unwrap();
    task.status = TaskStatus::Blocked;
    task.assignment.tmux_session = Some("helm-agent-PM-20260511-S006-claude".to_string());
    task.progress.blocker = Some("Waiting for product decision".to_string());
    store.save_task(&task).unwrap();

    helm_agent_with_home(home.path())
        .env("HELM_AGENT_TMUX_BIN", &tmux_bin)
        .args(["task", "sync", "PM-20260511-S006"])
        .assert()
        .success()
        .stdout(contains(
            "PM-20260511-S006 alive helm-agent-PM-20260511-S006-claude",
        ));

    let task = store.load_task("PM-20260511-S006").unwrap();
    assert_eq!(task.status, TaskStatus::Running);
    assert_eq!(
        task.progress.blocker.as_deref(),
        Some("Waiting for product decision")
    );
}

#[test]
fn sync_alive_does_not_reopen_ready_for_review_task() {
    let home = tempdir().unwrap();
    let temp = tempdir().unwrap();
    let tmux_bin = temp.path().join("fake-tmux");
    let record_path = temp.path().join("tmux-args.txt");
    fake_tmux_has_session_script(&tmux_bin, &record_path, 0);

    helm_agent_with_home(home.path())
        .args([
            "task",
            "create",
            "--id",
            "PM-20260511-S007",
            "--title",
            "Ready task with live tmux",
            "--project",
            "/repo/project",
        ])
        .assert()
        .success();

    let store = TaskStore::new(home.path().to_path_buf());
    let mut task = store.load_task("PM-20260511-S007").unwrap();
    task.status = TaskStatus::ReadyForReview;
    task.assignment.tmux_session = Some("helm-agent-PM-20260511-S007-claude".to_string());
    task.progress.last_event = "Ready for review".to_string();
    store.save_task(&task).unwrap();

    helm_agent_with_home(home.path())
        .env("HELM_AGENT_TMUX_BIN", &tmux_bin)
        .args(["task", "sync", "PM-20260511-S007"])
        .assert()
        .success()
        .stdout(contains(
            "PM-20260511-S007 alive helm-agent-PM-20260511-S007-claude",
        ));

    let task = store.load_task("PM-20260511-S007").unwrap();
    assert_eq!(task.status, TaskStatus::ReadyForReview);
    assert_eq!(task.progress.last_event, "Ready for review");
    let events = store.read_events("PM-20260511-S007").unwrap();
    assert!(
        !events.iter().any(|event| event.event_type == "sync_alive"),
        "{events:?}"
    );

    helm_agent_with_home(home.path())
        .args(["task", "review", "PM-20260511-S007", "--accept"])
        .assert()
        .success()
        .stdout(contains("Accepted PM-20260511-S007"));
}

#[test]
fn sync_help_and_missing_target_explain_target_modes() {
    let home = tempdir().unwrap();

    helm_agent_with_home(home.path())
        .args(["task", "sync", "--help"])
        .assert()
        .success()
        .stdout(contains("Sync recorded tmux session health"))
        .stdout(contains("Task id to sync"))
        .stdout(contains("Sync every active task"));

    helm_agent_with_home(home.path())
        .args(["task", "sync"])
        .assert()
        .failure()
        .stderr(contains("sync requires exactly one target: <id> or --all"));
}

#[test]
fn brief_help_explains_target_and_write_mode() {
    let home = tempdir().unwrap();

    helm_agent_with_home(home.path())
        .args(["task", "brief", "--help"])
        .assert()
        .success()
        .stdout(contains("Render or write a child-agent task brief"))
        .stdout(contains("Task id to render a child-agent brief for"))
        .stdout(contains("Write the brief to this task's session directory"));
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
fn dispatch_gate_rejects_handoff_and_paused_states_but_allows_needs_changes() {
    let home = tempdir().unwrap();
    let blocked_statuses = [
        (TaskStatus::ReadyForReview, "ready_for_review"),
        (TaskStatus::Reviewing, "reviewing"),
        (TaskStatus::Blocked, "blocked"),
        (TaskStatus::WaitingUser, "waiting_user"),
    ];
    let store = TaskStore::new(home.path().to_path_buf());

    for (index, (status, status_name)) in blocked_statuses.into_iter().enumerate() {
        let id = format!("PM-20260509-GATE-{index}");
        helm_agent_with_home(home.path())
            .args([
                "task",
                "create",
                "--id",
                id.as_str(),
                "--title",
                "Dispatch gate task",
                "--project",
                "/repo/project",
            ])
            .assert()
            .success();

        let mut task = store.load_task(&id).unwrap();
        task.status = status;
        store.save_task(&task).unwrap();

        helm_agent_with_home(home.path())
            .args([
                "task",
                "dispatch",
                id.as_str(),
                "--runtime",
                "claude",
                "--dry-run",
            ])
            .assert()
            .failure()
            .stderr(contains(format!(
                "cannot dispatch {id} with status {status_name}"
            )));
    }

    helm_agent_with_home(home.path())
        .args([
            "task",
            "create",
            "--id",
            "PM-20260509-GATE-OK",
            "--title",
            "Needs changes follow-up",
            "--project",
            "/repo/project",
        ])
        .assert()
        .success();
    let mut task = store.load_task("PM-20260509-GATE-OK").unwrap();
    task.status = TaskStatus::NeedsChanges;
    store.save_task(&task).unwrap();

    helm_agent_with_home(home.path())
        .args([
            "task",
            "dispatch",
            "PM-20260509-GATE-OK",
            "--runtime",
            "claude",
            "--dry-run",
        ])
        .assert()
        .success()
        .stdout(contains("Dry-run dispatch PM-20260509-GATE-OK"));
}

#[test]
fn board_groups_tasks_for_human_review() {
    let home = tempdir().unwrap();

    for (id, title) in [
        ("PM-20260511-B001", "Inbox task"),
        ("PM-20260511-B002", "Running task"),
        ("PM-20260511-B003", "Blocked task"),
        ("PM-20260511-B004", "Review task"),
        ("PM-20260511-B005", "Done task"),
        ("PM-20260511-B006", "Archived task"),
    ] {
        helm_agent_with_home(home.path())
            .args([
                "task",
                "create",
                "--id",
                id,
                "--title",
                title,
                "--project",
                "/repo",
            ])
            .assert()
            .success();
    }

    let store = TaskStore::new(home.path().to_path_buf());

    let mut running = store.load_task("PM-20260511-B002").unwrap();
    running.status = TaskStatus::Running;
    running.updated_at = OffsetDateTime::UNIX_EPOCH + Duration::seconds(3);
    running.progress.last_event = "Child agent is editing files".to_string();
    running.progress.next_action = "Check child progress".to_string();
    store.save_task(&running).unwrap();

    let mut blocked = store.load_task("PM-20260511-B003").unwrap();
    blocked.status = TaskStatus::Blocked;
    blocked.updated_at = OffsetDateTime::UNIX_EPOCH + Duration::seconds(5);
    blocked.progress.blocker = Some("Waiting for API contract".to_string());
    blocked.progress.last_event = "Blocked by API contract".to_string();
    blocked.progress.next_action = "Resolve blocker".to_string();
    store.save_task(&blocked).unwrap();

    let mut review = store.load_task("PM-20260511-B004").unwrap();
    review.status = TaskStatus::Triaged;
    review.risk = RiskLevel::Medium;
    review.updated_at = OffsetDateTime::UNIX_EPOCH + Duration::seconds(4);
    review.project.branch = Some("feature/auth".to_string());
    review.review.state = ReviewState::Required;
    review.review.reason = Some("Touches auth flow".to_string());
    review.progress.last_event = "Triaged risk=medium, review_reason=set".to_string();
    review.progress.next_action = "Dispatch or defer task".to_string();
    store.save_task(&review).unwrap();

    let mut done = store.load_task("PM-20260511-B005").unwrap();
    done.status = TaskStatus::Done;
    done.updated_at = OffsetDateTime::UNIX_EPOCH + Duration::seconds(1);
    done.progress.last_event = "Review accepted".to_string();
    done.progress.next_action = "Archive task when ready".to_string();
    store.save_task(&done).unwrap();

    let mut archived = store.load_task("PM-20260511-B006").unwrap();
    archived.status = TaskStatus::Archived;
    store.save_task(&archived).unwrap();

    let output = helm_agent_with_home(home.path())
        .args(["task", "board"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let board = String::from_utf8(output).unwrap();

    assert!(board.contains("Inbox"), "{board}");
    assert!(board.contains("PM-20260511-B001"), "{board}");
    assert!(board.contains("Running"), "{board}");
    assert!(board.contains("PM-20260511-B002"), "{board}");
    assert!(board.contains("Blocked"), "{board}");
    assert!(board.contains("PM-20260511-B003"), "{board}");
    assert!(
        board.contains("blocker: Waiting for API contract"),
        "{board}"
    );
    assert!(board.contains("Review"), "{board}");
    assert!(
        board.contains(
            "- PM-20260511-B004 [status=triaged review=required risk=medium runtime=- priority=normal] Review task"
        ),
        "{board}"
    );
    assert!(board.contains("project: /repo"), "{board}");
    assert!(board.contains("branch: feature/auth"), "{board}");
    assert!(board.contains("updated: 1970-01-01T00:00:04Z"), "{board}");
    assert!(board.contains("review: Touches auth flow"), "{board}");
    assert!(board.contains("Done"), "{board}");
    assert!(board.contains("PM-20260511-B005"), "{board}");
    assert!(!board.contains("PM-20260511-B006"), "{board}");

    let blocked_index = board.find("Blocked").unwrap();
    let review_index = board.find("Review").unwrap();
    let running_index = board.find("Running").unwrap();
    let inbox_index = board.find("Inbox").unwrap();
    assert!(blocked_index < review_index, "{board}");
    assert!(review_index < running_index, "{board}");
    assert!(running_index < inbox_index, "{board}");
}

#[test]
fn board_includes_recovery_context_after_dispatch_preview() {
    let home = tempdir().unwrap();

    helm_agent_with_home(home.path())
        .args([
            "task",
            "create",
            "--id",
            "PM-20260511-B007",
            "--title",
            "Recover delegated task",
            "--project",
            "/repo/project",
        ])
        .assert()
        .success();
    helm_agent_with_home(home.path())
        .args([
            "task",
            "dispatch",
            "PM-20260511-B007",
            "--runtime",
            "claude",
            "--dry-run",
        ])
        .assert()
        .success();

    helm_agent_with_home(home.path())
        .args(["task", "board"])
        .assert()
        .success()
        .stdout(contains("Queued"))
        .stdout(contains("PM-20260511-B007"))
        .stdout(contains(
            "attach: tmux attach -t helm-agent-PM-20260511-B007-claude",
        ))
        .stdout(contains("resume: claude --resume <session-id>"));
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

#[test]
fn main_agent_template_contains_required_operating_commands() {
    let template = fs::read_to_string("docs/agent-integrations/main-agent-template.md").unwrap();

    for required in [
        "helm-agent task board",
        "helm-agent task create",
        "helm-agent task triage",
        "helm-agent task sync",
        "helm-agent task brief",
        "helm-agent task dispatch --dry-run",
        "helm-agent task mark",
        "task review --accept",
        "--confirm",
        "Do not claim code-changing work is complete",
    ] {
        assert!(
            template.contains(required),
            "missing `{required}` from template:\n{template}"
        );
    }
}

#[test]
fn docs_cover_tmux_sync_commands() {
    let readme = fs::read_to_string("README.md").unwrap();
    let guide = fs::read_to_string("docs/agent-integrations/main-agent.md").unwrap();
    let combined = format!("{readme}\n{guide}");

    for required in [
        "helm-agent task sync PM-20260511-001",
        "helm-agent task sync --all",
        "before reporting delegated session health",
    ] {
        assert!(
            combined.contains(required),
            "missing `{required}` from docs:\n{combined}"
        );
    }
}

#[test]
fn docs_cover_task_brief_commands() {
    let readme = fs::read_to_string("README.md").unwrap();
    let guide = fs::read_to_string("docs/agent-integrations/main-agent.md").unwrap();
    let combined = format!("{readme}\n{guide}");

    for required in [
        "helm-agent task brief PM-20260511-001",
        "helm-agent task brief PM-20260511-001 --write",
        "child-agent brief",
    ] {
        assert!(
            combined.contains(required),
            "missing `{required}` from docs:\n{combined}"
        );
    }
}

#[test]
fn main_agent_guide_uses_consistent_common_task_id_for_brief_flow() {
    let guide = fs::read_to_string("docs/agent-integrations/main-agent.md").unwrap();

    for required in [
        "helm-agent task create --id PM-20260509-101",
        "helm-agent task triage PM-20260509-101",
        "helm-agent task brief PM-20260509-101",
        "helm-agent task brief PM-20260509-101 --write",
    ] {
        assert!(guide.contains(required), "missing `{required}`:\n{guide}");
    }
}
