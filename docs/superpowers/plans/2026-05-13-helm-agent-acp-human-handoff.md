# ACP Human Handoff Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make HelmAgent's ACP path support a public Claude Code preset, custom ACP registrations, and human TUI resume commands for ACP sessions.

**Architecture:** Keep ACP as the structured control plane and tmux as fallback. Store the ACP session id and cwd on the task, derive a human handoff command from the configured ACP agent, and surface it in status/brief/board outputs.

**Tech Stack:** Rust, `agent-client-protocol`, Clap, YAML config, existing `TaskStore`.

---

### Task 1: ACP Agent Resume Metadata

**Files:**
- Modify: `src/acp_adapter.rs`
- Modify: `tests/cli_task_flow.rs`

- [x] Add optional ACP config field for human resume command template.
- [x] Reuse existing task recovery resume field for ACP handoff command.
- [x] Test that a configured ACP agent can render `cd <cwd> && <custom resume command> <session-id>`.

### Task 2: Claude Code ACP Preset

**Files:**
- Modify: `src/cli.rs`
- Modify: `src/acp_adapter.rs`
- Modify: `tests/cli_task_flow.rs`

- [x] Add `helm-agent acp preset install claude-code`.
- [x] Register ACP agent command as `npx -y @zed-industries/claude-agent-acp`.
- [x] Keep custom wrapper support in `acp agent add --env CLAUDE_CODE_EXECUTABLE=...`.
- [x] Store resume template `cd {cwd} && claude --resume {session_id}`.

### Task 3: ACP Dispatch Handoff Output

**Files:**
- Modify: `src/cli.rs`
- Modify: `src/brief.rs`
- Modify: `src/output.rs`
- Modify: `tests/cli_task_flow.rs`

- [x] During ACP dispatch, set `assignment.acp_session_id` and `recovery.resume_command`.
- [x] Print handoff command in `task status`, `task resume`, and child brief.
- [x] Keep existing fake ACP tests working.

### Task 4: Documentation

**Files:**
- Modify: `README.md`
- Modify: `README.zh-CN.md`
- Modify: `docs/install.md`
- Modify: `docs/quickstart-real-run.md`

- [x] Document ACP-first Claude Code setup.
- [x] Document human handoff command shape and cwd requirement.
- [x] Clarify tmux as fallback.

### Task 5: Verification

- [x] `cargo test`
- [x] `cargo check`
- [x] `cargo fmt -- --check`
- [x] `git diff --check`
