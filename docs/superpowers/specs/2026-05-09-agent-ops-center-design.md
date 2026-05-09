# Agent Ops Center Design

Date: 2026-05-09
Status: design approved in conversation, pending written spec review

## 1. Problem

AI coding agents can now complete work faster than the human review loop can absorb. This creates a new bottleneck: the user must triage tasks, switch context, inspect code, talk to agents, and decide what to trust across many unrelated work streams.

The tool should reduce this burden by creating a local coordination layer for multiple agents. It should not replace Claude Code, Codex, or OpenCode. It should help the user decide what should be delegated, track what is happening, preserve recovery paths, and surface only the review items that need human attention.

Priority order:

1. Task routing
2. Context recovery
3. Review support
4. Full workflow automation

## 2. Product Shape

The tool is named Agent Ops Center for this design. It is a local agent coordination system.

The primary interaction is still a main agent running in Claude Code or Codex. The user talks to this main agent directly. The main agent creates tasks, triages them, chooses a child agent, starts the child agent, tracks progress, and asks the user for review when needed.

A board is useful, but it is a companion surface. The board should show status, blockers, recovery commands, and review queue items. It should not be the main place where the user has to manage every task manually.

The V1 goal is:

```text
Make agent delegation trackable, resumable, and reviewable without adding a heavy project management system.
```

## 3. Architecture

Agent Ops Center has a local CLI-first core and thin integrations for Claude Code, Codex, and later other agents. A background service can be added later for a richer board, but it is not required for V1.

```text
User
 |
Main Agent: Claude Code or Codex
 |
Agent Ops Center local CLI core
 |
+-- Task Store
+-- Policy Engine
+-- Agent Launcher
+-- Runtime Adapters
+-- Review/status board
 |
Child agent tmux sessions
+-- Claude Code
+-- Codex
+-- OpenCode
```

### Main Agent

The main agent is the conversational coordinator. It runs inside Claude Code or Codex and uses Agent Ops Center through a shared CLI.

Responsibilities:

- Accept natural language tasks from the user.
- Create task records.
- Break large requests into smaller tasks when needed.
- Classify task type, risk, cost, and scope.
- Choose an agent runtime or ask the user for confirmation.
- Dispatch child agents.
- Summarize progress and results.
- Maintain review and recovery context.

### Agent Ops Center Core

The core is the source of truth. It should avoid being tied to one agent runtime.

Responsibilities:

- Store tasks, events, logs, artifacts, review states, and recovery commands.
- Apply policy decisions.
- Launch child agents in tmux.
- Record native session IDs and ACP session IDs when available.
- Provide status, resume, and review commands.

### Child Agents

Child agents perform concrete work. They run in tmux sessions so the user can inspect or take over the work.

Responsibilities:

- Execute assigned tasks.
- Produce code changes, analysis, test output, or review findings.
- Write progress events directly when possible.
- Otherwise expose enough transcript output for the main agent or launcher to summarize.

## 4. V1 Scope

V1 should build a reliable local dispatcher rather than a complete automation platform.

Required:

- `aoc` CLI.
- Centralized task store.
- Default semi-automatic policy.
- Task state machine.
- tmux-based launcher.
- Basic Claude Code, Codex, and OpenCode adapter skeletons.
- `status`, `resume`, and `review` commands.
- Main agent usage instructions or thin skill/command wrappers.

Recommended but optional in V1:

- TUI board.
- Child-agent progress event command.
- Transcript summarization.
- ACP capability probing.

Out of scope for V1:

- Full Web Board.
- Heavy workflow profile isolation.
- Container sandboxing.
- External issue imports such as GitHub Issues, Linear, Slack, or chat tools.
- Automatic merge.
- Recursive child-agent delegation.
- Complex cost accounting.
- Cross-machine synchronization.

## 5. Storage Model

Use a centralized workspace as the source of truth.

Default location:

```text
~/.agent-ops-center/
  config.yaml
  policies/
    default.yaml
  tasks/
    2026/
      PM-20260509-001.yaml
  sessions/
    PM-20260509-001/
      events.jsonl
      transcript.log
      artifacts/
        diff.patch
        test-output.txt
        review-summary.md
```

V1 should avoid writing project-local metadata by default. This reduces the risk of interfering with existing Claude Code, Codex, OpenCode, Superpowers, or other workflow state.

