#!/bin/sh
set -eu

DEFAULT_REPO="https://github.com/liusiyuxyfx/HelmAgent.git"
HELM_AGENT_REPO="${HELM_AGENT_REPO:-$DEFAULT_REPO}"
HELM_AGENT_HOME="${HELM_AGENT_HOME:-$HOME/.helm-agent}"
HELM_AGENT_BIN_DIR="${HELM_AGENT_BIN_DIR:-$HOME/.cargo/bin}"
HELM_AGENT_ENV_FILE="$HELM_AGENT_HOME/env"
HELM_AGENT_TEMPLATE_FILE="$HELM_AGENT_HOME/main-agent-template.md"
HELM_AGENT_TEMPLATE_URL="${HELM_AGENT_TEMPLATE_URL:-https://raw.githubusercontent.com/liusiyuxyfx/HelmAgent/main/docs/agent-integrations/main-agent-template.md}"
DRY_RUN=0
PURGE=0

usage() {
    cat <<'USAGE'
Usage:
  install.sh install [--dry-run]
  install.sh update [--dry-run]
  install.sh repair [--dry-run]
  install.sh doctor [--dry-run]
  install.sh uninstall [--purge] [--dry-run]
  install.sh init-project <path> [--dry-run]

Environment:
  HELM_AGENT_REPO      Git repository to install from
  HELM_AGENT_HOME      Data directory, default: $HOME/.helm-agent
  HELM_AGENT_BIN_DIR   Binary directory for PATH checks, default: $HOME/.cargo/bin
  HELM_AGENT_TEMPLATE_URL  URL used when local template file is unavailable
USAGE
}

log() {
    printf '%s\n' "$*"
}

plan() {
    if [ "$DRY_RUN" -eq 1 ]; then
        printf 'DRY-RUN: %s\n' "$*"
    else
        printf '%s\n' "$*"
    fi
}

run() {
    plan "$*"
    if [ "$DRY_RUN" -eq 0 ]; then
        "$@"
    fi
}

have() {
    command -v "$1" >/dev/null 2>&1
}

parse_common_flags() {
    while [ "$#" -gt 0 ]; do
        case "$1" in
            --dry-run)
                DRY_RUN=1
                ;;
            *)
                return 1
                ;;
        esac
        shift
    done
}

parse_uninstall_flags() {
    while [ "$#" -gt 0 ]; do
        case "$1" in
            --dry-run)
                DRY_RUN=1
                ;;
            --purge)
                PURGE=1
                ;;
            *)
                return 1
                ;;
        esac
        shift
    done
}

require_tools() {
    missing=0
    for tool in cargo git rustc; do
        if have "$tool"; then
            log "ok: found $tool"
        else
            log "missing: $tool"
            missing=1
        fi
    done

    if [ "$missing" -ne 0 ] && [ "$DRY_RUN" -eq 0 ]; then
        log "Install Rust and Git before continuing."
        exit 1
    fi
}

ensure_home() {
    run mkdir -p "$HELM_AGENT_HOME"
}

write_env() {
    plan "write env $HELM_AGENT_ENV_FILE"
    if [ "$DRY_RUN" -eq 0 ]; then
        mkdir -p "$HELM_AGENT_HOME"
        {
            printf 'export HELM_AGENT_HOME="%s"\n' "$HELM_AGENT_HOME"
            printf 'export PATH="%s:$PATH"\n' "$HELM_AGENT_BIN_DIR"
        } > "$HELM_AGENT_ENV_FILE"
    fi
}

cargo_install() {
    run cargo install --git "$HELM_AGENT_REPO" --locked --force
}

cargo_uninstall() {
    if [ "$DRY_RUN" -eq 1 ]; then
        plan "cargo uninstall helm-agent"
        return 0
    fi

    if cargo install --list | grep -q '^helm-agent '; then
        run cargo uninstall helm-agent
    else
        log "ok: helm-agent is not installed"
    fi
}

install_template() {
    root="$(repo_root)"
    local_template="$root/docs/agent-integrations/main-agent-template.md"
    if [ -f "$root/install.sh" ] &&
        [ -f "$root/Cargo.toml" ] &&
        grep -q '^name = "helm-agent"' "$root/Cargo.toml" &&
        [ -f "$local_template" ]; then
        plan "install template $HELM_AGENT_TEMPLATE_FILE"
        if [ "$DRY_RUN" -eq 0 ]; then
            mkdir -p "$HELM_AGENT_HOME"
            cp "$local_template" "$HELM_AGENT_TEMPLATE_FILE"
        fi
        return 0
    fi

    plan "fetch template $HELM_AGENT_TEMPLATE_URL"
    plan "install template $HELM_AGENT_TEMPLATE_FILE"
    if [ "$DRY_RUN" -eq 0 ]; then
        if ! have curl; then
            log "missing: curl is required to fetch main-agent template"
            exit 1
        fi
        mkdir -p "$HELM_AGENT_HOME"
        curl -fsSL "$HELM_AGENT_TEMPLATE_URL" -o "$HELM_AGENT_TEMPLATE_FILE"
    fi
}

