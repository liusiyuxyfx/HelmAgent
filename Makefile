.PHONY: install update repair doctor uninstall uninstall-purge test fmt dogfood-dry-run real-run-dry-run real-run-tmux real-run-acp

install:
	sh ./install.sh install

update:
	sh ./install.sh update

repair:
	sh ./install.sh repair

doctor:
	sh ./install.sh doctor

uninstall:
	sh ./install.sh uninstall

uninstall-purge:
	sh ./install.sh uninstall --purge

test:
	cargo test

fmt:
	cargo fmt --check

dogfood-dry-run:
	HELM_AGENT_HOME=$$(mktemp -d /tmp/helm-agent-dogfood.XXXXXX); \
	HELM_AGENT_DOGFOOD_PROJECT=$$(mktemp -d /tmp/helm-agent-dogfood-project.XXXXXX); \
	export HELM_AGENT_HOME; \
	export HELM_AGENT_DOGFOOD_PROJECT; \
	cargo run --quiet --bin helm-agent -- project init --path "$$HELM_AGENT_DOGFOOD_PROJECT" --agent all; \
	cargo run --quiet --bin helm-agent -- task create --id PM-20260512-DOGFOOD --title "Dogfood HelmAgent loop" --project "$$HELM_AGENT_DOGFOOD_PROJECT"; \
	cargo run --quiet --bin helm-agent -- task triage PM-20260512-DOGFOOD --risk low --priority normal --runtime claude; \
	cargo run --quiet --bin helm-agent -- task dispatch --dry-run --runtime claude PM-20260512-DOGFOOD; \
	cargo run --quiet --bin helm-agent -- task sync --all; \
	cargo run --quiet --bin helm-agent -- task mark PM-20260512-DOGFOOD --ready-for-review --message "Dogfood dry-run artifacts are ready"; \
	cargo run --quiet --bin helm-agent -- task status PM-20260512-DOGFOOD; \
	printf '\nState kept for review:\n'; \
	printf '  HELM_AGENT_HOME=%s\n' "$$HELM_AGENT_HOME"; \
	printf '  HELM_AGENT_DOGFOOD_PROJECT=%s\n' "$$HELM_AGENT_DOGFOOD_PROJECT"; \
	printf '\nReview commands:\n'; \
	printf '  HELM_AGENT_HOME=%s cargo run --quiet --bin helm-agent -- task brief PM-20260512-DOGFOOD\n' "$$HELM_AGENT_HOME"; \
	printf '  HELM_AGENT_HOME=%s cargo run --quiet --bin helm-agent -- task review PM-20260512-DOGFOOD --request-changes "Describe the required fix"\n' "$$HELM_AGENT_HOME"; \
	printf '  HELM_AGENT_HOME=%s cargo run --quiet --bin helm-agent -- task review PM-20260512-DOGFOOD --accept\n' "$$HELM_AGENT_HOME"; \
	printf '\nCleanup command:\n'; \
	printf '  rm -rf "%s" "%s"\n' "$$HELM_AGENT_HOME" "$$HELM_AGENT_DOGFOOD_PROJECT"

real-run-dry-run:
	HELM_AGENT_BIN="cargo run --quiet --bin helm-agent --" sh scripts/real_run_smoke.sh --mode dry-run

real-run-tmux:
	HELM_AGENT_BIN="cargo run --quiet --bin helm-agent --" sh scripts/real_run_smoke.sh --mode tmux

real-run-acp:
	HELM_AGENT_BIN="cargo run --quiet --bin helm-agent --" sh scripts/real_run_smoke.sh --mode acp
