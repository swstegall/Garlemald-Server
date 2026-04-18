#!/usr/bin/env bash
# Run the web (login/signup) server. Uses ./configs/web.toml by default —
# the same default the binary itself uses. Extra args are passed through
# to the `web-server` CLI (e.g. --ip 0.0.0.0 --port 54993 --config /path/to.toml).
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=_lib.sh
source "$SCRIPT_DIR/_lib.sh"

profile="${PROFILE:-release}"

if [ "$profile" = "release" ]; then
    flag="--release"
    bin="$REPO_ROOT/target/release/web-server"
else
    flag=""
    bin="$REPO_ROOT/target/debug/web-server"
fi

ensure_env

cd "$REPO_ROOT"
say "Building web-server ($profile)"
# shellcheck disable=SC2086
cargo build -p web-server $flag

say "Starting web-server"
if [ ! -x "$bin" ]; then
    err "built binary not found at $bin"
    exit 1
fi
exec "$bin" "$@"
