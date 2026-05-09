use helm_agent::adapter::RuntimeAdapter;
use helm_agent::domain::AgentRuntime;
use helm_agent::launcher::LaunchPlan;
use std::path::Path;

#[test]
fn adapter_metadata_matches_runtime_capabilities() {
    let claude = RuntimeAdapter::for_runtime(AgentRuntime::Claude);
    assert_eq!(claude.command, "claude");
    assert_eq!(
        claude.resume_command("native-123"),
        "claude --resume native-123"
    );
    assert!(claude.native_resume_available);
    assert!(!claude.acp_supported);

    let codex = RuntimeAdapter::for_runtime(AgentRuntime::Codex);
    assert_eq!(codex.command, "codex");
    assert_eq!(
        codex.resume_command("native-123"),
        "codex resume native-123 --all"
    );
    assert!(codex.native_resume_available);
    assert!(!codex.acp_supported);

    let opencode = RuntimeAdapter::for_runtime(AgentRuntime::OpenCode);
    assert_eq!(opencode.command, "opencode");
    assert_eq!(
        opencode.resume_command("native-123"),
        "opencode resume native-123"
    );
    assert!(!opencode.native_resume_available);
    assert!(!opencode.acp_supported);
}

#[test]
fn dry_run_launch_plan_builds_tmux_and_recovery_commands() {
    let launch = LaunchPlan::dry_run(
        "PM-20260509-007",
        AgentRuntime::Claude,
        Path::new("/repo/project"),
        "native-123",
    );

    assert_eq!(launch.tmux_session, "helm-agent-PM-20260509-007-claude");
    assert_eq!(
        launch.start_command,
        "tmux new-session -d -s helm-agent-PM-20260509-007-claude -c /repo/project claude"
    );
    assert_eq!(
        launch.attach_command,
        "tmux attach -t helm-agent-PM-20260509-007-claude"
    );
    assert_eq!(launch.resume_command, "claude --resume native-123");
}

#[test]
fn codex_dry_run_launch_plan_uses_codex_resume_command() {
    let launch = LaunchPlan::dry_run(
        "PM-20260509-008",
        AgentRuntime::Codex,
        Path::new("/repo/project"),
        "codex-session",
    );

    assert_eq!(launch.tmux_session, "helm-agent-PM-20260509-008-codex");
    assert_eq!(
        launch.start_command,
        "tmux new-session -d -s helm-agent-PM-20260509-008-codex -c /repo/project codex"
    );
    assert_eq!(
        launch.attach_command,
        "tmux attach -t helm-agent-PM-20260509-008-codex"
    );
    assert_eq!(launch.resume_command, "codex resume codex-session --all");
}
