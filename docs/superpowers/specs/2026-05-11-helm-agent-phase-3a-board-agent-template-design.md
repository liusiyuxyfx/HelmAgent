# HelmAgent Phase 3A Board And Agent Template Design

## Goal

Make HelmAgent usable as a daily coordination surface for a main agent before adding a full UI. This phase adds a human-readable task board and a copyable main-agent operating template.

## Scope

In scope:

- `helm-agent task board`
- Board output grouped by work lane.
- Board output that highlights blocker/review/recovery context.
- A main-agent prompt/template document that can be copied into Codex, Claude Code, or project instructions.
- README and integration docs updated to point at the board and template.

Out of scope:

- Web UI, TUI, or persistent daemon.
- ACP protocol implementation.
- tmux session health probing.
- Automatic task creation from natural-language chat.

## `task board`

Command:

```bash
helm-agent task board
```

Rules:

- Reads all stored tasks through `TaskStore::list_tasks`.
- Excludes `archived` tasks by default.
- Sorts each lane by `updated_at` descending.
- Shows only lanes that contain tasks.
- Prints `No active tasks` when no non-archived tasks exist.

Lanes:

- `Inbox`: `inbox`
- `Triaged`: `triaged`
- `Queued`: `queued`
- `Running`: `running`
- `Blocked`: `blocked`, `waiting_user`
- `Review`: tasks with `review.state = required`, plus `ready_for_review`, `reviewing`, `needs_changes`
- `Done`: `done`

If a task could fit more than one lane, the board uses the first matching lane in this priority order:

1. `Blocked`
2. `Review`
3. exact status lane

This keeps human-attention work visible even when the task status is still `triaged`.

Task lines:

```text
- PM-20260511-001 [medium/claude/high] Add retry tests
  next: Human review required
  last: Patch and tests ready
  review: Touches retry policy
  attach: tmux attach -t helm-agent-PM-20260511-001-claude
  resume: claude --resume <session-id>
```

Line rules:

- First line always includes id, risk, runtime or `-`, priority, and title.
- `next` and `last` are always shown.
- `blocker`, `review`, `attach`, and `resume` are shown only when present.

## Main-Agent Template

Create:

```text
docs/agent-integrations/main-agent-template.md
```

The template is written as direct operating instructions for a coordinating agent. It should be copyable into `AGENTS.md`, Claude Code project instructions, or a Codex prompt.

Required behavior:

- Treat HelmAgent as the source of truth.
- Start each delegated task with `task create`.
- Use `task triage` before dispatch.
- Prefer free runtimes (`claude`, `opencode`) unless Codex is explicitly approved.
- Run `task board` before reporting multi-task status.
- Run `task dispatch --dry-run` before real dispatch.
- Require approval and `--confirm` for paid Codex or medium/high risk dispatch.
- Use `task mark --blocked` for blockers.
- Use `task mark --ready-for-review` only after artifacts and verification are available.
- Never claim completion until `task review --accept` has been run.
- Show attach/resume commands after delegation.

## Testing

Add CLI integration tests for:

- Board grouping across inbox, running, blocked, review, and done lanes.
- Board hiding archived tasks.
- Board surfacing triaged tasks with `review.state = required` in the Review lane.
- Board showing attach/resume context after dry-run dispatch.
- Main-agent template file existence and required command phrases.

## Acceptance

- `rtk cargo test` passes.
- `rtk cargo fmt --check` passes.
- Smoke flow can create, triage, dispatch dry-run, mark ready, and show a useful board.
- Docs do not overclaim ACP, Web UI, or automatic orchestration.