The centralized task store must hold both facts and summaries.

Facts:

- Task ID
- Title
- Status
- Project path
- Runtime
- tmux session name
- Native session ID, when available
- ACP session ID, when available
- Artifact paths
- Timestamps

Summaries:

- Current progress
- Last meaningful event
- Next action
- Blocker
- Review reason
- Residual risk

## 6. Task Lifecycle

Tasks use an explicit state machine.

```text
inbox
  Newly created. Not yet triaged.

triaged
  Goal, project, risk, scope, and suggested runtime are known.

waiting_user
  User confirmation is required.

queued
  Approved or allowed by policy. Waiting to start.

running
  Child agent is active in tmux.

blocked
  Human input, permission, dependency, or context is required.

ready_for_review
  Child agent claims the task is complete and has provided evidence.

reviewing
  User or main agent is checking the result.

needs_changes
  Review found issues and the task needs another pass.

done
  User or policy accepted the result.

archived
  Task is no longer active.
```

Code-changing tasks must not move directly from `running` to `done`. They must enter `ready_for_review` first unless a future policy explicitly allows automatic acceptance for a narrow low-risk task class.

Status changes must be event-driven. A task should not become `done` only because a child agent said it is done.

Example task record:

```yaml
id: PM-20260509-001
title: Fix login redirect bug
status: running
priority: normal
risk: low
project:
  path: /path/to/repo
  branch: main
assignment:
  runtime: claude
  workflow: cc-default
  tmux_session: aoc-PM-20260509-001-claude
  native_session_id: 58fa3206-cb0-47ad-b306-912f7d122a02
  acp_session_id: null
recovery:
  attach_command: tmux attach -t aoc-PM-20260509-001-claude
  resume_command: claude --resume 58fa3206-cb0-47ad-b306-912f7d122a02
progress:
  summary: Investigating auth redirect handler.
  last_event: Found failing test in auth middleware.
  next_action: Patch redirect target and rerun focused tests.
review:
  required: true
  reason: User-facing auth behavior changed.
  artifacts:
    - sessions/PM-20260509-001/artifacts/diff.patch
    - sessions/PM-20260509-001/artifacts/test-output.txt
```

## 7. Policy Engine

The V1 policy model is semi-automatic.

Default behavior:

- Prefer free agents for routine work.
- Require confirmation before using Codex.
- Allow low-risk read-only work to start automatically.
- Allow low-risk small code changes to start automatically on free agents, but require review before `done`.
- Require confirmation for high-risk, cross-project, destructive, paid, security, credential, dependency, database, or network-sensitive tasks.

Example policy:

```yaml
mode: semi_auto

cost:
  preferred_order:
    - opencode
    - claude
    - codex
  paid_requires_confirmation:
    - codex

auto_start:
  allowed:
    - docs_summary
    - code_search
    - test_investigation
    - small_refactor
    - focused_bugfix
  denied:
    - dependency_upgrade
    - auth_or_security
    - payment_or_billing
    - database_migration
    - destructive_operation
    - cross_project_change

risk_rules:
  high_requires_confirmation: true
  code_write_requires_review: true
  external_network_requires_confirmation: true
  secrets_or_credentials_requires_confirmation: true

limits:
  max_auto_runtime_minutes: 20
  max_auto_files_changed: 5
  max_auto_agents_per_task: 2
  recursive_dispatch: false
```

The policy engine should classify each task with:

- Task type
- Risk
- Runtime cost
- Scope
- Whether it writes files
- Whether it needs network access
- Whether it needs human judgment

Runtime preference:

- OpenCode: free, low-risk, local search, simple analysis, simple edits.
- Claude Code: more involved local development, existing Claude workflows, hook-aware tasks, possible teams usage.
- Codex: high-value reasoning, difficult bugs, architecture review, paid work explicitly approved by the user.

If the system is uncertain, it should enter `waiting_user` with a concise recommendation instead of silently proceeding.

## 8. Launcher, tmux, ACP, and Recovery

Execution is split into four layers:

```text
tmux manages running processes and manual takeover.
Native CLI sessions manage agent persistence.
ACP manages programmatic communication when available.
Task Store records facts and recovery commands.
```

The launcher flow:

