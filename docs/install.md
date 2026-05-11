# Installing HelmAgent

HelmAgent installs as a Rust CLI plus project-level agent instructions. The installer does not modify global Claude Code, Codex, hook, skill, or agent settings.

## Install

From GitHub:

```bash
curl -fsSL https://raw.githubusercontent.com/liusiyuxyfx/HelmAgent/main/install.sh | sh -s -- install
```

From a local checkout:

```bash
sh ./install.sh install
```

Equivalent local Make target:

```bash
make install
```

## Update

```bash
curl -fsSL https://raw.githubusercontent.com/liusiyuxyfx/HelmAgent/main/install.sh | sh -s -- update
```

Local checkout:

```bash
sh ./install.sh update
make update
```

Update reinstalls the binary with `cargo install --git ... --locked --force`. It does not remove task data.

## Repair

From GitHub:

```bash
curl -fsSL https://raw.githubusercontent.com/liusiyuxyfx/HelmAgent/main/install.sh | sh -s -- repair
```

Local checkout:

```bash
sh ./install.sh repair
make repair
```

Repair recreates the HelmAgent data directory, env file, and main-agent template if missing, reinstalls the binary if needed, and runs doctor.

## Doctor

From GitHub:

```bash
curl -fsSL https://raw.githubusercontent.com/liusiyuxyfx/HelmAgent/main/install.sh | sh -s -- doctor
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
curl -fsSL https://raw.githubusercontent.com/liusiyuxyfx/HelmAgent/main/install.sh | sh -s -- uninstall
sh ./install.sh uninstall
make uninstall
```

Remove the binary and HelmAgent data:

```bash
sh ./install.sh uninstall --purge
make uninstall-purge
```

Plain uninstall keeps `$HOME/.helm-agent` so task records are not deleted by accident. Use `--purge` only when you intentionally want to delete HelmAgent task data.

## Project Setup

Add HelmAgent coordinator instructions to one project:

```bash
sh ./install.sh init-project /path/to/project
```

This creates or updates `/path/to/project/AGENTS.md` with an include for:

```text
$HOME/.helm-agent/main-agent-template.md
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
HELM_AGENT_HOME=$HOME/.helm-agent
HELM_AGENT_BIN_DIR=$HOME/.cargo/bin
HELM_AGENT_TEMPLATE_URL=https://raw.githubusercontent.com/liusiyuxyfx/HelmAgent/main/docs/agent-integrations/main-agent-template.md
```

Load the environment manually when needed:

```bash
. "$HOME/.helm-agent/env"
```
