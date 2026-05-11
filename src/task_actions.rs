use crate::domain::{ReviewState, TaskEvent, TaskRecord, TaskStatus};
use crate::launcher::{Launcher, TmuxSessionState};
use crate::store::TaskStore;
use anyhow::{bail, Result};
use time::OffsetDateTime;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MarkAction {
    ReadyForReview,
    Blocked,
    Triaged,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReviewAction {
    Accept,
    RequestChanges(String),
}

pub fn record_event(
    store: &TaskStore,
    task_id: &str,
    event_type: impl Into<String>,
    message: impl Into<String>,
    now: OffsetDateTime,
) -> Result<TaskRecord> {
    let mut task = store.load_task(task_id)?;
    let event_type = event_type.into();
    let message = message.into();

    task.progress.last_event = message.clone();
    task.touch(now);
    store.save_task(&task)?;
    store.append_event(&TaskEvent::new(
        task_id.to_string(),
        event_type,
        message,
        now,
    ))?;
    Ok(task)
}

pub fn mark_task(
    store: &TaskStore,
    task_id: &str,
    action: MarkAction,
    message: impl Into<String>,
    now: OffsetDateTime,
) -> Result<TaskRecord> {
    let mut task = store.load_task(task_id)?;
    let message = message.into();
    let (status, event_type, next_action) = match action {
        MarkAction::ReadyForReview => {
            task.review.state = ReviewState::Required;
            task.progress.blocker = None;
            (
                TaskStatus::ReadyForReview,
                "ready_for_review",
                "Human review required",
            )
        }
        MarkAction::Blocked => {
            task.progress.blocker = Some(message.clone());
            (TaskStatus::Blocked, "blocked", "Resolve blocker")
        }
        MarkAction::Triaged => {
            task.progress.blocker = None;
            (TaskStatus::Triaged, "triaged", "Dispatch or defer task")
        }
    };

    task.status = status;
    task.progress.last_event = message.clone();
    task.progress.next_action = next_action.to_string();
    task.touch(now);
    store.save_task(&task)?;
    store.append_event(&TaskEvent::new(
        task_id.to_string(),
        event_type,
        message,
        now,
    ))?;
    Ok(task)
}

pub fn review_task(
    store: &TaskStore,
    task_id: &str,
    action: ReviewAction,
    now: OffsetDateTime,
) -> Result<TaskRecord> {
    let mut task = store.load_task(task_id)?;
    if !matches!(
        task.status,
        TaskStatus::ReadyForReview | TaskStatus::Reviewing
    ) {
        bail!(
            "cannot review {} with status {}",
            task_id,
            task.status.as_str()
        );
    }

    match action {
        ReviewAction::Accept => {
            task.status = TaskStatus::Done;
            task.review.state = ReviewState::Accepted;
            task.progress.last_event = "Review accepted".to_string();
            task.progress.next_action = "Archive task when ready".to_string();
            task.touch(now);
            store.save_task(&task)?;
            store.append_event(&TaskEvent::new(
                task_id.to_string(),
                "review_accepted",
                "Review accepted".to_string(),
                now,
            ))?;
        }
        ReviewAction::RequestChanges(message) => {
            task.status = TaskStatus::NeedsChanges;
            task.review.state = ReviewState::ChangesRequested;
            task.progress.last_event = message.clone();
            task.progress.next_action = "Dispatch follow-up changes".to_string();
            task.touch(now);
            store.save_task(&task)?;
            store.append_event(&TaskEvent::new(
                task_id.to_string(),
                "changes_requested",
                message,
                now,
            ))?;
        }
    }

    Ok(task)
}

pub fn sync_task(task: TaskRecord, store: &TaskStore, launcher: &Launcher) -> Result<String> {
    sync_task_at(task, store, launcher, OffsetDateTime::now_utc())
}

pub fn sync_task_at(
    mut task: TaskRecord,
    store: &TaskStore,
    launcher: &Launcher,
    now: OffsetDateTime,
) -> Result<String> {
    if matches!(task.status, TaskStatus::Done | TaskStatus::Archived) {
        return Ok(format!("{} skipped {}", task.id, task.status.as_str()));
    }

    let Some(session) = task.assignment.tmux_session.clone() else {
        return Ok(format!("{} no_session", task.id));
    };

    match launcher.session_state(&session)? {
        TmuxSessionState::Alive => {
            if matches!(
                task.status,
                TaskStatus::Queued | TaskStatus::Running | TaskStatus::Blocked
            ) {
                task.status = TaskStatus::Running;
                if task
                    .progress
                    .blocker
                    .as_deref()
                    .is_some_and(|blocker| blocker.starts_with("tmux session missing:"))
                {
                    task.progress.blocker = None;
                }
                task.progress.last_event = format!("tmux session alive: {session}");
                task.progress.next_action =
                    "Inspect child agent session or wait for review handoff".to_string();
                task.touch(now);
                store.save_task(&task)?;
                store.append_event(&TaskEvent::new(
                    task.id.clone(),
                    "sync_alive",
                    format!("tmux session alive: {session}"),
                    now,
                ))?;
            }
            Ok(format!("{} alive {}", task.id, session))
        }
        TmuxSessionState::Missing => {
            if matches!(task.status, TaskStatus::Running | TaskStatus::Blocked) {
                let message = format!("tmux session missing: {session}");
                task.status = TaskStatus::Blocked;
                task.progress.blocker = Some(message.clone());
                task.progress.last_event = message.clone();
                task.progress.next_action =
                    "Restart dispatch or inspect the task manually".to_string();
                task.touch(now);
                store.save_task(&task)?;
                store.append_event(&TaskEvent::new(
                    task.id.clone(),
                    "sync_missing",
                    format!("tmux session missing: {session}"),
                    now,
                ))?;
            }
            Ok(format!("{} missing {}", task.id, session))
        }
    }
}
