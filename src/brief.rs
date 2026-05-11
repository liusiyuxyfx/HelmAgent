use crate::domain::{AgentRuntime, ReviewState, TaskEvent, TaskRecord};
use std::fmt::Write as _;

const RECENT_EVENT_LIMIT: usize = 8;

pub fn render_task_brief(task: &TaskRecord, events: &[TaskEvent]) -> String {
    let mut output = String::new();

    writeln!(&mut output, "# Child Agent Task Brief: {}", task.id).expect("write to string");
    output.push('\n');

    writeln!(&mut output, "## Task").expect("write to string");
    writeln!(&mut output, "- Title: {}", task.title).expect("write to string");
    writeln!(&mut output, "- Project: {}", task.project.path.display()).expect("write to string");
    write_optional(&mut output, "Branch", task.project.branch.as_deref());
    writeln!(&mut output, "- Status: {}", task.status.as_str()).expect("write to string");
    writeln!(&mut output, "- Risk: {}", task.risk.as_str()).expect("write to string");
    writeln!(&mut output, "- Priority: {}", task.priority).expect("write to string");
    writeln!(
        &mut output,
        "- Runtime: {}",
        task.assignment
            .runtime
            .map(AgentRuntime::as_str)
            .unwrap_or("unassigned")
    )
    .expect("write to string");
    write_optional(&mut output, "Workflow", task.assignment.workflow.as_deref());
    write_optional(&mut output, "Tmux", task.assignment.tmux_session.as_deref());
    write_optional(
        &mut output,
        "Native session",
        task.assignment.native_session_id.as_deref(),
    );
    write_optional(
        &mut output,
        "ACP session",
        task.assignment.acp_session_id.as_deref(),
    );
    output.push('\n');

    writeln!(&mut output, "## Progress").expect("write to string");
    writeln!(&mut output, "- Summary: {}", task.progress.summary).expect("write to string");
    writeln!(&mut output, "- Last: {}", task.progress.last_event).expect("write to string");
    writeln!(&mut output, "- Next: {}", task.progress.next_action).expect("write to string");
    write_optional(&mut output, "Blocker", task.progress.blocker.as_deref());
    output.push('\n');

    writeln!(&mut output, "## Review").expect("write to string");
    writeln!(
        &mut output,
        "- Review: {}",
        review_state_as_str(task.review.state)
    )
    .expect("write to string");
    write_optional(&mut output, "Review reason", task.review.reason.as_deref());
    if task.review.artifacts.is_empty() {
        writeln!(&mut output, "- Artifacts: none").expect("write to string");
    } else {
        for artifact in &task.review.artifacts {
            writeln!(&mut output, "- Artifact: {artifact}").expect("write to string");
        }
    }
    output.push('\n');

    writeln!(&mut output, "## Recovery").expect("write to string");
    write_optional(
        &mut output,
        "Attach",
        task.recovery.attach_command.as_deref(),
    );
    write_optional(
        &mut output,
        "Resume",
        task.recovery.resume_command.as_deref(),
    );
    if let Some(brief_path) = task.recovery.brief_path.as_deref() {
        writeln!(&mut output, "- Brief: {}", brief_path.display()).expect("write to string");
    }
    if task.recovery.attach_command.is_none()
        && task.recovery.resume_command.is_none()
        && task.recovery.brief_path.is_none()
    {
        writeln!(&mut output, "- No recovery commands recorded").expect("write to string");
    }
    output.push('\n');

    writeln!(&mut output, "## Recent Events").expect("write to string");
    let recent: Vec<&TaskEvent> = events.iter().rev().take(RECENT_EVENT_LIMIT).collect();
    if recent.is_empty() {
        writeln!(&mut output, "- none").expect("write to string");
    } else {
        for event in recent.into_iter().rev() {
            writeln!(&mut output, "- {}: {}", event.event_type, event.message)
                .expect("write to string");
        }
    }
    output.push('\n');

    writeln!(&mut output, "## Child Agent Instructions").expect("write to string");
    writeln!(&mut output, "- Inspect the project before editing.").expect("write to string");
    writeln!(&mut output, "- Make only scoped changes for this task.").expect("write to string");
    writeln!(
        &mut output,
        "- Record progress: `helm-agent task event {} --type progress --message \"<short update>\"`.",
        task.id
    )
    .expect("write to string");
    writeln!(
        &mut output,
        "- Run verification before reporting completion."
    )
    .expect("write to string");
    writeln!(
        &mut output,
        "- Report changed files and verification artifacts."
    )
    .expect("write to string");
    writeln!(
        &mut output,
        "- Mark ready-for-review through HelmAgent: `helm-agent task mark {} --ready-for-review --message \"<artifacts and verification>\"`.",
        task.id
    )
    .expect("write to string");

    output
}

fn write_optional(output: &mut String, label: &str, value: Option<&str>) {
    if let Some(value) = value {
        writeln!(output, "- {label}: {value}").expect("write to string");
    }
}

fn review_state_as_str(state: ReviewState) -> &'static str {
    match state {
        ReviewState::NotRequired => "not_required",
        ReviewState::Required => "required",
        ReviewState::Accepted => "accepted",
        ReviewState::ChangesRequested => "changes_requested",
    }
}
