# HelmAgent Interactive Board Design

## Goal

Turn `helm-agent board serve` from a read-only text page into a local interactive review board. The board should let a human inspect active tasks, record progress events, mark blockers or review handoffs, accept/request review changes, and sync tmux-backed tasks without leaving the browser.

## Scope

In scope:

- Local HTTP server remains Rust stdlib based and loopback-only.
- `GET /` renders a dense app-style board with lanes and a task detail panel.
- `GET /api/tasks` returns active task records as JSON.
- `GET /api/tasks/<id>/events` returns task event history as JSON.
- `POST /api/tasks/<id>/event` records a progress, blocked, or ready-for-review event.
- `POST /api/tasks/<id>/mark` supports `blocked`, `ready_for_review`, and `triaged`.
- `POST /api/tasks/<id>/review` supports `accept` and `request_changes`.
- `POST /api/tasks/<id>/sync` runs the existing tmux health sync for one task.
- All POST routes require a per-server action token sent as `X-Helm-Agent-Token`.

Out of scope:

- Starting child agents from the web page.
- ACP dispatch.
- Multi-user auth or non-loopback serving.
- A JavaScript framework or frontend build step.

## Architecture

`src/task_actions.rs` owns reusable state transitions currently embedded in CLI handlers: event recording, marking, review acceptance/change requests, and one-task sync. `src/cli.rs` calls these helpers to preserve existing command behavior.

`src/web_board.rs` owns HTTP parsing, response rendering, API route dispatch, JSON request/response types, and the static HTML/CSS/JS payload. The server still rejects non-loopback `Host` headers and only binds loopback addresses. POST routes additionally check a generated action token.

The page uses plain HTML, CSS, and JavaScript. It fetches `/api/tasks`, renders lanes, shows selected task details, and calls action endpoints. The design is operational and dense rather than marketing-oriented.

## Error Handling

- Invalid host: `403 Forbidden`.
- Missing or wrong token on write routes: `403 Forbidden`.
- Unknown route: `404 Not Found`.
- Bad JSON or invalid action: `400 Bad Request`.
- Missing task: `404 Not Found`.
- Domain errors from task state rules: `409 Conflict`.
- Unexpected store or sync errors: `500 Internal Server Error`.

API responses use JSON objects:

```json
{ "ok": true, "task": { "...": "..." }, "events": [] }
```

or:

```json
{ "ok": false, "error": "message" }
```

## Testing

Add focused tests for:

- Interactive HTML includes token metadata and app controls.
- `GET /api/tasks` serializes active tasks and excludes archived tasks.
- POST without a token is rejected.
- POST event mutates `progress.last_event` and appends an event.
- POST mark blocked updates task status and blocker.
- POST review accept moves ready-for-review tasks to done.
- POST sync reuses the one-task sync behavior.
- HTTP responses include correct status, content type, and no-store cache header.

Acceptance commands:

```bash
rtk cargo test
rtk cargo fmt --check
rtk git diff --check
```
