# Installing HelmAgent

HelmAgent installs as a Rust CLI plus project-level agent instructions. The installer does not modify global Claude Code, Codex, hook, skill, or agent settings.

## Install

From GitHub:

```bash
INSTALLER=/tmp/helm-agent-install.sh
curl -fsSL https://raw.githubusercontent.com/liusiyuxyfx/HelmAgent/main/install.sh -o "$INSTALLER" && sh "$INSTALLER" install
```

From a local checkout:

```bash
sh ./install.sh install
```

Equivalent local Make target:

```bash
make install
```

Local checkout commands install the code from the current checkout with `cargo install --path .`.

## Update

```bash
INSTALLER=/tmp/helm-agent-install.sh
curl -fsSL https://raw.githubusercontent.com/liusiyuxyfx/HelmAgent/main/install.sh -o "$INSTALLER" && sh "$INSTALLER" update
```

Local checkout:

```bash
sh ./install.sh update
make update
```

Update refreshes the installed main-agent template and reinstalls the binary. Piped GitHub installs use `cargo install --git ... --locked --force`; local checkout installs use `cargo install --path . --locked --force`. It does not remove task data.

## Repair

From GitHub:

```bash
INSTALLER=/tmp/helm-agent-install.sh
curl -fsSL https://raw.githubusercontent.com/liusiyuxyfx/HelmAgent/main/install.sh -o "$INSTALLER" && sh "$INSTALLER" repair
```

Local checkout:

```bash
sh ./install.sh repair
make repair
```

Repair recreates the HelmAgent data directory, env file, and main-agent template if missing, reinstalls the binary, and runs doctor.

## Doctor

From GitHub:

```bash
INSTALLER=/tmp/helm-agent-install.sh
curl -fsSL https://raw.githubusercontent.com/liusiyuxyfx/HelmAgent/main/install.sh -o "$INSTALLER" && sh "$INSTALLER" doctor
```

Local checkout:

```bash
sh ./install.sh doctor
make doctor
```

Doctor checks:

- `cargo`
- `git`
- `rustc`
- `helm-agent`
- `HELM_AGENT_HOME`
- `$HELM_AGENT_HOME/env`
- `$HELM_AGENT_HOME/main-agent-template.md`
- `$HOME/.cargo/bin` in `PATH`
- `helm-agent task board`

## Uninstall

Remove only the binary:

```bash
INSTALLER=/tmp/helm-agent-install.sh
curl -fsSL https://raw.githubusercontent.com/liusiyuxyfx/HelmAgent/main/install.sh -o "$INSTALLER" && sh "$INSTALLER" uninstall
sh ./install.sh uninstall
make uninstall
```

Remove the binary and HelmAgent data:

```bash
sh ./install.sh uninstall --purge
make uninstall-purge
```

Plain uninstall keeps `$HOME/.helm-agent` so task records are not deleted by accident. Use `--purge` only when you intentionally want to delete HelmAgent task data.

`--purge` refuses unsafe values such as `/`, `.`, `..`, `$HOME`, relative paths, parent-path aliases, and non-HelmAgent-looking custom paths. Custom purge paths also require `HELM_AGENT_ALLOW_CUSTOM_PURGE=1`.

## Project Setup

Add HelmAgent coordinator instructions to one project:

```bash
INSTALLER=/tmp/helm-agent-install.sh
curl -fsSL https://raw.githubusercontent.com/liusiyuxyfx/HelmAgent/main/install.sh -o "$INSTALLER" && sh "$INSTALLER" init-project /path/to/project
sh ./install.sh init-project /path/to/project
```

This creates or updates `/path/to/project/AGENTS.md` with an include for:

```text
@$HOME/.helm-agent/main-agent-template.md
```

If the template has not been installed yet, `init-project` installs it first. The installer only modifies `$HOME/.helm-agent` and the project you pass. It does not touch global Claude Code or Codex configuration.

## Dry Run

Every mutating command supports `--dry-run`:

```bash
sh ./install.sh install --dry-run
sh ./install.sh update --dry-run
sh ./install.sh repair --dry-run
sh ./install.sh uninstall --dry-run
sh ./install.sh uninstall --purge --dry-run
sh ./install.sh init-project /path/to/project --dry-run
```

## Environment

The installer writes:

```bash
$HOME/.helm-agent/env
$HOME/.helm-agent/main-agent-template.md
```

with:

```bash
export HELM_AGENT_HOME="$HOME/.helm-agent"
export PATH="$HOME/.cargo/bin:$PATH"
```

Supported overrides:

```bash
HELM_AGENT_REPO=https://github.com/liusiyuxyfx/HelmAgent.git
HELM_AGENT_ALLOW_CUSTOM_PURGE=1
HELM_AGENT_HOME=$HOME/.helm-agent
HELM_AGENT_CARGO_ROOT=$HOME/.cargo
HELM_AGENT_BIN_DIR=$HOME/.cargo/bin
HELM_AGENT_TEMPLATE_URL=https://raw.githubusercontent.com/liusiyuxyfx/HelmAgent/main/docs/agent-integrations/main-agent-template.md
```

`HELM_AGENT_CARGO_ROOT` controls where `cargo install` writes the binary. `HELM_AGENT_BIN_DIR` controls the PATH line written to the env file and the PATH check in doctor; by default it is `$HELM_AGENT_CARGO_ROOT/bin`.

Load the environment manually when needed:

```bash
. "$HOME/.helm-agent/env"
```
