# HelmAgent Task Brief Plan

## Goal

Add a durable child-agent brief that can be printed, written to a session file, and reused after tmux dispatch.

## Tasks

1. Add regression tests first:
   - brief rendering includes task context, recovery, recent events, and child-agent instructions.
   - `helm-agent task brief <id>` prints Markdown without writing files.
   - `helm-agent task brief <id> --write` writes `sessions/<task-id>/brief.md` and records the path.
   - dispatch writes the brief automatically and exposes the path in resume/status/board output.
2. Implement a small `brief` module with `render_task_brief`.
3. Add `Recovery.brief_path` with serde defaults for old task YAML compatibility.
4. Add `TaskStore::brief_path` and `TaskStore::write_brief`.
5. Add `task brief` CLI handling and dispatch integration.
6. Update README and the main-agent operating template.
7. Run formatting, tests, and review before merge/install.

## Constraints

- Brief files must stay under the sanitized session directory.
- Stdout rendering must not mutate task state.
- Dispatch should keep existing tmux behavior unchanged except for creating and reporting the brief path.
