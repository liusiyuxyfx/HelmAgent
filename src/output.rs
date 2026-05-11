use crate::domain::{TaskEvent, TaskRecord};

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
