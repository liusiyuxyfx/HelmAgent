# HelmAgent Main-Agent Operating Template

Use this when acting as the coordinating agent for coding work. HelmAgent is the source of truth for delegated task state.

## Operating Rules

- Run `helm-agent task board` before reporting multi-task status.
- Run `helm-agent task status <id>` before reporting one task's status.
- Run `helm-agent task sync <id>` before reporting delegated tmux session health.
- Run `helm-agent task brief <id>` when preparing a child-agent handoff.
- Create a task before delegation:

```bash
helm-agent task create --id PM-YYYYMMDD-001 --title "<short task title>" --project .
```

- Triage before dispatch:

```bash
helm-agent task triage PM-YYYYMMDD-001 --risk low --priority normal --runtime claude
```

- Prefer free runtimes first: `claude` or `opencode`.
- Ask before using Codex unless the user has already approved it for this task or workspace.
- If a runtime uses a local wrapper command, export the override before dispatch:

```bash
export HELM_AGENT_CLAUDE_COMMAND="mc --code"
export HELM_AGENT_CLAUDE_RESUME_COMMAND="mc --code --resume <session-id>"
```

- Runtime command overrides are tmux shell command strings. Use only trusted values; use a wrapper script when the executable path needs complex quoting.
- Always preview dispatch before starting a child agent:

```bash
helm-agent task dispatch --dry-run --runtime claude PM-YYYYMMDD-001
```

- For paid Codex or medium/high risk work, get approval and pass `--confirm` on real dispatch:

```bash
helm-agent task dispatch --runtime codex --confirm PM-YYYYMMDD-001
helm-agent task dispatch --runtime claude --confirm PM-YYYYMMDD-001
```

- `--send-brief` is opt-in. Use it only on real dispatch when the brief path should be injected into tmux:

```bash
helm-agent task dispatch PM-YYYYMMDD-001 --runtime claude --send-brief
```

- If brief injection reports `Brief sent: no`, use the printed attach/resume/brief paths for manual recovery.

- After delegation, show the attach and resume commands from dispatch output or:

```bash
helm-agent task resume PM-YYYYMMDD-001
```

- Use the child-agent brief as the copyable handoff package:

```bash
helm-agent task brief PM-YYYYMMDD-001
helm-agent task brief PM-YYYYMMDD-001 --write
```

- Before reporting whether child-agent sessions are still active, sync tmux state:

```bash
helm-agent task sync PM-YYYYMMDD-001
helm-agent task sync --all
```

- Record progress as notes:

```bash
helm-agent task event PM-YYYYMMDD-001 --type progress --message "<short factual update>"
```

- Mark blockers with real state:

```bash
helm-agent task mark PM-YYYYMMDD-001 --blocked --message "<what blocks progress>"
```

- Mark review handoff only after implementation artifacts and verification are available:

```bash
helm-agent task mark PM-YYYYMMDD-001 --ready-for-review --message "<what is ready and how it was verified>"
```

- Do not claim code-changing work is complete until `task review --accept` has been run.
- If review has not accepted the work, report it as ready for review, blocked, running, or needs changes.
- Only the human or an explicitly authorized main agent should accept or request changes:

```bash
helm-agent task review PM-YYYYMMDD-001 --accept
helm-agent task review PM-YYYYMMDD-001 --request-changes "<required follow-up>"
```

## Status Report Shape

Use this shape when summarizing delegated work:

```text
Board: helm-agent task board
Sync: helm-agent task sync PM-YYYYMMDD-001
Task: PM-YYYYMMDD-001 - <title>
Status: <status from HelmAgent>
Runtime: <claude|opencode|codex>
Last: <last event>
Next: <next action>
Attach: <tmux attach command or none>
Resume: <native resume command or none>
Brief: <child-agent brief path or none>
Review: <not ready|ready for review|accepted|changes requested>
```

Report HelmAgent state, not memory or assumptions.
