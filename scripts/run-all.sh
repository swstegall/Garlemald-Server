#!/usr/bin/env bash
# Build all four servers once, then launch them together. SIGINT/SIGTERM
# on this script propagates to every child so Ctrl-C shuts the whole stack
# down cleanly. Per-server logs go to ./logs/{lobby,world,map,web}.log.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=_lib.sh
source "$SCRIPT_DIR/_lib.sh"

profile="${PROFILE:-release}"
if [ "$profile" = "release" ]; then
    flag="--release"
    bin_dir="$REPO_ROOT/target/release"
else
    flag=""
    bin_dir="$REPO_ROOT/target/debug"
fi

ensure_env

cd "$REPO_ROOT"
say "Building workspace ($profile)"
# shellcheck disable=SC2086
cargo build --workspace $flag

mkdir -p "$REPO_ROOT/logs"

pids=()
cleanup() {
    say "Stopping servers"
    for pid in "${pids[@]}"; do
        if kill -0 "$pid" 2>/dev/null; then
            kill "$pid" 2>/dev/null || true
        fi
    done
    wait 2>/dev/null || true
    ok "all servers stopped"
}
trap cleanup INT TERM EXIT

start() {
    local name="$1"; shift
    local log="$REPO_ROOT/logs/$name.log"
    say "Starting $name (log: $log)"
    "$bin_dir/$name" "$@" >"$log" 2>&1 &
    local pid=$!
    pids+=("$pid")
    ok "$name pid=$pid"
}

# Web first (so signup is reachable before the rest of the stack is warm),
# then lobby / world / map. The processes are independent — no startup
# barrier is required between them.
start web-server
start lobby-server
start world-server
start map-server

say "All four servers running. Tail logs with: tail -f logs/*.log"
say "Press Ctrl-C to stop."
# Wait on the group; cleanup() runs on any signal.
wait
