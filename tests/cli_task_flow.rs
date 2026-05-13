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

const RUNTIME_COMMAND_ENV_VARS: &[&str] = &[
    "HELM_AGENT_CLAUDE_COMMAND",
    "HELM_AGENT_CLAUDE_RESUME_COMMAND",
    "HELM_AGENT_CODEX_COMMAND",
    "HELM_AGENT_CODEX_RESUME_COMMAND",
    "HELM_AGENT_OPENCODE_COMMAND",
    "HELM_AGENT_OPENCODE_RESUME_COMMAND",
];

fn helm_agent_with_home(home: &std::path::Path) -> Command {
    let mut cmd = Command::cargo_bin("helm-agent").unwrap();
    cmd.env("HELM_AGENT_HOME", home);
    for var in RUNTIME_COMMAND_ENV_VARS {
        cmd.env_remove(var);
    }
    cmd
}

fn shell_quote_for_test(value: &str) -> String {
    if value.is_empty() {
        return "''".to_string();
    }
    if value
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '/' | '.' | '_' | '-' | '=' | ':'))
    {
        value.to_string()
    } else {
        format!("'{}'", value.replace('\'', "'\\''"))
    }
}

fn helm_agent_home_arg_for_test(home: &std::path::Path) -> String {
    format!("HELM_AGENT_HOME={}", home.canonicalize().unwrap().display())
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

fn failing_tmux_script(path: &Path) {
    fs::write(
        path,
        "#!/bin/sh\nprintf '%s\\n' 'tmux failed before launch' >&2\nexit 7\n",
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

fn fake_tmux_append_script(path: &Path, record_path: &Path, fail_send_keys: bool) {
    let record_path = record_path.display().to_string().replace('\'', "'\\''");
    let fail_send_keys = if fail_send_keys { "true" } else { "false" };
    fs::write(
        path,
        format!(
            "#!/bin/sh\nprintf '%s\\n' CALL >> '{record_path}'\nfor arg in \"$@\"; do\n  printf '%s\\n' \"$arg\"\ndone >> '{record_path}'\nif [ \"$1\" = send-keys ] && {fail_send_keys}; then\n  printf '%s\\n' 'send-keys failed' >&2\n  exit 8\nfi\n"
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

fn fake_tmux_doctor_script(path: &Path, record_path: &Path) {
    let record_path = record_path.display().to_string().replace('\'', "'\\''");
    fs::write(
        path,
        format!(
            "#!/bin/sh\nprintf '%s\\n' CALL >> '{record_path}'\nfor arg in \"$@\"; do\n  printf '%s\\n' \"$arg\"\ndone >> '{record_path}'\nif [ \"$1\" = '-V' ]; then\n  printf '%s\\n' 'tmux 3.6a'\nfi\n"
        ),
    )
    .unwrap();
    let mut permissions = fs::metadata(path).unwrap().permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(path, permissions).unwrap();
}

fn fake_acp_agent_script(path: &Path) {
    fs::write(
        path,
        r#"#!/usr/bin/env python3
import json
import os
import re
import subprocess
import sys

record_path = os.environ.get("HELM_ACP_RECORD")
stderr_bytes = int(os.environ.get("HELM_ACP_STDERR_BYTES", "0") or "0")
if stderr_bytes:
    sys.stderr.write("x" * stderr_bytes)
    sys.stderr.flush()

def walk_strings(value):
    if isinstance(value, str):
        yield value
    elif isinstance(value, dict):
        for child in value.values():
            yield from walk_strings(child)
    elif isinstance(value, list):
        for child in value:
            yield from walk_strings(child)

for line in sys.stdin:
    request = json.loads(line)
    method = request.get("method")
    if method == "initialize":
        result = {
            "protocolVersion": request.get("params", {}).get("protocolVersion", 1),
            "agentCapabilities": {},
            "agentInfo": {"name": "fake-acp-agent", "version": "test"},
        }
    elif method == "session/new":
        result = {"sessionId": "fake-acp-session-1"}
    elif method == "session/prompt":
        payload = json.dumps(request)
        if record_path:
            with open(record_path, "w", encoding="utf-8") as record:
                json.dump(request, record)
        mutate_bin = os.environ.get("HELM_ACP_MUTATE_BIN")
        mutate_task = os.environ.get("HELM_ACP_MUTATE_TASK")
        if mutate_bin and mutate_task:
            mutate_message = os.environ.get("HELM_ACP_MUTATE_MESSAGE", "child agent progress")
            subprocess.run(
                [mutate_bin, "task", "event", mutate_task, "--type", "progress", "--message", mutate_message],
                check=True,
                stdout=subprocess.DEVNULL,
                stderr=subprocess.DEVNULL,
            )
        if os.environ.get("HELM_ACP_CHMOD_BRIEF") == "1":
            text = "\n".join(walk_strings(request))
            matches = re.findall(r"Brief: ([^\n\r]+)", text)
            for candidate in matches:
                try:
                    path = candidate.strip()
                    if path.endswith("/brief.md"):
                        os.chmod(path, 0o444)
                except OSError:
                    pass
        chmod_task = os.environ.get("HELM_ACP_CHMOD_TASK")
        if chmod_task:
            task_path = os.path.join(os.environ["HELM_AGENT_HOME"], "tasks", chmod_task.split("-")[1][0:4], f"{chmod_task}.yaml")
            try:
                os.chmod(task_path, 0o444)
            except OSError:
                pass
        result = {"stopReason": os.environ.get("HELM_ACP_STOP_REASON", "end_turn")}
    else:
        print(json.dumps({
            "jsonrpc": "2.0",
            "id": request.get("id"),
            "error": {"code": -32601, "message": f"unknown method: {method}"},
        }), flush=True)
        continue

    print(json.dumps({
        "jsonrpc": "2.0",
        "id": request.get("id"),
        "result": result,
    }), flush=True)
    if method == "session/prompt" and os.environ.get("HELM_ACP_EXIT_AFTER_PROMPT") == "1":
        sys.exit(0)
"#,
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
fn acp_agent_add_list_and_remove_round_trip() {
    let home = tempdir().unwrap();

    helm_agent_with_home(home.path())
        .args([
            "acp",
            "agent",
            "add",
            "local-echo",
            "--command",
            "/bin/echo",
            "--arg",
            "hello",
            "--env",
            "HELM_TEST=1",
        ])
        .assert()
        .success()
        .stdout(contains("Added ACP agent local-echo"));

    helm_agent_with_home(home.path())
        .args(["acp", "agent", "list"])
        .assert()
        .success()
        .stdout(contains("local-echo"))
        .stdout(contains("/bin/echo"))
        .stdout(contains("hello"));

    let config = fs::read_to_string(home.path().join("acp").join("agents.yaml")).unwrap();
    assert!(config.contains("local-echo"), "{config}");
    assert!(config.contains("HELM_TEST"), "{config}");

    helm_agent_with_home(home.path())
        .args(["acp", "agent", "remove", "local-echo"])
        .assert()
        .success()
        .stdout(contains("Removed ACP agent local-echo"));

    helm_agent_with_home(home.path())
        .args(["acp", "agent", "list"])
        .assert()
        .success()
        .stdout(contains("No ACP agents"));
}

#[test]
fn acp_agent_add_rejects_invalid_env_pair() {
    let home = tempdir().unwrap();

    helm_agent_with_home(home.path())
        .args([
            "acp",
            "agent",
            "add",
            "broken",
            "--command",
            "/bin/echo",
            "--env",
            "NO_EQUALS",
        ])
        .assert()
        .failure()
        .stderr(contains("env must be KEY=VALUE"));
}

#[test]
fn acp_agent_check_verifies_configured_agent_handshake() {
    let home = tempdir().unwrap();
    let fake_agent = home.path().join("fake-acp-agent.py");
    let prompt_record = home.path().join("check-prompt.json");
    fake_acp_agent_script(&fake_agent);

    helm_agent_with_home(home.path())
        .args([
            "acp",
            "agent",
            "add",
            "fake",
            "--command",
            fake_agent.to_str().unwrap(),
            "--env",
            &format!("HELM_ACP_RECORD={}", prompt_record.display()),
        ])
        .assert()
        .success();

    helm_agent_with_home(home.path())
        .args(["acp", "agent", "check", "fake"])
        .assert()
        .success()
        .stdout(contains("ACP agent fake ok"))
        .stdout(contains("Session: fake-acp-session-1"))
        .stdout(contains("Stop:"));

    let prompt = fs::read_to_string(prompt_record).unwrap();
    assert!(prompt.contains("HelmAgent ACP check"), "{prompt}");
    let acp_dir = home.path().join("acp");
    let leftovers = fs::read_dir(&acp_dir)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().to_string())
        .collect::<Vec<_>>();
    assert_eq!(leftovers, vec!["agents.yaml"], "{leftovers:?}");
}

#[test]
fn acp_agent_check_reports_handshake_failure() {
    let home = tempdir().unwrap();
    let failing_agent = home.path().join("failing-acp-agent.sh");
    fs::write(&failing_agent, "#!/bin/sh\nexit 9\n").unwrap();
    let mut permissions = fs::metadata(&failing_agent).unwrap().permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&failing_agent, permissions).unwrap();

    helm_agent_with_home(home.path())
        .args([
            "acp",
            "agent",
            "add",
            "failing",
            "--command",
            failing_agent.to_str().unwrap(),
        ])
        .assert()
        .success();

    helm_agent_with_home(home.path())
        .args(["acp", "agent", "check", "failing"])
        .assert()
        .failure()
        .stderr(contains("ACP agent exited before handoff"));
}

#[test]
fn acp_agent_check_rejects_non_success_stop_reason() {
    let home = tempdir().unwrap();
    let fake_agent = home.path().join("fake-acp-agent.py");
    fake_acp_agent_script(&fake_agent);

    helm_agent_with_home(home.path())
        .args([
            "acp",
            "agent",
            "add",
            "fake",
            "--command",
            fake_agent.to_str().unwrap(),
            "--env",
            "HELM_ACP_STOP_REASON=max_tokens",
            "--env",
            "HELM_ACP_EXIT_AFTER_PROMPT=1",
        ])
        .assert()
        .success();

    helm_agent_with_home(home.path())
        .args(["acp", "agent", "check", "fake"])
        .assert()
        .failure()
        .stderr(contains(
            "ACP agent fake check failed: stop reason MaxTokens",
        ));
}

#[test]
fn acp_dispatch_requires_agent_name() {
    let home = tempdir().unwrap();

    helm_agent_with_home(home.path())
        .args([
            "task",
            "create",
            "--id",
            "PM-20260511-ACP-REQ",
            "--title",
            "ACP requires agent",
            "--project",
            "/repo/project",
        ])
        .assert()
        .success();

    helm_agent_with_home(home.path())
        .args([
            "task",
            "dispatch",
            "PM-20260511-ACP-REQ",
            "--runtime",
            "acp",
            "--dry-run",
        ])
        .assert()
        .failure()
        .stderr(contains("ACP dispatch requires --agent <name>"));
}

#[test]
fn acp_dry_run_dispatch_records_configured_agent_without_tmux() {
    let home = tempdir().unwrap();

    helm_agent_with_home(home.path())
        .args([
            "acp",
            "agent",
            "add",
            "local-fake",
            "--command",
            "/bin/echo",
            "--arg",
            "ready",
        ])
        .assert()
        .success();

    helm_agent_with_home(home.path())
        .args([
            "task",
            "create",
            "--id",
            "PM-20260511-ACP-DRY",
            "--title",
            "ACP dry run",
            "--project",
            "/repo/project",
        ])
        .assert()
        .success();

    helm_agent_with_home(home.path())
        .args([
            "task",
            "dispatch",
            "PM-20260511-ACP-DRY",
            "--runtime",
            "acp",
            "--agent",
            "local-fake",
            "--dry-run",
        ])
        .assert()
        .success()
        .stdout(contains("Dry-run ACP dispatch PM-20260511-ACP-DRY"))
        .stdout(contains("Agent: local-fake"))
        .stdout(contains("Command: /bin/echo ready"))
        .stdout(contains("Brief: "));

    let store = TaskStore::new(home.path().to_path_buf());
    let task = store.load_task("PM-20260511-ACP-DRY").unwrap();
    assert_eq!(task.status, TaskStatus::Queued);
    assert_eq!(task.assignment.runtime, Some(AgentRuntime::Acp));
    assert_eq!(task.assignment.tmux_session, None);
    assert_eq!(task.assignment.acp_session_id, None);
    assert_eq!(
        task.recovery.resume_command.as_deref(),
        Some("helm-agent task dispatch PM-20260511-ACP-DRY --runtime acp --agent local-fake --confirm")
    );
    let events = store.read_events("PM-20260511-ACP-DRY").unwrap();
    let event = events.last().unwrap();
    assert_eq!(event.event_type, "acp_dispatch_planned");
    assert_eq!(event.message, "local-fake: /bin/echo ready");
}

#[test]
fn acp_real_dispatch_sends_brief_to_fake_agent_and_records_session() {
    let home = tempdir().unwrap();
    let project = tempdir().unwrap();
    let fake_agent = home.path().join("fake-acp-agent.py");
    let prompt_record = home.path().join("prompt.json");
    fake_acp_agent_script(&fake_agent);

    helm_agent_with_home(home.path())
        .args([
            "acp",
            "agent",
            "add",
            "fake",
            "--command",
            fake_agent.to_str().unwrap(),
            "--env",
            &format!("HELM_ACP_RECORD={}", prompt_record.display()),
        ])
        .assert()
        .success();

    helm_agent_with_home(home.path())
        .args([
            "task",
            "create",
            "--id",
            "PM-20260511-ACP-REAL",
            "--title",
            "ACP real dispatch",
            "--project",
            project.path().to_str().unwrap(),
        ])
        .assert()
        .success();

    helm_agent_with_home(home.path())
        .args([
            "task",
            "dispatch",
            "PM-20260511-ACP-REAL",
            "--runtime",
            "acp",
            "--agent",
            "fake",
            "--confirm",
        ])
        .assert()
        .success()
        .stdout(contains("Completed ACP PM-20260511-ACP-REAL"))
        .stdout(contains("Session: fake-acp-session-1"))
        .stdout(contains("Brief: "));

    let store = TaskStore::new(home.path().to_path_buf());
    let task = store.load_task("PM-20260511-ACP-REAL").unwrap();
    assert_eq!(task.status, TaskStatus::ReadyForReview);
    assert_eq!(task.review.state, ReviewState::Required);
    assert_eq!(task.assignment.runtime, Some(AgentRuntime::Acp));
    assert_eq!(
        task.assignment.acp_session_id.as_deref(),
        Some("fake-acp-session-1")
    );
    assert!(task.assignment.tmux_session.is_none());

    let prompt = fs::read_to_string(prompt_record).unwrap();
    assert!(prompt.contains("session/prompt"), "{prompt}");
    assert!(
        prompt.contains("Child Agent Task Brief: PM-20260511-ACP-REAL"),
        "{prompt}"
    );

    let events = store.read_events("PM-20260511-ACP-REAL").unwrap();
    assert!(events
        .iter()
        .any(|event| event.event_type == "acp_dispatch_prepared"));
    assert!(events
        .iter()
        .any(|event| event.event_type == "acp_dispatch_completed"));
}

#[test]
fn acp_real_dispatch_failure_marks_needs_changes_and_writes_final_brief() {
    let home = tempdir().unwrap();
    let project = tempdir().unwrap();
    let failing_agent = home.path().join("failing-acp-agent.sh");
    fs::write(
        &failing_agent,
        "#!/bin/sh\nprintf '%s\\n' 'fake ACP failure' >&2\nexit 9\n",
    )
    .unwrap();
    let mut permissions = fs::metadata(&failing_agent).unwrap().permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&failing_agent, permissions).unwrap();

    helm_agent_with_home(home.path())
        .args([
            "acp",
            "agent",
            "add",
            "failing",
            "--command",
            failing_agent.to_str().unwrap(),
        ])
        .assert()
        .success();

    helm_agent_with_home(home.path())
        .args([
            "task",
            "create",
            "--id",
            "PM-20260511-ACP-FAIL",
            "--title",
            "ACP failing dispatch",
            "--project",
            project.path().to_str().unwrap(),
        ])
        .assert()
        .success();

    helm_agent_with_home(home.path())
        .args([
            "task",
            "dispatch",
            "PM-20260511-ACP-FAIL",
            "--runtime",
            "acp",
            "--agent",
            "failing",
            "--confirm",
        ])
        .assert()
        .failure()
        .stderr(contains("ACP agent exited before handoff"));

    let store = TaskStore::new(home.path().to_path_buf());
    let task = store.load_task("PM-20260511-ACP-FAIL").unwrap();
    assert_eq!(task.status, TaskStatus::NeedsChanges);
    assert_eq!(task.review.state, ReviewState::ChangesRequested);
    assert!(task.progress.last_event.contains("ACP dispatch failed"));
    let events = store.read_events("PM-20260511-ACP-FAIL").unwrap();
    assert!(events
        .iter()
        .any(|event| event.event_type == "acp_dispatch_failed"));
    let brief = fs::read_to_string(task.recovery.brief_path.as_ref().unwrap()).unwrap();
    assert!(brief.contains("Status: needs_changes"), "{brief}");
    assert!(brief.contains("acp_dispatch_failed"), "{brief}");
}

#[test]
fn acp_dispatch_without_confirm_does_not_mutate_task() {
    let home = tempdir().unwrap();
    let project = tempdir().unwrap();

    helm_agent_with_home(home.path())
        .args([
            "acp",
            "agent",
            "add",
            "local-fake",
            "--command",
            "/bin/echo",
        ])
        .assert()
        .success();

    helm_agent_with_home(home.path())
        .args([
            "task",
            "create",
            "--id",
            "PM-20260511-ACP-NOCONFIRM",
            "--title",
            "ACP no confirm",
            "--project",
            project.path().to_str().unwrap(),
        ])
        .assert()
        .success();

    helm_agent_with_home(home.path())
        .args([
            "task",
            "dispatch",
            "PM-20260511-ACP-NOCONFIRM",
            "--runtime",
            "acp",
            "--agent",
            "local-fake",
        ])
        .assert()
        .failure()
        .stderr(contains("requires --confirm"));

    let store = TaskStore::new(home.path().to_path_buf());
    let task = store.load_task("PM-20260511-ACP-NOCONFIRM").unwrap();
    assert_eq!(task.status, TaskStatus::Inbox);
    assert_eq!(task.assignment.runtime, None);
    assert_eq!(
        store
            .read_events("PM-20260511-ACP-NOCONFIRM")
            .unwrap()
            .len(),
        1
    );
}

#[test]
fn acp_failed_dispatch_can_be_retried_after_agent_config_is_fixed() {
    let home = tempdir().unwrap();
    let project = tempdir().unwrap();
    let failing_agent = home.path().join("failing-acp-agent.sh");
    let fake_agent = home.path().join("fake-acp-agent.py");
    let prompt_record = home.path().join("retry-prompt.json");
    fs::write(&failing_agent, "#!/bin/sh\nexit 9\n").unwrap();
    let mut permissions = fs::metadata(&failing_agent).unwrap().permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&failing_agent, permissions).unwrap();
    fake_acp_agent_script(&fake_agent);

    helm_agent_with_home(home.path())
        .args([
            "acp",
            "agent",
            "add",
            "retry",
            "--command",
            failing_agent.to_str().unwrap(),
        ])
        .assert()
        .success();
    helm_agent_with_home(home.path())
        .args([
            "task",
            "create",
            "--id",
            "PM-20260511-ACP-RETRY",
            "--title",
            "ACP retry",
            "--project",
            project.path().to_str().unwrap(),
        ])
        .assert()
        .success();
    helm_agent_with_home(home.path())
        .args([
            "task",
            "dispatch",
            "PM-20260511-ACP-RETRY",
            "--runtime",
            "acp",
            "--agent",
            "retry",
            "--confirm",
        ])
        .assert()
        .failure();

    helm_agent_with_home(home.path())
        .args([
            "acp",
            "agent",
            "add",
            "retry",
            "--command",
            fake_agent.to_str().unwrap(),
            "--env",
            &format!("HELM_ACP_RECORD={}", prompt_record.display()),
        ])
        .assert()
        .success();
    helm_agent_with_home(home.path())
        .args([
            "task",
            "dispatch",
            "PM-20260511-ACP-RETRY",
            "--runtime",
            "acp",
            "--agent",
            "retry",
            "--confirm",
        ])
        .assert()
        .success()
        .stdout(contains("Completed ACP PM-20260511-ACP-RETRY"));

    let store = TaskStore::new(home.path().to_path_buf());
    let task = store.load_task("PM-20260511-ACP-RETRY").unwrap();
    assert_eq!(task.status, TaskStatus::ReadyForReview);
    assert_eq!(
        task.assignment.acp_session_id.as_deref(),
        Some("fake-acp-session-1")
    );
}

#[test]
fn acp_real_dispatch_does_not_block_on_noisy_agent_stderr() {
    let home = tempdir().unwrap();
    let project = tempdir().unwrap();
    let fake_agent = home.path().join("noisy-acp-agent.py");
    fake_acp_agent_script(&fake_agent);

    helm_agent_with_home(home.path())
        .args([
            "acp",
            "agent",
            "add",
            "noisy",
            "--command",
            fake_agent.to_str().unwrap(),
            "--env",
            "HELM_ACP_STDERR_BYTES=1048576",
        ])
        .assert()
        .success();

    helm_agent_with_home(home.path())
        .args([
            "task",
            "create",
            "--id",
            "PM-20260511-ACP-NOISY",
            "--title",
            "ACP noisy stderr",
            "--project",
            project.path().to_str().unwrap(),
        ])
        .assert()
        .success();

    let mut cmd = helm_agent_with_home(home.path());
    cmd.timeout(std::time::Duration::from_secs(5))
        .args([
            "task",
            "dispatch",
            "PM-20260511-ACP-NOISY",
            "--runtime",
            "acp",
            "--agent",
            "noisy",
            "--confirm",
        ])
        .assert()
        .success()
        .stdout(contains("Completed ACP PM-20260511-ACP-NOISY"));
}

#[test]
fn acp_real_dispatch_preserves_child_recorded_progress_in_final_brief() {
    let home = tempdir().unwrap();
    let project = tempdir().unwrap();
    let fake_agent = home.path().join("mutating-acp-agent.py");
    fake_acp_agent_script(&fake_agent);

    helm_agent_with_home(home.path())
        .args([
            "acp",
            "agent",
            "add",
            "mutating",
            "--command",
            fake_agent.to_str().unwrap(),
            "--env",
            concat!("HELM_ACP_MUTATE_BIN=", env!("CARGO_BIN_EXE_helm-agent")),
            "--env",
            "HELM_ACP_MUTATE_TASK=PM-20260511-ACP-MUTATE",
            "--env",
            "HELM_ACP_MUTATE_MESSAGE=child wrote progress",
        ])
        .assert()
        .success();
    helm_agent_with_home(home.path())
        .args([
            "task",
            "create",
            "--id",
            "PM-20260511-ACP-MUTATE",
            "--title",
            "ACP child records progress",
            "--project",
            project.path().to_str().unwrap(),
        ])
        .assert()
        .success();

    helm_agent_with_home(home.path())
        .args([
            "task",
            "dispatch",
            "PM-20260511-ACP-MUTATE",
            "--runtime",
            "acp",
            "--agent",
            "mutating",
            "--confirm",
        ])
        .assert()
        .success();

    let store = TaskStore::new(home.path().to_path_buf());
    let task = store.load_task("PM-20260511-ACP-MUTATE").unwrap();
    assert_eq!(task.status, TaskStatus::ReadyForReview);
    let events = store.read_events("PM-20260511-ACP-MUTATE").unwrap();
    assert!(events
        .iter()
        .any(|event| event.message == "child wrote progress"));
    let brief = fs::read_to_string(task.recovery.brief_path.as_ref().unwrap()).unwrap();
    assert!(brief.contains("child wrote progress"), "{brief}");
}

#[test]
fn acp_real_dispatch_accepts_agent_that_exits_after_prompt_response() {
    let home = tempdir().unwrap();
    let project = tempdir().unwrap();
    let fake_agent = home.path().join("exiting-acp-agent.py");
    fake_acp_agent_script(&fake_agent);

    helm_agent_with_home(home.path())
        .args([
            "acp",
            "agent",
            "add",
            "exiting",
            "--command",
            fake_agent.to_str().unwrap(),
            "--env",
            "HELM_ACP_EXIT_AFTER_PROMPT=1",
        ])
        .assert()
        .success();
    helm_agent_with_home(home.path())
        .args([
            "task",
            "create",
            "--id",
            "PM-20260511-ACP-EXIT",
            "--title",
            "ACP exits after prompt",
            "--project",
            project.path().to_str().unwrap(),
        ])
        .assert()
        .success();

    helm_agent_with_home(home.path())
        .args([
            "task",
            "dispatch",
            "PM-20260511-ACP-EXIT",
            "--runtime",
            "acp",
            "--agent",
            "exiting",
            "--confirm",
        ])
        .assert()
        .success()
        .stdout(contains("Completed ACP PM-20260511-ACP-EXIT"));
}

#[test]
fn acp_real_dispatch_times_out_unresponsive_agent_and_allows_retry() {
    let home = tempdir().unwrap();
    let project = tempdir().unwrap();
    let sleeping_agent = home.path().join("sleeping-acp-agent.sh");
    let fake_agent = home.path().join("fake-acp-agent.py");
    fs::write(&sleeping_agent, "#!/bin/sh\nsleep 30\n").unwrap();
    let mut permissions = fs::metadata(&sleeping_agent).unwrap().permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&sleeping_agent, permissions).unwrap();
    fake_acp_agent_script(&fake_agent);

    helm_agent_with_home(home.path())
        .args([
            "acp",
            "agent",
            "add",
            "timeout",
            "--command",
            sleeping_agent.to_str().unwrap(),
        ])
        .assert()
        .success();
    helm_agent_with_home(home.path())
        .args([
            "task",
            "create",
            "--id",
            "PM-20260511-ACP-TIMEOUT",
            "--title",
            "ACP timeout",
            "--project",
            project.path().to_str().unwrap(),
        ])
        .assert()
        .success();

    let mut timeout_cmd = helm_agent_with_home(home.path());
    timeout_cmd
        .env("HELM_AGENT_ACP_TIMEOUT_MS", "200")
        .timeout(std::time::Duration::from_secs(5))
        .args([
            "task",
            "dispatch",
            "PM-20260511-ACP-TIMEOUT",
            "--runtime",
            "acp",
            "--agent",
            "timeout",
            "--confirm",
        ])
        .assert()
        .failure()
        .stderr(contains("ACP handoff timed out"));

    let store = TaskStore::new(home.path().to_path_buf());
    let task = store.load_task("PM-20260511-ACP-TIMEOUT").unwrap();
    assert_eq!(task.status, TaskStatus::NeedsChanges);

    helm_agent_with_home(home.path())
        .args([
            "acp",
            "agent",
            "add",
            "timeout",
            "--command",
            fake_agent.to_str().unwrap(),
        ])
        .assert()
        .success();
    helm_agent_with_home(home.path())
        .args([
            "task",
            "dispatch",
            "PM-20260511-ACP-TIMEOUT",
            "--runtime",
            "acp",
            "--agent",
            "timeout",
            "--confirm",
        ])
        .assert()
        .success()
        .stdout(contains("Completed ACP PM-20260511-ACP-TIMEOUT"));
}

#[test]
fn acp_success_local_brief_persistence_failure_keeps_task_retryable() {
    let home = tempdir().unwrap();
    let project = tempdir().unwrap();
    let fake_agent = home.path().join("chmod-acp-agent.py");
    fake_acp_agent_script(&fake_agent);

    helm_agent_with_home(home.path())
        .args([
            "acp",
            "agent",
            "add",
            "chmod",
            "--command",
            fake_agent.to_str().unwrap(),
            "--env",
            "HELM_ACP_CHMOD_TASK=PM-20260511-ACP-WRITEFAIL",
        ])
        .assert()
        .success();
    helm_agent_with_home(home.path())
        .args([
            "task",
            "create",
            "--id",
            "PM-20260511-ACP-WRITEFAIL",
            "--title",
            "ACP write failure",
            "--project",
            project.path().to_str().unwrap(),
        ])
        .assert()
        .success();

    let mut dispatch = helm_agent_with_home(home.path());
    dispatch
        .env("HELM_AGENT_ACP_TIMEOUT_MS", "3000")
        .timeout(std::time::Duration::from_secs(8))
        .args([
            "task",
            "dispatch",
            "PM-20260511-ACP-WRITEFAIL",
            "--runtime",
            "acp",
            "--agent",
            "chmod",
            "--confirm",
        ])
        .assert()
        .failure()
        .stderr(contains("ACP completion state update failed"));

    let store = TaskStore::new(home.path().to_path_buf());
    let task = store.load_task("PM-20260511-ACP-WRITEFAIL").unwrap();
    assert_ne!(task.status, TaskStatus::ReadyForReview);
    assert!(matches!(
        task.status,
        TaskStatus::Queued | TaskStatus::NeedsChanges
    ));
    let brief = fs::read_to_string(task.recovery.brief_path.as_ref().unwrap()).unwrap();
    assert!(!brief.contains("Status: ready_for_review"), "{brief}");
    assert!(!brief.contains("acp_dispatch_completed"), "{brief}");
}

#[test]
fn board_html_renders_interactive_escaped_task_board() {
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
        .stdout(contains("data-helm-board-app"))
        .stdout(contains("helm-agent-action-token"))
        .stdout(contains("Render &lt;board&gt; safely"))
        .stdout(contains("Add Event"));
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
    let env_arg = helm_agent_home_arg_for_test(home.path());

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
        .stdout(contains(format!(
            "Start: tmux new-session -d -e {} -s helm-agent-PM-20260509-004-codex -c /repo/project codex",
            shell_quote_for_test(&env_arg)
        )))
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
        format!(
            "tmux new-session -d -e {} -s helm-agent-PM-20260509-004-codex -c /repo/project codex",
            shell_quote_for_test(&env_arg)
        )
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
    let env_arg = helm_agent_home_arg_for_test(home.path());

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
        .stdout(contains(format!(
            "Start: tmux new-session -d -e {} -s helm-agent-PM-20260509-005-opencode -c /repo/project opencode",
            shell_quote_for_test(&env_arg)
        )))
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
    let env_arg = helm_agent_home_arg_for_test(home.path());

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
        .stdout(contains(format!(
            "Start: tmux new-session -d -e {} -s helm-agent-PM-20260509-006-claude -c '/repo/my project' claude",
            shell_quote_for_test(&env_arg)
        )))
        .stdout(contains(
            "Attach: tmux attach -t helm-agent-PM-20260509-006-claude",
        ))
        .stdout(contains("Resume: claude --resume <session-id>"));

    assert_eq!(
        fs::read_to_string(record_path).unwrap(),
        format!(
            "new-session\n-d\n-e\n{env_arg}\n-s\nhelm-agent-PM-20260509-006-claude\n-c\n/repo/my project\nclaude\n"
        )
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
        format!(
            "tmux new-session -d -e {} -s helm-agent-PM-20260509-006-claude -c '/repo/my project' claude",
            shell_quote_for_test(&env_arg)
        )
    );
    assert_eq!(
        events
            .iter()
            .filter(|event| event.event_type == "dispatch_prepared")
            .count(),
        1
    );
    assert_eq!(
        events
            .iter()
            .filter(|event| event.event_type == "dispatch_started")
            .count(),
        1
    );

    helm_agent_with_home(home.path())
        .args(["task", "status", "PM-20260509-006"])
        .assert()
        .success()
        .stdout(contains("[running]"));
}

#[test]
fn dispatch_respects_runtime_command_env_override() {
    let home = tempdir().unwrap();
    let temp = tempdir().unwrap();
    let tmux_bin = temp.path().join("fake-tmux");
    let record_path = temp.path().join("tmux-args.txt");
    fake_tmux_script(&tmux_bin, &record_path);
    let env_arg = helm_agent_home_arg_for_test(home.path());

    helm_agent_with_home(home.path())
        .args([
            "task",
            "create",
            "--id",
            "PM-20260512-CMD",
            "--title",
            "Dispatch with command override",
            "--project",
            "/repo/project",
        ])
        .assert()
        .success();

    helm_agent_with_home(home.path())
        .env("HELM_AGENT_TMUX_BIN", &tmux_bin)
        .env("HELM_AGENT_CLAUDE_COMMAND", "mc --code")
        .env(
            "HELM_AGENT_CLAUDE_RESUME_COMMAND",
            "mc --code --resume <session-id>",
        )
        .args(["task", "dispatch", "PM-20260512-CMD", "--runtime", "claude"])
        .assert()
        .success()
        .stdout(contains("Started PM-20260512-CMD"))
        .stdout(contains(format!(
            "Start: tmux new-session -d -e {} -s helm-agent-PM-20260512-CMD-claude -c /repo/project 'mc --code'",
            shell_quote_for_test(&env_arg)
        )))
        .stdout(contains("Resume: mc --code --resume <session-id>"));

    assert_eq!(
        fs::read_to_string(record_path).unwrap(),
        format!(
            "new-session\n-d\n-e\n{env_arg}\n-s\nhelm-agent-PM-20260512-CMD-claude\n-c\n/repo/project\nmc --code\n"
        )
    );

    let store = TaskStore::new(home.path().to_path_buf());
    let task = store.load_task("PM-20260512-CMD").unwrap();
    assert_eq!(task.progress.last_event, "Dispatch started");
    assert_eq!(
        task.recovery.resume_command.as_deref(),
        Some("mc --code --resume <session-id>")
    );
    let events = store.read_events("PM-20260512-CMD").unwrap();
    assert!(events.iter().any(|event| {
        event.event_type == "dispatch_started"
            && event
                .message
                .contains(&format!(
                    "tmux new-session -d -e {} -s helm-agent-PM-20260512-CMD-claude -c /repo/project 'mc --code'",
                    shell_quote_for_test(&env_arg)
                ))
    }));
}

#[test]
fn runtime_profile_set_persists_and_doctor_reports_effective_command() {
    let home = tempdir().unwrap();

    helm_agent_with_home(home.path())
        .args([
            "runtime",
            "profile",
            "set",
            "claude",
            "--command",
            "mc --code",
            "--resume",
            "mc --code --resume <session-id>",
        ])
        .assert()
        .success()
        .stdout(contains("Saved runtime profile"))
        .stdout(contains("claude command: mc --code"))
        .stdout(contains("claude resume: mc --code --resume <session-id>"));

    let profile = fs::read_to_string(home.path().join("runtime/profile.yaml")).unwrap();
    assert!(profile.contains("claude:"), "{profile}");
    assert!(profile.contains("command: mc --code"), "{profile}");
    assert!(
        profile.contains("resume: mc --code --resume <session-id>"),
        "{profile}"
    );

    helm_agent_with_home(home.path())
        .args(["runtime", "profile", "doctor"])
        .assert()
        .success()
        .stdout(contains("Runtime profile:"))
        .stdout(contains("claude command: mc --code (profile)"))
        .stdout(contains(
            "claude resume: mc --code --resume <session-id> (profile)",
        ));
}

#[test]
fn runtime_doctor_reports_default_claude_when_profile_is_absent() {
    let home = tempdir().unwrap();

    helm_agent_with_home(home.path())
        .args(["runtime", "doctor"])
        .assert()
        .success()
        .stdout(contains("Runtime profile:"))
        .stdout(contains("claude command: claude (default)"))
        .stdout(contains(
            "claude resume: claude --resume <session-id> (default)",
        ));
}

#[test]
fn runtime_doctor_uses_configured_tmux_bin_and_probes_env_support() {
    let home = tempdir().unwrap();
    let temp = tempdir().unwrap();
    let tmux_bin = temp.path().join("fake-tmux-doctor");
    let record_path = temp.path().join("tmux-doctor-args.txt");
    fake_tmux_doctor_script(&tmux_bin, &record_path);

    helm_agent_with_home(home.path())
        .env("HELM_AGENT_TMUX_BIN", &tmux_bin)
        .args(["runtime", "doctor"])
        .assert()
        .success()
        .stdout(contains(format!("tmux: ok: {}", tmux_bin.display())))
        .stdout(contains("tmux new-session -e: ok"));

    let recorded = fs::read_to_string(record_path).unwrap();
    assert!(recorded.contains("-V"), "{recorded}");
    assert!(recorded.contains("new-session"), "{recorded}");
    assert!(recorded.contains("-e"), "{recorded}");
    assert!(recorded.contains("HELM_AGENT_DOCTOR=1"), "{recorded}");
}

#[test]
fn runtime_profile_set_rejects_empty_override_values() {
    let home = tempdir().unwrap();

    helm_agent_with_home(home.path())
        .args(["runtime", "profile", "set", "claude", "--command", "   "])
        .assert()
        .failure()
        .stderr(contains(
            "runtime profile set requires a non-empty command or resume value",
        ));

    assert!(!home.path().join("runtime/profile.yaml").exists());
}

#[test]
fn dispatch_derives_resume_command_from_profile_command_when_resume_is_absent() {
    let home = tempdir().unwrap();
    fs::create_dir_all(home.path().join("runtime")).unwrap();
    fs::write(
        home.path().join("runtime/profile.yaml"),
        "runtimes:\n  claude:\n    command: mc --code\n",
    )
    .unwrap();

    let temp = tempdir().unwrap();
    let tmux_bin = temp.path().join("fake-tmux");
    let record_path = temp.path().join("tmux-args.txt");
    fake_tmux_script(&tmux_bin, &record_path);

    helm_agent_with_home(home.path())
        .args([
            "task",
            "create",
            "--id",
            "PM-20260513-DERIVED",
            "--title",
            "Derive runtime resume",
            "--project",
            "/repo/project",
        ])
        .assert()
        .success();

    helm_agent_with_home(home.path())
        .env("HELM_AGENT_TMUX_BIN", &tmux_bin)
        .args([
            "task",
            "dispatch",
            "PM-20260513-DERIVED",
            "--runtime",
            "claude",
        ])
        .assert()
        .success()
        .stdout(contains("Resume: mc --code --resume <session-id>"));

    let store = TaskStore::new(home.path().to_path_buf());
    let task = store.load_task("PM-20260513-DERIVED").unwrap();
    assert_eq!(
        task.recovery.resume_command.as_deref(),
        Some("mc --code --resume <session-id>")
    );
}

#[test]
fn dispatch_uses_runtime_profile_command_when_env_override_absent() {
    let home = tempdir().unwrap();
    fs::create_dir_all(home.path().join("runtime")).unwrap();
    fs::write(
        home.path().join("runtime/profile.yaml"),
        "runtimes:\n  claude:\n    command: mc --code\n    resume: mc --code --resume <session-id>\n",
    )
    .unwrap();

    let temp = tempdir().unwrap();
    let tmux_bin = temp.path().join("fake-tmux");
    let record_path = temp.path().join("tmux-args.txt");
    fake_tmux_script(&tmux_bin, &record_path);
    let env_arg = helm_agent_home_arg_for_test(home.path());

    helm_agent_with_home(home.path())
        .args([
            "task",
            "create",
            "--id",
            "PM-20260513-PROFILE",
            "--title",
            "Dispatch with runtime profile",
            "--project",
            "/repo/project",
        ])
        .assert()
        .success();

    helm_agent_with_home(home.path())
        .env("HELM_AGENT_TMUX_BIN", &tmux_bin)
        .args([
            "task",
            "dispatch",
            "PM-20260513-PROFILE",
            "--runtime",
            "claude",
        ])
        .assert()
        .success()
        .stdout(contains("Started PM-20260513-PROFILE"))
        .stdout(contains(format!(
            "Start: tmux new-session -d -e {} -s helm-agent-PM-20260513-PROFILE-claude -c /repo/project 'mc --code'",
            shell_quote_for_test(&env_arg)
        )))
        .stdout(contains("Resume: mc --code --resume <session-id>"));

    assert_eq!(
        fs::read_to_string(record_path).unwrap(),
        format!(
            "new-session\n-d\n-e\n{env_arg}\n-s\nhelm-agent-PM-20260513-PROFILE-claude\n-c\n/repo/project\nmc --code\n"
        )
    );

    let store = TaskStore::new(home.path().to_path_buf());
    let task = store.load_task("PM-20260513-PROFILE").unwrap();
    assert_eq!(
        task.recovery.resume_command.as_deref(),
        Some("mc --code --resume <session-id>")
    );
}

#[test]
fn runtime_env_override_takes_precedence_over_runtime_profile() {
    let home = tempdir().unwrap();
    fs::create_dir_all(home.path().join("runtime")).unwrap();
    fs::write(
        home.path().join("runtime/profile.yaml"),
        "runtimes:\n  claude:\n    command: profile-claude\n    resume: profile-claude --resume <session-id>\n",
    )
    .unwrap();

    let temp = tempdir().unwrap();
    let tmux_bin = temp.path().join("fake-tmux");
    let record_path = temp.path().join("tmux-args.txt");
    fake_tmux_script(&tmux_bin, &record_path);
    let env_arg = helm_agent_home_arg_for_test(home.path());

    helm_agent_with_home(home.path())
        .args([
            "task",
            "create",
            "--id",
            "PM-20260513-ENV",
            "--title",
            "Env wins over runtime profile",
            "--project",
            "/repo/project",
        ])
        .assert()
        .success();

    helm_agent_with_home(home.path())
        .env("HELM_AGENT_TMUX_BIN", &tmux_bin)
        .env("HELM_AGENT_CLAUDE_COMMAND", "env-claude")
        .env(
            "HELM_AGENT_CLAUDE_RESUME_COMMAND",
            "env-claude --resume <session-id>",
        )
        .args(["task", "dispatch", "PM-20260513-ENV", "--runtime", "claude"])
        .assert()
        .success()
        .stdout(contains("Start:"))
        .stdout(contains("env-claude"))
        .stdout(contains("Resume: env-claude --resume <session-id>"));

    assert_eq!(
        fs::read_to_string(record_path).unwrap(),
        format!(
            "new-session\n-d\n-e\n{env_arg}\n-s\nhelm-agent-PM-20260513-ENV-claude\n-c\n/repo/project\nenv-claude\n"
        )
    );
}

#[test]
fn real_dispatch_failed_tmux_launch_records_prepared_not_started() {
    let home = tempdir().unwrap();
    let temp = tempdir().unwrap();
    let tmux_bin = temp.path().join("failing-tmux");
    failing_tmux_script(&tmux_bin);

    helm_agent_with_home(home.path())
        .args([
            "task",
            "create",
            "--id",
            "PM-20260511-LAUNCH-FAIL",
            "--title",
            "Dispatch launch failure",
            "--project",
            "/repo/project",
        ])
        .assert()
        .success();

    helm_agent_with_home(home.path())
        .env("HELM_AGENT_TMUX_BIN", &tmux_bin)
        .args([
            "task",
            "dispatch",
            "PM-20260511-LAUNCH-FAIL",
            "--runtime",
            "claude",
        ])
        .assert()
        .failure()
        .stderr(contains("tmux failed before launch"));

    let store = TaskStore::new(home.path().to_path_buf());
    let task = store.load_task("PM-20260511-LAUNCH-FAIL").unwrap();
    assert_eq!(task.status, TaskStatus::Queued);
    assert_eq!(task.progress.last_event, "Dispatch prepared");
    let events = store.read_events("PM-20260511-LAUNCH-FAIL").unwrap();
    assert!(events
        .iter()
        .any(|event| event.event_type == "dispatch_prepared"));
    assert!(
        !events
            .iter()
            .any(|event| event.event_type == "dispatch_started"),
        "{events:?}"
    );
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
    let events = store.read_events("PM-20260511-POST").unwrap();
    assert!(events
        .iter()
        .any(|event| event.event_type == "dispatch_prepared"));
    assert!(
        events
            .iter()
            .any(|event| event.event_type == "dispatch_started"),
        "{events:?}"
    );
    assert!(events
        .iter()
        .any(|event| event.event_type == "dispatch_state_warning"));
}

#[test]
fn send_brief_dry_run_is_rejected_before_tmux_launch() {
    let home = tempdir().unwrap();
    let temp = tempdir().unwrap();
    let tmux_bin = temp.path().join("fake-tmux");
    let record_path = temp.path().join("tmux-args.txt");
    fake_tmux_append_script(&tmux_bin, &record_path, false);

    helm_agent_with_home(home.path())
        .args([
            "task",
            "create",
            "--id",
            "PM-20260511-SEND-DRY",
            "--title",
            "Reject send brief dry run",
            "--project",
            "/repo/project",
        ])
        .assert()
        .success();

    helm_agent_with_home(home.path())
        .env("HELM_AGENT_TMUX_BIN", &tmux_bin)
        .args([
            "task",
            "dispatch",
            "PM-20260511-SEND-DRY",
            "--runtime",
            "claude",
            "--dry-run",
            "--send-brief",
        ])
        .assert()
        .failure()
        .stderr(contains("--send-brief cannot be used with --dry-run"));

    assert!(!record_path.exists());
}

#[test]
fn send_brief_real_dispatch_sends_brief_path_and_records_event() {
    let home = tempdir().unwrap();
    let temp = tempdir().unwrap();
    let tmux_bin = temp.path().join("fake-tmux");
    let record_path = temp.path().join("tmux-args.txt");
    fake_tmux_append_script(&tmux_bin, &record_path, false);
    let env_arg = helm_agent_home_arg_for_test(home.path());

    helm_agent_with_home(home.path())
        .args([
            "task",
            "create",
            "--id",
            "PM-20260511-SEND",
            "--title",
            "Send child brief",
            "--project",
            "/repo/project",
        ])
        .assert()
        .success();

    helm_agent_with_home(home.path())
        .env("HELM_AGENT_TMUX_BIN", &tmux_bin)
        .args([
            "task",
            "dispatch",
            "PM-20260511-SEND",
            "--runtime",
            "claude",
            "--send-brief",
        ])
        .assert()
        .success()
        .stdout(contains("Started PM-20260511-SEND"))
        .stdout(contains("Brief: "))
        .stdout(contains("Brief sent: yes"));

    let record = fs::read_to_string(&record_path).unwrap();
    assert!(record.contains(&format!(
        "CALL\nnew-session\n-d\n-e\n{env_arg}\n-s\nhelm-agent-PM-20260511-SEND-claude"
    )));
    assert!(record.contains("CALL\nsend-keys\n-t\n=helm-agent-PM-20260511-SEND-claude:"));
    assert!(record.contains("Use this HelmAgent child-agent brief before starting work:"));
    assert!(record.contains("sessions/PM-20260511-SEND/brief.md"));
    assert!(record.ends_with("Enter\n"), "{record}");

    let store = TaskStore::new(home.path().to_path_buf());
    let events = store.read_events("PM-20260511-SEND").unwrap();
    assert!(events
        .iter()
        .any(|event| { event.event_type == "brief_sent" && event.message.contains("brief.md") }));
}

#[test]
fn send_brief_failure_after_launch_keeps_recovery_available() {
    let home = tempdir().unwrap();
    let temp = tempdir().unwrap();
    let tmux_bin = temp.path().join("fake-tmux");
    let record_path = temp.path().join("tmux-args.txt");
    fake_tmux_append_script(&tmux_bin, &record_path, true);

    helm_agent_with_home(home.path())
        .args([
            "task",
            "create",
            "--id",
            "PM-20260511-SEND-FAIL",
            "--title",
            "Send child brief failure",
            "--project",
            "/repo/project",
        ])
        .assert()
        .success();

    helm_agent_with_home(home.path())
        .env("HELM_AGENT_TMUX_BIN", &tmux_bin)
        .args([
            "task",
            "dispatch",
            "PM-20260511-SEND-FAIL",
            "--runtime",
            "claude",
            "--send-brief",
        ])
        .assert()
        .success()
        .stdout(contains("Started PM-20260511-SEND-FAIL"))
        .stdout(contains(
            "Attach: tmux attach -t helm-agent-PM-20260511-SEND-FAIL-claude",
        ))
        .stdout(contains("Brief: "))
        .stdout(contains("Brief sent: no"))
        .stderr(contains("Warning: Brief send failed after tmux start"));

    let store = TaskStore::new(home.path().to_path_buf());
    let task = store.load_task("PM-20260511-SEND-FAIL").unwrap();
    assert_eq!(task.status, TaskStatus::Running);
    assert!(task.recovery.brief_path.is_some());
    let events = store.read_events("PM-20260511-SEND-FAIL").unwrap();
    assert!(events.iter().any(|event| {
        event.event_type == "brief_send_warning" && event.message.contains("send-keys failed")
    }));
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
fn dispatch_help_explains_dispatch_flags() {
    let home = tempdir().unwrap();

    helm_agent_with_home(home.path())
        .args(["task", "dispatch", "--help"])
        .assert()
        .success()
        .stdout(contains("Task id to dispatch"))
        .stdout(contains("Child-agent runtime to start"))
        .stdout(contains(
            "Record the planned tmux dispatch without launching a child agent",
        ))
        .stdout(contains(
            "Send the generated brief path into the tmux child-agent session",
        ))
        .stdout(contains("Confirm paid or elevated-risk real dispatch"));
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
    let env_arg = helm_agent_home_arg_for_test(home.path());

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
        format!(
            "new-session\n-d\n-e\n{env_arg}\n-s\nhelm-agent-PM-20260509-007-codex\n-c\n/repo/project\ncodex\n"
        )
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
        "helm-agent task sync --all",
        "helm-agent task brief",
        "helm-agent task dispatch --dry-run",
        "helm-agent board serve --host 127.0.0.1 --port 8765",
        "--send-brief",
        "helm-agent task mark",
        "task review --accept",
        "--confirm",
        "Do not claim code-changing work is complete",
        "Dogfood Loop",
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

#[test]
fn docs_cover_send_brief_as_opt_in_real_dispatch() {
    let readme = fs::read_to_string("README.md").unwrap();
    let guide = fs::read_to_string("docs/agent-integrations/main-agent.md").unwrap();
    let template = fs::read_to_string("docs/agent-integrations/main-agent-template.md").unwrap();
    let combined = format!("{readme}\n{guide}\n{template}");

    for required in [
        "helm-agent task dispatch PM-20260509-101 --runtime claude --send-brief",
        "helm-agent task dispatch PM-20260511-001 --runtime claude --send-brief",
        "helm-agent task dispatch PM-YYYYMMDD-001 --runtime claude --send-brief",
        "`--send-brief` is opt-in",
        "Brief sent: no",
    ] {
        assert!(
            combined.contains(required),
            "missing `{required}` from docs:\n{combined}"
        );
    }
}

#[test]
fn send_brief_docs_keep_ids_consistent_per_document() {
    let readme = fs::read_to_string("README.md").unwrap();
    let template = fs::read_to_string("docs/agent-integrations/main-agent-template.md").unwrap();

    assert!(
        readme.contains("helm-agent task brief PM-20260511-001 --write")
            && readme
                .contains("helm-agent task dispatch PM-20260511-001 --runtime claude --send-brief"),
        "{readme}"
    );
    assert!(
        !readme.contains("helm-agent task dispatch PM-20260509-101 --runtime claude --send-brief"),
        "{readme}"
    );
    assert!(
        template.contains("helm-agent task dispatch PM-YYYYMMDD-001 --runtime claude --send-brief"),
        "{template}"
    );
    assert!(
        !template
            .contains("helm-agent task dispatch PM-20260509-101 --runtime claude --send-brief"),
        "{template}"
    );
}

#[test]
fn dogfood_runbook_and_make_target_cover_dry_run_loop() {
    let runbook = fs::read_to_string("docs/dogfood.md").unwrap();
    let makefile = fs::read_to_string("Makefile").unwrap();

    for required in [
        "HELM_AGENT_HOME",
        "helm-agent project init --path",
        "helm-agent task create --id PM-20260512-DOGFOOD",
        "helm-agent task triage PM-20260512-DOGFOOD",
        "helm-agent task dispatch --dry-run --runtime claude PM-20260512-DOGFOOD",
        "helm-agent task sync --all",
        "helm-agent task mark PM-20260512-DOGFOOD --ready-for-review",
        "Human or authorized main agent only",
    ] {
        assert!(
            runbook.contains(required),
            "missing `{required}` from dogfood docs"
        );
    }

    for required in [
        "dogfood-dry-run:",
        "HELM_AGENT_HOME",
        "cargo run --quiet --bin helm-agent -- project init --path",
        "cargo run --quiet --bin helm-agent -- task create --id PM-20260512-DOGFOOD",
        "cargo run --quiet --bin helm-agent -- task triage PM-20260512-DOGFOOD",
        "cargo run --quiet --bin helm-agent -- task dispatch --dry-run --runtime claude PM-20260512-DOGFOOD",
        "cargo run --quiet --bin helm-agent -- task sync --all",
        "cargo run --quiet --bin helm-agent -- task mark PM-20260512-DOGFOOD --ready-for-review",
        "cargo run --quiet --bin helm-agent -- task review PM-20260512-DOGFOOD --accept",
    ] {
        assert!(
            makefile.contains(required),
            "missing `{required}` from Makefile"
        );
    }
}
