.PHONY: install update repair doctor uninstall uninstall-purge test fmt dogfood-dry-run

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
	trap 'rm -rf "$$HELM_AGENT_HOME" "$$HELM_AGENT_DOGFOOD_PROJECT"' EXIT; \
	cargo run --quiet --bin helm-agent -- project init --path "$$HELM_AGENT_DOGFOOD_PROJECT" --agent all; \
	cargo run --quiet --bin helm-agent -- task create --id PM-20260512-DOGFOOD --title "Dogfood HelmAgent loop" --project "$$HELM_AGENT_DOGFOOD_PROJECT"; \
	cargo run --quiet --bin helm-agent -- task triage PM-20260512-DOGFOOD --risk low --priority normal --runtime claude; \
	cargo run --quiet --bin helm-agent -- task dispatch --dry-run --runtime claude PM-20260512-DOGFOOD; \
	cargo run --quiet --bin helm-agent -- task sync --all; \
	cargo run --quiet --bin helm-agent -- task mark PM-20260512-DOGFOOD --ready-for-review --message "Dogfood dry-run artifacts are ready"; \
	cargo run --quiet --bin helm-agent -- task review PM-20260512-DOGFOOD --accept
