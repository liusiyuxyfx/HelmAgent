use helm_agent::domain::{TaskEvent, TaskRecord};
use helm_agent::paths::{helm_agent_home, HELM_AGENT_HOME_ENV};
use helm_agent::store::TaskStore;
use std::env;
use std::sync::Mutex;
use tempfile::tempdir;
use time::OffsetDateTime;

static ENV_LOCK: Mutex<()> = Mutex::new(());

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
fn helm_agent_home_uses_env_override() {
    let _guard = ENV_LOCK.lock().unwrap();
    let original = env::var_os(HELM_AGENT_HOME_ENV);
    let temp = tempdir().unwrap();

    env::set_var(HELM_AGENT_HOME_ENV, temp.path());
    let resolved = helm_agent_home().unwrap();

    match original {
        Some(value) => env::set_var(HELM_AGENT_HOME_ENV, value),
        None => env::remove_var(HELM_AGENT_HOME_ENV),
    }

    assert_eq!(resolved, temp.path());
}
