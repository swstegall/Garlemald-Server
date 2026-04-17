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
| `common`       |            — | Shared library: blowfish, packet/subpacket framing, ini reader, hash table, math helpers. |

All three binaries talk to the same MySQL database. The map server additionally loads a tree of **Lua scripts** (zones, NPCs, commands, directors) at runtime via [`mlua`](https://crates.io/crates/mlua) with vendored Lua 5.4.

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
- **A C compiler** (`cc`, `clang`, or `gcc`). `mlua` vendors Lua 5.4 and builds it from source.
- **MySQL 5.7+ / MariaDB 10+** reachable from the machine running the servers.
- **Project Meteor data assets** — the `Data/` directory from `project-meteor-mirror/` (or upstream). You need:
  - `Data/sql/*.sql` — schema + seed data for the `ffxiv_server` database.
  - `Data/scripts/` — Lua scripts the map server loads at boot.
  - `Data/*_config.ini` — example config files you can copy next to each binary.

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

Follow the [upstream Project Meteor wiki](http://ffxivclassic.fragmenterworks.com/wiki/index.php/Setting_up_the_project) for the authoritative version — Garlemald consumes the same schema unchanged.

The short version:

1. Create a database (the upstream default name is `ffxiv_server`):
   ```sql
   CREATE DATABASE ffxiv_server CHARACTER SET utf8 COLLATE utf8_general_ci;
   ```
2. Import every `.sql` file in `project-meteor-mirror/Data/sql/`:
   ```bash
   cd ../project-meteor-mirror/Data/sql
   for f in *.sql; do mysql -u root -p ffxiv_server < "$f"; done
   ```
3. Edit the `servers` table so the `world-server` and `map-server` rows point at the IPs/ports your clients will reach (not `127.0.0.1` unless you're running everything locally). The lobby reads those rows and tells the client where to connect next.

## Configuring

Each binary reads an INI file from the current working directory by default (`--config <path>` overrides it). Copy the examples from the mirror to start:

```bash
cp ../project-meteor-mirror/Data/lobby_config.ini ./lobby_config.ini
cp ../project-meteor-mirror/Data/world_config.ini ./world_config.ini
cp ../project-meteor-mirror/Data/map_config.ini   ./map_config.ini
```

Fill in the `[Database]` section (`host`, `port`, `database`, `username`, `password`) for each file. `[General]` controls `server_ip` (bind address) and `server_port`. Defaults if a key is missing:

| Key                       | Lobby            | World            | Map              |
| ------------------------- | ---------------- | ---------------- | ---------------- |
| `General.server_ip`       | `127.0.0.1`      | `127.0.0.1`      | `127.0.0.1`      |
| `General.server_port`     | `54994`          | `54992`          | `1989`           |
| `General.script_root`     | —                | —                | `./scripts`      |
| `Database.port`           | `3306`           | `3306`           | `3306`           |

The map server expects `script_root` to point at the `Data/scripts` tree from Project Meteor (symlink or copy it next to the binary).

### Command-line overrides

Every server accepts the same flags, which override the INI values after load:

```
--ip <ADDR>         bind IP
--port <PORT>       bind port
--host <HOST>       MySQL host
--db <NAME>         MySQL database
--user <USER>       MySQL username
-p, --password <…>  MySQL password
--config <PATH>     path to the INI file (default ./{lobby,world,map}_config.ini)
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
./scripts/run-lobby.sh --ip 0.0.0.0 --port 54994 --config /etc/garlemald/lobby.ini
```

### Directly

```bash
./target/release/lobby-server --config ./lobby_config.ini
./target/release/world-server --config ./world_config.ini
./target/release/map-server   --config ./map_config.ini
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

The map-server integration tests can boot the server with `load_from_database = false` in `map_config.ini` to skip the DB loaders and exercise the runtime against an offline mock.

## References

- Upstream Project Meteor: <https://bitbucket.org/Ioncannon/project-meteor-server/>
- Setup wiki (schema, client patching, full context): <http://ffxivclassic.fragmenterworks.com/wiki/index.php/Setting_up_the_project>
- Reference C# sources live at `../project-meteor-mirror/` in this workspace.
