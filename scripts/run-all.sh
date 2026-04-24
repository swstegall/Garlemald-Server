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

# `map-server`'s GM-console stdin reader lets operators run commands
# (`!warp`, `!giveexp`, etc.) over stdin, but when we launch the stack
# in the background (`& ; wait`), bash ties children's stdin to
# /dev/null — so the reader never sees any input. Set up a named FIFO
# wired into map-server's stdin and expose its path via env. External
# tools (chat-driven GM paths, test harnesses, `echo ... > $FIFO`) can
# feed commands through it.
GM_FIFO="${GM_FIFO:-$REPO_ROOT/logs/map-server.gm.fifo}"
mkdir -p "$(dirname "$GM_FIFO")"
[ -p "$GM_FIFO" ] || { rm -f "$GM_FIFO"; mkfifo "$GM_FIFO"; }
export GM_FIFO
ok "GM command fifo: $GM_FIFO (echo commands into this file to drive the stdin reader)"

# Keep the FIFO's write end open on this shell's FD 9 for the lifetime
# of run-all.sh. map-server reads via stdin (FD 0) → FIFO's read end.
# Without this, the first caller that opens the FIFO for writing and
# closes it would EOF map-server's stdin reader, so further writes
# would block on waiting for a reader. Holding FD 9 open keeps the
# writer count ≥ 1 until the stack shuts down.
# NOTE: open RW (`9<>`) rather than write-only (`9>`). Write-only opens
# of a FIFO block until a reader attaches, which would deadlock us
# here (map-server hasn't started yet). RW opens proceed immediately.
exec 9<>"$GM_FIFO"

start_map_server() {
    local log="$REPO_ROOT/logs/map-server.log"
    say "Starting map-server (log: $log)"
    "$bin_dir/map-server" <"$GM_FIFO" >"$log" 2>&1 &
    local pid=$!
    pids+=("$pid")
    ok "map-server pid=$pid"
}

# Web first (so signup is reachable before the rest of the stack is warm),
# then lobby / world / map. The processes are independent — no startup
# barrier is required between them.
start web-server
start lobby-server
start world-server
start_map_server

say "All four servers running. Tail logs with: tail -f logs/*.log"
say "Press Ctrl-C to stop."
# Wait on the group; cleanup() runs on any signal.
wait
