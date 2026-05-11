#!/bin/sh
set -eu

DEFAULT_REPO="https://github.com/liusiyuxyfx/HelmAgent.git"
HELM_AGENT_REPO_OVERRIDE="${HELM_AGENT_REPO+x}"
HELM_AGENT_REPO="${HELM_AGENT_REPO:-$DEFAULT_REPO}"
HELM_AGENT_HOME="${HELM_AGENT_HOME:-$HOME/.helm-agent}"
HELM_AGENT_CARGO_ROOT="${HELM_AGENT_CARGO_ROOT:-${CARGO_INSTALL_ROOT:-$HOME/.cargo}}"
HELM_AGENT_BIN_DIR="${HELM_AGENT_BIN_DIR:-$HELM_AGENT_CARGO_ROOT/bin}"
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
  HELM_AGENT_ALLOW_CUSTOM_PURGE=1  Allow --purge outside the default data directory
  HELM_AGENT_HOME      Data directory, default: $HOME/.helm-agent
  HELM_AGENT_CARGO_ROOT  Cargo install root, default: $HOME/.cargo
  HELM_AGENT_BIN_DIR   PATH/diagnostic binary directory, default: $HELM_AGENT_CARGO_ROOT/bin
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
            printf 'export HELM_AGENT_HOME="%s"\n' "$(escape_double_quoted "$HELM_AGENT_HOME")"
            printf 'export PATH="%s:$PATH"\n' "$(escape_double_quoted "$HELM_AGENT_BIN_DIR")"
        } > "$HELM_AGENT_ENV_FILE"
    fi
}

escape_double_quoted() {
    printf '%s' "$1" | sed 's/\\/\\\\/g; s/"/\\"/g; s/\$/\\$/g; s/`/\\`/g'
}

cargo_install() {
    if [ -z "$HELM_AGENT_REPO_OVERRIDE" ] && is_local_checkout; then
        run cargo install --path "$(repo_root)" --locked --force --root "$HELM_AGENT_CARGO_ROOT"
    else
        run cargo install --git "$HELM_AGENT_REPO" --locked --force --root "$HELM_AGENT_CARGO_ROOT"
    fi
}

is_default_home() {
    [ "$HELM_AGENT_HOME" = "$HOME/.helm-agent" ]
}

cargo_uninstall() {
    if [ "$DRY_RUN" -eq 1 ]; then
        plan "cargo uninstall helm-agent"
        return 0
    fi

    if ! have cargo; then
        log "missing: cargo is required to uninstall helm-agent"
        exit 1
    fi

    if cargo install --list --root "$HELM_AGENT_CARGO_ROOT" | grep -q '^helm-agent '; then
        run cargo uninstall helm-agent --root "$HELM_AGENT_CARGO_ROOT"
    else
        log "ok: helm-agent is not installed"
    fi
}

install_template() {
    root="$(repo_root)"
    local_template="$root/docs/agent-integrations/main-agent-template.md"
    if is_local_checkout && [ -f "$local_template" ]; then
        plan "install template $HELM_AGENT_TEMPLATE_FILE"
        if [ "$DRY_RUN" -eq 0 ]; then
            install_template_from_file "$local_template"
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
        install_template_from_url "$HELM_AGENT_TEMPLATE_URL"
    fi
}

ensure_template_target_safe() {
    if [ -L "$HELM_AGENT_TEMPLATE_FILE" ]; then
        log "refusing to update symlink template $HELM_AGENT_TEMPLATE_FILE"
        exit 1
    fi
    if [ -e "$HELM_AGENT_TEMPLATE_FILE" ] && [ ! -f "$HELM_AGENT_TEMPLATE_FILE" ]; then
        log "refusing to update non-file template $HELM_AGENT_TEMPLATE_FILE"
        exit 1
    fi
}

template_temp_file() {
    mktemp "$HELM_AGENT_HOME/.main-agent-template.md.XXXXXX"
}

install_template_from_file() {
    source_file=$1
    mkdir -p "$HELM_AGENT_HOME"
    ensure_template_target_safe
    tmp_file="$(template_temp_file)"
    if ! cp "$source_file" "$tmp_file"; then
        rm -f -- "$tmp_file"
        exit 1
    fi
    mv -f -- "$tmp_file" "$HELM_AGENT_TEMPLATE_FILE"
}

install_template_from_url() {
    template_url=$1
    mkdir -p "$HELM_AGENT_HOME"
    ensure_template_target_safe
    tmp_file="$(template_temp_file)"
    if ! curl -fsSL "$template_url" -o "$tmp_file"; then
        rm -f -- "$tmp_file"
        exit 1
    fi
    mv -f -- "$tmp_file" "$HELM_AGENT_TEMPLATE_FILE"
}

