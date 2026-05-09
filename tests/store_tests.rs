use helm_agent::domain::{TaskEvent, TaskRecord};
use helm_agent::paths::{helm_agent_home, HELM_AGENT_HOME_ENV};
use helm_agent::store::TaskStore;
use std::env;
use std::ffi::OsString;
use std::fs;
use std::sync::Mutex;
use tempfile::tempdir;
use time::OffsetDateTime;

static ENV_LOCK: Mutex<()> = Mutex::new(());

struct EnvRestore {
    key: &'static str,
    original: Option<OsString>,
}

impl EnvRestore {
    fn set_path(key: &'static str, value: &std::path::Path) -> Self {
        let original = env::var_os(key);
        env::set_var(key, value);
        Self { key, original }
    }
}

impl Drop for EnvRestore {
    fn drop(&mut self) {
        match &self.original {
            Some(value) => env::set_var(self.key, value),
            None => env::remove_var(self.key),
        }
    }
}

#[test]
fn save_and_load_task_record() {
    let temp = tempdir().unwrap();
    let store = TaskStore::new(temp.path().to_path_buf());
    let now = OffsetDateTime::UNIX_EPOCH;
    let task = TaskRecord::new(
        "PM-20260509-001".to_string(),
        "Fix login redirect bug".to_string(),
        "/repo".into(),
        now,
    );

    store.save_task(&task).unwrap();
    let loaded = store.load_task("PM-20260509-001").unwrap();

    assert_eq!(loaded.title, "Fix login redirect bug");
    assert_eq!(
        store.task_path("PM-20260509-001"),
        temp.path()
            .join("tasks")
            .join("2026")
            .join("PM-20260509-001.yaml")
    );
}

#[test]
fn append_and_read_events() {
    let temp = tempdir().unwrap();
    let store = TaskStore::new(temp.path().to_path_buf());
    let event = TaskEvent::progress(
        "PM-20260509-001".to_string(),
        "Found redirect handler".to_string(),
        OffsetDateTime::UNIX_EPOCH,
    );

    store.append_event(&event).unwrap();
    let events = store.read_events("PM-20260509-001").unwrap();

    assert_eq!(events.len(), 1);
    assert_eq!(events[0].message, "Found redirect handler");
}

#[test]
fn session_paths_are_rooted_by_task_id() {
    let temp = tempdir().unwrap();
    let store = TaskStore::new(temp.path().to_path_buf());

    assert_eq!(store.root(), &temp.path().to_path_buf());
    assert_eq!(
        store.session_dir("PM-20260509-001"),
        temp.path().join("sessions").join("PM-20260509-001")
    );
    assert_eq!(
        store.events_path("PM-20260509-001"),
        temp.path()
            .join("sessions")
            .join("PM-20260509-001")
            .join("events.jsonl")
    );
}

#[test]
fn unsafe_task_ids_are_sanitized_for_paths() {
    let temp = tempdir().unwrap();
    let store = TaskStore::new(temp.path().to_path_buf());

    assert_eq!(
        store.task_path("../PM-20260509-001/escape"),
        temp.path()
            .join("tasks")
            .join("unknown")
            .join("___PM-20260509-001_escape.yaml")
    );
    assert_eq!(
        store.session_dir("/tmp/PM-20260509-001"),
        temp.path().join("sessions").join("_tmp_PM-20260509-001")
    );
}

#[test]
fn event_parse_errors_include_line_number() {
    let temp = tempdir().unwrap();
    let store = TaskStore::new(temp.path().to_path_buf());
    let event = TaskEvent::progress(
        "PM-20260509-001".to_string(),
        "Found redirect handler".to_string(),
        OffsetDateTime::UNIX_EPOCH,
    );

    store.append_event(&event).unwrap();
    let valid = serde_json::to_string(&event).unwrap();
    fs::write(
        store.events_path("PM-20260509-001"),
        format!("{valid}\nnot-json\n"),
    )
    .unwrap();

    let error = store
        .read_events("PM-20260509-001")
        .expect_err("invalid jsonl should fail");
    let error_text = format!("{error:#}");

    assert!(error_text.contains("line 2"), "{error_text}");
}

#[test]
fn helm_agent_home_uses_env_override() {
    let _guard = ENV_LOCK.lock().unwrap();
    let temp = tempdir().unwrap();
    let _restore = EnvRestore::set_path(HELM_AGENT_HOME_ENV, temp.path());
    let resolved = helm_agent_home().unwrap();

    assert_eq!(resolved, temp.path());
}
