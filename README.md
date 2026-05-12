# HelmAgent

[English](README.md) | [简体中文](README.zh-CN.md)

HelmAgent is a local coordination layer for humans working with multiple coding agents.

It gives a main agent a durable task board, child-agent handoff briefs, dispatch records,
review checkpoints, and recovery commands, so fast AI work stays inspectable instead of
turning into scattered terminal sessions and chat history.

HelmAgent is designed to run locally. It stores task state under `HELM_AGENT_HOME`, can
launch child agents through `tmux`, and can hand off work to ACP-compatible agents over
stdio.

## Features

- Durable local task records for inbox, triage, queued, running, blocked, review, and done states.
- Main-agent operating guidance for Codex, Claude Code, OpenCode, or all supported runtimes.
- Project-local `AGENTS.md` and `CLAUDE.md` includes, without modifying global agent settings.
- Child-agent task briefs with scope, recovery commands, recent events, and review instructions.
- `tmux` dispatch previews and real child-agent sessions for Claude, Codex, and OpenCode.
- ACP agent registry and one-shot ACP brief handoff for compatible stdio agents.
- Human review workflow with ready-for-review, changes-requested, and accepted states.
- Local web board for browsing tasks and recording progress from a browser.
- Install, update, repair, doctor, and uninstall commands for normal CLI lifecycle management.

## Status

HelmAgent is early-stage but usable as a local CLI. The current focus is reliable local
coordination, explicit review gates, and safe recovery when delegated work needs human
attention.

## Requirements

- macOS or another Unix-like shell environment.
- Rust toolchain with `cargo`, `rustc`, and `git`.
- `$HOME/.cargo/bin` on `PATH`.
- `tmux` for tmux-backed child-agent dispatch.
- ACP-compatible agent executable only if you want to use `--runtime acp`.

## Install

Install from GitHub:

```bash
INSTALLER=/tmp/helm-agent-install.sh
curl -fsSL https://raw.githubusercontent.com/liusiyuxyfx/HelmAgent/main/install.sh -o "$INSTALLER" && sh "$INSTALLER" install
```

Install from a local checkout:

```bash
git clone https://github.com/liusiyuxyfx/HelmAgent.git
cd HelmAgent
make install
```

The installer places the binary through `cargo install` and writes local support files
under `$HOME/.helm-agent` by default.

## Update, Repair, And Uninstall

From GitHub:

```bash
INSTALLER=/tmp/helm-agent-install.sh

curl -fsSL https://raw.githubusercontent.com/liusiyuxyfx/HelmAgent/main/install.sh -o "$INSTALLER" && sh "$INSTALLER" update
curl -fsSL https://raw.githubusercontent.com/liusiyuxyfx/HelmAgent/main/install.sh -o "$INSTALLER" && sh "$INSTALLER" repair
curl -fsSL https://raw.githubusercontent.com/liusiyuxyfx/HelmAgent/main/install.sh -o "$INSTALLER" && sh "$INSTALLER" doctor
curl -fsSL https://raw.githubusercontent.com/liusiyuxyfx/HelmAgent/main/install.sh -o "$INSTALLER" && sh "$INSTALLER" uninstall
```

From a local checkout:

```bash
make update
make repair
make doctor
make uninstall
```

Plain uninstall keeps `$HOME/.helm-agent` so task records are not deleted by accident.
Use `make uninstall-purge` or `sh ./install.sh uninstall --purge` only when you
intentionally want to remove HelmAgent data.

See [docs/install.md](docs/install.md) for dry-run mode, purge safeguards, legacy
`init-project`, and environment overrides.

## Quick Start

Initialize one project so your main agent can discover HelmAgent instructions:

```bash
helm-agent project init --path /path/to/project --agent all
```

This adds project-local includes to `AGENTS.md` and `CLAUDE.md` pointing at the installed
template under `$HOME/.helm-agent/main-agent-template.md`.

Start or instruct a main agent with the generated operating prompt:

```bash
helm-agent agent prompt --runtime codex
helm-agent agent prompt --runtime claude
helm-agent agent prompt --runtime opencode
```

Create and triage a task:

```bash
helm-agent task create --id PM-20260511-001 --title "Add retry tests" --project .
helm-agent task triage PM-20260511-001 --risk medium --priority high --runtime claude --review-reason "Touches retry policy"
```

