use helm_agent::adapter::RuntimeAdapter;
use helm_agent::domain::AgentRuntime;
use helm_agent::launcher::{DispatchPlan, Launcher, TmuxSessionState};
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::path::PathBuf;
use tempfile::tempdir;

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
        "#!/bin/sh\nprintf '%s\\n' 'tmux failed: duplicate session' >&2\nexit 7\n",
    )
    .unwrap();
    let mut permissions = fs::metadata(path).unwrap().permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(path, permissions).unwrap();
}

fn has_session_tmux_script(path: &Path, record_path: &Path, exit_code: i32) {
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
fn adapter_metadata_matches_runtime_capabilities() {
    let claude = RuntimeAdapter::for_runtime(AgentRuntime::Claude);
    assert_eq!(claude.command, "claude");
    assert_eq!(
        claude.native_resume_template,
        "claude --resume <session-id>"
    );
    assert!(claude.native_resume_available);
    assert!(!claude.acp_supported);

    let codex = RuntimeAdapter::for_runtime(AgentRuntime::Codex);
    assert_eq!(codex.command, "codex");
    assert_eq!(
        codex.native_resume_template,
        "codex resume <session-id> --all"
    );
    assert!(codex.native_resume_available);
    assert!(!codex.acp_supported);

    let opencode = RuntimeAdapter::for_runtime(AgentRuntime::OpenCode);
    assert_eq!(opencode.command, "opencode");
    assert_eq!(
        opencode.native_resume_template,
        "opencode resume <session-id>"
    );
    assert!(!opencode.native_resume_available);
    assert!(!opencode.acp_supported);
}

#[test]
fn dry_run_preview_builds_tmux_and_recovery_commands() {
    let dispatch = DispatchPlan {
        task_id: "PM-20260509-007".to_string(),
        runtime: AgentRuntime::Claude,
        cwd: PathBuf::from("/repo/project"),
    };
    let launch = Launcher::new().dry_run(&dispatch);

    assert_eq!(launch.tmux_session, "helm-agent-PM-20260509-007-claude");
    assert_eq!(
        launch.start_command,
        "tmux new-session -d -s helm-agent-PM-20260509-007-claude -c /repo/project claude"
    );
    assert_eq!(
        launch.attach_command,
        "tmux attach -t helm-agent-PM-20260509-007-claude"
    );
    assert_eq!(
        launch.resume_command.as_deref(),
        Some("claude --resume <session-id>")
    );
}

#[test]
fn codex_dry_run_preview_uses_placeholder_resume_template() {
    let dispatch = DispatchPlan {
        task_id: "PM-20260509-008".to_string(),
        runtime: AgentRuntime::Codex,
        cwd: PathBuf::from("/repo/project"),
    };
    let launch = Launcher::new().dry_run(&dispatch);

    assert_eq!(launch.tmux_session, "helm-agent-PM-20260509-008-codex");
    assert_eq!(
        launch.start_command,
        "tmux new-session -d -s helm-agent-PM-20260509-008-codex -c /repo/project codex"
    );
    assert_eq!(
        launch.attach_command,
        "tmux attach -t helm-agent-PM-20260509-008-codex"
    );
    assert_eq!(
        launch.resume_command.as_deref(),
        Some("codex resume <session-id> --all")
    );
}

#[test]
fn opencode_dry_run_preview_has_no_native_resume_command() {
    let dispatch = DispatchPlan {
        task_id: "PM-20260509-009".to_string(),
        runtime: AgentRuntime::OpenCode,
        cwd: PathBuf::from("/repo/project"),
    };
    let launch = Launcher::new().dry_run(&dispatch);

    assert_eq!(launch.tmux_session, "helm-agent-PM-20260509-009-opencode");
    assert_eq!(
        launch.start_command,
        "tmux new-session -d -s helm-agent-PM-20260509-009-opencode -c /repo/project opencode"
    );
    assert_eq!(
        launch.attach_command,
        "tmux attach -t helm-agent-PM-20260509-009-opencode"
    );
    assert_eq!(launch.resume_command, None);
}

#[test]
fn dry_run_preview_shell_quotes_cwd_with_spaces() {
    let dispatch = DispatchPlan {
        task_id: "PM-20260509-010".to_string(),
        runtime: AgentRuntime::Claude,
        cwd: PathBuf::from("/repo/my project"),
    };
    let launch = Launcher::new().dry_run(&dispatch);

    assert_eq!(
        launch.start_command,
        "tmux new-session -d -s helm-agent-PM-20260509-010-claude -c '/repo/my project' claude"
    );
}

#[test]
fn dry_run_preview_shell_quotes_single_quote_in_cwd() {
    let dispatch = DispatchPlan {
        task_id: "PM-20260509-011".to_string(),
        runtime: AgentRuntime::Codex,
        cwd: PathBuf::from("/repo/owner's project"),
    };
    let launch = Launcher::new().dry_run(&dispatch);

    assert_eq!(
        launch.start_command,
        "tmux new-session -d -s helm-agent-PM-20260509-011-codex -c '/repo/owner'\\''s project' codex"
    );
}

#[test]
fn dry_run_preview_shell_quotes_empty_cwd() {
    let dispatch = DispatchPlan {
        task_id: "PM-20260509-015".to_string(),
        runtime: AgentRuntime::Codex,
        cwd: PathBuf::from(""),
    };
    let launch = Launcher::new().dry_run(&dispatch);

    assert_eq!(
        launch.start_command,
        "tmux new-session -d -s helm-agent-PM-20260509-015-codex -c '' codex"
    );
}

#[test]
fn dry_run_preview_shell_quotes_unsafe_session_tokens() {
    let dispatch = DispatchPlan {
        task_id: "PM 20260509'013".to_string(),
        runtime: AgentRuntime::Codex,
        cwd: PathBuf::from("/repo/project"),
    };
    let launch = Launcher::new().dry_run(&dispatch);

    assert_eq!(launch.tmux_session, "helm-agent-PM 20260509'013-codex");
    assert_eq!(
        launch.start_command,
        "tmux new-session -d -s 'helm-agent-PM 20260509'\\''013-codex' -c /repo/project codex"
    );
    assert_eq!(
        launch.attach_command,
        "tmux attach -t 'helm-agent-PM 20260509'\\''013-codex'"
    );
}

#[test]
fn launch_executes_tmux_with_expected_arguments() {
    let temp = tempdir().unwrap();
    let tmux_bin = temp.path().join("fake-tmux");
    let record_path = temp.path().join("tmux-args.txt");
    fake_tmux_script(&tmux_bin, &record_path);

    let dispatch = DispatchPlan {
        task_id: "PM-20260509-012".to_string(),
        runtime: AgentRuntime::Codex,
        cwd: PathBuf::from("/repo/my project"),
    };
    let launch = Launcher::with_tmux_bin(tmux_bin).launch(&dispatch).unwrap();

    assert_eq!(launch.tmux_session, "helm-agent-PM-20260509-012-codex");
    assert_eq!(
        launch.start_command,
        "tmux new-session -d -s helm-agent-PM-20260509-012-codex -c '/repo/my project' codex"
    );
    assert_eq!(
        fs::read_to_string(record_path).unwrap(),
        "new-session\n-d\n-s\nhelm-agent-PM-20260509-012-codex\n-c\n/repo/my project\ncodex\n"
    );
}

#[test]
fn launch_error_includes_tmux_output_and_session_name() {
    let temp = tempdir().unwrap();
    let tmux_bin = temp.path().join("failing-tmux");
    failing_tmux_script(&tmux_bin);

    let dispatch = DispatchPlan {
        task_id: "PM-20260509-014".to_string(),
        runtime: AgentRuntime::Claude,
        cwd: PathBuf::from("/repo/project"),
    };
    let error = Launcher::with_tmux_bin(tmux_bin)
        .launch(&dispatch)
        .unwrap_err()
        .to_string();

    assert!(error.contains("helm-agent-PM-20260509-014-claude"));
    assert!(error.contains("tmux failed: duplicate session"));
}

#[test]
fn send_keys_invokes_tmux_with_literal_message_and_enter() {
    let temp = tempdir().unwrap();
    let tmux_bin = temp.path().join("fake-tmux");
    let record_path = temp.path().join("tmux-args.txt");
    fake_tmux_script(&tmux_bin, &record_path);

    Launcher::with_tmux_bin(tmux_bin)
        .send_keys(
            "helm-agent-PM-20260511-SEND-claude",
            "Use brief\n/path/to/brief.md",
        )
        .unwrap();

    assert_eq!(
        fs::read_to_string(record_path).unwrap(),
        "send-keys\n-t\n=helm-agent-PM-20260511-SEND-claude\nUse brief\n/path/to/brief.md\nEnter\n"
    );
}

#[test]
fn send_keys_error_includes_tmux_output_and_session_name() {
    let temp = tempdir().unwrap();
    let tmux_bin = temp.path().join("failing-tmux");
    failing_tmux_script(&tmux_bin);

    let error = Launcher::with_tmux_bin(tmux_bin)
        .send_keys("helm-agent-PM-20260511-FAIL-claude", "Use brief")
        .unwrap_err()
        .to_string();

    assert!(error.contains("helm-agent-PM-20260511-FAIL-claude"));
    assert!(error.contains("tmux failed: duplicate session"));
}

#[test]
fn session_state_invokes_tmux_has_session() {
    let temp = tempdir().unwrap();
    let tmux_bin = temp.path().join("fake-tmux");
    let record_path = temp.path().join("tmux-args.txt");
    has_session_tmux_script(&tmux_bin, &record_path, 0);

    let state = Launcher::with_tmux_bin(tmux_bin)
        .session_state("helm-agent-PM-20260511-001-claude")
        .unwrap();

    assert_eq!(state, TmuxSessionState::Alive);
    assert_eq!(
        fs::read_to_string(record_path).unwrap(),
        "has-session\n-t\n=helm-agent-PM-20260511-001-claude\n"
    );
}

#[test]
fn session_state_reports_missing_for_nonzero_tmux_exit() {
    let temp = tempdir().unwrap();
    let tmux_bin = temp.path().join("fake-tmux");
    let record_path = temp.path().join("tmux-args.txt");
    has_session_tmux_script(&tmux_bin, &record_path, 1);

    let state = Launcher::with_tmux_bin(tmux_bin)
        .session_state("helm-agent-PM-20260511-002-codex")
        .unwrap();

    assert_eq!(state, TmuxSessionState::Missing);
}
