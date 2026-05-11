.PHONY: install update repair doctor uninstall uninstall-purge test fmt

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