Open the task board:

```bash
helm-agent task board
helm-agent board serve --host 127.0.0.1 --port 8765
```

Prepare or start a child-agent handoff:

```bash
helm-agent task dispatch PM-20260511-001 --runtime claude --dry-run
helm-agent task dispatch PM-20260511-001 --runtime claude --send-brief
```

Mark the task for human review:

```bash
helm-agent task mark PM-20260511-001 --ready-for-review --message "Implementation and tests are ready"
helm-agent task review PM-20260511-001 --request-changes "Add a regression test before merging"
helm-agent task review PM-20260511-001 --accept
```

## ACP Agents

HelmAgent can register ACP-compatible agents and send a generated task brief as a
one-shot prompt over stdio.

```bash
helm-agent acp agent add local-acp --command /path/to/acp-agent --arg=--stdio
helm-agent acp agent list
helm-agent task dispatch PM-20260511-001 --runtime acp --agent local-acp --dry-run
helm-agent task dispatch PM-20260511-001 --runtime acp --agent local-acp --confirm
```

ACP dispatch records the ACP session id and moves the task to `ready_for_review` after
the handoff completes. Failed or timed-out ACP dispatches move the task to
`needs_changes` so the agent config can be fixed and retried.

## Common Commands

List tasks:

```bash
helm-agent task list
helm-agent task list --review
helm-agent task list --status blocked --status ready_for_review
```

Inspect or resume one task:

```bash
helm-agent task status PM-20260511-001
helm-agent task resume PM-20260511-001
```

Generate a child-agent brief:

```bash
helm-agent task brief PM-20260511-001
helm-agent task brief PM-20260511-001 --write
```

Record progress manually:

```bash
helm-agent task event PM-20260511-001 --type progress --message "Tests are running"
helm-agent task mark PM-20260511-001 --blocked --message "Waiting for API contract confirmation"
helm-agent task mark PM-20260511-001 --ready-for-review --message "Ready for review"
```

Sync tmux-backed session health before reporting delegated session health:

```bash
helm-agent task sync PM-20260511-001
helm-agent task sync --all
```

## Data And Isolation

By default HelmAgent writes only:

```text
$HOME/.helm-agent/
```

and project files you explicitly initialize:

```text
AGENTS.md
CLAUDE.md
```

It does not install global Claude Code hooks, Codex config, skills, agents, or ACP
servers. Project initialization uses include lines so existing workflows can stay
separate.

Useful environment variables:

```bash
export HELM_AGENT_HOME="$HOME/.helm-agent"
export HELM_AGENT_TMUX_BIN=tmux
export HELM_AGENT_ACP_TIMEOUT_MS=300000
export HELM_AGENT_CLAUDE_COMMAND="mc --code"
export HELM_AGENT_CLAUDE_RESUME_COMMAND="mc --code --resume <session-id>"
export HELM_AGENT_CODEX_COMMAND=codex
export HELM_AGENT_CODEX_RESUME_COMMAND="codex resume <session-id> --all"
export HELM_AGENT_OPENCODE_COMMAND=opencode
```

Runtime command overrides are optional. They are useful when the executable HelmAgent
should launch differs from the runtime name. For example, if Claude Code is exposed as
`mc --code` on your machine, set `HELM_AGENT_CLAUDE_COMMAND` and
`HELM_AGENT_CLAUDE_RESUME_COMMAND` before dispatch. These values are passed to tmux as
trusted shell command strings; use a wrapper script if the command path needs complex
quoting. Set `HELM_AGENT_OPENCODE_RESUME_COMMAND` only when your OpenCode version
supports native resume.

## Development

Run tests:

```bash
cargo test
```

Run the CLI from the checkout:

```bash
cargo run --bin helm-agent -- task create --id PM-20260512-DEV --title "Example task" --project .
cargo run --bin helm-agent -- task status PM-20260512-DEV
```

Before submitting changes:

```bash
cargo fmt -- --check
cargo test
git diff --check
```

## Documentation

- [Install Guide](docs/install.md)
- [Main-Agent Integration](docs/agent-integrations/main-agent.md)

## License

MIT. See [LICENSE](LICENSE).
