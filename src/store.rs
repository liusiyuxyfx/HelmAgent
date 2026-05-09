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
        let year = task_id
            .split('-')
            .nth(1)
            .and_then(|date| date.get(0..4))
            .unwrap_or("unknown");

        self.root
            .join("tasks")
            .join(year)
            .join(format!("{task_id}.yaml"))
    }

    pub fn session_dir(&self, task_id: &str) -> PathBuf {
        self.root.join("sessions").join(task_id)
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

        for line in reader.lines() {
            let line = line.with_context(|| format!("read line from {}", path.display()))?;
            if line.trim().is_empty() {
                continue;
            }
            events.push(
                serde_json::from_str(&line)
                    .with_context(|| format!("parse event from {}", path.display()))?,
            );
        }

        Ok(events)
    }
}
