# HelmAgent ACP Adapter Design

## Goal

Add a first usable Agent Client Protocol adapter so HelmAgent can hand a task brief to an ACP-compatible child agent over stdio while keeping tmux dispatch unchanged.

## Scope

In scope:

- Store named ACP agents in `HELM_AGENT_HOME/acp/agents.yaml`.
- Manage ACP agents with `helm-agent acp agent add|list|remove`.
- Add `acp` as a dispatch runtime with `helm-agent task dispatch <id> --runtime acp --agent <name>`.
- Keep `--dry-run` for ACP dispatch, showing the configured command and brief path without launching.
- Real ACP dispatch starts the configured command as a child process, connects over stdio with the official `agent-client-protocol` Rust crate, initializes, creates a session for the task project path, sends the generated task brief, records the ACP session id, and appends task events.
- Human recovery remains task-record based: ACP session id, brief path, and a re-run command are recorded.

Out of scope:

- ACP session resume after the external agent exits.
- Interactive ACP terminal/file-system permission UI.
- Web board dispatch buttons.
- ACP proxy chains or MCP injection.
- Long-running daemon management of ACP child processes.

## Architecture

`src/acp_adapter.rs` has two halves:

- Config persistence: read/write named agent configs under the HelmAgent home.
- Runtime client: spawn an ACP agent process and run a bounded one-shot handoff using `agent-client-protocol` 0.11.1.

The runtime uses the current SDK shape:

1. Create `ByteStreams` from child stdin/stdout.
2. Build a `Client`.
3. Handle `SessionNotification` as a no-op in this first CLI handoff.
4. Handle `RequestPermissionRequest` by selecting a reject option when one is offered, otherwise returning cancelled.
5. Send `InitializeRequest::new(ProtocolVersion::V1)`.
6. Validate that the initialize response negotiated `ProtocolVersion::V1`.
7. Create a session with `NewSessionRequest::new(task.project.path)`.
8. Send the task brief with `PromptRequest::new(session_id, ...)`, capture the `session_id`, and treat the handoff as complete when a prompt response returns.
9. Apply a default 300s handoff timeout, overridable with `HELM_AGENT_ACP_TIMEOUT_MS` for tests or local tuning.
10. On Unix, run the child in its own process group and terminate the group after the one-shot handoff.

`src/cli.rs` keeps tmux dispatch behavior intact. Only when `--runtime acp` is selected does it require `--agent <name>` and call `acp_adapter`.

## Data Format

```yaml
agents:
  local-claude:
    command: claude-acp
    args: []
    env: {}
```

## Error Handling

- Missing ACP agent config: fail before changing task state.
- Real ACP dispatch without `--confirm`: require confirmation, because ACP child cost and write behavior are runtime-specific.
- Child process spawn or protocol errors: mark the task `needs_changes`, request changes for review, append an `acp_dispatch_failed` event, rewrite the brief, and return a CLI failure.
- If the child exits before the protocol future completes, HelmAgent waits a short grace period so agents that exit immediately after a valid prompt response are still accepted.
- Permission requests from the ACP agent are conservatively rejected when the agent offers a reject option.

## Testing

Add tests for:

- ACP config add/list/remove CLI flow.
- Requiring `--agent` when dispatch runtime is `acp`.
- ACP dry-run dispatch records runtime, brief, and recovery text without launching.
- Real ACP dispatch against a small fake ACP process records `assignment.acp_session_id`, sends the brief prompt, and marks the task ready for review.
- Failed ACP dispatch moves the task to `needs_changes`, rewrites the brief, and can be retried after the agent config is fixed.
- Noisy ACP agent stderr does not block protocol handoff.
- Child-recorded HelmAgent events are preserved in the final brief after ACP success.
- Unresponsive agents time out and leave the task retryable.
- Agents that exit immediately after sending the prompt response are accepted.

Acceptance commands:

```bash
rtk cargo test
rtk cargo fmt --check
rtk git diff --check
```