1. Create or update task record.
2. Apply policy.
3. Create tmux session.
4. Start native agent CLI inside tmux.
5. Send the task prompt through ACP if available, otherwise through CLI/tmux fallback.
6. Capture native session ID when possible.
7. Capture ACP session ID when possible.
8. Write attach and resume commands to the task record.
9. Start transcript and event capture.

Session naming:

```text
aoc-<task-id>-<runtime>
```

Examples:

```text
aoc-PM-20260509-001-claude
aoc-PM-20260509-002-opencode
aoc-PM-20260509-003-codex
```

Recovery paths:

```text
Running process:
  tmux attach -t <session>

Exited/interrupted Claude Code session:
  claude --resume <session-id>

Exited/interrupted Codex session:
  codex resume <session-id> --all

Programmatic ACP recovery:
  ACP session/load or session/resume, only if the adapter reports support.
```

ACP is not the only recovery mechanism. ACP session IDs must not be assumed to equal native Claude Code or Codex session IDs.

Adapter acceptance rule:

An adapter can be used for automatic dispatch only if it can:

- Create a tmux session.
- Record an attach command.
- Record or generate a native resume command, or explicitly mark native resume as unavailable.
- Save transcript or log output.
- Move failures into `blocked` with a visible reason.

If native session ID capture is not reliable for an adapter, the adapter can still be used in confirm-required mode, with `tmux attach` as the guaranteed recovery path.

## 9. Workflow Isolation

V1 should keep isolation lightweight.

The tool should not modify these by default:

- `~/.claude`
- `~/.codex`
- Project `.claude`
- Project `.codex`
- Existing workflow plugin settings

Instead, V1 should isolate by behavior:

- Launch child agents through explicit commands.
- Record runtime, workflow label, cwd, tmux session, and session IDs.
- Avoid writing project-local metadata unless requested.
- Keep Agent Ops Center state under `~/.agent-ops-center`.

Stronger isolation is a later extension:

- Dedicated `CLAUDE_CONFIG_DIR` per workflow.
- Dedicated `CODEX_HOME` per workflow.
- Git worktree per write task.
- Container or sandbox for untrusted workflows.

The design must not depend on project config overriding global config, because Claude Code and Codex both use layered configuration behavior rather than complete project-level replacement.

## 10. CLI and Integration Interface

The shared CLI is the stable integration point.

Suggested commands:

```bash
aoc task create --title "Fix login redirect bug" --project /repo --description -
aoc task triage PM-20260509-001
aoc task dispatch PM-20260509-001 --auto
aoc task status
aoc task status PM-20260509-001
aoc task resume PM-20260509-001
aoc task review PM-20260509-001 --accept
aoc task review PM-20260509-001 --request-changes "Add a failing regression test"
aoc task event PM-20260509-001 --type progress --message "Found redirect handler"
aoc board
```

Integration principle:

```text
Agent plugins are thin and stateless.
The CLI and task store are stateful.
All agents share one task store.
```

Claude Code integration:

- Provide a skill or slash-command style instruction for using `aoc`.
- Do not install or modify global hooks by default.
- Let the user opt in to deeper integration later.

Codex integration:

- Provide equivalent skill/rule/plugin guidance for invoking `aoc`.
- Keep compatibility with existing Codex Superpowers or other workflows.

Child agent integration:

- Child agents receive a structured task prompt.
- If they can call `aoc task event`, they should report progress directly.
- If they cannot, launcher transcript capture and main-agent summarization are acceptable.

## 11. Review Experience

The review queue should contain only items that need human attention.

Review item fields:

```yaml
summary: What changed
risk: low | medium | high
evidence:
  - diff.patch
  - test-output.txt
  - transcript.log
recommended_action:
  - accept
  - request_changes
  - ask_agent_followup
  - take_over
commands:
  attach: tmux attach -t aoc-PM-20260509-001-claude
  resume: claude --resume <session-id>
```

Main Agent completion summaries must include:

- Result
- Changed scope
- Verification performed
- Residual risk
- Whether human review is required
- Recommended next action

Review transitions:

```text
accept -> done
request_changes -> needs_changes
ask_agent_followup -> queued or running
take_over -> reviewing or blocked
```

The review surface should default to concise summaries and link to evidence. Full transcript should remain available but should not be the first thing the user sees.

