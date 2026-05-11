# HelmAgent Task Brief Design

## Goal

Give every child-agent task a stable, reviewable brief so a human or main agent can hand the same context to Claude Code, Codex, or OpenCode after tmux dispatch.

## Scope

- Add `helm-agent task brief <id>` to render a Markdown child-agent brief to stdout.
- Add `helm-agent task brief <id> --write` to write the brief to `$HELM_AGENT_HOME/sessions/<task-id>/brief.md`.
- During `task dispatch`, write the brief automatically and record its path on the task.
- Show the brief path in `task status`, `task resume`, and board output when available.

## Brief Content

The brief includes:

- Task id, title, project path, branch, status, risk, priority, runtime.
- Current progress fields: summary, last event, next action, blocker.
- Review state, review reason, and artifacts.
- Recovery commands: attach and native resume when known.
- Recent task events.
- Child-agent operating instructions: inspect the project, make only scoped changes, run verification, report artifacts, and mark ready-for-review through HelmAgent when done.

## Safety

Brief files are rooted under the existing sanitized session directory. The brief command must not write outside `$HELM_AGENT_HOME`. Rendering to stdout is non-mutating.

## Out Of Scope

- Automatic `tmux send-keys` prompt injection.
- ACP prompt transport.
- Native Claude/Codex session id capture.

These can be added after brief generation is stable.
