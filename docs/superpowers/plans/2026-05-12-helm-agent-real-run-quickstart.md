# HelmAgent Real Run Quickstart Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a repeatable quickstart and smoke script for validating HelmAgent with dry-run, real tmux, and ACP child-agent paths.

**Architecture:** Keep this as a thin wrapper around existing CLI behavior. The shell script owns temporary state and safety gates; Makefile exposes convenient developer targets; docs explain the human review loop.

**Tech Stack:** POSIX shell, Make, Rust integration tests, existing HelmAgent CLI.

---

## File Map

- Create `tests/real_run_smoke_tests.rs`: contract tests for quickstart docs, Make targets, and script safety gates.
- Create `scripts/real_run_smoke.sh`: smoke orchestration script with `dry-run`, `tmux`, and `acp` modes.
- Modify `Makefile`: add `real-run-dry-run`, `real-run-tmux`, and `real-run-acp` targets.
- Create `docs/quickstart-real-run.md`: user-facing quickstart.
- Modify `README.md` and `README.zh-CN.md`: link to the quickstart.

## Tasks

### Task 1: Contract Tests

- [x] Add failing tests for quickstart docs, Make targets, and script safety gates.
- [x] Run `cargo test --test real_run_smoke_tests` and confirm it fails because the files and targets are absent.

### Task 2: Script And Targets

- [x] Add `scripts/real_run_smoke.sh`.
- [x] Add Make targets for dry-run, tmux, and ACP modes.
- [x] Ensure real tmux dispatch requires `HELM_AGENT_REAL_RUN_CONFIRM=1`.

### Task 3: Documentation

- [x] Add `docs/quickstart-real-run.md`.
- [x] Link the quickstart from English and Chinese README files.

### Task 4: Verification

- [x] Run `cargo test --test real_run_smoke_tests`.
- [x] Run `make real-run-dry-run`.
- [x] Run `cargo fmt -- --check`.
- [x] Run `cargo test`.
- [x] Run `cargo check`.
- [x] Run `git diff --check`.
