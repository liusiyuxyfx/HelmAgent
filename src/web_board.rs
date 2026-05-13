use crate::{
    domain::{TaskRecord, TaskStatus},
    launcher::Launcher,
    output,
    store::TaskStore,
    task_actions::{self, MarkAction, ReviewAction},
};
use anyhow::{bail, Context, Result};
use serde::Deserialize;
use serde_json::json;
use std::fs::File;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream, ToSocketAddrs};
use std::time::{SystemTime, UNIX_EPOCH};
use time::OffsetDateTime;

pub const DEFAULT_REFRESH_SECONDS: u64 = 5;
const ACTION_TOKEN_HEADER: &str = "x-helm-agent-token";

pub fn render_task_board_html(tasks: &[TaskRecord]) -> String {
    render_task_board_html_with_refresh(tasks, DEFAULT_REFRESH_SECONDS)
}

pub fn render_task_board_html_with_refresh(tasks: &[TaskRecord], refresh_seconds: u64) -> String {
    render_task_board_html_document(tasks, "", refresh_seconds)
}

pub fn render_task_board_html_with_token(tasks: &[TaskRecord], action_token: &str) -> String {
    render_task_board_html_document(tasks, action_token, DEFAULT_REFRESH_SECONDS)
}

fn render_task_board_html_document(
    tasks: &[TaskRecord],
    action_token: &str,
    refresh_seconds: u64,
) -> String {
    let board = output::task_board(tasks);
    let escaped_board = escape_html(&board);
    let escaped_token = escape_html(action_token);
    let html = r#"<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<meta name="helm-agent-action-token" content="{action_token}">
<title>HelmAgent Board</title>
<style>
:root {
  color-scheme: light;
  --bg: #f6f7f9;
  --panel: #ffffff;
  --text: #17202a;
  --muted: #617182;
  --line: #d8dee6;
  --accent: #1f6feb;
  --danger: #b42318;
  --ok: #16794c;
}
* { box-sizing: border-box; }
body {
  margin: 0;
  min-height: 100vh;
  background: var(--bg);
  color: var(--text);
  font-family: ui-sans-serif, system-ui, -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif;
}
button, input, select, textarea {
  font: inherit;
}
button {
  min-height: 2rem;
  border: 1px solid var(--line);
  border-radius: 6px;
  background: #fff;
  color: var(--text);
  padding: 0.35rem 0.6rem;
  cursor: pointer;
}
button.primary { background: var(--accent); border-color: var(--accent); color: #fff; }
button.danger { color: var(--danger); }
button.ok { color: var(--ok); }
button:disabled { cursor: not-allowed; opacity: 0.55; }
.shell { display: grid; grid-template-rows: auto 1fr; min-height: 100vh; }
.topbar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 1rem;
  padding: 0.85rem 1rem;
  border-bottom: 1px solid var(--line);
  background: var(--panel);
}
.title { display: flex; align-items: baseline; gap: 0.75rem; min-width: 0; }
h1 { margin: 0; font-size: 1.15rem; line-height: 1.2; letter-spacing: 0; }
.status { color: var(--muted); font-size: 0.88rem; white-space: nowrap; }
.top-actions { display: flex; align-items: center; flex-wrap: wrap; gap: 0.5rem; }
.filters { display: flex; align-items: center; flex-wrap: wrap; gap: 0.35rem; }
.filters button[aria-pressed="true"] { border-color: var(--accent); color: var(--accent); }
.layout {
  display: grid;
  grid-template-columns: minmax(0, 1fr) minmax(20rem, 25rem);
  gap: 1rem;
  padding: 1rem;
  min-height: 0;
}
.lanes {
  display: grid;
  grid-template-columns: repeat(4, minmax(14rem, 1fr));
  gap: 0.75rem;
  align-content: start;
  min-width: 0;
}
.lane {
  min-height: 10rem;
  background: var(--panel);
  border: 1px solid var(--line);
  border-radius: 8px;
  overflow: hidden;
}
.lane h2 {
  margin: 0;
  padding: 0.65rem 0.75rem;
  font-size: 0.9rem;
  border-bottom: 1px solid var(--line);
}
.task-list { display: grid; gap: 0.45rem; padding: 0.55rem; }
.task-row {
  width: 100%;
  min-height: 5rem;
  display: grid;
  gap: 0.35rem;
  text-align: left;
  border-radius: 6px;
  border: 1px solid var(--line);
  background: #fff;
  padding: 0.55rem;
}
.task-row[aria-current="true"] { border-color: var(--accent); box-shadow: inset 3px 0 0 var(--accent); }
.task-id { font-size: 0.78rem; color: var(--muted); overflow-wrap: anywhere; }
.task-title { font-weight: 650; line-height: 1.25; overflow-wrap: anywhere; }
.task-meta { display: flex; flex-wrap: wrap; gap: 0.35rem; color: var(--muted); font-size: 0.75rem; }
.detail {
  min-width: 0;
  background: var(--panel);
  border: 1px solid var(--line);
  border-radius: 8px;
  padding: 0.85rem;
  align-self: start;
  display: grid;
  gap: 0.75rem;
}
.detail h2 { margin: 0; font-size: 1rem; letter-spacing: 0; }
.field { display: grid; gap: 0.25rem; }
.field-heading { display: flex; align-items: center; justify-content: space-between; gap: 0.5rem; }
.field label { color: var(--muted); font-size: 0.78rem; }
.field button {
  min-height: 1.6rem;
  padding: 0.18rem 0.45rem;
  font-size: 0.75rem;
}
.field div { overflow-wrap: anywhere; }
.actions { display: grid; gap: 0.6rem; }
.action-row { display: grid; grid-template-columns: 1fr auto; gap: 0.45rem; }
.action-grid { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: 0.45rem; }
textarea, input {
  width: 100%;
  min-width: 0;
  border: 1px solid var(--line);
  border-radius: 6px;
  background: #fff;
  color: var(--text);
  padding: 0.45rem 0.55rem;
}
textarea { min-height: 4.5rem; resize: vertical; }
.events { display: grid; gap: 0.35rem; max-height: 14rem; overflow: auto; border-top: 1px solid var(--line); padding-top: 0.6rem; }
.event { font-size: 0.8rem; color: var(--muted); overflow-wrap: anywhere; }
.text-board { margin: 0 1rem 1rem; white-space: pre-wrap; word-break: break-word; }
details { margin: 0 1rem 1rem; }
summary { cursor: pointer; color: var(--muted); }
@media (max-width: 920px) {
  .layout { grid-template-columns: 1fr; }
  .lanes { grid-template-columns: repeat(2, minmax(0, 1fr)); }
}
@media (max-width: 560px) {
  .topbar { align-items: flex-start; flex-direction: column; }
  .lanes { grid-template-columns: 1fr; }
  .layout { padding: 0.75rem; }
  .action-grid { grid-template-columns: 1fr; }
}
</style>
</head>
<body>
<main class="shell" data-helm-board-app>
<header class="topbar">
  <div class="title">
    <h1>HelmAgent Board</h1>
    <span id="board-status" class="status">Loading</span>
  </div>
  <div class="top-actions">
    <div class="filters" aria-label="Task filters">
      <button type="button" data-status-filter="all" aria-pressed="true">All</button>
      <button type="button" data-status-filter="running">Running</button>
      <button type="button" data-status-filter="blocked">Blocked</button>
      <button type="button" data-status-filter="review">Review</button>
      <button type="button" data-status-filter="queued">Queued</button>
    </div>
    <button id="refresh-button" type="button">Refresh</button>
  </div>
</header>
<section class="layout">
  <div id="lanes" class="lanes" aria-live="polite"></div>
  <aside class="detail" aria-live="polite">
    <h2 id="detail-title">Select a task</h2>
    <div class="field"><label>Task</label><div id="detail-id">-</div></div>
    <div class="field"><label>Project</label><div id="detail-project">-</div></div>
    <div class="field"><label>Runtime</label><div id="detail-runtime">-</div></div>
    <div class="field"><label>Review</label><div id="detail-review">-</div></div>
    <div class="field"><label>Last</label><div id="detail-last">-</div></div>
    <div class="field"><label>Next</label><div id="detail-next">-</div></div>
    <div class="field">
      <div class="field-heading"><label>Brief</label><button id="copy-brief-button" type="button">Copy Brief</button></div>
      <div id="detail-brief">-</div>
    </div>
    <div class="field">
      <div class="field-heading"><label>Resume</label><button id="copy-resume-button" type="button">Copy Resume</button></div>
      <div id="detail-resume">-</div>
    </div>
    <div class="actions">
      <div class="action-row">
        <input id="event-message" placeholder="Progress message">
        <button id="event-button" class="primary" type="button">Add Event</button>
      </div>
      <div class="action-row">
        <input id="block-message" placeholder="Blocker message">
        <button id="block-button" class="danger" type="button">Block</button>
      </div>
      <div class="action-grid">
        <button id="ready-button" type="button">Ready For Review</button>
        <button id="triage-button" type="button">Triaged</button>
        <button id="accept-button" class="ok" type="button">Accept Review</button>
        <button id="sync-button" type="button">Sync</button>
      </div>
      <div class="action-row">
        <input id="changes-message" placeholder="Change request">
        <button id="changes-button" type="button">Request Changes</button>
      </div>
    </div>
    <div id="events" class="events"></div>
  </aside>
</section>
<details>
  <summary>Text Board</summary>
  <pre class="text-board">{escaped_board}</pre>
</details>
</main>
<script>
const token = document.querySelector('meta[name="helm-agent-action-token"]').content;
const lanesEl = document.getElementById('lanes');
const statusEl = document.getElementById('board-status');
let tasks = [];
let selectedId = null;
let showStatus = 'all';

const laneDefs = [
  ['Blocked', task => task.status === 'blocked' || task.status === 'waiting_user'],
  ['Review', task => task.review.state === 'required' || ['ready_for_review', 'reviewing', 'needs_changes'].includes(task.status)],
  ['Running', task => task.status === 'running'],
  ['Queued', task => task.status === 'queued'],
  ['Triaged', task => task.status === 'triaged'],
  ['Inbox', task => task.status === 'inbox'],
  ['Done', task => task.status === 'done']
];

function text(value) {
  return value === null || value === undefined || value === '' ? '-' : String(value);
}

async function api(path, options = {}) {
  const response = await fetch(path, {
    ...options,
    headers: {
      'Content-Type': 'application/json',
      ...(options.headers || {}),
      ...(options.method === 'POST' ? { 'X-Helm-Agent-Token': token } : {})
    }
  });
  const payload = await response.json();
  if (!response.ok || payload.ok === false) {
    throw new Error(payload.error || `HTTP ${response.status}`);
  }
  return payload;
}

async function loadTasks() {
  statusEl.textContent = 'Loading';
  const payload = await api('/api/tasks');
  tasks = payload.tasks;
  const visibleTasks = filteredTasks();
  if (!selectedId && visibleTasks.length > 0) selectedId = visibleTasks[0].id;
  if (selectedId && !visibleTasks.some(task => task.id === selectedId)) {
    selectedId = visibleTasks.length > 0 ? visibleTasks[0].id : null;
  }
  render();
  statusEl.textContent = `${visibleTasks.length} shown / ${tasks.length} active`;
}

function laneFor(task) {
  const found = laneDefs.find(([, predicate]) => predicate(task));
  return found ? found[0] : null;
}

function matchesFilter(task) {
  if (showStatus === 'all') return true;
  if (showStatus === 'review') {
    return task.review.state === 'required' || ['ready_for_review', 'reviewing', 'needs_changes'].includes(task.status);
  }
  if (showStatus === 'blocked') return task.status === 'blocked' || task.status === 'waiting_user';
  return task.status === showStatus;
}

function filteredTasks() {
  return tasks.filter(matchesFilter);
}

function render() {
  lanesEl.replaceChildren();
  const visibleTasks = filteredTasks();
  document.querySelectorAll('[data-status-filter]').forEach(button => {
    button.setAttribute('aria-pressed', button.dataset.statusFilter === showStatus ? 'true' : 'false');
  });
  for (const [title] of laneDefs) {
    const laneTasks = visibleTasks.filter(task => laneFor(task) === title);
    if (laneTasks.length === 0) continue;
    const lane = document.createElement('section');
    lane.className = 'lane';
    const heading = document.createElement('h2');
    heading.textContent = `${title} ${laneTasks.length}`;
    const list = document.createElement('div');
    list.className = 'task-list';
    for (const task of laneTasks) list.appendChild(taskButton(task));
    lane.append(heading, list);
    lanesEl.appendChild(lane);
  }
  if (lanesEl.children.length === 0) {
    const empty = document.createElement('section');
    empty.className = 'lane';
    empty.innerHTML = '<h2>No active tasks</h2>';
    lanesEl.appendChild(empty);
  }
  renderDetail();
}

function taskButton(task) {
  const button = document.createElement('button');
  button.type = 'button';
  button.className = 'task-row';
  button.setAttribute('aria-current', task.id === selectedId ? 'true' : 'false');
  button.addEventListener('click', () => {
    selectedId = task.id;
    render();
  });
  const id = document.createElement('div');
  id.className = 'task-id';
  id.textContent = task.id;
  const title = document.createElement('div');
  title.className = 'task-title';
  title.textContent = task.title;
  const meta = document.createElement('div');
  meta.className = 'task-meta';
  meta.textContent = `${task.status} - ${task.risk} - ${task.assignment.runtime || '-'}`;
  button.append(id, title, meta);
  return button;
}

function selectedTask() {
  return tasks.find(task => task.id === selectedId) || null;
}

function renderDetail() {
  const task = selectedTask();
  const eventsEl = document.getElementById('events');
  document.querySelectorAll('.actions button, .actions input').forEach(el => { el.disabled = !task; });
  document.getElementById('copy-brief-button').disabled = !task || !task.recovery.brief_path;
  document.getElementById('copy-resume-button').disabled = !task || !task.recovery.resume_command;
  document.getElementById('detail-title').textContent = task ? task.title : 'Select a task';
  document.getElementById('detail-id').textContent = task ? task.id : '-';
  document.getElementById('detail-project').textContent = task ? task.project.path : '-';
  document.getElementById('detail-runtime').textContent = task ? text(task.assignment.runtime) : '-';
  document.getElementById('detail-review').textContent = task ? text(task.review.state) : '-';
  document.getElementById('detail-last').textContent = task ? text(task.progress.last_event) : '-';
  document.getElementById('detail-next').textContent = task ? text(task.progress.next_action) : '-';
  document.getElementById('detail-brief').textContent = task ? text(task.recovery.brief_path) : '-';
  document.getElementById('detail-resume').textContent = task ? text(task.recovery.resume_command) : '-';
  if (task) {
    loadEvents(task.id);
  } else {
    eventsEl.replaceChildren();
  }
}

async function loadEvents(id) {
  const eventsEl = document.getElementById('events');
  try {
    const payload = await api(`/api/tasks/${encodeURIComponent(id)}/events`);
    if (id !== selectedId) return;
    eventsEl.replaceChildren(...payload.events.slice(-30).reverse().map(event => {
      const row = document.createElement('div');
      row.className = 'event';
      row.textContent = `${event.event_type}: ${event.message}`;
      return row;
    }));
  } catch (error) {
    if (id !== selectedId) return;
    eventsEl.textContent = error.message;
  }
}

async function mutate(path, body) {
  const task = selectedTask();
  if (!task) return;
  statusEl.textContent = 'Saving';
  try {
    await api(`/api/tasks/${encodeURIComponent(task.id)}${path}`, {
      method: 'POST',
      body: JSON.stringify(body || {})
    });
    await loadTasks();
  } catch (error) {
    statusEl.textContent = error.message;
  }
}

async function copyDetailText(elementId) {
  const value = document.getElementById(elementId).textContent;
  if (!value || value === '-') return;
  try {
    if (navigator.clipboard && window.isSecureContext) {
      await navigator.clipboard.writeText(value);
    } else {
      const textarea = document.createElement('textarea');
      try {
        textarea.value = value;
        textarea.setAttribute('readonly', '');
        textarea.style.position = 'fixed';
        textarea.style.left = '-9999px';
        document.body.appendChild(textarea);
        textarea.focus();
        textarea.select();
        textarea.setSelectionRange(0, textarea.value.length);
        if (!document.execCommand('copy')) {
          throw new Error('copy command rejected');
        }
      } finally {
        textarea.remove();
      }
    }
    statusEl.textContent = 'Copied';
  } catch (error) {
    statusEl.textContent = `Copy failed: ${error.message}`;
  }
}

document.getElementById('refresh-button').addEventListener('click', loadTasks);
document.getElementById('copy-brief-button').addEventListener('click', () => copyDetailText('detail-brief'));
document.getElementById('copy-resume-button').addEventListener('click', () => copyDetailText('detail-resume'));
document.querySelectorAll('[data-status-filter]').forEach(button => {
  button.addEventListener('click', () => {
    showStatus = button.dataset.statusFilter;
    const visibleTasks = filteredTasks();
    if (selectedId && !visibleTasks.some(task => task.id === selectedId)) {
      selectedId = visibleTasks.length > 0 ? visibleTasks[0].id : null;
    }
    render();
    statusEl.textContent = `${visibleTasks.length} shown / ${tasks.length} active`;
  });
});
document.getElementById('event-button').addEventListener('click', () => mutate('/event', {
  event_type: 'progress',
  message: document.getElementById('event-message').value || 'Progress updated'
}));
document.getElementById('block-button').addEventListener('click', () => mutate('/mark', {
  action: 'blocked',
  message: document.getElementById('block-message').value || 'Blocked'
}));
document.getElementById('ready-button').addEventListener('click', () => mutate('/mark', {
  action: 'ready_for_review',
  message: 'Ready for review'
}));
document.getElementById('triage-button').addEventListener('click', () => mutate('/mark', {
  action: 'triaged',
  message: 'Moved back to triage'
}));
document.getElementById('accept-button').addEventListener('click', () => mutate('/review', { action: 'accept' }));
document.getElementById('changes-button').addEventListener('click', () => mutate('/review', {
  action: 'request_changes',
  message: document.getElementById('changes-message').value || 'Changes requested'
}));
document.getElementById('sync-button').addEventListener('click', () => mutate('/sync', {}));

loadTasks().catch(error => { statusEl.textContent = error.message; });
</script>
</body>
</html>
"#;

    let _ = refresh_seconds;
    html.replace("{action_token}", &escaped_token)
        .replace("{escaped_board}", &escaped_board)
}

