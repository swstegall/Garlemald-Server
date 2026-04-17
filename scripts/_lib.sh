#!/usr/bin/env bash
# Shared helpers for the garlemald-server scripts. Source, don't execute.

# Repo root — scripts live in <repo>/scripts, so one level up.
REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
export REPO_ROOT

# Minimum supported Rust — kept in sync with the workspace's
# `rust-toolchain.toml` channel.
MIN_RUST_VERSION="1.95.0"

c_red()    { printf '\033[31m%s\033[0m' "$*"; }
c_green()  { printf '\033[32m%s\033[0m' "$*"; }
c_yellow() { printf '\033[33m%s\033[0m' "$*"; }
c_bold()   { printf '\033[1m%s\033[0m' "$*"; }

say()  { printf '%s %s\n' "$(c_bold '==>')" "$*" >&2; }
ok()   { printf '  %s %s\n' "$(c_green '✓')" "$*" >&2; }
warn() { printf '  %s %s\n' "$(c_yellow '!')" "$*" >&2; }
err()  { printf '  %s %s\n' "$(c_red '✗')" "$*" >&2; }

# Compare dotted semver versions. Returns 0 if $1 >= $2.
version_ge() {
    local a="$1" b="$2"
    [ "$(printf '%s\n%s\n' "$a" "$b" | sort -V | head -n1)" = "$b" ]
}

require_cmd() {
    local name="$1" hint="${2-}"
    if ! command -v "$name" >/dev/null 2>&1; then
        err "missing required command: $name"
        [ -n "$hint" ] && warn "$hint"
        return 1
    fi
    return 0
}

# Called by every script to make sure the toolchain is usable. Returns
# non-zero if the environment cannot build the workspace.
check_env() {
    local fail=0

    require_cmd cargo "install Rust from https://rustup.rs" || fail=1
    require_cmd rustc "install Rust from https://rustup.rs" || fail=1

    if [ "$fail" -eq 0 ]; then
        local rust_version
        rust_version="$(rustc --version | awk '{print $2}')"
        if version_ge "$rust_version" "$MIN_RUST_VERSION"; then
            ok "rustc $rust_version (>= $MIN_RUST_VERSION)"
        else
            err "rustc $rust_version is older than required $MIN_RUST_VERSION"
            warn "run: rustup update"
            fail=1
        fi
    fi

    # clippy + rustfmt are part of the pinned toolchain but verify they
    # were actually installed.
    if command -v cargo >/dev/null 2>&1; then
        if cargo clippy --version >/dev/null 2>&1; then
            ok "clippy available"
        else
            warn "clippy not installed — run: rustup component add clippy"
        fi
        if cargo fmt --version >/dev/null 2>&1; then
            ok "rustfmt available"
        else
            warn "rustfmt not installed — run: rustup component add rustfmt"
        fi
    fi

    # `cc` is needed by Lua's vendored build (mlua pulls in cc-rs).
    if command -v cc >/dev/null 2>&1 || command -v clang >/dev/null 2>&1 || command -v gcc >/dev/null 2>&1; then
        ok "C compiler available (needed for mlua vendored Lua)"
    else
        err "no C compiler on PATH — install Xcode CLI tools or build-essential"
        fail=1
    fi

    if [ ! -f "$REPO_ROOT/Cargo.toml" ]; then
        err "Cargo.toml not found at $REPO_ROOT — script must live under <repo>/scripts"
        fail=1
    fi

    return "$fail"
}

# Fail fast with a readable message if the env is not suitable.
ensure_env() {
    if ! check_env; then
        err "environment check failed"
        exit 1
    fi
}
