#!/usr/bin/env bash
# Run the world server. Extra args are forwarded to the `world-server` CLI.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=_lib.sh
source "$SCRIPT_DIR/_lib.sh"

profile="${PROFILE:-release}"
if [ "$profile" = "release" ]; then
    flag="--release"
    bin="$REPO_ROOT/target/release/world-server"
else
    flag=""
    bin="$REPO_ROOT/target/debug/world-server"
fi

ensure_env

cd "$REPO_ROOT"
say "Building world-server ($profile)"
# shellcheck disable=SC2086
cargo build -p world-server $flag

say "Starting world-server"
if [ ! -x "$bin" ]; then
    err "built binary not found at $bin"
    exit 1
fi
exec "$bin" "$@"