pub fn load_task_board_tasks(store: &TaskStore) -> Result<Vec<TaskRecord>> {
    let mut tasks = store.list_tasks()?;
    tasks.retain(|task| task.status != TaskStatus::Archived);
    tasks.sort_by(|left, right| right.updated_at.cmp(&left.updated_at));
    Ok(tasks)
}

pub fn serve_task_board(store: &TaskStore, host: &str, port: u16) -> Result<()> {
    let bind_address = loopback_bind_address(host, port)?;
    let listener = TcpListener::bind(bind_address)
        .with_context(|| format!("bind HelmAgent board server on {host}:{port}"))?;
    let action_token = generate_action_token();
    let local_address = listener.local_addr().unwrap_or(bind_address);
    println!("Serving HelmAgent board at http://{local_address}");

    for stream in listener.incoming() {
        let stream = stream.context("accept board connection")?;
        handle_connection(stream, store, &action_token)?;
    }

    Ok(())
}

fn handle_connection(mut stream: TcpStream, store: &TaskStore, action_token: &str) -> Result<()> {
    let request = read_http_request(&mut stream)?;
    let response = handle_board_http_request(&request, store, action_token);
    stream
        .write_all(response.as_bytes())
        .context("write board response")?;
    Ok(())
}

pub fn handle_board_http_request(request: &str, store: &TaskStore, action_token: &str) -> String {
    if !is_allowed_board_request_host(request) {
        return forbidden_http_response();
    }

    let parsed = match ParsedRequest::parse(request) {
        Ok(parsed) => parsed,
        Err(error) => return json_error_http_response(400, "Bad Request", &error.to_string()),
    };

    match route_board_request(&parsed, store, action_token) {
        Ok(response) => response,
        Err(BoardRouteError::BadRequest(message)) => {
            json_error_http_response(400, "Bad Request", &message)
        }
        Err(BoardRouteError::Forbidden(message)) => {
            json_error_http_response(403, "Forbidden", &message)
        }
        Err(BoardRouteError::NotFound(message)) => {
            json_error_http_response(404, "Not Found", &message)
        }
        Err(BoardRouteError::Conflict(message)) => {
            json_error_http_response(409, "Conflict", &message)
        }
        Err(BoardRouteError::Internal(message)) => {
            json_error_http_response(500, "Internal Server Error", &message)
        }
    }
}

