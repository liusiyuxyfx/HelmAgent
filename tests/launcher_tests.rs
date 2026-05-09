use helm_agent::adapter::RuntimeAdapter;
use helm_agent::domain::AgentRuntime;
use helm_agent::launcher::{DispatchPlan, Launcher};
use std::path::PathBuf;

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
