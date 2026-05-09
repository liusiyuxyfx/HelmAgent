use crate::domain::{TaskEvent, TaskRecord};

pub fn task_status(task: &TaskRecord, events: &[TaskEvent]) -> String {
    let last_event = events
        .last()
        .map(|event| event.message.as_str())
        .unwrap_or(task.progress.last_event.as_str());

    format!(
        "{id} [{status:?}]\nTitle: {title}\nProject: {project}\nProgress: {progress}\nNext: {next}\n",
        id = task.id,
        status = task.status,
        title = task.title,
        project = task.project.path.display(),
        progress = last_event,
        next = task.progress.next_action,
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
