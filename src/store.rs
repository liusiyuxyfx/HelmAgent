use crate::domain::{TaskEvent, TaskRecord};
use anyhow::{Context, Result};
use std::fs::{self, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct TaskStore {
    root: PathBuf,
}

impl TaskStore {
    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }

    pub fn root(&self) -> &PathBuf {
        &self.root
    }

    pub fn task_path(&self, task_id: &str) -> PathBuf {
        let year = task_year(task_id);
        let task_component = safe_task_component(task_id);

        self.root
            .join("tasks")
            .join(year)
            .join(format!("{task_component}.yaml"))
    }

    pub fn session_dir(&self, task_id: &str) -> PathBuf {
        self.root
            .join("sessions")
            .join(safe_task_component(task_id))
    }

    pub fn events_path(&self, task_id: &str) -> PathBuf {
        self.session_dir(task_id).join("events.jsonl")
    }

    pub fn save_task(&self, task: &TaskRecord) -> Result<()> {
        let path = self.task_path(&task.id);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("create task directory {}", parent.display()))?;
        }

        let yaml = serde_yaml::to_string(task).context("serialize task record")?;
        fs::write(&path, yaml).with_context(|| format!("write task {}", path.display()))?;
        Ok(())
    }

    pub fn load_task(&self, task_id: &str) -> Result<TaskRecord> {
        let path = self.task_path(task_id);
        let content =
            fs::read_to_string(&path).with_context(|| format!("read task {}", path.display()))?;
        serde_yaml::from_str(&content).with_context(|| format!("parse task {}", path.display()))
    }

    pub fn append_event(&self, event: &TaskEvent) -> Result<()> {
        let path = self.events_path(&event.task_id);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("create session directory {}", parent.display()))?;
        }

        let line = serde_json::to_string(event).context("serialize task event")?;
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .with_context(|| format!("open events log {}", path.display()))?;
        writeln!(file, "{line}").with_context(|| format!("append event {}", path.display()))?;
        Ok(())
    }

    pub fn read_events(&self, task_id: &str) -> Result<Vec<TaskEvent>> {
        let path = self.events_path(task_id);
        if !path.exists() {
            return Ok(Vec::new());
        }

        let file = fs::File::open(&path).with_context(|| format!("open {}", path.display()))?;
        let reader = BufReader::new(file);
        let mut events = Vec::new();

        for (index, line) in reader.lines().enumerate() {
            let line_number = index + 1;
            let line =
                line.with_context(|| format!("read line {line_number} from {}", path.display()))?;
            if line.trim().is_empty() {
                continue;
            }
            events.push(serde_json::from_str(&line).with_context(|| {
                format!("parse event line {line_number} from {}", path.display())
            })?);
        }

        Ok(events)
    }
}

fn task_year(task_id: &str) -> &str {
    if !task_id.chars().all(is_safe_task_char) {
        return "unknown";
    }

    task_id
        .split('-')
        .nth(1)
        .and_then(|date| date.get(0..4))
        .filter(|year| year.len() == 4 && year.chars().all(|ch| ch.is_ascii_digit()))
        .unwrap_or("unknown")
}

fn safe_task_component(task_id: &str) -> String {
    let sanitized: String = task_id
        .chars()
        .map(|ch| if is_safe_task_char(ch) { ch } else { '_' })
        .collect();

    if sanitized.is_empty() {
        "unknown-task".to_string()
    } else {
        sanitized
    }
}

fn is_safe_task_char(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || ch == '-' || ch == '_'
}
