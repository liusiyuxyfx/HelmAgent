use helm_agent::domain::{AgentRuntime, ReviewState, RiskLevel, TaskEvent, TaskRecord, TaskStatus};
use time::OffsetDateTime;

#[test]
fn task_status_serializes_as_snake_case() {
    let status = TaskStatus::ReadyForReview;
    let yaml = serde_yaml::to_string(&status).unwrap();
    assert!(yaml.contains("ready_for_review"));
}

#[test]
fn task_record_round_trips_through_yaml() {
    let now = OffsetDateTime::parse(
        "2026-05-09T10:00:00Z",
        &time::format_description::well_known::Rfc3339,
    )
    .unwrap();

    let task = TaskRecord::new(
        "PM-20260509-001".to_string(),
        "Fix login redirect bug".to_string(),
        "/repo".into(),
        now,
    );

    let yaml = serde_yaml::to_string(&task).unwrap();
    let parsed: TaskRecord = serde_yaml::from_str(&yaml).unwrap();

    assert_eq!(parsed.id, "PM-20260509-001");
    assert_eq!(parsed.status, TaskStatus::Inbox);
    assert_eq!(parsed.risk, RiskLevel::Low);
    assert_eq!(parsed.project.path.to_string_lossy(), "/repo");
    assert_eq!(parsed.review.state, ReviewState::NotRequired);
}

#[test]
fn task_event_round_trips_through_json() {
    let event = TaskEvent::progress(
        "PM-20260509-001".to_string(),
        "Found redirect handler".to_string(),
        OffsetDateTime::UNIX_EPOCH,
    );

    let json = serde_json::to_string(&event).unwrap();
    let parsed: TaskEvent = serde_json::from_str(&json).unwrap();

    assert_eq!(parsed.task_id, "PM-20260509-001");
    assert_eq!(parsed.event_type, "progress");
    assert_eq!(parsed.message, "Found redirect handler");
}

#[test]
fn runtime_display_names_match_cli_values() {
    assert_eq!(AgentRuntime::Claude.as_str(), "claude");
    assert_eq!(AgentRuntime::Codex.as_str(), "codex");
    assert_eq!(AgentRuntime::OpenCode.as_str(), "opencode");
}

#[test]
fn runtime_serialization_matches_cli_values() {
    let runtimes = [
        (AgentRuntime::Claude, "claude"),
        (AgentRuntime::Codex, "codex"),
        (AgentRuntime::OpenCode, "opencode"),
    ];

    for (runtime, value) in runtimes {
        let yaml = serde_yaml::to_string(&runtime).unwrap();
        assert!(yaml.contains(value), "{yaml}");

        let parsed: AgentRuntime = serde_yaml::from_str(value).unwrap();
        assert_eq!(parsed, runtime);
    }
}
