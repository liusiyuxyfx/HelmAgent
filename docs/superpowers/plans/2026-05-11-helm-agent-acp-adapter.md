# HelmAgent ACP Adapter Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [x]`) syntax for tracking.

**Goal:** Let HelmAgent register ACP agents and dispatch task briefs to them over the official Rust ACP SDK.

**Architecture:** Add `acp_adapter.rs` for ACP config and runtime handoff, add `AgentRuntime::Acp`, then route only ACP dispatches through the adapter while preserving tmux dispatch for Claude/Codex/OpenCode. Use a tokio runtime internally for the async SDK.

**Tech Stack:** Rust, Clap, Serde YAML, `agent-client-protocol` 0.11.1, Tokio stdio process transport.

---

## Files

- Modify `Cargo.toml`: add ACP/Tokio dependencies.
- Create `src/acp_adapter.rs`: config store, output helpers, one-shot ACP dispatch.
- Modify `src/lib.rs`: export `acp_adapter`.
- Modify `src/domain.rs`: add `AgentRuntime::Acp`.
- Modify `src/cli.rs`: add `acp agent` commands and ACP dispatch branch.
- Modify `tests/cli_task_flow.rs`: add config, dry-run, and fake-agent dispatch tests.
- Add `docs/superpowers/specs/2026-05-11-helm-agent-acp-adapter-design.md`.

## Task 1: ACP Agent Config

- [x] **Step 1: Write failing CLI tests**

Add tests for `helm-agent acp agent add`, `list`, and `remove`.

- [x] **Step 2: Run failing tests**

Run:

```bash
rtk cargo test --test cli_task_flow acp_agent_
```

Expected: command does not exist.

- [x] **Step 3: Implement config module and CLI commands**

Create `AcpAgentConfig`, read/write `acp/agents.yaml`, and add Clap commands.

- [x] **Step 4: Verify**

Run:

```bash
rtk cargo test --test cli_task_flow acp_agent_
```

Expected: config tests pass.

## Task 2: ACP Dispatch Flow

- [x] **Step 1: Write failing dispatch tests**

Add tests for required `--agent`, ACP dry-run, and fake ACP agent real dispatch.

- [x] **Step 2: Run failing tests**

Run:

```bash
rtk cargo test --test cli_task_flow acp_dispatch_
```

Expected: compile or command failures because ACP runtime does not exist.

- [x] **Step 3: Implement ACP dispatch**

Add `AgentRuntime::Acp`, `--agent`, policy confirmation, dry-run output, and real one-shot ACP handoff.

- [x] **Step 4: Verify**

Run:

```bash
rtk cargo test --test cli_task_flow acp_dispatch_
```

Expected: ACP dispatch tests pass.

## Task 3: Final Verification And Review

- [x] **Step 1: Run full verification**

```bash
rtk cargo fmt
rtk cargo test
rtk cargo fmt -- --check
rtk git diff --check
```

- [x] **Step 2: Multi-agent review**

Reviewers check SDK correctness, CLI compatibility, and state safety.

- [x] **Step 3: Fix findings and merge**

Re-run full verification, commit, merge to `main`, push, reinstall local binary.
