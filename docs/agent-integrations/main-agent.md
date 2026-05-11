# Main-Agent Integration

HelmAgent should be the source of truth for delegated coding work. Claude Code, Codex, and other main agents should record task state in HelmAgent before handing work to a child agent, then read HelmAgent before reporting progress or completion to a human.

## Project Setup

Install project-local guidance before starting the main agent:

```bash
helm-agent project init --path . --agent all
```

Runtime-specific setup:

```bash
helm-agent project init --path . --agent codex
helm-agent project init --path . --agent claude
helm-agent project init --path . --agent opencode
```

Manual startup prompt:

```bash
helm-agent agent prompt --runtime codex
helm-agent agent prompt --runtime claude
helm-agent agent prompt --runtime opencode
```

## Main-Agent Rules

- Create a HelmAgent task before delegating work.
- Run `helm-agent task status <id>` before reporting task state.
- Run `helm-agent task board` before reporting multi-task state.
- Run `helm-agent task sync <id>` before reporting whether a delegated tmux session is still active.
- Run `helm-agent task brief <id>` before handoff when a child agent or human needs copyable task context.
- Use `helm-agent task triage <id>` to record risk, priority, preferred runtime, and review reason before dispatch.
- Run `helm-agent task dispatch --dry-run --runtime <runtime> <id>` before starting a child agent.
- Do not claim code-changing work is complete until `task review --accept` has been run. Before that, report it as ready for review once the task is marked ready and the artifacts are presented to the human.
- Only the human or an explicitly authorized main agent should run `helm-agent task review --accept` or `helm-agent task review --request-changes`.
- Show attach and resume commands whenever work is delegated or recovered.
- Ask before using Codex unless the user has already approved it for the task or workspace.
- Use `--confirm` only after approval when a real dispatch is blocked by policy, such as Codex or elevated-risk work.
- Prefer free agents for small, low-risk tasks.

## Common Commands

Create a task:

```bash
helm-agent task create --id PM-20260509-101 --title "Add retry tests" --project .
```

Triage the task before dispatch:

```bash
helm-agent task triage PM-20260509-101 --risk medium --priority high --runtime claude --review-reason "Touches retry policy"
```

Record progress notes:

```bash
helm-agent task event PM-20260509-101 --type progress --message "Created the failing regression test"
```

Use `task event` for notes only. Use `task mark` when the task state should change.

Mark a blocker or ready-for-review state:

```bash
helm-agent task mark PM-20260509-101 --blocked --message "Waiting for API contract confirmation"
helm-agent task mark PM-20260509-101 --ready-for-review --message "Implementation and tests are ready"
```

List active tasks or the human review queue:

```bash
helm-agent task list
helm-agent task list --review
helm-agent task list --status blocked --status ready_for_review
helm-agent task board
helm-agent board html
```

Preview child-agent dispatch before starting anything:

```bash
helm-agent task dispatch --dry-run --runtime claude PM-20260509-101
helm-agent task dispatch --dry-run --runtime codex PM-20260509-101
helm-agent task dispatch --dry-run --runtime opencode PM-20260509-101
```

Check current state before reporting:

```bash
helm-agent task status PM-20260509-101
```

Show recovery commands:

```bash
helm-agent task resume PM-20260509-101
```

Render or persist a child-agent brief:

```bash
helm-agent task brief PM-20260509-101
helm-agent task brief PM-20260509-101 --write
```

Dry-run and real dispatch write the child-agent brief automatically and record the path on the task.

Sync recorded tmux sessions before reporting child-agent health:

```bash
helm-agent task sync PM-20260509-101
helm-agent task sync --all
```

Human or authorized reviewer commands, after the task is ready for review:

```bash
helm-agent task review PM-20260509-101 --accept
helm-agent task review PM-20260509-101 --request-changes "Add a regression test before merging"
```

Start a real tmux-backed child-agent session after the dry run looks correct. If the task is medium or high risk, get approval and pass `--confirm`:

```bash
helm-agent task dispatch --runtime claude --confirm PM-20260509-101
```

Start Codex only after approval:

```bash
helm-agent task dispatch --runtime codex --confirm PM-20260509-101
```

If a workspace uses a non-default tmux binary, set `HELM_AGENT_TMUX_BIN` before real dispatch. HelmAgent-created tmux sessions use the `helm-agent-` session prefix.

Serve the read-only browser board when useful:

```bash
helm-agent board serve --host 127.0.0.1 --port 8765
```

## Delegation Summary Template

```text
Task: PM-20260509-101 - Add retry tests
Runtime: claude
Reason: Small isolated test and implementation task
Status: ready_for_review
Attach: tmux attach -t helm-agent-PM-20260509-101-claude
Resume: claude --resume <session-id>
Brief: /Users/you/.helm-agent/sessions/PM-20260509-101/brief.md
Review: Inspect artifacts, then run helm-agent task review PM-20260509-101 --accept or --request-changes "<message>"
```

## Reporting Guidance

Main agents should report HelmAgent state, not memory or assumptions. If `helm-agent task status <id>` says the task is running, report it as running. If the child agent says it is done but the task is not marked ready for review or artifacts have not been presented to the human, report that review handoff is still pending. If the task is ready for review but not accepted, report that implementation is ready for review, not complete.

Use `helm-agent task sync <id>` before describing tmux session health. `sync` can mark a running task blocked when its recorded tmux session is gone, while dry-run queued tasks stay queued.

For copyable coordinator instructions, use [Main-Agent Operating Template](main-agent-template.md).
