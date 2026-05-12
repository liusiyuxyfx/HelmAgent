# HelmAgent Usage Readiness Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make the existing main-agent, board, and ACP surfaces usable enough for daily HelmAgent dogfood.

**Architecture:** Keep the current Rust CLI and loopback HTML board. Improve small, testable seams: template text, board JS/API test coverage, and an ACP check command that reuses the existing adapter handshake.

**Tech Stack:** Rust 2021, clap, serde, tokio, agent-client-protocol, hand-written loopback HTML/JS.

---

## File Map

- Modify `docs/agent-integrations/main-agent-template.md`: dogfood checklist and board/review operating rules.
- Modify `docs/agent-integrations/main-agent.md`: user-facing guidance for the same flow.
- Modify `src/web_board.rs`: add filter controls, richer detail display, and remove duplicate event fetch on task click.
- Modify `tests/web_board_tests.rs`: cover visible board wiring and missing API cases.
- Modify `src/acp_adapter.rs`: add a reusable check prompt helper.
- Modify `src/cli.rs`: add `helm-agent acp agent check <name>`.
- Modify `tests/cli_task_flow.rs`: cover ACP check success/failure and guidance template expectations.

## Tasks

### Task 1: Main-Agent Dogfood Guidance

- [x] Add assertions in `tests/cli_task_flow.rs` that `docs/agent-integrations/main-agent-template.md` contains the dogfood loop commands: `task board`, `board serve`, `task sync --all`, and `task review --accept`.
- [x] Update `docs/agent-integrations/main-agent-template.md` with the concise checklist.
- [x] Update `docs/agent-integrations/main-agent.md` to mirror the workflow.
- [x] Run `cargo test --test cli_task_flow main_agent`.

### Task 2: Board Interactivity Polish

- [x] Add tests in `tests/web_board_tests.rs` for `GET /api/tasks/{id}/events`, wrong-token rejection, mark ready, mark triaged, and JS action/filter wiring.
- [x] Update `src/web_board.rs` to add status filter buttons, detail fields for runtime/review/brief/resume, and avoid duplicate event fetches.
- [x] Run `cargo test --test web_board_tests`.

### Task 3: ACP Agent Check

- [x] Add tests in `tests/cli_task_flow.rs` for `helm-agent acp agent check <name>` success and failure using the existing fake ACP agent helpers.
- [x] Add the clap subcommand and handler in `src/cli.rs`.
- [x] Reuse `acp_adapter::dispatch_prompt` with a small check prompt and temporary project directory.
- [x] Run focused ACP CLI tests.

### Task 4: Final Verification

- [x] Run `cargo fmt`.
- [x] Run `cargo test`.
- [x] Run `cargo fmt -- --check`.
- [x] Run `cargo check`.
- [x] Run `git diff --check`.
- [x] Smoke the installed-style flow locally with `helm-agent board serve` or direct HTTP tests.
