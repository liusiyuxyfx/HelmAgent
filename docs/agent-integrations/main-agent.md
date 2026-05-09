# Main-Agent Integration

HelmAgent should be the source of truth for delegated coding work. Claude Code, Codex, and other main agents should record task state in HelmAgent before handing work to a child agent, then read HelmAgent before reporting progress or completion to a human.

## Main-Agent Rules

- Create a HelmAgent task before delegating work.
- Run `helm-agent task status <id>` before reporting task state.
- Run `helm-agent task dispatch --dry-run --runtime <runtime> <id>` before starting a child agent.
- Do not claim code-changing work is complete until the task is ready for review or the review is accepted.
- Show attach and resume commands whenever work is delegated or recovered.
- Ask before using Codex unless the user has already approved it for the task or workspace.
- Prefer free agents for small, low-risk tasks.

## Common Commands

Create a task:

```bash
helm-agent task create --id PM-20260509-101 --title "Add retry tests" --project .
```

Record progress, a blocker, or readiness for review:

```bash
helm-agent task event PM-20260509-101 --type progress --message "Created the failing regression test"
helm-agent task event PM-20260509-101 --type blocked --message "Waiting for API contract confirmation"
helm-agent task event PM-20260509-101 --type ready_for_review --message "Implementation and tests are ready"
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

Review delegated work:

```bash
helm-agent task review PM-20260509-101 --accept
helm-agent task review PM-20260509-101 --request-changes "Add a regression test before merging"
```

Start a real tmux-backed child-agent session after the dry run looks correct:

```bash
helm-agent task dispatch --runtime claude PM-20260509-101
```

If a workspace uses a non-default tmux binary, set `HELM_AGENT_TMUX_BIN` before real dispatch. HelmAgent-created tmux sessions use the `helm-agent-` session prefix.

## Delegation Summary Template

```text
Task: PM-20260509-101 - Add retry tests
Runtime: claude
Reason: Small isolated test and implementation task
Status: queued
Attach: tmux attach -t helm-agent-PM-20260509-101-claude
Resume: claude --resume <session-id>
Review: helm-agent task review PM-20260509-101 --accept
```

## Reporting Guidance

Main agents should report HelmAgent state, not memory or assumptions. If `helm-agent task status <id>` says the task is running, report it as running. If the child agent says it is done but the task is not marked ready for review or accepted, report that review is still pending.
