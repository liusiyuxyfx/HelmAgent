# HelmAgent Dogfood Runbook

Use this when developing HelmAgent with HelmAgent itself. The dry-run flow uses an
isolated `HELM_AGENT_HOME`, so it does not modify your normal task board.

## Dry Run

```bash
make dogfood-dry-run
```

The target leaves the generated temp state in place so the printed review
commands can be run after the target exits. Remove those paths with the printed
cleanup command when the inspection is done.

The target runs the same coordinator loop a main agent should follow:

```bash
export HELM_AGENT_HOME=/tmp/helm-agent-dogfood
export HELM_AGENT_DOGFOOD_PROJECT=/tmp/helm-agent-dogfood-project
helm-agent project init --path "$HELM_AGENT_DOGFOOD_PROJECT" --agent all
helm-agent task create --id PM-20260512-DOGFOOD --title "Dogfood HelmAgent loop" --project "$HELM_AGENT_DOGFOOD_PROJECT"
helm-agent task triage PM-20260512-DOGFOOD --risk low --priority normal --runtime claude
helm-agent task dispatch --dry-run --runtime claude PM-20260512-DOGFOOD
helm-agent task sync --all
helm-agent task mark PM-20260512-DOGFOOD --ready-for-review --message "Dogfood dry-run artifacts are ready"
# Human or authorized main agent only:
helm-agent task review PM-20260512-DOGFOOD --accept
```

## Real Work

For real HelmAgent development, use your normal `HELM_AGENT_HOME`, keep Codex gated
behind approval, and only run `task review --accept` after the human has reviewed the
implementation artifacts and verification output.
