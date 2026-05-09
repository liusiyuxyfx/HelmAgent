# HelmAgent

HelmAgent is a local coordination CLI for coding agents with task records, tmux sessions, recovery commands, and review checkpoints.

## V1 Focus

- Durable task records under `HELM_AGENT_HOME`.
- Main-agent workflows that keep HelmAgent as the source of truth for task state.
- Child-agent dispatch previews and tmux-backed sessions.
- Attach, resume, and review checkpoints for recovering delegated work.

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

## Agent Integrations

See [Main-Agent Integration](docs/agent-integrations/main-agent.md) for rules and command examples for Claude Code, Codex, and other main agents.