fn route_board_request(
    request: &ParsedRequest,
    store: &TaskStore,
    action_token: &str,
) -> std::result::Result<String, BoardRouteError> {
    let path = request
        .path
        .split('?')
        .next()
        .unwrap_or(request.path.as_str());

    match (request.method.as_str(), path) {
        ("GET", "/") => {
            let tasks = load_task_board_tasks(store)
                .map_err(|error| BoardRouteError::Internal(error.to_string()))?;
            Ok(board_http_response(&render_task_board_html_with_token(
                &tasks,
                action_token,
            )))
        }
        ("GET", "/api/tasks") => {
            let tasks = load_task_board_tasks(store)
                .map_err(|error| BoardRouteError::Internal(error.to_string()))?;
            Ok(json_http_response(
                200,
                "OK",
                &json!({ "ok": true, "tasks": tasks }),
            ))
        }
        _ => route_task_api_request(request, store, action_token, path),
    }
}

fn route_task_api_request(
    request: &ParsedRequest,
    store: &TaskStore,
    action_token: &str,
    path: &str,
) -> std::result::Result<String, BoardRouteError> {
    let Some((encoded_task_id, action)) = parse_task_api_path(path) else {
        return Err(BoardRouteError::NotFound("route not found".to_string()));
    };
    let task_id = percent_decode(encoded_task_id)?;

    if !store.task_path(&task_id).exists() {
        return Err(BoardRouteError::NotFound(format!(
            "task not found: {task_id}"
        )));
    }

    match (request.method.as_str(), action) {
        ("GET", "events") => {
            let events = store
                .read_events(&task_id)
                .map_err(|error| BoardRouteError::Internal(error.to_string()))?;
            Ok(json_http_response(
                200,
                "OK",
                &json!({ "ok": true, "events": events }),
            ))
        }
        ("POST", "event") => {
            validate_action_token(request, action_token)?;
            let body: EventRequest = parse_json_body(request)?;
            let event_type = match body.event_type.as_str() {
                "progress" | "blocked" | "ready_for_review" => body.event_type,
                _ => {
                    return Err(BoardRouteError::BadRequest(format!(
                        "invalid event_type: {}",
                        body.event_type
                    )))
                }
            };
            let task = task_actions::record_event(
                store,
                &task_id,
                event_type,
                body.message,
                OffsetDateTime::now_utc(),
            )
            .map_err(|error| BoardRouteError::Conflict(error.to_string()))?;
            Ok(json_http_response(
                200,
                "OK",
                &json!({ "ok": true, "task": task }),
            ))
        }
        ("POST", "mark") => {
            validate_action_token(request, action_token)?;
            let body: MarkRequest = parse_json_body(request)?;
            let action = match body.action.as_str() {
                "ready_for_review" => MarkAction::ReadyForReview,
                "blocked" => MarkAction::Blocked,
                "triaged" => MarkAction::Triaged,
                _ => {
                    return Err(BoardRouteError::BadRequest(format!(
                        "invalid mark action: {}",
                        body.action
                    )))
                }
            };
            let task = task_actions::mark_task(
                store,
                &task_id,
                action,
                body.message,
                OffsetDateTime::now_utc(),
            )
            .map_err(|error| BoardRouteError::Conflict(error.to_string()))?;
            Ok(json_http_response(
                200,
                "OK",
                &json!({ "ok": true, "task": task }),
            ))
        }
        ("POST", "review") => {
            validate_action_token(request, action_token)?;
            let body: ReviewRequest = parse_json_body(request)?;
            let action = match body.action.as_str() {
                "accept" => ReviewAction::Accept,
                "request_changes" => ReviewAction::RequestChanges(
                    body.message
                        .unwrap_or_else(|| "Changes requested".to_string()),
                ),
                _ => {
                    return Err(BoardRouteError::BadRequest(format!(
                        "invalid review action: {}",
                        body.action
                    )))
                }
            };
            let task =
                task_actions::review_task(store, &task_id, action, OffsetDateTime::now_utc())
                    .map_err(|error| BoardRouteError::Conflict(error.to_string()))?;
            Ok(json_http_response(
                200,
                "OK",
                &json!({ "ok": true, "task": task }),
            ))
        }
        ("POST", "sync") => {
            validate_action_token(request, action_token)?;
            let task = store
                .load_task(&task_id)
                .map_err(|error| BoardRouteError::Internal(error.to_string()))?;
            let result = task_actions::sync_task(task, store, &Launcher::new())
                .map_err(|error| BoardRouteError::Conflict(error.to_string()))?;
            let task = store
                .load_task(&task_id)
                .map_err(|error| BoardRouteError::Internal(error.to_string()))?;
            Ok(json_http_response(
                200,
                "OK",
                &json!({ "ok": true, "result": result, "task": task }),
            ))
        }
        ("POST", _) => {
            validate_action_token(request, action_token)?;
            Err(BoardRouteError::NotFound("route not found".to_string()))
        }
        _ => Err(BoardRouteError::NotFound("route not found".to_string())),
    }
}

