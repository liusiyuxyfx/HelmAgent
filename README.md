# HelmAgent

HelmAgent is a local coordination CLI for coding agents with task records, tmux sessions, recovery commands, and review checkpoints.

## Install

Install from GitHub:

```bash
INSTALLER=/tmp/helm-agent-install.sh
curl -fsSL https://raw.githubusercontent.com/liusiyuxyfx/HelmAgent/main/install.sh -o "$INSTALLER" && sh "$INSTALLER" install
```

Update, repair, diagnose, and uninstall:

```bash
INSTALLER=/tmp/helm-agent-install.sh
curl -fsSL https://raw.githubusercontent.com/liusiyuxyfx/HelmAgent/main/install.sh -o "$INSTALLER" && sh "$INSTALLER" update
curl -fsSL https://raw.githubusercontent.com/liusiyuxyfx/HelmAgent/main/install.sh -o "$INSTALLER" && sh "$INSTALLER" repair
curl -fsSL https://raw.githubusercontent.com/liusiyuxyfx/HelmAgent/main/install.sh -o "$INSTALLER" && sh "$INSTALLER" doctor
curl -fsSL https://raw.githubusercontent.com/liusiyuxyfx/HelmAgent/main/install.sh -o "$INSTALLER" && sh "$INSTALLER" uninstall
```

Local checkout:

```bash
make install
make update
make repair
make doctor
make uninstall
```

See [Install Guide](docs/install.md) for `--dry-run`, `--purge`, legacy `init-project`, and environment options.

Initialize one project after install so the main agent can discover HelmAgent instructions:

```bash
helm-agent project init --path /path/to/project --agent all
```

This adds project-local `AGENTS.md` and `CLAUDE.md` includes for the installed coordinator template under `$HOME/.helm-agent/main-agent-template.md`.

Print a main-agent bootstrap prompt:

```bash
helm-agent agent prompt --runtime codex
helm-agent agent prompt --runtime claude
```

Open a read-only local board:

```bash
helm-agent board serve --host 127.0.0.1 --port 8765
```

## Current Focus

- Durable task records under `HELM_AGENT_HOME`.
- Main-agent workflows that keep HelmAgent as the source of truth for task state.
- Child-agent dispatch previews and tmux-backed sessions.
- Policy gates that require `--confirm` before paid Codex or elevated-risk real dispatches.
- Attach, resume, and review checkpoints for recovering delegated work.
- Review-queue commands for triage, status updates, and human handoff.

## Development

Run the test suite:

```bash
cargo test
```

Run the CLI during development:

```bash
cargo run --bin helm-agent -- task create --id PM-20260509-001 --title "Example task" --project .
cargo run --bin helm-agent -- task status PM-20260509-001
```

`HELM_AGENT_HOME` overrides where task records are stored. `HELM_AGENT_TMUX_BIN` overrides the tmux binary used for real dispatches.

## Common Commands

Create and triage a task:

```bash
helm-agent task create --id PM-20260511-001 --title "Add retry tests" --project .
helm-agent task triage PM-20260511-001 --risk medium --priority high --runtime claude --review-reason "Touches retry policy"
```

List active tasks or the human review queue:

```bash
helm-agent task list
helm-agent task list --review
helm-agent task list --status blocked --status ready_for_review
helm-agent task board
helm-agent board html
```

Mark real task state:

```bash
helm-agent task mark PM-20260511-001 --blocked --message "Waiting for API contract confirmation"
helm-agent task mark PM-20260511-001 --ready-for-review --message "Implementation and tests are ready"
helm-agent task review PM-20260511-001 --request-changes "Add a regression test before merging"
```

## Agent Integrations

See [Main-Agent Integration](docs/agent-integrations/main-agent.md) for rules and command examples for Claude Code, Codex, and other main agents.

Use `helm-agent project init --path . --agent all` to wire a project for Codex and Claude Code. Use `helm-agent agent prompt --runtime <codex|claude|opencode|all>` when starting a main agent manually.
