use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use time::OffsetDateTime;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    Inbox,
    Triaged,
    WaitingUser,
    Queued,
    Running,
    Blocked,
    ReadyForReview,
    Reviewing,
    NeedsChanges,
    Done,
    Archived,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RiskLevel {
    Low,
    Medium,
    High,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentRuntime {
    Claude,
    Codex,
    #[serde(rename = "opencode")]
    OpenCode,
}

impl AgentRuntime {
    pub fn as_str(self) -> &'static str {
        match self {
            AgentRuntime::Claude => "claude",
            AgentRuntime::Codex => "codex",
            AgentRuntime::OpenCode => "opencode",
        }
    }
}

impl TaskStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            TaskStatus::Inbox => "inbox",
            TaskStatus::Triaged => "triaged",
            TaskStatus::WaitingUser => "waiting_user",
            TaskStatus::Queued => "queued",
            TaskStatus::Running => "running",
            TaskStatus::Blocked => "blocked",
            TaskStatus::ReadyForReview => "ready_for_review",
            TaskStatus::Reviewing => "reviewing",
            TaskStatus::NeedsChanges => "needs_changes",
            TaskStatus::Done => "done",
            TaskStatus::Archived => "archived",
        }
    }
}

impl RiskLevel {
    pub fn as_str(self) -> &'static str {
        match self {
            RiskLevel::Low => "low",
            RiskLevel::Medium => "medium",
            RiskLevel::High => "high",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReviewState {
    NotRequired,
    Required,
    Accepted,
    ChangesRequested,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectRef {
    pub path: PathBuf,
    pub branch: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Assignment {
    pub runtime: Option<AgentRuntime>,
    pub workflow: Option<String>,
    pub tmux_session: Option<String>,
    pub native_session_id: Option<String>,
    pub acp_session_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Recovery {
    pub attach_command: Option<String>,
    pub resume_command: Option<String>,
    #[serde(default)]
    pub brief_path: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Progress {
    pub summary: String,
    pub last_event: String,
    pub next_action: String,
    pub blocker: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Review {
    pub state: ReviewState,
    pub reason: Option<String>,
    pub artifacts: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskRecord {
    pub id: String,
    pub title: String,
    pub status: TaskStatus,
    pub priority: String,
    pub risk: RiskLevel,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
    pub project: ProjectRef,
    pub assignment: Assignment,
    pub recovery: Recovery,
    pub progress: Progress,
    pub review: Review,
}

impl TaskRecord {
    pub fn new(id: String, title: String, project_path: PathBuf, now: OffsetDateTime) -> Self {
        Self {
            id,
            title,
            status: TaskStatus::Inbox,
            priority: "normal".to_string(),
            risk: RiskLevel::Low,
            created_at: now,
            updated_at: now,
            project: ProjectRef {
                path: project_path,
                branch: None,
            },
            assignment: Assignment {
                runtime: None,
                workflow: None,
                tmux_session: None,
                native_session_id: None,
                acp_session_id: None,
            },
            recovery: Recovery {
                attach_command: None,
                resume_command: None,
                brief_path: None,
            },
            progress: Progress {
                summary: "Created task".to_string(),
                last_event: "Task created".to_string(),
                next_action: "Triage task".to_string(),
                blocker: None,
            },
            review: Review {
                state: ReviewState::NotRequired,
                reason: None,
                artifacts: Vec::new(),
            },
        }
    }

    pub fn touch(&mut self, now: OffsetDateTime) {
        self.updated_at = now;
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskEvent {
    pub task_id: String,
    pub event_type: String,
    pub message: String,
    pub created_at: OffsetDateTime,
}

impl TaskEvent {
    pub fn new(
        task_id: String,
        event_type: impl Into<String>,
        message: String,
        created_at: OffsetDateTime,
    ) -> Self {
        Self {
            task_id,
            event_type: event_type.into(),
            message,
            created_at,
        }
    }

    pub fn progress(task_id: String, message: String, created_at: OffsetDateTime) -> Self {
        Self::new(task_id, "progress", message, created_at)
    }
}
