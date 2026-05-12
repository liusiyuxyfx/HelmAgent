# HelmAgent Real Run Quickstart Design

## Goal

Provide a lightweight, repeatable path for validating HelmAgent with a real local workflow after installation, without making real child-agent dispatch accidental.

## Scope

- Add a documented quickstart for safe dry run, real tmux dispatch, and ACP dispatch.
- Add Make targets that execute those paths from the checkout.
- Add a shell smoke script that uses temporary state by default and requires explicit confirmation before real tmux dispatch.
- Keep review acceptance manual.

## Design

The script is a thin orchestration wrapper around existing CLI commands. It creates or reuses `HELM_AGENT_HOME`, creates or reuses a project directory, initializes project guidance, creates and triages a smoke task, then dispatches by mode:

- `dry-run`: preview tmux dispatch, sync, and mark ready for review.
- `tmux`: require `HELM_AGENT_REAL_RUN_CONFIRM=1`, launch the runtime in tmux with `--send-brief`, sync, and print review commands.
- `acp`: optionally register an ACP command, run `helm-agent acp agent check`, dispatch with `--runtime acp`, sync, and print review commands.

Temporary state is cleaned up by default. `HELM_AGENT_REAL_RUN_KEEP=1` keeps the home/project for debugging.

## Safety

Real tmux dispatch is opt-in through `HELM_AGENT_REAL_RUN_CONFIRM=1`. The script does not run `task review --accept`; it prints the review commands so the human or authorized main agent chooses the outcome.

## Testing

Tests assert that the quickstart, Makefile targets, and script contain the required safe and real-run command paths. Runtime verification uses `make real-run-dry-run` plus the normal Rust test suite.
