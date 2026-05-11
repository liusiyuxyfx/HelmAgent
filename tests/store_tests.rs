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
            .join("%2E%2E%2FPM-20260509-001%2Fescape.yaml")
    );
    assert_eq!(
        store.session_dir("/tmp/PM-20260509-001"),
        temp.path()
            .join("sessions")
            .join("%2Ftmp%2FPM-20260509-001")
    );
}

#[test]
fn unsafe_task_ids_do_not_alias_safe_ids() {
    let temp = tempdir().unwrap();
    let store = TaskStore::new(temp.path().to_path_buf());
    let now = OffsetDateTime::UNIX_EPOCH;
    let task = TaskRecord::new(
        "A/B".to_string(),
        "Unsafe id task".to_string(),
        "/repo".into(),
        now,
    );

    store.save_task(&task).unwrap();

    assert_ne!(store.task_path("A/B"), store.task_path("A_B"));
    assert_ne!(store.events_path("A/B"), store.events_path("A_B"));
    assert!(store.load_task("A_B").is_err());
}

#[test]
fn empty_task_id_does_not_alias_reserved_safe_id() {
    let temp = tempdir().unwrap();
    let store = TaskStore::new(temp.path().to_path_buf());
    let now = OffsetDateTime::UNIX_EPOCH;
    let task = TaskRecord::new(
        String::new(),
        "Empty id task".to_string(),
        "/repo".into(),
        now,
    );

    store.save_task(&task).unwrap();

    assert_eq!(
        store.task_path(""),
        temp.path().join("tasks").join("unknown").join("%00.yaml")
    );
    assert_ne!(store.task_path(""), store.task_path("unknown-task"));
    assert!(store.load_task("unknown-task").is_err());
}

#[test]
fn load_task_rejects_mismatched_record_id() {
    let temp = tempdir().unwrap();
    let store = TaskStore::new(temp.path().to_path_buf());
    let now = OffsetDateTime::UNIX_EPOCH;
    let task = TaskRecord::new(
        "A/B".to_string(),
        "Mismatched id task".to_string(),
        "/repo".into(),
        now,
    );
    let path = store.task_path("A_B");
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, serde_yaml::to_string(&task).unwrap()).unwrap();

    let error = store
        .load_task("A_B")
        .expect_err("mismatched record id should fail");
    let error_text = format!("{error:#}");

    assert!(error_text.contains("task id mismatch"), "{error_text}");
}

#[test]
fn read_events_rejects_mismatched_task_id() {
    let temp = tempdir().unwrap();
    let store = TaskStore::new(temp.path().to_path_buf());
    let event = TaskEvent::progress(
        "A/B".to_string(),
        "Wrong log".to_string(),
        OffsetDateTime::UNIX_EPOCH,
    );
    let path = store.events_path("A_B");
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(
        path,
        format!("{}\n", serde_json::to_string(&event).unwrap()),
    )
    .unwrap();

    let error = store
        .read_events("A_B")
        .expect_err("mismatched event task id should fail");
    let error_text = format!("{error:#}");

    assert!(
        error_text.contains("event task id mismatch"),
        "{error_text}"
    );
}

#[test]
fn list_tasks_returns_all_saved_tasks() {
    let temp = tempdir().unwrap();
    let store = TaskStore::new(temp.path().to_path_buf());
    let now = OffsetDateTime::UNIX_EPOCH;
    let first = TaskRecord::new(
        "PM-20260511-001".to_string(),
        "First task".to_string(),
        "/repo".into(),
        now,
    );
    let second = TaskRecord::new(
        "A/B".to_string(),
        "Unsafe id task".to_string(),
        "/repo".into(),
        now,
    );

    store.save_task(&first).unwrap();
    store.save_task(&second).unwrap();

    let mut tasks = store.list_tasks().unwrap();
    tasks.sort_by(|left, right| left.id.cmp(&right.id));

    assert_eq!(tasks.len(), 2);
    assert_eq!(tasks[0].id, "A/B");
    assert_eq!(tasks[1].id, "PM-20260511-001");
}

#[test]
fn list_tasks_rejects_record_from_unexpected_path() {
    let temp = tempdir().unwrap();
    let store = TaskStore::new(temp.path().to_path_buf());
    let now = OffsetDateTime::UNIX_EPOCH;
    let task = TaskRecord::new(
        "A/B".to_string(),
        "Wrong path".to_string(),
        "/repo".into(),
        now,
    );
    let wrong_path = store.task_path("A_B");
    fs::create_dir_all(wrong_path.parent().unwrap()).unwrap();
    fs::write(wrong_path, serde_yaml::to_string(&task).unwrap()).unwrap();

    let error = store.list_tasks().expect_err("mismatched path should fail");
    let error_text = format!("{error:#}");

    assert!(error_text.contains("task id mismatch"), "{error_text}");
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