print_guidance() {
    cat <<GUIDANCE

Project integration:
  helm-agent task board
  Add this line to a project AGENTS.md when supported:
    @$HELM_AGENT_TEMPLATE_FILE

Load environment in your shell:
  . "$HELM_AGENT_ENV_FILE"
GUIDANCE
}

repo_root() {
    script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" 2>/dev/null && pwd)
    printf '%s\n' "$script_dir"
}

run_help_check() {
    if have helm-agent; then
        run helm-agent --help
    else
        plan "helm-agent --help"
    fi
}

install_cmd() {
    require_tools
    ensure_home
    write_env
    install_template
    cargo_install
    run_help_check
    print_guidance
}

update_cmd() {
    require_tools
    cargo_install
    run_help_check
}

path_contains_bin_dir() {
    case ":$PATH:" in
        *":$HELM_AGENT_BIN_DIR:"*) return 0 ;;
        *) return 1 ;;
    esac
}

doctor_cmd() {
    status=0
    log "doctor: checking HelmAgent installation"

    for tool in cargo git rustc; do
        if have "$tool"; then
            log "ok: $tool"
        else
            log "missing: $tool"
            status=1
        fi
    done

    if have helm-agent; then
        log "ok: helm-agent"
    else
        log "missing: helm-agent"
        status=1
    fi

    if [ -d "$HELM_AGENT_HOME" ]; then
        log "ok: HELM_AGENT_HOME $HELM_AGENT_HOME"
    else
        log "missing: HELM_AGENT_HOME $HELM_AGENT_HOME"
        status=1
    fi

    if [ -f "$HELM_AGENT_ENV_FILE" ]; then
        log "ok: env $HELM_AGENT_ENV_FILE"
    else
        log "missing: env $HELM_AGENT_ENV_FILE"
        status=1
    fi

    if [ -f "$HELM_AGENT_TEMPLATE_FILE" ]; then
        log "ok: template $HELM_AGENT_TEMPLATE_FILE"
    else
        log "missing: template $HELM_AGENT_TEMPLATE_FILE"
        status=1
    fi

    if path_contains_bin_dir; then
        log "ok: PATH contains $HELM_AGENT_BIN_DIR"
    else
        log "missing: PATH does not contain $HELM_AGENT_BIN_DIR"
        status=1
    fi

    if have helm-agent; then
        if helm-agent task board >/dev/null 2>&1; then
            log "ok: helm-agent task board"
        else
            log "failed: helm-agent task board"
            status=1
        fi
    else
        plan "helm-agent task board"
    fi

    if [ "$DRY_RUN" -eq 1 ]; then
        log "doctor: dry-run complete"
        return 0
    fi
    return "$status"
}

repair_cmd() {
    ensure_home
    write_env
    install_template
    if have helm-agent; then
        log "ok: helm-agent already installed"
    else
        cargo_install
    fi
    doctor_cmd
}

uninstall_cmd() {
    cargo_uninstall
    if [ "$PURGE" -eq 1 ]; then
        plan "remove data $HELM_AGENT_HOME"
        if [ "$DRY_RUN" -eq 0 ]; then
            rm -rf "$HELM_AGENT_HOME"
        fi
    else
        log "keep data $HELM_AGENT_HOME"
    fi
}

init_project_cmd() {
    project_path=$1
    agents_file="$project_path/AGENTS.md"
    include_line="@$HELM_AGENT_TEMPLATE_FILE"

    plan "init project $project_path"
    plan "update $agents_file"
    plan "include $include_line"

    if [ "$DRY_RUN" -eq 0 ]; then
        if [ ! -f "$HELM_AGENT_TEMPLATE_FILE" ]; then
            install_template
        fi
        mkdir -p "$project_path"
        if [ ! -f "$agents_file" ]; then
            : > "$agents_file"
        fi
        if ! grep -Fxq "$include_line" "$agents_file"; then
            if [ -s "$agents_file" ]; then
                printf '\n' >> "$agents_file"
            fi
            printf '%s\n' "$include_line" >> "$agents_file"
        fi
    fi
}

if [ "$#" -lt 1 ]; then
    usage >&2
    exit 1
fi

command_name=$1
shift

case "$command_name" in
    install)
        parse_common_flags "$@" || { usage >&2; exit 1; }
        install_cmd
        ;;
    update)
        parse_common_flags "$@" || { usage >&2; exit 1; }
        update_cmd
        ;;
    repair)
        parse_common_flags "$@" || { usage >&2; exit 1; }
        repair_cmd
        ;;
    doctor)
        parse_common_flags "$@" || { usage >&2; exit 1; }
        doctor_cmd
        ;;
    uninstall)
        parse_uninstall_flags "$@" || { usage >&2; exit 1; }
        uninstall_cmd
        ;;
    init-project)
        if [ "$#" -lt 1 ]; then
            usage >&2
            exit 1
        fi
        project_path=$1
        shift
        parse_common_flags "$@" || { usage >&2; exit 1; }
        init_project_cmd "$project_path"
        ;;
    -h|--help|help)
        usage
        ;;
    *)
        usage >&2
        exit 1
        ;;
esac
