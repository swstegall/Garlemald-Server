#!/usr/bin/env bash
# Run the lobby server. Uses ./configs/lobby.toml by default — the same
# default the binary itself uses. Extra args are passed through to the
# `lobby-server` CLI (e.g. --ip 0.0.0.0 --port 54994 --config /path/to.toml).
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=_lib.sh
source "$SCRIPT_DIR/_lib.sh"

profile="${PROFILE:-release}"

if [ "$profile" = "release" ]; then
    flag="--release"
    bin="$REPO_ROOT/target/release/lobby-server"
else
    flag=""
    bin="$REPO_ROOT/target/debug/lobby-server"
fi

ensure_env

cd "$REPO_ROOT"
say "Building lobby-server ($profile)"
# shellcheck disable=SC2086
cargo build -p lobby-server $flag

say "Starting lobby-server"
if [ ! -x "$bin" ]; then
    err "built binary not found at $bin"
    exit 1
fi
exec "$bin" "$@"
