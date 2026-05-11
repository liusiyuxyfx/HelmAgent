# HelmAgent Interactive Board Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a local browser board for inspecting and updating HelmAgent tasks.

**Architecture:** Extract shared task mutations into `task_actions.rs`, then make `web_board.rs` route both HTML and JSON API requests through those helpers. Keep the server loopback-only and require a generated token for all write requests.

**Tech Stack:** Rust stdlib HTTP server, Serde JSON/YAML, existing `TaskStore`, existing tmux `Launcher`, no frontend build step.

---

## Files

- Create `src/task_actions.rs`: reusable task state transitions for CLI and Web.
- Modify `src/lib.rs`: export `task_actions`.
- Modify `src/cli.rs`: delegate event, mark, review, and sync to `task_actions`.
- Modify `src/web_board.rs`: add request parser, JSON API routes, action token, and interactive HTML.
- Modify `tests/web_board_tests.rs`: add route/action tests.
- Add `docs/superpowers/specs/2026-05-11-helm-agent-interactive-board-design.md`.

## Task 1: Shared Task Actions

- [ ] **Step 1: Write failing action tests**

Add tests that call `task_actions::record_event`, `mark_task`, and `review_task` against a temp `TaskStore`.

- [ ] **Step 2: Run failing tests**

Run:

```bash
rtk cargo test --test web_board_tests task_action_
```

Expected: compile failure because `task_actions` does not exist.

- [ ] **Step 3: Implement `task_actions.rs`**

Move the CLI's event, mark, review, and one-task sync semantics into public helpers while preserving current status validation and event names.

- [ ] **Step 4: Re-run tests**

Run:

```bash
rtk cargo test --test web_board_tests task_action_
```

Expected: action tests pass.

## Task 2: Interactive HTML And API Routes

- [ ] **Step 1: Write failing web tests**

Add tests for tokenized HTML, `GET /api/tasks`, rejected tokenless POST, and successful event/mark/review POSTs.

- [ ] **Step 2: Run failing tests**

Run:

```bash
rtk cargo test --test web_board_tests api_
```

Expected: compile failure or 404/old HTML failures because routes are missing.

- [ ] **Step 3: Implement request handling**

Add a pure `handle_board_request` function and route table, then wire `serve_task_board` stream handling to it.

- [ ] **Step 4: Implement page UI**

Render a compact app with lanes, a detail panel, and action forms backed by `fetch`.

- [ ] **Step 5: Re-run tests**

Run:

```bash
rtk cargo test --test web_board_tests
```

Expected: all web board tests pass.

## Task 3: Verification And Review

- [ ] **Step 1: Run full verification**

```bash
rtk cargo fmt
rtk cargo test
rtk cargo fmt -- --check
rtk git diff --check
```

- [ ] **Step 2: Smoke the local server**

Start `helm-agent board serve --port 0` or render `board html`; verify the HTML contains the interactive app shell and token metadata.

- [ ] **Step 3: Request multi-agent review**

Use separate reviewers for state semantics and web/API safety.

- [ ] **Step 4: Fix review findings and merge**

Re-run full verification after fixes, commit, merge to `main`, push, and reinstall the local binary.
