#!/usr/bin/env bash
# Build the whole workspace. Release profile by default; pass --debug for
# dev profile. Additional args are forwarded to `cargo build`.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=_lib.sh
source "$SCRIPT_DIR/_lib.sh"

profile="release"
cargo_args=()
for a in "$@"; do
    case "$a" in
        --debug)   profile="debug" ;;
        --release) profile="release" ;;
        -h|--help)
            cat <<EOF
Usage: $(basename "$0") [--debug|--release] [cargo args...]

Builds all four workspace crates: common, lobby-server, world-server, map-server.
Default profile is release (matches deployment). Pass --debug for faster
rebuilds during development. Any further args are forwarded to cargo.
EOF
            exit 0
            ;;
        *)         cargo_args+=("$a") ;;
    esac
done

ensure_env

cd "$REPO_ROOT"
if [ "$profile" = "release" ]; then
    say "Building workspace (release)"
    cargo build --workspace --release "${cargo_args[@]+"${cargo_args[@]}"}"
    ok "binaries at target/release/{lobby-server,world-server,map-server}"
else
    say "Building workspace (debug)"
    cargo build --workspace "${cargo_args[@]+"${cargo_args[@]}"}"
    ok "binaries at target/debug/{lobby-server,world-server,map-server}"
fi
