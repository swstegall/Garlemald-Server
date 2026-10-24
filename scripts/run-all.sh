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

# Pre-flight: every service's port must be free. A stack that is still
# running (the usual case) would otherwise make the new services die on
# "Address already in use" while the old ones keep serving. (#211)
config_port() { # <service> <default>: the first `port = N` in configs/<service>.toml
    local p
    p=$(sed -n 's/^port[[:space:]]*=[[:space:]]*\([0-9][0-9]*\).*/\1/p' "configs/$1.toml" 2>/dev/null | head -n 1)
    echo "${p:-$2}"
}
port_holder() { # <port>: "pid command" of the listener, empty if free
    if command -v lsof >/dev/null 2>&1; then
        # lsof exits 1 when nothing listens; don't let pipefail/set -e end the script
        { lsof -nP -iTCP:"$1" -sTCP:LISTEN 2>/dev/null || true; } | awk 'NR == 2 { print $2, $1 }'
    elif (exec 3<>"/dev/tcp/127.0.0.1/$1") 2>/dev/null; then
        echo "unknown process"
    fi
}
busy=0
for svc in web:54993 lobby:54994 world:54992 map:1989; do
    port=$(config_port "${svc%%:*}" "${svc##*:}")
    holder=$(port_holder "$port")
    if [ -n "$holder" ]; then
        err "${svc%%:*} port $port is already in use by $holder"
        busy=1
    fi
done
if [ "$busy" -ne 0 ]; then
    err "a previous stack is probably still running; stop it (Ctrl-C in its terminal, or kill the pids above) and run again"
    trap - EXIT
    exit 1
fi

say "Building workspace ($profile)"
# shellcheck disable=SC2086
cargo build --workspace $flag

mkdir -p "$REPO_ROOT/logs"
export GARLEMALD_PACKET_LOG_DIR="${GARLEMALD_PACKET_LOG_DIR:-$REPO_ROOT/logs/packet-logs}"
# Capture pre-encryption (plaintext) bytes so warp bundles can be diffed
# against the reference pmeteor captures when debugging content warps.
export GARLEMALD_PACKET_LOG_PLAINTEXT="${GARLEMALD_PACKET_LOG_PLAINTEXT:-1}"
mkdir -p "$GARLEMALD_PACKET_LOG_DIR"

pids=()
names=()
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
    names+=("$name")
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
    names+=("map-server")
    ok "map-server pid=$pid"
}

# Web first (so signup is reachable before the rest of the stack is warm),
# then lobby / world / map. The processes are independent — no startup
# barrier is required between them.
start web-server
start lobby-server
start world-server
start_map_server

# Report the first service that has exited (name + log tail) and return
# non-zero; returns zero while all are alive. (#211)
check_alive() {
    local i
    for i in "${!pids[@]}"; do
        if ! kill -0 "${pids[$i]}" 2>/dev/null; then
            err "${names[$i]} (pid ${pids[$i]}) exited; last lines of logs/${names[$i]}.log:"
            tail -n 15 "$REPO_ROOT/logs/${names[$i]}.log" >&2 || true
            return 1
        fi
    done
    return 0
}

# Startup gate: a service that dies on a bad config, DB error or bind
# failure does so within a few seconds. cleanup() (EXIT trap) stops the
# survivors.
for _ in 1 2 3 4 5; do
    sleep 1
    check_alive || exit 1
done

say "All four servers running. Tail logs with: tail -f logs/*.log"
say "Press Ctrl-C to stop."
# Supervise instead of a bare `wait`, so a service that dies later is
# reported at once rather than silently shrinking the group (bash 3.2 on
# macOS has no `wait -n`). cleanup() runs on any signal.
while check_alive; do
    sleep 2
done
exit 1
