# HelmAgent Agent Integration Sprint Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the next usable HelmAgent loop: project onboarding, main-agent prompt output, tmux recovery UX, and a read-only board.

**Architecture:** Keep task storage unchanged. Add small focused modules for guidance and web board rendering, then expose them through narrow CLI commands. Dispatch remains tmux-backed and state stays in existing YAML task records.

**Tech Stack:** Rust, clap, std filesystem, std HTTP listener, existing YAML/JSONL store.

---

## Files

- Create `src/guidance.rs`: template lookup, prompt rendering, project include updates.
- Create `src/web_board.rs`: escaped read-only HTML rendering and small local server helper.
- Modify `src/cli.rs`: add `project`, `agent`, and top-level `board` commands.
- Modify `src/lib.rs`: export new modules.
- Add tests in `tests/guidance_tests.rs`, `tests/web_board_tests.rs`, and `tests/cli_task_flow.rs`.
- Update `README.md` and `docs/agent-integrations/main-agent.md` with the main-agent-first workflow.

## Task 1: Guidance Module

- [ ] Add tests for idempotent `AGENTS.md` and `CLAUDE.md` include updates.
- [ ] Add tests for runtime-specific prompt output.
- [ ] Implement `src/guidance.rs`.
- [ ] Run `rtk cargo test --test guidance_tests`.

## Task 2: Web Board Module

- [ ] Add tests for HTML escaping and empty board rendering.
- [ ] Implement `src/web_board.rs`.
- [ ] Run `rtk cargo test --test web_board_tests`.

## Task 3: CLI Integration

- [ ] Add CLI tests for `project init`, `agent prompt`, and `board html`.
- [ ] Wire new commands in `src/cli.rs`.
- [ ] Run `rtk cargo test --test cli_task_flow`.

## Task 4: Dispatch/Resume UX

- [ ] Review current dispatch/resume output against the spec.
- [ ] Add tests only for missing practical recovery text.
- [ ] Make the smallest output improvements needed.
- [ ] Run launcher and CLI task tests.

## Task 5: Docs, Verification, Review

- [ ] Update README and integration docs.
- [ ] Run `rtk cargo test`.
- [ ] Run `rtk cargo fmt --check`.
- [ ] Run `rtk git diff --check`.
- [ ] Request multi-agent review before merging.
