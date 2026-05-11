# HelmAgent Install System Design

## Goal

Provide an open-source style installation surface for HelmAgent that covers install, update, uninstall, repair, doctor, and project initialization without touching global agent workflow configuration by default.

## User Commands

Remote usage:

```bash
curl -fsSL https://raw.githubusercontent.com/liusiyuxyfx/HelmAgent/main/install.sh | sh -s -- install
curl -fsSL https://raw.githubusercontent.com/liusiyuxyfx/HelmAgent/main/install.sh | sh -s -- update
curl -fsSL https://raw.githubusercontent.com/liusiyuxyfx/HelmAgent/main/install.sh | sh -s -- repair
curl -fsSL https://raw.githubusercontent.com/liusiyuxyfx/HelmAgent/main/install.sh | sh -s -- doctor
curl -fsSL https://raw.githubusercontent.com/liusiyuxyfx/HelmAgent/main/install.sh | sh -s -- uninstall
```

Local usage:

```bash
./install.sh install
./install.sh update
./install.sh repair
./install.sh doctor
./install.sh uninstall
./install.sh uninstall --purge
./install.sh init-project /path/to/project
```

Development shortcuts:

```bash
make install
make update
make repair
make doctor
make uninstall
make uninstall-purge
```

## Installer Rules

- Shell: POSIX `sh`.
- Default repository: `https://github.com/liusiyuxyfx/HelmAgent.git`.
- Default data directory: `$HOME/.helm-agent`.
- Env file: `$HOME/.helm-agent/env`.
- Binary installation uses:

```bash
cargo install --git "$HELM_AGENT_REPO" --locked --force
```

- `HELM_AGENT_REPO` can override the Git remote.
- `HELM_AGENT_HOME` can override the data directory.
- `HELM_AGENT_BIN_DIR` can override the binary directory used for PATH diagnostics; default is `$HOME/.cargo/bin`.
- `--dry-run` prints planned operations and does not mutate files or run cargo.

## Actions

### `install`

- Check required tools: `cargo`, `git`, `rustc`.
- Create `$HELM_AGENT_HOME`.
- Write `$HELM_AGENT_HOME/env` with:

```bash
export HELM_AGENT_HOME="$HOME/.helm-agent"
export PATH="$HOME/.cargo/bin:$PATH"
```

- Run cargo install.
- Run `helm-agent --help` when the binary is available.
- Print project integration guidance.

### `update`

- Run cargo install with `--force`.
- Do not modify task data.
- Re-run `helm-agent --help` when the binary is available.

### `doctor`

- Report whether `cargo`, `git`, `rustc`, and `helm-agent` are available.
- Report whether `$HELM_AGENT_HOME` and env file exist.
- Report whether `$HELM_AGENT_BIN_DIR` is on `PATH`.
- Report whether `helm-agent task board` can execute.
- Exit non-zero if any required check fails.

### `repair`

- Ensure `$HELM_AGENT_HOME` and env file exist.
- Reinstall binary if missing.
- Run doctor at the end.

### `uninstall`

- Run `cargo uninstall helm-agent`.
- Keep `$HELM_AGENT_HOME` by default.
- With `--purge`, remove `$HELM_AGENT_HOME`.

### `init-project <path>`

- Append a single include line to `<path>/AGENTS.md`:

```markdown
@<repo>/docs/agent-integrations/main-agent-template.md
```

- Create `AGENTS.md` if missing.
- Do not duplicate the include line if already present.
- Do not modify global Claude Code or Codex settings.

## Safety

- No global hook, skill, agent, or settings files are modified.
- `uninstall` keeps user task data unless `--purge` is explicit.
- `--dry-run` must be safe to run repeatedly.

## Tests

Use integration tests around `install.sh --dry-run` because install/update/uninstall should not mutate the test machine.

Required test coverage:

- `install --dry-run` prints cargo install, env creation, and guidance.
- `update --dry-run` prints cargo install without purge/data deletion.
- `repair --dry-run` prints env repair and doctor.
- `uninstall --dry-run` keeps data.
- `uninstall --purge --dry-run` reports data deletion.
- `init-project --dry-run <path>` prints the `AGENTS.md` target and template include.
- Unknown command fails.

## Acceptance

- `rtk cargo test` passes.
- `rtk cargo fmt --check` passes.
- `rtk shellcheck install.sh` is optional; if `shellcheck` is unavailable, record that.
- README and `docs/install.md` show install, update, repair, doctor, uninstall, purge, and project init commands.
