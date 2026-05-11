# HelmAgent Agent Integration Sprint Design

## Goal

Make HelmAgent usable through a main agent, not by forcing the human to memorize CLI commands.

## Scope

This sprint builds the smallest practical loop:

- Project-level onboarding for Codex, Claude Code, OpenCode, or all supported agents.
- A command that prints main-agent operating instructions on demand.
- Better tmux dispatch/resume output for recovering child-agent sessions.
- A read-only local board view that can be served in a browser.

ACP remains out of scope for this sprint. ACP should become an adapter transport after the tmux recovery loop is reliable.

## User Workflow

Project onboarding:

```bash
helm-agent project init --path . --agent all
```

Main-agent startup:

```bash
helm-agent agent prompt --runtime codex
helm-agent task board
```

Delegation and recovery:

```bash
helm-agent task dispatch PM-YYYYMMDD-001 --runtime opencode --confirm
helm-agent task resume PM-YYYYMMDD-001
```

Read-only board:

```bash
helm-agent board serve --host 127.0.0.1 --port 8765
```

## Project Integration Rules

`project init` writes only project-local files. It must not modify global Claude Code, Codex, OpenCode, hook, skill, or settings files.

Agent targets:

- `codex`: update `AGENTS.md`
- `claude`: update `CLAUDE.md`
- `opencode`: update `AGENTS.md`
- `all`: update both `AGENTS.md` and `CLAUDE.md`

The include line is:

```text
@<HELM_AGENT_HOME>/main-agent-template.md
```

Writes are idempotent. Running the command twice must not duplicate the include line.

## Agent Prompt Rules

`agent prompt` prints a concise bootstrap prompt plus the installed main-agent template content. Runtime-specific output should tell the main agent to:

- Treat HelmAgent as task state source of truth.
- Run `helm-agent task board` before status reports.
- Create, triage, dispatch, mark, and review tasks through HelmAgent.
- Ask for confirmation before paid Codex or elevated-risk dispatch.

## Board Rules

The web board is read-only in this sprint. It may use a simple blocking local HTTP server. The page should escape task content, auto-refresh, and show the same task lanes as `helm-agent task board`.

## Dispatch/Resume Rules

Existing tmux dispatch remains the default recovery mechanism. This sprint can improve output and prompts, but should not depend on ACP or native agent session ID capture.

## Acceptance

- `helm-agent project init --path <tmp> --agent all` writes `AGENTS.md` and `CLAUDE.md` idempotently.
- `helm-agent agent prompt --runtime codex` prints the HelmAgent coordinator instructions.
- `helm-agent board serve --once` or equivalent testable board rendering path returns escaped HTML.
- Existing dispatch/resume tests still pass.
- `rtk cargo test` and `rtk cargo fmt --check` pass.
