use helm_agent::domain::{AgentRuntime, TaskRecord, TaskStatus};
use helm_agent::store::TaskStore;
use helm_agent::task_actions::{self, MarkAction, ReviewAction};
use helm_agent::web_board;
use tempfile::tempdir;
use time::{Duration, OffsetDateTime};

fn task(id: &str, title: &str) -> TaskRecord {
    let mut task = TaskRecord::new(
        id.to_string(),
        title.to_string(),
        "/repo".into(),
        OffsetDateTime::UNIX_EPOCH,
    );
    task.status = TaskStatus::Running;
    task.assignment.runtime = Some(AgentRuntime::Codex);
    task.progress.last_event = "Started implementation".to_string();
    task.progress.next_action = "Run tests".to_string();
    task
}

#[test]
fn empty_board_renders_no_active_tasks_without_meta_refresh() {
    let html = web_board::render_task_board_html_with_refresh(&[], 15);

    assert!(html.contains("<!doctype html>"), "{html}");
    assert!(!html.contains(r#"http-equiv="refresh""#), "{html}");
    assert!(html.contains("No active tasks"), "{html}");
}

#[test]
fn interactive_board_html_contains_action_token_and_app_controls() {
    let html = web_board::render_task_board_html_with_token(
        &[task("PM-20260511-021", "Interactive board")],
        "test-token",
    );

    assert!(
        html.contains(r#"<meta name="helm-agent-action-token" content="test-token">"#),
        "{html}"
    );
    assert!(!html.contains(r#"http-equiv="refresh""#), "{html}");
    assert!(html.contains("data-helm-board-app"), "{html}");
    assert!(html.contains("Add Event"), "{html}");
    assert!(html.contains("Ready For Review"), "{html}");
    assert!(html.contains("Request Changes"), "{html}");
    assert!(html.contains("Sync"), "{html}");
    assert!(html.contains("data-status-filter"), "{html}");
    assert!(html.contains("showStatus = 'all'"), "{html}");
    assert!(html.contains("mutate('/event'"), "{html}");
    assert!(html.contains("mutate('/mark'"), "{html}");
    assert!(html.contains("mutate('/review'"), "{html}");
    assert!(html.contains("mutate('/sync'"), "{html}");
    assert!(html.contains("detail-runtime"), "{html}");
    assert!(html.contains("detail-review"), "{html}");
    assert!(html.contains("detail-brief"), "{html}");
    assert!(html.contains("detail-resume"), "{html}");
    assert!(html.contains("Copy Brief"), "{html}");
    assert!(html.contains("Copy Resume"), "{html}");
    assert!(html.contains("copyDetailText('detail-brief'"), "{html}");
    assert!(html.contains("copyDetailText('detail-resume'"), "{html}");
    assert!(html.contains("navigator.clipboard.writeText"), "{html}");
    assert!(html.contains("eventsEl.replaceChildren();"), "{html}");
    assert!(html.contains("if (id !== selectedId) return;"), "{html}");
}

#[test]
fn task_content_comes_from_text_board() {
    let html = web_board::render_task_board_html(&[task("PM-20260511-001", "Implement web board")]);

    assert!(html.contains("Running"), "{html}");
    assert!(html.contains("PM-20260511-001"), "{html}");
    assert!(html.contains("status=running"), "{html}");
    assert!(html.contains("runtime=codex"), "{html}");
    assert!(html.contains("Implement web board"), "{html}");
    assert!(html.contains("next: Run tests"), "{html}");
    assert!(html.contains("last: Started implementation"), "{html}");
}

#[test]
fn task_board_text_is_html_escaped() {
    let mut task = task(
        r#"PM-<1>&""#,
        r#"Escape <script>alert("x")</script> & 'quoted'"#,
    );
    task.project.path = r#"/tmp/<board>&""#.into();
    task.progress.next_action = r#"Check <next> & "quotes""#.to_string();
    task.progress.last_event = "Last 'event'".to_string();

    let html = web_board::render_task_board_html(&[task]);

    assert!(html.contains("PM-&lt;1&gt;&amp;&quot;"), "{html}");
    assert!(
        html.contains(
            "Escape &lt;script&gt;alert(&quot;x&quot;)&lt;/script&gt; &amp; &#39;quoted&#39;"
        ),
        "{html}"
    );
    assert!(html.contains("/tmp/&lt;board&gt;&amp;&quot;"), "{html}");
    assert!(
        html.contains("Check &lt;next&gt; &amp; &quot;quotes&quot;"),
        "{html}"
    );
    assert!(html.contains("Last &#39;event&#39;"), "{html}");
    assert!(!html.contains(r#"<script>alert("x")</script>"#), "{html}");
}

#[test]
fn http_response_wraps_board_html_as_no_store_html() {
    let body = "<!doctype html>\n<body>board</body>\n";
    let response = web_board::board_http_response(body);

    assert!(response.starts_with("HTTP/1.1 200 OK\r\n"), "{response}");
    assert!(
        response.contains("Content-Type: text/html; charset=utf-8\r\n"),
        "{response}"
    );
    assert!(
        response.contains("Cache-Control: no-store\r\n"),
        "{response}"
    );
    assert!(
        response.contains(&format!("Content-Length: {}\r\n", body.len())),
        "{response}"
    );
    assert!(response.ends_with(body), "{response}");
}

#[test]
fn board_request_host_must_be_loopback() {
    assert!(web_board::is_allowed_board_request_host(
        "GET / HTTP/1.1\r\nHost: localhost:8765\r\n\r\n"
    ));
    assert!(web_board::is_allowed_board_request_host(
        "GET / HTTP/1.1\r\nHost: 127.0.0.1:8765\r\n\r\n"
    ));
    assert!(web_board::is_allowed_board_request_host(
        "GET / HTTP/1.1\r\nHost: [::1]:8765\r\n\r\n"
    ));
    assert!(!web_board::is_allowed_board_request_host(
        "GET / HTTP/1.1\r\nHost: example.invalid:8765\r\n\r\n"
    ));
    assert!(!web_board::is_allowed_board_request_host(
        "GET / HTTP/1.1\r\n\r\n"
    ));
}

#[test]
fn board_serve_rejects_non_loopback_bind_hosts() {
    web_board::validate_loopback_bind_host("127.0.0.1", 8765).unwrap();
    web_board::validate_loopback_bind_host("localhost", 8765).unwrap();
    assert!(web_board::loopback_bind_address("localhost", 8765)
        .unwrap()
        .ip()
        .is_loopback());

    let err = web_board::validate_loopback_bind_host("0.0.0.0", 8765)
        .unwrap_err()
        .to_string();
    assert!(err.contains("only supports loopback hosts"), "{err}");
}

#[test]
fn forbidden_response_is_no_store_plain_text() {
    let response = web_board::forbidden_http_response();

    assert!(
        response.starts_with("HTTP/1.1 403 Forbidden\r\n"),
        "{response}"
    );
    assert!(
        response.contains("Content-Type: text/plain; charset=utf-8\r\n"),
        "{response}"
    );
    assert!(
        response.contains("Cache-Control: no-store\r\n"),
        "{response}"
    );
    assert!(response.ends_with("Forbidden\n"), "{response}");
}

#[test]
fn api_tasks_returns_active_tasks_as_json() {
    let home = tempdir().unwrap();
    let store = TaskStore::new(home.path().to_path_buf());
    let active = task("PM-20260511-031", "API task");
    let mut archived = task("PM-20260511-032", "Hidden archived task");
    archived.status = TaskStatus::Archived;
    store.save_task(&active).unwrap();
    store.save_task(&archived).unwrap();

    let response = web_board::handle_board_http_request(
        "GET /api/tasks HTTP/1.1\r\nHost: localhost:8765\r\n\r\n",
        &store,
        "token",
    );

    assert!(response.starts_with("HTTP/1.1 200 OK\r\n"), "{response}");
    assert!(
        response.contains("Content-Type: application/json; charset=utf-8\r\n"),
        "{response}"
    );
    assert!(response.contains(r#""ok":true"#), "{response}");
    assert!(response.contains("PM-20260511-031"), "{response}");
    assert!(!response.contains("PM-20260511-032"), "{response}");
}

#[test]
fn api_write_routes_reject_missing_action_token() {
    let home = tempdir().unwrap();
    let store = TaskStore::new(home.path().to_path_buf());
    store
        .save_task(&task("PM-20260511-033", "Reject write"))
        .unwrap();
    let body = r#"{"event_type":"progress","message":"Should fail"}"#;
    let request = format!(
        "POST /api/tasks/PM-20260511-033/event HTTP/1.1\r\nHost: localhost:8765\r\nContent-Length: {}\r\n\r\n{}",
        body.len(),
        body
    );

    let response = web_board::handle_board_http_request(&request, &store, "token");

    assert!(
        response.starts_with("HTTP/1.1 403 Forbidden\r\n"),
        "{response}"
    );
    assert!(response.contains("invalid action token"), "{response}");
}

#[test]
fn api_write_routes_reject_wrong_action_token() {
    let home = tempdir().unwrap();
    let store = TaskStore::new(home.path().to_path_buf());
    store
        .save_task(&task("PM-20260511-040", "Reject wrong token"))
        .unwrap();
    let body = r#"{"event_type":"progress","message":"Should fail"}"#;
    let request = format!(
        "POST /api/tasks/PM-20260511-040/event HTTP/1.1\r\nHost: localhost:8765\r\nX-Helm-Agent-Token: wrong\r\nContent-Length: {}\r\n\r\n{}",
        body.len(),
        body
    );

    let response = web_board::handle_board_http_request(&request, &store, "token");

    assert!(
        response.starts_with("HTTP/1.1 403 Forbidden\r\n"),
        "{response}"
    );
    assert!(response.contains("invalid action token"), "{response}");
}

#[test]
fn api_events_returns_task_event_log() {
    let home = tempdir().unwrap();
    let store = TaskStore::new(home.path().to_path_buf());
    store
        .save_task(&task("PM-20260511-041", "Events via API"))
        .unwrap();
    store
        .append_event(&helm_agent::domain::TaskEvent::progress(
            "PM-20260511-041".to_string(),
            "First event".to_string(),
            OffsetDateTime::UNIX_EPOCH + Duration::seconds(1),
        ))
        .unwrap();

    let response = web_board::handle_board_http_request(
        "GET /api/tasks/PM-20260511-041/events HTTP/1.1\r\nHost: localhost:8765\r\n\r\n",
        &store,
        "token",
    );

    assert!(response.starts_with("HTTP/1.1 200 OK\r\n"), "{response}");
    assert!(response.contains(r#""ok":true"#), "{response}");
    assert!(response.contains("First event"), "{response}");
}

#[test]
fn api_event_records_progress_with_valid_token() {
    let home = tempdir().unwrap();
    let store = TaskStore::new(home.path().to_path_buf());
    store
        .save_task(&task("PM-20260511-034", "Record via API"))
        .unwrap();
    let body = r#"{"event_type":"progress","message":"API progress"}"#;
    let request = format!(
        "POST /api/tasks/PM-20260511-034/event HTTP/1.1\r\nHost: localhost:8765\r\nX-Helm-Agent-Token: token\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
        body.len(),
        body
    );

    let response = web_board::handle_board_http_request(&request, &store, "token");

    assert!(response.starts_with("HTTP/1.1 200 OK\r\n"), "{response}");
    assert!(response.contains(r#""ok":true"#), "{response}");
    let updated = store.load_task("PM-20260511-034").unwrap();
    assert_eq!(updated.progress.last_event, "API progress");
    let events = store.read_events("PM-20260511-034").unwrap();
    assert_eq!(events[0].event_type, "progress");
}

#[test]
fn api_routes_decode_encoded_task_ids() {
    let home = tempdir().unwrap();
    let store = TaskStore::new(home.path().to_path_buf());
    store
        .save_task(&task("PM/Space 037", "Encoded id"))
        .unwrap();
    let body = r#"{"event_type":"progress","message":"Decoded id works"}"#;
    let request = format!(
        "POST /api/tasks/PM%2FSpace%20037/event HTTP/1.1\r\nHost: localhost:8765\r\nX-Helm-Agent-Token: token\r\nContent-Length: {}\r\n\r\n{}",
        body.len(),
        body
    );

    let response = web_board::handle_board_http_request(&request, &store, "token");

    assert!(response.starts_with("HTTP/1.1 200 OK\r\n"), "{response}");
    let updated = store.load_task("PM/Space 037").unwrap();
    assert_eq!(updated.progress.last_event, "Decoded id works");
}

#[test]
fn api_mark_ready_for_review_updates_review_state() {
    let home = tempdir().unwrap();
    let store = TaskStore::new(home.path().to_path_buf());
    store
        .save_task(&task("PM-20260511-042", "Ready via API"))
        .unwrap();
    let body = r#"{"action":"ready_for_review","message":"Ready now"}"#;
    let request = format!(
        "POST /api/tasks/PM-20260511-042/mark HTTP/1.1\r\nHost: localhost:8765\r\nX-Helm-Agent-Token: token\r\nContent-Length: {}\r\n\r\n{}",
        body.len(),
        body
    );

    let response = web_board::handle_board_http_request(&request, &store, "token");

    assert!(response.starts_with("HTTP/1.1 200 OK\r\n"), "{response}");
    let updated = store.load_task("PM-20260511-042").unwrap();
    assert_eq!(updated.status, TaskStatus::ReadyForReview);
    assert_eq!(
        updated.review.state,
        helm_agent::domain::ReviewState::Required
    );
    assert_eq!(updated.progress.last_event, "Ready now");
}

#[test]
fn api_mark_triaged_moves_task_back_to_triage() {
    let home = tempdir().unwrap();
    let store = TaskStore::new(home.path().to_path_buf());
    let mut record = task("PM-20260511-043", "Triaged via API");
    record.status = TaskStatus::Blocked;
    record.progress.blocker = Some("Old blocker".to_string());
    store.save_task(&record).unwrap();
    let body = r#"{"action":"triaged","message":"Needs another pass"}"#;
    let request = format!(
        "POST /api/tasks/PM-20260511-043/mark HTTP/1.1\r\nHost: localhost:8765\r\nX-Helm-Agent-Token: token\r\nContent-Length: {}\r\n\r\n{}",
        body.len(),
        body
    );

    let response = web_board::handle_board_http_request(&request, &store, "token");

    assert!(response.starts_with("HTTP/1.1 200 OK\r\n"), "{response}");
    let updated = store.load_task("PM-20260511-043").unwrap();
    assert_eq!(updated.status, TaskStatus::Triaged);
    assert_eq!(updated.progress.blocker, None);
    assert_eq!(updated.progress.next_action, "Dispatch or defer task");
}

#[test]
fn api_mark_blocked_updates_status_and_blocker() {
    let home = tempdir().unwrap();
    let store = TaskStore::new(home.path().to_path_buf());
    store
        .save_task(&task("PM-20260511-035", "Mark via API"))
        .unwrap();
    let body = r#"{"action":"blocked","message":"Need credentials"}"#;
    let request = format!(
        "POST /api/tasks/PM-20260511-035/mark HTTP/1.1\r\nHost: localhost:8765\r\nX-Helm-Agent-Token: token\r\nContent-Length: {}\r\n\r\n{}",
        body.len(),
        body
    );

    let response = web_board::handle_board_http_request(&request, &store, "token");

    assert!(response.starts_with("HTTP/1.1 200 OK\r\n"), "{response}");
    let updated = store.load_task("PM-20260511-035").unwrap();
    assert_eq!(updated.status, TaskStatus::Blocked);
    assert_eq!(
        updated.progress.blocker.as_deref(),
        Some("Need credentials")
    );
}

#[test]
fn api_review_accept_finishes_reviewable_task() {
    let home = tempdir().unwrap();
    let store = TaskStore::new(home.path().to_path_buf());
    let mut task = task("PM-20260511-036", "Review via API");
    task.status = TaskStatus::ReadyForReview;
    store.save_task(&task).unwrap();
    let body = r#"{"action":"accept"}"#;
    let request = format!(
        "POST /api/tasks/PM-20260511-036/review HTTP/1.1\r\nHost: localhost:8765\r\nX-Helm-Agent-Token: token\r\nContent-Length: {}\r\n\r\n{}",
        body.len(),
        body
    );

    let response = web_board::handle_board_http_request(&request, &store, "token");

    assert!(response.starts_with("HTTP/1.1 200 OK\r\n"), "{response}");
    let updated = store.load_task("PM-20260511-036").unwrap();
    assert_eq!(updated.status, TaskStatus::Done);
}

#[test]
fn api_review_request_changes_records_change_request() {
    let home = tempdir().unwrap();
    let store = TaskStore::new(home.path().to_path_buf());
    let mut task = task("PM-20260511-038", "Request changes via API");
    task.status = TaskStatus::ReadyForReview;
    store.save_task(&task).unwrap();
    let body = r#"{"action":"request_changes","message":"Add regression test"}"#;
    let request = format!(
        "POST /api/tasks/PM-20260511-038/review HTTP/1.1\r\nHost: localhost:8765\r\nX-Helm-Agent-Token: token\r\nContent-Length: {}\r\n\r\n{}",
        body.len(),
        body
    );

    let response = web_board::handle_board_http_request(&request, &store, "token");

    assert!(response.starts_with("HTTP/1.1 200 OK\r\n"), "{response}");
    let updated = store.load_task("PM-20260511-038").unwrap();
    assert_eq!(updated.status, TaskStatus::NeedsChanges);
    assert_eq!(updated.progress.last_event, "Add regression test");
    assert_eq!(updated.progress.next_action, "Dispatch follow-up changes");
    let events = store.read_events("PM-20260511-038").unwrap();
    assert_eq!(events[0].event_type, "changes_requested");
    assert_eq!(events[0].message, "Add regression test");
}

#[test]
fn api_sync_returns_no_session_result_for_unsessioned_task() {
    let home = tempdir().unwrap();
    let store = TaskStore::new(home.path().to_path_buf());
    store
        .save_task(&task("PM-20260511-039", "Sync via API"))
        .unwrap();
    let request = "POST /api/tasks/PM-20260511-039/sync HTTP/1.1\r\nHost: localhost:8765\r\nX-Helm-Agent-Token: token\r\nContent-Length: 2\r\n\r\n{}";

    let response = web_board::handle_board_http_request(request, &store, "token");

    assert!(response.starts_with("HTTP/1.1 200 OK\r\n"), "{response}");
    assert!(
        response.contains(r#""result":"PM-20260511-039 no_session""#),
        "{response}"
    );
}

#[test]
fn loaded_board_tasks_hide_archived_and_sort_newest_first() {
    let home = tempdir().unwrap();
    let store = TaskStore::new(home.path().to_path_buf());
    let mut older = task("PM-20260511-001", "Older task");
    let mut newer = task("PM-20260511-002", "Newer task");
    let mut archived = task("PM-20260511-003", "Archived task");
    older.updated_at = OffsetDateTime::UNIX_EPOCH + Duration::seconds(10);
    newer.updated_at = OffsetDateTime::UNIX_EPOCH + Duration::seconds(20);
    archived.updated_at = OffsetDateTime::UNIX_EPOCH + Duration::seconds(30);
    archived.status = TaskStatus::Archived;

    store.save_task(&older).unwrap();
    store.save_task(&newer).unwrap();
    store.save_task(&archived).unwrap();

    let tasks = web_board::load_task_board_tasks(&store).unwrap();

    assert_eq!(
        tasks
            .iter()
            .map(|task| task.id.as_str())
            .collect::<Vec<_>>(),
        vec!["PM-20260511-002", "PM-20260511-001"]
    );
}

#[test]
fn task_action_record_event_updates_last_event_and_appends_log() {
    let home = tempdir().unwrap();
    let store = TaskStore::new(home.path().to_path_buf());
    let task = task("PM-20260511-011", "Record event");
    store.save_task(&task).unwrap();

    let updated = task_actions::record_event(
        &store,
        "PM-20260511-011",
        "progress",
        "Tests are running",
        OffsetDateTime::UNIX_EPOCH + Duration::seconds(40),
    )
    .unwrap();

    assert_eq!(updated.progress.last_event, "Tests are running");
    assert_eq!(
        updated.updated_at,
        OffsetDateTime::UNIX_EPOCH + Duration::seconds(40)
    );
    let events = store.read_events("PM-20260511-011").unwrap();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].event_type, "progress");
    assert_eq!(events[0].message, "Tests are running");
}

#[test]
fn task_action_mark_blocked_sets_blocker_and_status() {
    let home = tempdir().unwrap();
    let store = TaskStore::new(home.path().to_path_buf());
    let task = task("PM-20260511-012", "Blocked task");
    store.save_task(&task).unwrap();

    let updated = task_actions::mark_task(
        &store,
        "PM-20260511-012",
        MarkAction::Blocked,
        "Waiting for credentials",
        OffsetDateTime::UNIX_EPOCH + Duration::seconds(50),
    )
    .unwrap();

    assert_eq!(updated.status, TaskStatus::Blocked);
    assert_eq!(
        updated.progress.blocker.as_deref(),
        Some("Waiting for credentials")
    );
    assert_eq!(updated.progress.next_action, "Resolve blocker");
    let events = store.read_events("PM-20260511-012").unwrap();
    assert_eq!(events[0].event_type, "blocked");
}

#[test]
fn task_action_review_accept_requires_reviewable_status_and_finishes_task() {
    let home = tempdir().unwrap();
    let store = TaskStore::new(home.path().to_path_buf());
    let mut task = task("PM-20260511-013", "Review task");
    task.status = TaskStatus::ReadyForReview;
    store.save_task(&task).unwrap();

    let updated = task_actions::review_task(
        &store,
        "PM-20260511-013",
        ReviewAction::Accept,
        OffsetDateTime::UNIX_EPOCH + Duration::seconds(60),
    )
    .unwrap();

    assert_eq!(updated.status, TaskStatus::Done);
    assert_eq!(updated.progress.last_event, "Review accepted");
    assert_eq!(updated.progress.next_action, "Archive task when ready");
    let events = store.read_events("PM-20260511-013").unwrap();
    assert_eq!(events[0].event_type, "review_accepted");
}
