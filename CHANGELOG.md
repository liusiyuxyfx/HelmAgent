# Changelog

## v0.1.0 - 2026-05-13

First usable HelmAgent release.

### Added

- Local task records with inbox, triage, queued, running, blocked, review, done, and archived states.
- `helm-agent-coordinator` skill and project guidance includes for `AGENTS.md` and `CLAUDE.md`.
- Child-agent briefs with task scope, recent events, review instructions, attach commands, and resume commands.
- Tmux-backed dispatch previews and real child-agent sessions for Claude, Codex, and OpenCode.
- ACP agent registry, `helm-agent acp preset install claude-code`, ACP agent checks, and ACP human handoff resume commands.
- Interactive local board through `helm-agent board serve`, including task actions and copy controls for brief and resume data.
- Install, update, repair, doctor, uninstall, purge, and project initialization workflow through `install.sh`, `make`, and `helm-agent doctor`.
- Runtime profile diagnostics for local command and resume overrides.

### Safety Notes

- HelmAgent stores local state under `HELM_AGENT_HOME`, defaulting to `$HOME/.helm-agent`.
- Project initialization uses local include files and does not install global Claude Code hooks, Codex config, global skills, global agents, or ACP servers.
- Real child-agent dispatch remains explicit; dry-run, review gates, and recovery commands are part of the normal workflow.
