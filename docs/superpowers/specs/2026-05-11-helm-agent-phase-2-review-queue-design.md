# HelmAgent Phase 2 Review Queue Design

## Goal

Phase 2 turns HelmAgent from a task recorder into a small review queue and triage console. The main agent should be able to list active work, move tasks into real review/blocker states, and set policy inputs before dispatch.

## Scope

In scope:

- `helm-agent task list`
- `helm-agent task mark`
- `helm-agent task triage`
- CLI output helpers for compact task tables
- file-store enumeration for task records
- documentation updates for the main-agent flow

Out of scope:

- TUI or web board
- ACP or teams integration
- background daemon
- automatic acceptance of completed code work
- native session ID capture from child agents

## User Flow

1. A main agent creates a task.
2. The main agent triages risk, priority, suggested runtime, and review reason.
3. The main agent dispatches or dry-runs a child agent.
4. The main agent records progress.
5. The main agent marks the task as blocked or ready for review.
6. The human uses `task list --review` to see pending review items.
7. The human or explicitly authorized main agent uses `task review --accept` or `--request-changes`.

## Commands

### `task list`

Default:

```bash
helm-agent task list
```

Lists non-archived tasks sorted by `updated_at` descending.

Filters:

```bash
helm-agent task list --status running
helm-agent task list --status queued --status running
helm-agent task list --review
```

`--review` shows tasks that need human attention:

- tasks with `review.state = required`
- `ready_for_review`
- `needs_changes`
- `reviewing`

Output columns:

- id
- status
- risk
- runtime
- title
- last_event
- next_action
- review_reason

### `task mark`

```bash
helm-agent task mark PM-20260511-001 --ready-for-review --message "Tests pass; review patch"
helm-agent task mark PM-20260511-001 --blocked --message "Waiting for API contract"
helm-agent task mark PM-20260511-001 --triaged --message "Scope and risk classified"
```

Rules:

- Exactly one state flag is required.
- `--message` is required.
- `--ready-for-review` sets `status = ready_for_review`, `review.state = required`, `progress.last_event = message`, and `progress.next_action = "Human review required"`.
- `--blocked` sets `status = blocked`, `progress.blocker = message`, `progress.last_event = message`, and `progress.next_action = "Resolve blocker"`.
- `--triaged` sets `status = triaged`, `progress.last_event = message`, and `progress.next_action = "Dispatch or defer task"`.
- Every mark appends a typed event: `ready_for_review`, `blocked`, or `triaged`.

### `task triage`

```bash
helm-agent task triage PM-20260511-001 --risk medium --priority high --runtime claude --review-reason "Touches auth flow"
```

Rules:

- At least one option is required.
- `--risk` accepts `low`, `medium`, `high`.
- `--priority` accepts `low`, `normal`, `high`.
- `--runtime` accepts `claude`, `codex`, `opencode`.
- `--review-reason` sets `review.reason` and `review.state = required`.
- When risk is set to `medium` or `high`, `review.state = required`.
- When risk is set to `low`, `review.state` is cleared to `not_required` only if no review reason is present.
- Triage appends a `triaged` event summarizing changed fields.

### Review and Dispatch Gates

- `task review --accept` and `task review --request-changes` require `status = ready_for_review` or `status = reviewing`.
- `task dispatch` only starts or plans work from `inbox`, `triaged`, `queued`, or `needs_changes`.
- `done`, `archived`, `ready_for_review`, `reviewing`, `blocked`, and `waiting_user` tasks must not be dispatched.

## Data Model

No new persisted top-level fields are required.

Existing fields are used:

- `TaskRecord.status`
- `TaskRecord.priority`
- `TaskRecord.risk`
- `TaskRecord.assignment.runtime`
- `TaskRecord.progress.last_event`
- `TaskRecord.progress.next_action`
- `TaskRecord.progress.blocker`
- `TaskRecord.review.state`
- `TaskRecord.review.reason`

## Store

`TaskStore` gains `list_tasks() -> Result<Vec<TaskRecord>>`.

Implementation:

- scan `$HELM_AGENT_HOME/tasks/**/**/*.yaml`
- parse records through the same task-id mismatch guard used by `load_task`
- ignore non-yaml files
- sort results in the output layer or CLI by `updated_at` descending

If a corrupt task file is found, listing fails with path context. Silent omission would hide data loss.

## Policy Interaction

Phase 2 does not change the policy engine. It gives policy real inputs:

- `task triage --risk medium|high` makes real dispatch require `--confirm`
- `task triage --runtime codex` records suggested runtime but does not dispatch
- `task list --review` surfaces review-required tasks

## Error Handling

- `task mark` with no state flag fails.
- `task mark` with multiple state flags fails through Clap conflicts.
- `task mark` without message fails.
- `task triage` with no options fails.
- invalid risk/runtime/status values fail through Clap.
- `task list` on an empty store prints an empty table with no error.

## Testing

Add focused tests for:

- list output excludes archived tasks and sorts newest first
- list status filters
- list review filter
- mark ready-for-review state transition
- mark blocked state transition
- mark requiring exactly one state flag and message
- triage risk/runtime/priority/review reason
- triage no-op rejection
- policy gate honoring medium risk after triage
- docs command examples matching CLI help

## Documentation

Update:

- `README.md`
- `docs/agent-integrations/main-agent.md`

The docs should say that `ready_for_review` and `blocked` are real states in Phase 2. They should keep the human-review gate: only a human or explicitly authorized main agent should run review accept/request-changes.
