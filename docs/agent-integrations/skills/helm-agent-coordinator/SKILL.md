---
name: helm-agent-coordinator
description: Use when coordinating delegated AI coding work, managing multiple local agent tasks, dispatching through HelmAgent, or reporting review-ready task state.
---

# HelmAgent Coordinator

## Rule

HelmAgent is the source of truth for delegated task state. Do not rely on memory when a `helm-agent` command can read the current state.

## Flow

1. Inspect active work before reporting status:

```bash
helm-agent task board
```

2. Create and triage before delegation:

```bash
helm-agent task create --id PM-YYYYMMDD-001 --title "<short task title>" --project .
helm-agent task triage PM-YYYYMMDD-001 --risk low --priority normal --runtime claude
```

3. Prefer low-cost runtimes first. Use `claude` or `opencode` for routine work. Ask before using `codex` unless the user already approved it.

4. Preview dispatch before starting work:

```bash
helm-agent task dispatch PM-YYYYMMDD-001 --runtime claude --dry-run
helm-agent task dispatch PM-YYYYMMDD-001 --runtime acp --agent claude-code --dry-run
```

5. Start real work only after the preview is reasonable. Use `--confirm` when policy requires it:

```bash
helm-agent task dispatch PM-YYYYMMDD-001 --runtime claude --send-brief
helm-agent task dispatch PM-YYYYMMDD-001 --runtime acp --agent claude-code --confirm
```

`--send-brief` is opt-in for tmux dispatch. If dispatch prints `Brief sent: no`, use `helm-agent task brief <id>` and `helm-agent task resume <id>` for manual recovery.

6. Use the brief as the child-agent handoff package:

```bash
helm-agent task brief PM-YYYYMMDD-001
helm-agent task brief PM-YYYYMMDD-001 --write
```

7. Show recovery commands when a human may need to inspect or continue the session:

```bash
helm-agent task resume PM-YYYYMMDD-001
```

8. Sync tmux state before reporting delegated session health:

```bash
helm-agent task sync PM-YYYYMMDD-001
helm-agent task sync --all
```

9. Record progress and blockers as facts:

```bash
helm-agent task event PM-YYYYMMDD-001 --type progress --message "<short factual update>"
helm-agent task mark PM-YYYYMMDD-001 --blocked --message "<what blocks progress>"
```

10. Do not claim code-changing work is complete until review is accepted. Mark ready only after artifacts and verification are available:

```bash
helm-agent task mark PM-YYYYMMDD-001 --ready-for-review --message "<artifacts and verification>"
helm-agent task review PM-YYYYMMDD-001 --accept
helm-agent task review PM-YYYYMMDD-001 --request-changes "<required follow-up>"
```

## Status Report

Use this shape:

```text
Board: helm-agent task board
Task: PM-YYYYMMDD-001 - <title>
Status: <status from HelmAgent>
Runtime: <claude|opencode|codex|acp>
Last: <last event>
Next: <next action>
Attach: <tmux attach command or none>
Resume: <native or ACP resume command or none>
Brief: <child-agent brief path or none>
Review: <not ready|ready for review|accepted|changes requested>
```
