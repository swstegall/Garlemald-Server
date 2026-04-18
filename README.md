# Garlemald Server

A Rust port of [Project Meteor](https://bitbucket.org/Ioncannon/project-meteor-server/) — a server emulator for **Final Fantasy XIV 1.23b**. The original is a C# codebase; Garlemald re-implements the same three-server architecture, wire protocol, and database contract in async Rust on top of Tokio.

This repo is a sibling of `project-meteor-mirror/` in the parent workspace. The C# tree is kept alongside as the reference implementation — when behavior is ambiguous, the C# source is authoritative.

## What this is

FFXIV 1.23b shipped with a three-tier server: a lobby for authentication and character selection, a world server for account-wide state, and a map server that runs the actual game world. Garlemald preserves that split:

| Crate          | Default port | Role                                                                                      |
| -------------- | -----------: | ----------------------------------------------------------------------------------------- |
| `lobby-server` |        54994 | Client auth, account login, character list, character create/delete.                      |
| `world-server` |        54992 | World-level services: party, linkshell, friend list, MOTD, world metadata.                |
| `map-server`   |         1989 | Game runtime: zones, actors, NPCs, battle, inventory, events, Lua scripting, tick loop.   |
| `common`       |            — | Shared library: blowfish, packet/subpacket framing, SQLite helper, hash table, math helpers. |

All three binaries share a single **SQLite** database file (default `./data/garlemald.db`, created with schema on first run). The map server additionally loads a tree of **Lua scripts** (zones, NPCs, commands, directors) at runtime via [`mlua`](https://crates.io/crates/mlua) with vendored Lua 5.4.

## How it works

### Connection flow

```
    Client ──► lobby-server (54994)   auth + character list
                    │
                    ▼  (handoff via world/map endpoints baked into the character row)
    Client ──► world-server (54992)   social + world state
    Client ──► map-server   (1989)    zone/actor state, driven by a 100ms tick
```

The lobby hands the client back a set of IP/port pairs for the world + map servers, read from the `servers` table in the DB. The client then opens a second connection to the world/map pair for the selected character.

### Wire protocol

Packets are the same Project Meteor frame: an outer `BasePacket` header followed by one or more `SubPacket`s. Transport is Blowfish-encrypted once the session key has been exchanged. The `common` crate implements the framing; see `common/src/packet.rs`, `common/src/subpacket.rs`, and `common/src/blowfish.rs`.

### Map server internals

- `world_manager` holds every loaded zone, private area, and seamless boundary.
- `actor`, `npc`, `battle`, `status`, `inventory`, `event`, `director`, `group`, `social`, `achievement` each own a slice of runtime state.
- `runtime::GameTicker` walks every zone every 100ms and drains four typed outboxes (status / battle / area / inventory) into packets, DB writes, and Lua calls.
- `lua::LuaEngine` is the single entry point into scripts; the tree it reads is the same `scripts/` directory shipped with Project Meteor.

## Layout

```
garlemald-server/
├── Cargo.toml           workspace manifest
├── rust-toolchain.toml  pinned toolchain (currently 1.95.0)
├── common/              shared library crate
├── lobby-server/        binary crate
├── world-server/        binary crate
├── map-server/          binary crate
├── scripts/             build + run wrappers (bash)
└── logs/                created on first run; per-server log files land here
```

## Prerequisites

- **Rust 1.95.0** (pinned in `rust-toolchain.toml` — `rustup` will install it automatically on first build). Includes `clippy` and `rustfmt`.
- **A C compiler** (`cc`, `clang`, or `gcc`). `mlua` vendors Lua 5.4 and builds it from source; `rusqlite` vendors SQLite.
- **Project Meteor data assets** (optional, for real gameplay) — `project-meteor-mirror/Data/scripts/` is what the map server loads at boot. The `Data/sql/*.sql` dumps are no longer consumed directly; Garlemald ships its own SQLite schema in `common/sql/schema.sql` and applies it automatically.

## Building

The workspace builds with plain cargo:

```bash
cargo build --workspace --release
```

Or use the helper scripts under `scripts/` — they run the environment check first and print whether `cargo`, the toolchain version, a C compiler, and `clippy`/`rustfmt` are present:

```bash
./scripts/build.sh             # release (default)
./scripts/build.sh --debug     # dev profile
./scripts/check-env.sh         # env check only
./scripts/test.sh              # cargo test --workspace
```

Built binaries land at `target/{release,debug}/{lobby-server,world-server,map-server}`.

## Database setup

None required — Garlemald uses a local SQLite file (default `./data/garlemald.db`) that is created automatically on first boot. `common/sql/schema.sql` ships the full DDL for every table the three servers touch, plus a single seeded row in `servers` (`id=1, Fernehalwes, 127.0.0.1:54992`) so the lobby → world handoff works out of the box.

If you want to point at a different file (e.g. to keep multiple test rigs side-by-side), edit `database.path` in the relevant `configs/*.toml` or pass `--db-path /path/to/your.db` on the command line.

## Configuring

Each binary reads a TOML file from `./configs/` by default. Localhost defaults ship in the repo:

```
configs/lobby.toml   configs/world.toml   configs/map.toml
```

Their contents (shown below for `lobby.toml`) are the same defaults the `Config::Default` impl uses, so any or all of them can be deleted — the binaries still boot with sensible localhost values.

```toml
[server]
bind_ip = "127.0.0.1"
port = 54994
show_timestamp = true

[database]
path = "./data/garlemald.db"
```

Defaults if a key (or the whole file) is missing:

| Key                             | Lobby            | World            | Map              |
| ------------------------------- | ---------------- | ---------------- | ---------------- |
| `server.bind_ip`                | `127.0.0.1`      | `127.0.0.1`      | `127.0.0.1`      |
| `server.port`                   | `54994`          | `54992`          | `1989`           |
| `server.world_id`               | —                | `1`              | `1`              |
| `database.path`                 | `./data/garlemald.db` | `./data/garlemald.db` | `./data/garlemald.db` |
| `scripting.script_root`         | —                | —                | `./scripts`      |
| `scripting.load_from_database`  | —                | —                | `true`           |

The map server expects `scripting.script_root` to point at the `Data/scripts` tree from Project Meteor (symlink or copy it next to the binary).

### Command-line overrides

Every server accepts these flags, which override the TOML values after load:

```
--ip <ADDR>         bind IP
--port <PORT>       bind port
--db-path <PATH>    SQLite file path
--world-id <ID>     servers.id row (world + map only)
--config <PATH>     path to the TOML file (default ./configs/{lobby,world,map}.toml)
```

## Running

### All three at once

The `run-all.sh` wrapper builds the workspace, then spawns all three servers with logs tee'd to `logs/{lobby,world,map}-server.log`. Ctrl-C shuts the whole stack down cleanly.

```bash
./scripts/run-all.sh
tail -f logs/*.log           # in another terminal
```

### One server at a time

```bash
./scripts/run-lobby.sh
./scripts/run-world.sh
./scripts/run-map.sh
```

Extra args after the script name are forwarded to the binary, e.g.:

```bash
./scripts/run-lobby.sh --ip 0.0.0.0 --port 54994 --config /etc/garlemald/lobby.toml
```

### Directly

```bash
./target/release/lobby-server --config ./configs/lobby.toml
./target/release/world-server --config ./configs/world.toml
./target/release/map-server   --config ./configs/map.toml
```

The map server also runs an interactive stdin console — type commands at the running process the same way the C# server's `Console.ReadLine` loop accepted them.

## Logging

Logging uses [`tracing`](https://crates.io/crates/tracing) with an env filter. Default level is `info`. Raise or narrow it with `RUST_LOG`:

```bash
RUST_LOG=debug ./target/release/map-server
RUST_LOG=map_server=trace,lobby_server=debug,info ./scripts/run-all.sh
```

## Testing

```bash
cargo test --workspace        # or ./scripts/test.sh
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all -- --check
```

The map-server integration tests can boot the server with `scripting.load_from_database = false` in `configs/map.toml` to skip the DB loaders and exercise the runtime against a fresh schema.

## References

- Upstream Project Meteor: <https://bitbucket.org/Ioncannon/project-meteor-server/>
- Setup wiki (schema, client patching, full context): <http://ffxivclassic.fragmenterworks.com/wiki/index.php/Setting_up_the_project>
- Reference C# sources live at `../project-meteor-mirror/` in this workspace.
