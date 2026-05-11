use crate::domain::{TaskEvent, TaskRecord};
use anyhow::{bail, Context, Result};
use std::fmt::Write as _;
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
        let task: TaskRecord = serde_yaml::from_str(&content)
            .with_context(|| format!("parse task {}", path.display()))?;

        if task.id != task_id {
            bail!(
                "task id mismatch: requested {task_id}, loaded {} from {}",
                task.id,
                path.display()
            );
        }

        Ok(task)
    }

    pub fn list_tasks(&self) -> Result<Vec<TaskRecord>> {
        let tasks_dir = self.root.join("tasks");
        if !tasks_dir.exists() {
            return Ok(Vec::new());
        }

        let mut tasks = Vec::new();
        for year_entry in fs::read_dir(&tasks_dir)
            .with_context(|| format!("read tasks directory {}", tasks_dir.display()))?
        {
            let year_entry =
                year_entry.with_context(|| format!("read entry from {}", tasks_dir.display()))?;
            let year_path = year_entry.path();
            if !year_path.is_dir() {
                continue;
            }

            for task_entry in fs::read_dir(&year_path)
                .with_context(|| format!("read task year directory {}", year_path.display()))?
            {
                let task_entry = task_entry
                    .with_context(|| format!("read entry from {}", year_path.display()))?;
                let path = task_entry.path();
                if path.extension().and_then(|value| value.to_str()) != Some("yaml") {
                    continue;
                }

                let content = fs::read_to_string(&path)
                    .with_context(|| format!("read task {}", path.display()))?;
                let task: TaskRecord = serde_yaml::from_str(&content)
                    .with_context(|| format!("parse task {}", path.display()))?;
                if self.task_path(&task.id) != path {
                    bail!(
                        "task id mismatch: loaded {} from unexpected path {}",
                        task.id,
                        path.display()
                    );
                }
                tasks.push(task);
            }
        }

        Ok(tasks)
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
            let event: TaskEvent = serde_json::from_str(&line).with_context(|| {
                format!("parse event line {line_number} from {}", path.display())
            })?;
            if event.task_id != task_id {
                bail!(
                    "event task id mismatch on line {line_number} from {}: requested {task_id}, loaded {}",
                    path.display(),
                    event.task_id
                );
            }
            events.push(event);
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
    let mut sanitized = String::new();
    for byte in task_id.bytes() {
        let ch = byte as char;
        if is_safe_task_char(ch) {
            sanitized.push(ch);
        } else {
            write!(&mut sanitized, "%{byte:02X}").expect("write to string");
        }
    }

    if sanitized.is_empty() {
        "%00".to_string()
    } else {
        sanitized
    }
}

fn is_safe_task_char(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || ch == '-' || ch == '_'
}
