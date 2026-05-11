# HelmAgent Tmux Sync Plan

## Task 1: Launcher Probe

- Add a `TmuxSessionState` enum.
- Add `Launcher::session_state(&self, session: &str) -> Result<TmuxSessionState>`.
- Use `tmux has-session -t <session>`.
- Add launcher tests with fake tmux scripts.

## Task 2: CLI Sync

- Add `TaskSubcommand::Sync`.
- Support exactly one target mode: `<id>` or `--all`.
- Implement status mutation rules from the design.
- Append `sync_alive`, `sync_missing`, or `sync_no_session` events.

## Task 3: Docs

- Document `helm-agent task sync <id>` and `helm-agent task sync --all` in README and main-agent integration docs.
- Add sync to main-agent operating template rules.

## Task 4: Verification

- Run targeted tests for launcher and CLI sync.
- Run full `cargo test`, `cargo check`, `cargo fmt --check`, and `git diff --check`.
- Request parallel review before merge.
