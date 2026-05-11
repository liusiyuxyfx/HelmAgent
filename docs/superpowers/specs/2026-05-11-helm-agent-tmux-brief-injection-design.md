# HelmAgent Tmux Brief Injection Design

## Goal

Make child-agent handoff less manual by letting real tmux dispatch optionally send the generated brief into the child-agent session.

## Scope

- Add `helm-agent task dispatch <id> --runtime <runtime> --send-brief`.
- `--send-brief` is opt-in and only valid for real dispatch, not `--dry-run`.
- Dispatch still prepares and records `brief.md` before tmux launch.
- After tmux launch succeeds, HelmAgent sends a short handoff message into the tmux session with the brief path.
- HelmAgent records a `brief_sent` event on success.
- On send failure after tmux launch, HelmAgent still prints attach, resume, and brief paths, and records a warning event where possible.

## Handoff Message

The first version sends the brief path instead of pasting the whole brief body. This keeps the tmux input small, avoids quoting large Markdown through `tmux send-keys`, and lets a human reopen the same durable file.

The message shape is:

```text
Use this HelmAgent child-agent brief before starting work:
<brief-path>
```

It is submitted with `tmux send-keys -t <session> <message> Enter`.

## Safety And Consistency

- The brief file is still written through `TaskStore::write_brief`, which keeps it inside the sanitized session directory.
- Pre-launch persistence remains required. HelmAgent must not start tmux if the queued/recoverable task state and brief cannot be written first.
- Post-launch send failure is non-fatal because the child process is already running. HelmAgent reports the warning and leaves attach/resume/brief available for manual recovery.
- `--send-brief --dry-run` fails early so previews stay side-effect-free beyond existing dry-run task recording.

## Out Of Scope

- Sending full Markdown brief content.
- ACP transport.
- Native Claude/Codex session-id capture.
- Automatically enabling send-brief by default.