fn validate_action_token(
    request: &ParsedRequest,
    action_token: &str,
) -> std::result::Result<(), BoardRouteError> {
    let valid = request
        .header(ACTION_TOKEN_HEADER)
        .is_some_and(|value| value == action_token);
    if valid {
        Ok(())
    } else {
        Err(BoardRouteError::Forbidden(
            "invalid action token".to_string(),
        ))
    }
}

fn parse_json_body<T>(request: &ParsedRequest) -> std::result::Result<T, BoardRouteError>
where
    T: for<'de> Deserialize<'de>,
{
    serde_json::from_str(&request.body)
        .map_err(|error| BoardRouteError::BadRequest(error.to_string()))
}

fn parse_task_api_path(path: &str) -> Option<(&str, &str)> {
    let rest = path.strip_prefix("/api/tasks/")?;
    let mut parts = rest.split('/');
    let task_id = parts.next()?;
    let action = parts.next()?;
    if task_id.is_empty() || action.is_empty() || parts.next().is_some() {
        return None;
    }
    Some((task_id, action))
}

fn percent_decode(value: &str) -> std::result::Result<String, BoardRouteError> {
    let bytes = value.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0;

    while index < bytes.len() {
        if bytes[index] == b'%' {
            let high = bytes
                .get(index + 1)
                .copied()
                .and_then(hex_value)
                .ok_or_else(|| {
                    BoardRouteError::BadRequest(format!("invalid percent encoding: {value}"))
                })?;
            let low = bytes
                .get(index + 2)
                .copied()
                .and_then(hex_value)
                .ok_or_else(|| {
                    BoardRouteError::BadRequest(format!("invalid percent encoding: {value}"))
                })?;
            decoded.push((high << 4) | low);
            index += 3;
        } else {
            decoded.push(bytes[index]);
            index += 1;
        }
    }

    String::from_utf8(decoded)
        .map_err(|error| BoardRouteError::BadRequest(format!("invalid UTF-8 task id: {error}")))
}

