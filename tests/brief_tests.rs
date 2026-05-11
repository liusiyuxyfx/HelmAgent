use helm_agent::brief::render_task_brief;
use helm_agent::domain::{AgentRuntime, ReviewState, RiskLevel, TaskEvent, TaskRecord, TaskStatus};
use time::OffsetDateTime;

#[test]
fn render_task_brief_includes_context_recovery_events_and_instructions() {
    let mut task = TaskRecord::new(
        "PM-20260511-BRIEF".to_string(),
        "Add task brief".to_string(),
        "/repo/project".into(),
        OffsetDateTime::UNIX_EPOCH,
    );
    task.status = TaskStatus::Running;
    task.priority = "high".to_string();
    task.risk = RiskLevel::Medium;
    task.project.branch = Some("feature/task-brief".to_string());
    task.assignment.runtime = Some(AgentRuntime::Claude);
    task.assignment.tmux_session = Some("helm-agent-PM-20260511-BRIEF-claude".to_string());
    task.recovery.attach_command =
        Some("tmux attach -t helm-agent-PM-20260511-BRIEF-claude".to_string());
    task.recovery.resume_command = Some("claude --resume <session-id>".to_string());
    task.recovery.brief_path =
        Some("/home/user/.helm-agent/sessions/PM-20260511-BRIEF/brief.md".into());
    task.progress.summary = "Implementation scoped".to_string();
    task.progress.last_event = "Dispatcher prepared child context".to_string();
    task.progress.next_action = "Open tmux session".to_string();
    task.progress.blocker = Some("Waiting for reviewer".to_string());
    task.review.state = ReviewState::Required;
    task.review.reason = Some("Touches dispatch flow".to_string());
    task.review.artifacts = vec!["cargo test".to_string(), "diff summary".to_string()];

    let events = vec![
        TaskEvent::new(
            task.id.clone(),
            "created",
            "Task created".to_string(),
            OffsetDateTime::UNIX_EPOCH,
        ),
        TaskEvent::new(
            task.id.clone(),
            "progress",
            "Found affected files".to_string(),
            OffsetDateTime::UNIX_EPOCH,
        ),
    ];

    let markdown = render_task_brief(&task, &events);

    assert!(markdown.contains("# Child Agent Task Brief: PM-20260511-BRIEF"));
    assert!(markdown.contains("- Title: Add task brief"));
    assert!(markdown.contains("- Project: /repo/project"));
    assert!(markdown.contains("- Branch: feature/task-brief"));
    assert!(markdown.contains("- Status: running"));
    assert!(markdown.contains("- Risk: medium"));
    assert!(markdown.contains("- Priority: high"));
    assert!(markdown.contains("- Runtime: claude"));
    assert!(markdown.contains("- Tmux: helm-agent-PM-20260511-BRIEF-claude"));
    assert!(markdown.contains("- Attach: tmux attach -t helm-agent-PM-20260511-BRIEF-claude"));
    assert!(markdown.contains("- Resume: claude --resume <session-id>"));
    assert!(
        markdown.contains("- Brief: /home/user/.helm-agent/sessions/PM-20260511-BRIEF/brief.md")
    );
    assert!(markdown.contains("- Summary: Implementation scoped"));
    assert!(markdown.contains("- Blocker: Waiting for reviewer"));
    assert!(markdown.contains("- Review: required"));
    assert!(markdown.contains("- Review reason: Touches dispatch flow"));
    assert!(markdown.contains("- Artifact: cargo test"));
    assert!(markdown.contains("- Artifact: diff summary"));
    assert!(markdown.contains("- progress: Found affected files"));
    assert!(markdown.contains("Inspect the project before editing"));
    assert!(markdown.contains("Make only scoped changes"));
    assert!(markdown.contains(
        "helm-agent task event PM-20260511-BRIEF --type progress --message \"<short update>\""
    ));
    assert!(markdown.contains("Run verification before reporting completion"));
    assert!(markdown.contains(
        "helm-agent task mark PM-20260511-BRIEF --ready-for-review --message \"<artifacts and verification>\""
    ));
}
