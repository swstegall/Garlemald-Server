# Garlemald Server

[![License: AGPL v3](https://img.shields.io/badge/license-AGPL--3.0--or--later-blue.svg)](LICENSE.md)
[![Rust](https://img.shields.io/badge/rust-1.95-orange.svg)](rust-toolchain.toml)
[![Platform](https://img.shields.io/badge/platform-macOS%20%7C%20Linux%20%7C%20Windows-lightgrey.svg)](#quick-start)
[![Discord](https://img.shields.io/badge/discord-join-5865F2.svg)](https://discord.gg/CVjwWs6jnX)

A private-server emulator for **FINAL FANTASY XIV v1.23b** — the final
patch of the original 1.0 release, not *A Realm Reborn* — written from
the ground up in Rust.

Garlemald Server is a port of the upstream C#
[Project Meteor Server](https://bitbucket.org/Ioncannon/project-meteor-server/).
The lobby / world / map services, the 1.x wire protocol, the
Blowfish-keyed session handshake, and the Lua-driven content system are
preserved. The external stack (MySQL, PHP, WAMP, IIS) collapses into a
single Cargo workspace that runs on any platform Rust targets, with
SQLite in place of MySQL and an embedded axum HTTP service in place of
the PHP auth frontend.

Only a client reporting version `2012.09.19.0001` will negotiate the
wire protocol correctly. The companion
[Garlemald Client](https://github.com/swstegall/Garlemald-Client) will
patch a stock 1.x install up to that version automatically.

> Created with [Claude](https://claude.ai/).

## Highlights

- Four-binary Cargo workspace — `lobby-server`, `world-server`,
  `map-server`, and `web-server` — sharing one `common` crate for
  protocol, packet log, database, and Lua bindings
- Byte-for-byte compatible with the 1.23b client: PE patch, ZiPatch
  archives, TCP + Blowfish framing, 1.x opcodes, actor-spawn fields,
  FACEINFO bitfield, motion packs, and the full Director /
  ContentArea / PrivateArea / Zone / Weather state machines
- **1,142 Lua scripts** loaded at boot via [`mlua`](https://github.com/mlua-rs/mlua)
  5.4 — quests, directors, shops, aetherytes, populace, elevators,
  commands — with a coroutine scheduler that honours the upstream
  `_WAIT_TIME` / `_WAIT_SIGNAL` yield idioms
- **SQLite-backed** via `tokio-rusqlite`; the full schema is created
  and seeded from 40 bundled SQL files on first run, with migration
  tracking in a `schema_migrations` table — no external database
  server to provision
- **TOML configuration** (`configs/{lobby,world,map,web}.toml`) in
  place of the upstream INI files, with localhost defaults that boot
  straight into a playable single-user server
- Argon2-hashed account storage, session-token handoff between the
  web frontend and the lobby, and env-gated per-server packet logging
  for protocol debugging

## Project status

This is an **in-progress port**, not a finished product. Account
flow, character create/select, zone loading, NPC and monster spawn,
chat, movement, guildleves, and status effects are working end-to-end
against a retail 1.23b client. Combat damage formulae, stat recalc,
inventory event dispatch, level/XP, and the quest-reward pipeline are
partially implemented. See `porting-progress-context.md` in the parent
workspace for the subsystem status matrix and roadmap.

## Architecture

```
         ┌──────────────┐     signup / login (HTTP)
Client ─►│  web-server  │───┐
         │  :54993      │   │
         └──────────────┘   │ session token
                            ▼
         ┌──────────────┐ character list,
Client ─►│ lobby-server │ world handoff
         │  :54994      │
         └──────┬───────┘
                │ world handoff
                ▼
         ┌──────────────┐     zone-in
Client ─►│ world-server │─┐
         │  :54992      │ │
         └──────────────┘ │
                          ▼
         ┌──────────────┐   actors, AI,
Client ─►│  map-server  │   Lua directors,
         │  :1989       │   navmesh, combat
         └──────────────┘
                ▲
                │  shared SQLite (./data/garlemald.db)
                │  shared Lua script root (./scripts/lua/)
                ▼
         ┌──────────────┐
         │  common/     │   protocol · packet log · db · lua bindings
         └──────────────┘
```

## Quick start

Requires Rust 1.95 (pinned in `rust-toolchain.toml`; `rustup` installs
it automatically on first build).

```sh
# Build every workspace member once, then launch the full stack.
# Per-server logs land in ./logs/{lobby,world,map,web}.log, and
# Ctrl-C propagates a clean shutdown to every child.
./scripts/run-all.sh
```

Individual services can be started on their own for debugging:

```sh
./scripts/run-web.sh     # HTTP signup/login on :54993
./scripts/run-lobby.sh   # Lobby / character-list on :54994
./scripts/run-world.sh   # World handoff on :54992
./scripts/run-map.sh     # Map / zone / AI on :1989
```

Windows hosts have `.cmd` equivalents of every script in the same
directory.

On a fresh checkout the first run will create `./data/garlemald.db`,
apply the bundled schema, seed every reference table, and boot the
stack on localhost. Point
[Garlemald Client](https://github.com/swstegall/Garlemald-Client) or a
patched 1.23b client at `127.0.0.1:54994` and sign up via the web
endpoint to get a playable session.

## Configuration

Each service reads its own TOML file under `configs/`. All four share
the same SQLite database by default.

| Service       | Config               | Default bind      | Notes                                                      |
|---------------|----------------------|-------------------|------------------------------------------------------------|
| `web-server`  | `configs/web.toml`   | `127.0.0.1:54993` | Signup / login; issues session tokens                      |
| `lobby-server`| `configs/lobby.toml` | `127.0.0.1:54994` | Character list and world handoff                           |
| `world-server`| `configs/world.toml` | `127.0.0.1:54992` | World registration (`world_id` keyed to the `servers` row) |
| `map-server`  | `configs/map.toml`   | `127.0.0.1:1989`  | Zone, actor, AI, and Lua host                              |

Override any config path with `--config <path>` on the corresponding
binary. Environment knobs (packet logging, Lua script root, etc.) are
documented in each `configs/*.toml` file.

## Layout

```
garlemald-server/
├── common/            # Protocol, packet log, db, Lua bindings (shared crate)
├── lobby-server/      # Lobby / character list binary
├── world-server/      # World registration / handoff binary
├── map-server/        # Zone / actor / AI / Lua host binary
├── web-server/        # axum HTTP signup + login binary
├── configs/           # Per-service TOML configs (localhost defaults)
├── scripts/
│   ├── run-*.sh       # Build + launch helpers (bash and .cmd variants)
│   └── lua/           # 1,142 upstream Lua scripts — quests, directors, shops, ...
├── data/              # SQLite database and WAL (created on first run)
├── data-backups/      # Zip snapshots of known-good save states
└── logs/              # Per-server log files
```

## Attribution and licensing

Garlemald Server stands on the shoulders of
[Project Meteor Server](https://bitbucket.org/Ioncannon/project-meteor-server/)
and [Seventh Umbral](https://github.com/Meteor-Project/SeventhUmbral)
for the FFXIV 1.x side, and of
[LandSandBoat](https://github.com/LandSandBoat/server) (and its
**DarkStar Project** ancestor) for the FFXI-inherited portions of the
combat, mob-AI, enmity, skillchain, status-effect, and mod-shelf
subsystems that 1.x carried forward from FFXI. Additional
indebtedness goes to the broader FFXIV 1.0 preservation community —
the FFXIV Classic wiki, the Project Meteor Discord, the Mirke
Menagerie archive, Gamer Escape, and the many packet-capture and
spreadsheet contributors whose work made the port feasible. See
[`NOTICE.md`](NOTICE.md) for the full attribution list (including the
GPL-3 → AGPL-3 combined-work rules that apply to any verbatim
LandSandBoat translations) and [`LICENSE.md`](LICENSE.md) for the full
terms of the **GNU Affero General Public License, version 3 or later**,
under which this project is distributed.

## Sister projects

- **[Garlemald Client](https://github.com/swstegall/Garlemald-Client)** —
  cross-platform Rust launcher that speaks this server's lobby / patch
  handshake and manages its own Wine runtime on macOS and Linux.
- **[XIV 1.0 Apple Silicon Installer](https://github.com/swstegall/XIV-1.0-Apple-Silicon-Installer)** —
  bash installer that stands up a working 1.23b client on Apple
  Silicon Macs, which the client above can then drive against this
  server.

## Community

Development discussion, bug reports, and questions for the maintainer
happen on the Garlemald Server Discord:

<https://discord.gg/CVjwWs6jnX>