## 12. Board

V1 can start with CLI status output. A TUI board is the preferred first board because it fits the terminal workflow.

Minimum board fields:

- Task ID
- Title
- Status
- Runtime
- Last meaningful progress
- Blocker
- Next action
- Review required
- Attach command
- Resume command

Web Board can be added later by reading the same task store.

## 13. Technology Choices

V1 should use Rust for the core Agent Ops Center CLI.

Rust is a good fit for the core because Agent Ops Center needs a reliable local binary that manages files, subprocesses, tmux sessions, logs, and state transitions. The main benefit is not raw performance. The main benefit is a single distributable binary, strong typing for task and policy state, and dependable local process orchestration.

Recommended V1 stack:

```text
Rust core:
  clap        CLI command parsing
  serde      YAML/JSON serialization
  anyhow     application-level error handling
  thiserror  typed domain errors
  tokio      async process/log handling when needed
  tracing    structured logs
  directories config/data path discovery

Storage:
  V1: YAML task records + events.jsonl
  Later: SQLite when querying/filtering becomes painful

Board:
  V1: CLI status output
  Optional V1/V1.5: ratatui + crossterm TUI
  Later Web Board: Rust axum backend + React/Vite frontend, or a frontend that reads an API exposed by the Rust core

Agent integration:
  Claude Code/Codex/OpenCode integration should start as Markdown instructions, skills, or thin wrappers that call the shared aoc CLI.
  Do not put durable state inside agent-specific plugins.
```

Technology boundaries:

- Rust owns task storage, policy decisions, launcher behavior, adapter capability records, recovery commands, and CLI output.
- Markdown/YAML owns prompts, task templates, policy configuration, and agent usage instructions.
- Shell is allowed only for thin wrappers around real agent commands.
- Web UI is not part of the V1 core.
- A daemon/service is not part of V1 unless the board later needs live updates.

Avoid in V1:

- A database-first design.
- A permanent background service.
- A full Web app before the dispatcher is stable.
- Rewriting agent workflow logic inside Rust.
- Heavy profile isolation or container sandboxing.

The first implementation should therefore be a Rust CLI with boring file storage and explicit commands. This keeps the core robust while leaving room for a TUI, Web Board, SQLite, stronger isolation, and richer ACP support later.

## 14. Risks and Mitigations

ACP resume is inconsistent across implementations.

- Mitigation: Use tmux attach and native CLI resume as the human recovery paths.

Native session ID capture may be unreliable.

- Mitigation: Mark adapter capability clearly. Require confirmation when only tmux recovery is available.

Child agents may not report structured progress.

- Mitigation: Capture transcript and let the main agent or a summarizer write progress events.

Existing workflows may interfere with each other.

- Mitigation: V1 does not modify global settings. Stronger config-home isolation is a later feature.

Automation may increase review burden.

- Mitigation: Review queue shows summaries, evidence, risk, and recommended action rather than raw logs.

Codex cost may grow unexpectedly.

- Mitigation: Codex requires confirmation by default.

## 15. Acceptance Criteria

V1 is successful when:

1. The user can create a task through the main agent.
2. The system creates a task ID and central task record.
3. Low-risk tasks can be automatically dispatched to a free agent.
4. Codex dispatch asks for confirmation.
5. Child agents run in tmux sessions.
6. `aoc task status` shows state, progress, next action, and review need.
7. `aoc task resume <id>` shows attach and native resume commands when available.
8. Running child agents can be recovered with `tmux attach`.
9. Exited Claude Code and Codex sessions can be recovered through recorded native resume commands when adapter support is available.
10. Code-changing tasks enter `ready_for_review` before `done`.
11. Review accept/request-changes updates task state.
12. After context switching, the user can understand the next action within one minute from status output.

## 16. Implementation Order

Recommended order:

1. Create `aoc` CLI and file-backed task store.
2. Implement task state machine.
3. Implement default policy.
4. Implement tmux launcher.
5. Add adapter skeletons for Claude Code, Codex, and OpenCode.
6. Implement status, resume, and review commands.
7. Add main-agent usage instructions or thin skills.
8. Add TUI board or Web Board after the dispatcher is reliable.

The first implementation should prefer boring local files and clear commands over complex services. The system can grow into a service-backed board later without changing the core task model.
