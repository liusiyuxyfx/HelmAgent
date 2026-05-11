use crate::domain::{TaskEvent, TaskRecord};
use anyhow::{bail, Context, Result};
use std::fmt::Write as _;
use std::fs::{self, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

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

    pub fn brief_path(&self, task_id: &str) -> PathBuf {
        self.session_dir(task_id).join("brief.md")
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

    pub fn write_brief(&self, task_id: &str, content: &str) -> Result<PathBuf> {
        let path = self.brief_path(task_id);
        let session_dir = self.ensure_session_dir(task_id)?;
        if path.parent() != Some(session_dir.as_path()) {
            bail!("brief path escaped session directory {}", path.display());
        }

        write_file_atomically(&path, content)
            .with_context(|| format!("write brief {}", path.display()))?;
        Ok(path)
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

    fn ensure_session_dir(&self, task_id: &str) -> Result<PathBuf> {
        let sessions_dir = self.root.join("sessions");
        fs::create_dir_all(&sessions_dir)
            .with_context(|| format!("create sessions directory {}", sessions_dir.display()))?;
        ensure_plain_directory(&sessions_dir, "sessions directory")?;

        let session_dir = self.session_dir(task_id);
        match fs::symlink_metadata(&session_dir) {
            Ok(metadata) => {
                if metadata.file_type().is_symlink() {
                    bail!(
                        "refuse to use symlink session directory {}",
                        session_dir.display()
                    );
                }
                if !metadata.is_dir() {
                    bail!(
                        "refuse to use non-directory session path {}",
                        session_dir.display()
                    );
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                fs::create_dir(&session_dir).with_context(|| {
                    format!("create session directory {}", session_dir.display())
                })?;
            }
            Err(error) => {
                return Err(error).with_context(|| {
                    format!("inspect session directory {}", session_dir.display())
                });
            }
        }

        let canonical_sessions = sessions_dir
            .canonicalize()
            .with_context(|| format!("canonicalize {}", sessions_dir.display()))?;
        let canonical_session = session_dir
            .canonicalize()
            .with_context(|| format!("canonicalize {}", session_dir.display()))?;
        if !canonical_session.starts_with(&canonical_sessions) {
            bail!(
                "session directory {} escapes {}",
                canonical_session.display(),
                canonical_sessions.display()
            );
        }

        Ok(session_dir)
    }
}

fn ensure_plain_directory(path: &Path, label: &str) -> Result<()> {
    let metadata =
        fs::symlink_metadata(path).with_context(|| format!("inspect {}", path.display()))?;
    if metadata.file_type().is_symlink() {
        bail!("refuse to use symlink {label} {}", path.display());
    }
    if !metadata.is_dir() {
        bail!("refuse to use non-directory {label} {}", path.display());
    }
    Ok(())
}

fn write_file_atomically(path: &Path, content: &str) -> Result<()> {
    let parent = path
        .parent()
        .with_context(|| format!("brief path has no parent {}", path.display()))?;
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .with_context(|| format!("brief path has invalid file name {}", path.display()))?;

    for attempt in 0..100 {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let temp_path = parent.join(format!(
            ".{file_name}.{}.{}.{}.tmp",
            std::process::id(),
            unique,
            attempt
        ));

        let mut file = match create_new_file_no_follow(&temp_path) {
            Ok(file) => file,
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => {
                return Err(error)
                    .with_context(|| format!("create temp brief {}", temp_path.display()));
            }
        };

        if let Err(error) = file.write_all(content.as_bytes()) {
            let _ = fs::remove_file(&temp_path);
            return Err(error).with_context(|| format!("write temp brief {}", temp_path.display()));
        }
        drop(file);

        if let Err(error) = fs::rename(&temp_path, path) {
            let _ = fs::remove_file(&temp_path);
            return Err(error).with_context(|| {
                format!(
                    "replace brief {} with {}",
                    path.display(),
                    temp_path.display()
                )
            });
        }

        return Ok(());
    }

    bail!("could not create unique temp file for {}", path.display())
}

#[cfg(unix)]
fn create_new_file_no_follow(path: &Path) -> std::io::Result<fs::File> {
    use std::os::unix::fs::OpenOptionsExt;

    OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o644)
        .custom_flags(libc::O_NOFOLLOW)
        .open(path)
}

#[cfg(not(unix))]
fn create_new_file_no_follow(path: &Path) -> std::io::Result<fs::File> {
    OpenOptions::new().write(true).create_new(true).open(path)
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
