# HelmAgent Real Run Quickstart

Use this guide from a HelmAgent source checkout after installation is healthy and you want to verify HelmAgent with a real project, a real child-agent path, and a visible review loop.

## 1. Safe Smoke Test

Start with a dry run. It uses temporary `HELM_AGENT_HOME` and project directories, initializes project guidance, creates a task, previews dispatch, syncs session health, and leaves the task at review.

```bash
make real-run-dry-run
```

This proves the local CLI, project guidance injection, task board state, brief generation, dispatch preview, sync path, and review state without launching a child agent.

## 2. Open The Board

For real use, keep a board open while the main agent coordinates work:

```bash
helm-agent board serve --host 127.0.0.1 --port 8765
```

Use the board to inspect task state, record progress, mark blockers, move work to review, and accept or request changes after reading the artifacts. To inspect a smoke task, run the board with the same `HELM_AGENT_HOME` printed by the smoke script.

## 3. Real Tmux Child Agent

Run the dry run first, then opt in to real `tmux` dispatch:

```bash
HELM_AGENT_REAL_RUN_CONFIRM=1 make real-run-tmux
```

Useful runtime overrides:

```bash
HELM_AGENT_REAL_RUN_RUNTIME=claude HELM_AGENT_REAL_RUN_CONFIRM=1 make real-run-tmux
HELM_AGENT_REAL_RUN_RUNTIME=opencode HELM_AGENT_REAL_RUN_CONFIRM=1 make real-run-tmux
HELM_AGENT_REAL_RUN_RUNTIME=codex HELM_AGENT_REAL_RUN_CONFIRM=1 make real-run-tmux
```

If Claude Code is launched through a local wrapper, configure it once before running
the smoke:

```bash
helm-agent runtime profile set claude \
  --command "mc --code" \
  --resume "mc --code --resume <session-id>"
helm-agent runtime profile doctor
```

`make real-run-tmux` copies `runtime/profile.yaml` from your current HelmAgent home
into its temporary smoke home, so the real child session uses the same wrapper
without requiring shell exports. Override the source with
`HELM_AGENT_REAL_RUN_PROFILE_HOME=/path` if needed.

The target starts a child-agent tmux session with `--send-brief`, runs `helm-agent task sync --all`, and prints the review commands. It never accepts the task automatically. Real tmux runs keep their temporary `HELM_AGENT_HOME` and project directory by default so the child session, brief, and review commands keep working.

Before running the printed review commands from a kept smoke run, use the printed
home:

```bash
export HELM_AGENT_HOME=/tmp/helm-agent-real-run.xxxxxx
```

## 4. Real ACP Agent

If you have an ACP-compatible stdio agent, register or reuse it, then run the ACP smoke path.

Register in the smoke run:

```bash
HELM_AGENT_REAL_RUN_CONFIRM=1 HELM_AGENT_REAL_RUN_ACP_COMMAND=/path/to/acp-agent make real-run-acp
```

Register with one argument:

```bash
HELM_AGENT_REAL_RUN_CONFIRM=1 \
HELM_AGENT_REAL_RUN_ACP_COMMAND=/path/to/acp-agent \
HELM_AGENT_REAL_RUN_ACP_ARG=--stdio \
make real-run-acp
```

Reuse an existing registration:

```bash
helm-agent acp agent list
helm-agent acp agent check local-acp
HELM_AGENT_REAL_RUN_CONFIRM=1 HELM_AGENT_REAL_RUN_HOME="$HELM_AGENT_HOME" HELM_AGENT_REAL_RUN_ACP_NAME=local-acp make real-run-acp
```

The ACP path runs `helm-agent acp agent check` before dispatch and requires `HELM_AGENT_REAL_RUN_CONFIRM=1`. If the check fails, fix the ACP command before sending real work. Real ACP runs keep their temporary state by default for review.

## 5. Review Gate

After a child-agent path runs, inspect the task:

```bash
helm-agent task status <Task ID printed by the smoke script>
helm-agent task brief <Task ID printed by the smoke script>
```

Then explicitly choose one review outcome:

```bash
helm-agent task review <Task ID printed by the smoke script> --request-changes "Describe the required fix"
helm-agent task review <Task ID printed by the smoke script> --accept
```

Only accept after the human or an explicitly authorized main agent has reviewed the artifacts and verification output.

## 6. Keep Or Inspect Temporary State

Dry runs clean up temporary state by default. Keep a dry-run home/project for debugging:

```bash
HELM_AGENT_REAL_RUN_KEEP=1 make real-run-dry-run
```

The script prints the temporary `HELM_AGENT_HOME` and project path so you can inspect task YAML, generated briefs, and events.
