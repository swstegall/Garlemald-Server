#!/usr/bin/env bash
# Run the map server. Extra args are forwarded to the `map-server` CLI.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=_lib.sh
source "$SCRIPT_DIR/_lib.sh"

profile="${PROFILE:-release}"
if [ "$profile" = "release" ]; then
    flag="--release"
    bin="$REPO_ROOT/target/release/map-server"
else
    flag=""
    bin="$REPO_ROOT/target/debug/map-server"
fi

ensure_env

cd "$REPO_ROOT"
say "Building map-server ($profile)"
# shellcheck disable=SC2086
cargo build -p map-server $flag

say "Starting map-server"
if [ ! -x "$bin" ]; then
    err "built binary not found at $bin"
    exit 1
fi
exec "$bin" "$@"
