use helm_agent::domain::{AgentRuntime, TaskRecord, TaskStatus};
use helm_agent::store::TaskStore;
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
fn empty_board_renders_no_active_tasks_with_refresh() {
    let html = web_board::render_task_board_html_with_refresh(&[], 15);

    assert!(html.contains("<!doctype html>"), "{html}");
    assert!(
        html.contains(r#"<meta http-equiv="refresh" content="15">"#),
        "{html}"
    );
    assert!(html.contains("No active tasks"), "{html}");
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
    assert!(!html.contains("<script>"), "{html}");
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