print_guidance() {
    cat <<GUIDANCE

Project integration:
  helm-agent project init --path /path/to/project --agent all
  helm-agent agent prompt --runtime codex
  helm-agent board serve --host 127.0.0.1 --port 8765
  helm-agent task board
  Legacy manual include:
    @$HELM_AGENT_TEMPLATE_FILE

Load environment in your shell:
  . "$HELM_AGENT_ENV_FILE"
GUIDANCE
}

repo_root() {
    script_path=$0
    case "$script_path" in
        */*) [ -f "$script_path" ] || { pwd; return 0; } ;;
        *)
            if [ ! -f "$script_path" ]; then
                pwd
                return 0
            fi
            ;;
    esac
    script_dir=$(CDPATH= cd -- "$(dirname -- "$script_path")" 2>/dev/null && pwd)
    printf '%s\n' "$script_dir"
}

is_local_checkout() {
    script_path=$0
    script_base=$(basename -- "$script_path")
    [ "$script_base" = "install.sh" ] || return 1

    case "$script_path" in
        */*) [ -f "$script_path" ] || return 1 ;;
        *)
            [ -f "$script_path" ] || return 1
            ;;
    esac

    root="$(repo_root)"
    [ -f "$root/install.sh" ] &&
        [ -f "$root/Cargo.toml" ] &&
        grep -q '^name = "helm-agent"' "$root/Cargo.toml"
}

run_help_check() {
    if [ "$DRY_RUN" -eq 1 ]; then
        plan "helm-agent --help"
    elif have helm-agent; then
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
    ensure_home
    install_template
    cargo_install
    run_help_check
}

assert_safe_purge_path() {
    case "$HELM_AGENT_HOME" in
        /*) ;;
        *)
            log "refusing to purge unsafe HELM_AGENT_HOME: $HELM_AGENT_HOME"
            exit 1
            ;;
    esac

    target_base=$(basename -- "$HELM_AGENT_HOME")
    case "$HELM_AGENT_HOME" in
        "/" | */. | */./* | */.. | */../*)
            log "refusing to purge unsafe HELM_AGENT_HOME: $HELM_AGENT_HOME"
            exit 1
            ;;
    esac

    case "$target_base" in
        "" | "." | ".." | -*)
            log "refusing to purge unsafe HELM_AGENT_HOME: $HELM_AGENT_HOME"
            exit 1
            ;;
    esac

    if is_default_home; then
        return 0
    fi

    target_parent=$(dirname -- "$HELM_AGENT_HOME")
    if [ ! -d "$target_parent" ]; then
        log "refusing to purge path with missing parent: $HELM_AGENT_HOME"
        exit 1
    fi

    target_parent_real=$(CDPATH= cd -- "$target_parent" 2>/dev/null && pwd -P)
    target_real="$target_parent_real/$target_base"
    home_real=$(CDPATH= cd -- "$HOME" 2>/dev/null && pwd -P)

    case "$target_real" in
        "/" | "$home_real" | "$home_real/" | "$home_real/.." | "$home_real/." | "$home_real/../"*)
            log "refusing to purge unsafe HELM_AGENT_HOME: $HELM_AGENT_HOME"
            exit 1
            ;;
    esac

    case "$home_real/" in
        "$target_real"/*)
            log "refusing to purge unsafe HELM_AGENT_HOME: $HELM_AGENT_HOME"
            exit 1
            ;;
    esac

    if [ "${HELM_AGENT_ALLOW_CUSTOM_PURGE:-0}" != "1" ]; then
        log "refusing to purge custom HELM_AGENT_HOME without HELM_AGENT_ALLOW_CUSTOM_PURGE=1: $HELM_AGENT_HOME"
        exit 1
    fi

    case "$target_base" in
        .helm-agent | helm-agent | helm-agent-* | *helm-agent*) ;;
        *)
            log "refusing to purge non-HelmAgent path: $HELM_AGENT_HOME"
            exit 1
            ;;
    esac
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

    if [ "$DRY_RUN" -eq 1 ]; then
        plan "helm-agent task board"
    elif have helm-agent; then
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
    require_tools
    ensure_home
    write_env
    install_template
    cargo_install
    doctor_cmd
}

uninstall_cmd() {
    if [ "$PURGE" -eq 1 ]; then
        assert_safe_purge_path
    fi

    cargo_uninstall
    if [ "$PURGE" -eq 1 ]; then
        plan "remove data $HELM_AGENT_HOME"
        if [ "$DRY_RUN" -eq 0 ]; then
            rm -rf -- "$HELM_AGENT_HOME"
        fi
    else
        log "keep data $HELM_AGENT_HOME"
    fi
}

init_project_cmd() {
    project_path=$1

    plan "init project $project_path"
    plan "helm-agent project init --path $project_path --agent codex"

    if [ "$DRY_RUN" -eq 0 ]; then
        if is_local_checkout && have cargo; then
            run cargo run --quiet --bin helm-agent -- project init --path "$project_path" --agent codex
        elif have helm-agent; then
            run helm-agent project init --path "$project_path" --agent codex
        else
            log "missing: helm-agent is required for safe project initialization; run install first"
            exit 1
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
