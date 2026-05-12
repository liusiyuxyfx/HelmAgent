#!/bin/sh
set -eu

usage() {
  cat <<'USAGE'
Usage: scripts/real_run_smoke.sh --mode dry-run|tmux|acp

Environment:
  HELM_AGENT_BIN                         HelmAgent command. Default: helm-agent
  HELM_AGENT_REAL_RUN_CONFIRM=1          Required for real tmux or ACP dispatch.
  HELM_AGENT_REAL_RUN_RUNTIME=claude     Runtime for dry-run/tmux. Default: claude
  HELM_AGENT_REAL_RUN_PROJECT=/path      Project to initialize. Default: temp dir
  HELM_AGENT_REAL_RUN_HOME=/path         HelmAgent home. Default: temp dir
  HELM_AGENT_REAL_RUN_ID=PM-YYYYMMDD-ID  Task id. Default: timestamp-based id
  HELM_AGENT_REAL_RUN_KEEP=1             Keep temp home/project after exit.
  HELM_AGENT_REAL_RUN_ACP_NAME=name      ACP agent name. Default: real-run-acp
  HELM_AGENT_REAL_RUN_ACP_COMMAND=cmd    Optional ACP command to register first.
  HELM_AGENT_REAL_RUN_ACP_ARG=arg        Optional single ACP arg to register.
USAGE
}

MODE="dry-run"
while [ "$#" -gt 0 ]; do
  case "$1" in
    --mode)
      MODE="${2:-}"
      shift 2
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      printf 'unknown argument: %s\n' "$1" >&2
      usage >&2
      exit 2
      ;;
  esac
done

case "$MODE" in
  dry-run|tmux|acp) ;;
  *)
    printf 'unsupported mode: %s\n' "$MODE" >&2
    usage >&2
    exit 2
    ;;
esac

HELM_AGENT_BIN="${HELM_AGENT_BIN:-helm-agent}"
TASK_ID="${HELM_AGENT_REAL_RUN_ID:-PM-$(date +%Y%m%d%H%M%S)-REALRUN}"
TASK_TITLE="${HELM_AGENT_REAL_RUN_TITLE:-HelmAgent real run smoke}"
RUNTIME="${HELM_AGENT_REAL_RUN_RUNTIME:-claude}"
KEEP="${HELM_AGENT_REAL_RUN_KEEP:-0}"
CONFIRM="${HELM_AGENT_REAL_RUN_CONFIRM:-0}"
ACP_NAME="${HELM_AGENT_REAL_RUN_ACP_NAME:-real-run-acp}"
ACP_COMMAND="${HELM_AGENT_REAL_RUN_ACP_COMMAND:-}"
ACP_ARG="${HELM_AGENT_REAL_RUN_ACP_ARG:-}"

if [ "$MODE" = "tmux" ] && [ "$CONFIRM" != "1" ]; then
  cat >&2 <<CONFIRMATION
Refusing real tmux dispatch without HELM_AGENT_REAL_RUN_CONFIRM=1.

Preview first:
  make real-run-dry-run

Then run a real tmux child-agent session:
  HELM_AGENT_REAL_RUN_CONFIRM=1 make real-run-tmux
CONFIRMATION
  exit 3
fi

if [ "$MODE" = "acp" ] && [ "$CONFIRM" != "1" ]; then
  cat >&2 <<CONFIRMATION
Refusing real ACP dispatch without HELM_AGENT_REAL_RUN_CONFIRM=1.

Check connectivity first:
  helm-agent acp agent check <name>

Then run a real ACP child-agent handoff:
  HELM_AGENT_REAL_RUN_CONFIRM=1 make real-run-acp
CONFIRMATION
  exit 3
fi

OWN_HOME=0
OWN_PROJECT=0
if [ -n "${HELM_AGENT_REAL_RUN_HOME:-}" ]; then
  RUN_HOME="$HELM_AGENT_REAL_RUN_HOME"
  mkdir -p "$RUN_HOME"
else
  RUN_HOME="$(mktemp -d /tmp/helm-agent-real-run.XXXXXX)"
  OWN_HOME=1
fi

if [ -n "${HELM_AGENT_REAL_RUN_PROJECT:-}" ]; then
  RUN_PROJECT="$HELM_AGENT_REAL_RUN_PROJECT"
  mkdir -p "$RUN_PROJECT"
else
  RUN_PROJECT="$(mktemp -d /tmp/helm-agent-real-run-project.XXXXXX)"
  OWN_PROJECT=1
fi

cleanup() {
  if [ "$KEEP" = "1" ] || [ "$MODE" != "dry-run" ]; then
    printf 'kept HELM_AGENT_HOME=%s\n' "$RUN_HOME"
    printf 'kept project=%s\n' "$RUN_PROJECT"
    return
  fi
  if [ "$OWN_HOME" = "1" ]; then
    rm -rf "$RUN_HOME"
  fi
  if [ "$OWN_PROJECT" = "1" ]; then
    rm -rf "$RUN_PROJECT"
  fi
}
trap cleanup EXIT

export HELM_AGENT_HOME="$RUN_HOME"

run_helm() {
  $HELM_AGENT_BIN "$@"
}

print_next_review_steps() {
  cat <<STEPS

Next review commands:
  helm-agent board serve --host 127.0.0.1 --port 8765
  helm-agent task status $TASK_ID
  helm-agent task review $TASK_ID --request-changes "Describe the required fix"
  helm-agent task review $TASK_ID --accept
STEPS
}

printf 'HelmAgent real-run mode: %s\n' "$MODE"
printf 'HELM_AGENT_HOME=%s\n' "$RUN_HOME"
printf 'Project=%s\n' "$RUN_PROJECT"
printf 'Task=%s\n' "$TASK_ID"

run_helm project init --path "$RUN_PROJECT" --agent all
run_helm task create --id "$TASK_ID" --title "$TASK_TITLE" --project "$RUN_PROJECT"

if [ "$MODE" = "acp" ]; then
  run_helm task triage "$TASK_ID" --risk low --priority normal --runtime acp --review-reason "Real ACP smoke requires human review"
  if [ -n "$ACP_COMMAND" ]; then
    if [ -n "$ACP_ARG" ]; then
      run_helm acp agent add "$ACP_NAME" --command "$ACP_COMMAND" --arg "$ACP_ARG"
    else
      run_helm acp agent add "$ACP_NAME" --command "$ACP_COMMAND"
    fi
  fi
  run_helm acp agent check "$ACP_NAME"
  run_helm task dispatch "$TASK_ID" --runtime acp --agent "$ACP_NAME" --confirm
  run_helm task sync --all
  run_helm task status "$TASK_ID"
  print_next_review_steps
  exit 0
fi

run_helm task triage "$TASK_ID" --risk low --priority normal --runtime "$RUNTIME" --review-reason "Real tmux smoke requires human review"

if [ "$MODE" = "dry-run" ]; then
  run_helm task dispatch "$TASK_ID" --runtime "$RUNTIME" --dry-run
  run_helm task sync --all
  run_helm task mark "$TASK_ID" --ready-for-review --message "Dry-run smoke completed; inspect before accepting"
  run_helm task status "$TASK_ID"
  print_next_review_steps
  exit 0
fi

run_helm task dispatch "$TASK_ID" --runtime "$RUNTIME" --confirm --send-brief
run_helm task sync --all
run_helm task status "$TASK_ID"
print_next_review_steps
