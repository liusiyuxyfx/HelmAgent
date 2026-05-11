# HelmAgent Tmux Sync Design

## Goal

Add a small recovery loop for delegated tmux sessions so a main agent can ask HelmAgent whether recorded child-agent sessions still exist before reporting progress.

## Scope

- Add `helm-agent task sync <id>` to probe one task's recorded tmux session.
- Add `helm-agent task sync --all` to probe all active tasks that have a recorded tmux session.
- Use `tmux has-session -t =<session>` through the configured `HELM_AGENT_TMUX_BIN` so tmux uses exact target matching.
- Keep ACP and native agent session-id capture out of scope.

## State Rules

- If a task has no recorded tmux session, report `no_session` and do not mutate the task.
- If a recorded session exists, set `running` for tasks in `queued`, `running`, or `blocked`, clear only HelmAgent-created tmux-missing blockers, and record a `sync_alive` event.
- If a recorded session is missing and the task is `running`, set `blocked`, record blocker text, and append a `sync_missing` event.
- If a recorded session is missing and the task is `queued`, keep it queued. This preserves dry-run dispatch previews, where a tmux session is intentionally not started.
- Skip `done` and `archived` tasks during `--all`.

## Output

Each sync prints one line per task:

```text
PM-20260511-001 alive helm-agent-PM-20260511-001-claude
PM-20260511-002 missing helm-agent-PM-20260511-002-codex
PM-20260511-003 no_session
```

The command should be useful to both humans and main agents without requiring JSON in this iteration.

## Safety

Tmux session names are passed as process arguments, never through a shell. A missing tmux binary is an error. A non-zero `tmux has-session` means the session is missing.

## Tests

- Launcher tests cover `has-session -t =<session>` for alive and missing sessions.
- CLI tests cover one-task sync, `--all`, no-session output, missing running session to blocked, and queued dry-run session staying queued.