fn hex_value(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

#[derive(Debug, Deserialize)]
struct EventRequest {
    event_type: String,
    message: String,
}

#[derive(Debug, Deserialize)]
struct MarkRequest {
    action: String,
    message: String,
}

#[derive(Debug, Deserialize)]
struct ReviewRequest {
    action: String,
    message: Option<String>,
}

#[derive(Debug)]
enum BoardRouteError {
    BadRequest(String),
    Forbidden(String),
    NotFound(String),
    Conflict(String),
    Internal(String),
}

#[derive(Debug)]
struct ParsedRequest {
    method: String,
    path: String,
    headers: Vec<(String, String)>,
    body: String,
}

impl ParsedRequest {
    fn parse(request: &str) -> Result<Self> {
        let (head, body) = request
            .split_once("\r\n\r\n")
            .context("request missing header terminator")?;
        let mut lines = head.lines();
        let request_line = lines.next().context("request missing request line")?;
        let mut request_parts = request_line.split_whitespace();
        let method = request_parts
            .next()
            .context("request missing method")?
            .to_string();
        let path = request_parts
            .next()
            .context("request missing path")?
            .to_string();
        if request_parts.next().is_none() {
            bail!("request missing HTTP version");
        }

        let mut headers = Vec::new();
        for line in lines {
            if line.trim().is_empty() {
                continue;
            }
            let (name, value) = line
                .split_once(':')
                .with_context(|| format!("invalid header line: {line}"))?;
            headers.push((name.trim().to_ascii_lowercase(), value.trim().to_string()));
        }

        Ok(Self {
            method,
            path,
            headers,
            body: body.to_string(),
        })
    }

    fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(header_name, _)| header_name == name)
            .map(|(_, value)| value.as_str())
    }
}

