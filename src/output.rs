use crate::domain::{ReviewState, TaskEvent, TaskRecord, TaskStatus};
use std::fmt::Write as _;

pub fn task_status(task: &TaskRecord, events: &[TaskEvent]) -> String {
    let last_event = events
        .last()
        .map(|event| event.message.as_str())
        .unwrap_or(task.progress.last_event.as_str());
    let review = task
        .review
        .reason
        .as_deref()
        .map(|reason| format!("Review: {reason}\n"))
        .unwrap_or_default();

    format!(
        "{id} [{status}]\nTitle: {title}\nProject: {project}\nProgress: {progress}\nNext: {next}\n{review}",
        id = task.id,
        status = task.status.as_str(),
        title = task.title,
        project = task.project.path.display(),
        progress = last_event,
        next = task.progress.next_action,
        review = review,
    )
}

pub fn resume_text(task: &TaskRecord) -> String {
    let attach = task
        .recovery
        .attach_command
        .as_deref()
        .unwrap_or("No tmux session recorded");
    let resume = task
        .recovery
        .resume_command
        .as_deref()
        .unwrap_or("No native resume command recorded");

    format!("{id}\nAttach: {attach}\nResume: {resume}\n", id = task.id)
}

pub fn task_list(tasks: &[TaskRecord]) -> String {
    let mut output = String::new();

    for task in tasks {
        let runtime = task
            .assignment
            .runtime
            .map(|runtime| runtime.as_str())
            .unwrap_or("-");
        let review = task.review.reason.as_deref().unwrap_or("-");
        output.push_str(&format!(
            "{id}\t{status}\t{risk}\t{runtime}\t{title}\t{last}\t{next}\t{review}\n",
            id = task.id,
            status = task.status.as_str(),
            risk = task.risk.as_str(),
            runtime = runtime,
            title = task.title,
            last = task.progress.last_event,
            next = task.progress.next_action,
            review = review,
        ));
    }

    output
}

pub fn task_board(tasks: &[TaskRecord]) -> String {
    if tasks.is_empty() {
        return "No active tasks\n".to_string();
    }

    let lanes = [
        BoardLane::Inbox,
        BoardLane::Triaged,
        BoardLane::Queued,
        BoardLane::Running,
        BoardLane::Blocked,
        BoardLane::Review,
        BoardLane::Done,
    ];
    let mut output = String::new();

    for lane in lanes {
        let lane_tasks: Vec<&TaskRecord> = tasks
            .iter()
            .filter(|task| board_lane(task) == Some(lane))
            .collect();
        if lane_tasks.is_empty() {
            continue;
        }

        if !output.is_empty() {
            output.push('\n');
        }
        writeln!(&mut output, "{}", lane.title()).expect("write to string");

        for task in lane_tasks {
            let runtime = task
                .assignment
                .runtime
                .map(|runtime| runtime.as_str())
                .unwrap_or("-");
            writeln!(
                &mut output,
                "- {id} [{risk}/{runtime}/{priority}] {title}",
                id = task.id,
                risk = task.risk.as_str(),
                runtime = runtime,
                priority = task.priority,
                title = task.title,
            )
            .expect("write to string");
            writeln!(&mut output, "  next: {}", task.progress.next_action)
                .expect("write to string");
            writeln!(&mut output, "  last: {}", task.progress.last_event).expect("write to string");

            if let Some(blocker) = task.progress.blocker.as_deref() {
                writeln!(&mut output, "  blocker: {blocker}").expect("write to string");
            }
            if let Some(reason) = task.review.reason.as_deref() {
                writeln!(&mut output, "  review: {reason}").expect("write to string");
            }
            if let Some(attach) = task.recovery.attach_command.as_deref() {
                writeln!(&mut output, "  attach: {attach}").expect("write to string");
            }
            if let Some(resume) = task.recovery.resume_command.as_deref() {
                writeln!(&mut output, "  resume: {resume}").expect("write to string");
            }
        }
    }

    if output.is_empty() {
        "No active tasks\n".to_string()
    } else {
        output
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BoardLane {
    Inbox,
    Triaged,
    Queued,
    Running,
    Blocked,
    Review,
    Done,
}

impl BoardLane {
    fn title(self) -> &'static str {
        match self {
            BoardLane::Inbox => "Inbox",
            BoardLane::Triaged => "Triaged",
            BoardLane::Queued => "Queued",
            BoardLane::Running => "Running",
            BoardLane::Blocked => "Blocked",
            BoardLane::Review => "Review",
            BoardLane::Done => "Done",
        }
    }
}

fn board_lane(task: &TaskRecord) -> Option<BoardLane> {
    if matches!(task.status, TaskStatus::Blocked | TaskStatus::WaitingUser) {
        return Some(BoardLane::Blocked);
    }
    if task.review.state == ReviewState::Required
        || matches!(
            task.status,
            TaskStatus::ReadyForReview | TaskStatus::Reviewing | TaskStatus::NeedsChanges
        )
    {
        return Some(BoardLane::Review);
    }

    match task.status {
        TaskStatus::Inbox => Some(BoardLane::Inbox),
        TaskStatus::Triaged => Some(BoardLane::Triaged),
        TaskStatus::Queued => Some(BoardLane::Queued),
        TaskStatus::Running => Some(BoardLane::Running),
        TaskStatus::Done => Some(BoardLane::Done),
        TaskStatus::Archived
        | TaskStatus::Blocked
        | TaskStatus::WaitingUser
        | TaskStatus::ReadyForReview
        | TaskStatus::Reviewing
        | TaskStatus::NeedsChanges => None,
    }
}
