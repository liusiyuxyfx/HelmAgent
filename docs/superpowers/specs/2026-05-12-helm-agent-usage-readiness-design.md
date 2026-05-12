# HelmAgent Usage Readiness Design

## Goal

Make HelmAgent ready for daily dogfood by tightening three existing surfaces:
main-agent instructions, the interactive board, and ACP adapter verification.

## Scope

- Main-agent guidance should tell Codex, Claude Code, and OpenCode coordinators how to run the full loop: create, triage, dry-run, dispatch, sync, review, and accept.
- The web board should remain the existing loopback HTML app, but become easier to operate for review-heavy work.
- ACP should keep using the official `agent-client-protocol` Rust crate and expose a practical verification command for configured agents.

## Non-Goals

- No frontend framework.
- No websocket server.
- No long-running ACP daemon.
- No global Claude Code, Codex, or OpenCode config mutation.

## Design

### Main-Agent Readiness

The installed main-agent template remains the single source of coordinator behavior.
`project init` keeps adding project-local includes only. The template should add a
dogfood checklist and make the board server, task sync, and review acceptance rules
visible enough that a coordinator can operate HelmAgent without remembering commands.

### Interactive Board

The current board already has token-protected write APIs. This iteration keeps the
same endpoints and improves usability with status filters, task detail fields, and
test coverage for all visible actions. The server remains loopback-only, and POSTs
continue to require the per-server token embedded in the page.

### ACP Verification

ACP dispatch already uses `agent-client-protocol` over stdio. Add a lightweight
`helm-agent acp agent check <name>` command that loads a configured agent, starts it,
runs the same initialize/session/prompt handshake against a temporary project
directory, and prints the session id and stop reason. This proves a configured ACP
agent is reachable before a real task dispatch.

## Testing

- Guidance tests cover the dogfood checklist and runtime override guidance.
- Web board tests cover events, mark actions, review actions, token rejection, and
  the HTML wiring for filters/actions.
- ACP tests use the existing fake ACP agent to check successful `agent check` and a
  failing process to check failure output.
- Full gate remains `cargo test`, `cargo fmt -- --check`, `cargo check`, and
  `git diff --check`.