fn read_http_request(stream: &mut TcpStream) -> Result<String> {
    let mut bytes = Vec::new();
    let mut buffer = [0_u8; 4096];

    loop {
        let bytes_read = stream.read(&mut buffer).context("read board request")?;
        if bytes_read == 0 {
            break;
        }
        bytes.extend_from_slice(&buffer[..bytes_read]);
        if is_complete_http_request(&bytes) {
            break;
        }
        if bytes.len() > 1_048_576 {
            bail!("board request is too large");
        }
    }

    String::from_utf8(bytes).context("board request is not valid UTF-8")
}

fn is_complete_http_request(bytes: &[u8]) -> bool {
    let Some(header_end) = find_bytes(bytes, b"\r\n\r\n") else {
        return false;
    };
    let headers = String::from_utf8_lossy(&bytes[..header_end]);
    let content_length = headers
        .lines()
        .find_map(|line| {
            line.split_once(':').and_then(|(name, value)| {
                name.eq_ignore_ascii_case("content-length")
                    .then(|| value.trim().parse::<usize>().ok())
                    .flatten()
            })
        })
        .unwrap_or(0);
    bytes.len() >= header_end + 4 + content_length
}

fn find_bytes(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

pub fn validate_loopback_bind_host(host: &str, port: u16) -> Result<()> {
    loopback_bind_address(host, port)?;
    Ok(())
}

pub fn loopback_bind_address(host: &str, port: u16) -> Result<SocketAddr> {
    let addresses = (host, port)
        .to_socket_addrs()
        .with_context(|| format!("resolve board host {host}:{port}"))?
        .collect::<Vec<_>>();

    if addresses.is_empty() {
        bail!("board host did not resolve: {host}");
    }
    if addresses.iter().any(|address| !address.ip().is_loopback()) {
        bail!("board serve only supports loopback hosts by default: {host}");
    }

    Ok(addresses[0])
}

pub fn is_allowed_board_request_host(request: &str) -> bool {
    request
        .lines()
        .find_map(host_header_value)
        .map(is_allowed_loopback_host_header)
        .unwrap_or(false)
}

pub fn board_http_response(body: &str) -> String {
    http_response(200, "OK", "text/html; charset=utf-8", body)
}

fn json_http_response(status_code: u16, reason: &str, body: &serde_json::Value) -> String {
    let body = serde_json::to_string(body).expect("serialize board JSON response");
    http_response(
        status_code,
        reason,
        "application/json; charset=utf-8",
        &body,
    )
}

fn json_error_http_response(status_code: u16, reason: &str, message: &str) -> String {
    json_http_response(
        status_code,
        reason,
        &json!({ "ok": false, "error": message }),
    )
}

pub fn forbidden_http_response() -> String {
    let body = "Forbidden\n";
    http_response(403, "Forbidden", "text/plain; charset=utf-8", body)
}

fn http_response(status_code: u16, reason: &str, content_type: &str, body: &str) -> String {
    format!(
        "HTTP/1.1 {status_code} {reason}\r\nContent-Type: {content_type}\r\nCache-Control: no-store\r\nContent-Length: {}\r\n\r\n{}",
        body.len(),
        body
    )
}

fn host_header_value(line: &str) -> Option<&str> {
    line.split_once(':')
        .and_then(|(name, value)| name.eq_ignore_ascii_case("host").then_some(value.trim()))
}

fn is_allowed_loopback_host_header(value: &str) -> bool {
    let host = host_header_host(value);
    matches!(host, "localhost" | "127.0.0.1" | "[::1]" | "::1")
}

fn host_header_host(value: &str) -> &str {
    if let Some(end) = value.strip_prefix('[').and_then(|rest| rest.find(']')) {
        return &value[..=end + 1];
    }

    value.split(':').next().unwrap_or(value)
}

fn generate_action_token() -> String {
    let mut bytes = [0_u8; 32];
    if File::open("/dev/urandom")
        .and_then(|mut file| file.read_exact(&mut bytes))
        .is_ok()
    {
        return hex_encode(&bytes);
    }

    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let fallback = format!(
        "{}-{}-{nanos}",
        std::process::id(),
        std::thread::panicking()
    );
    hex_encode(fallback.as_bytes())
}

fn hex_encode(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(HEX[(byte >> 4) as usize] as char);
        output.push(HEX[(byte & 0x0f) as usize] as char);
    }
    output
}

fn escape_html(text: &str) -> String {
    let mut escaped = String::with_capacity(text.len());

    for ch in text.chars() {
        match ch {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            '"' => escaped.push_str("&quot;"),
            '\'' => escaped.push_str("&#39;"),
            _ => escaped.push(ch),
        }
    }

    escaped
}
