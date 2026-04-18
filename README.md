# Garlemald Server

## Contents

1. [NOTICE](#notice)
2. [Introduction](#introduction)
   - [What's different from Project Meteor Server](#whats-different-from-project-meteor-server)
   - [Server Layout](#server-layout)
3. [Requirements](#requirements)
   - [Running the server](#running-the-server)
   - [Running the client](#running-the-client)
   - [Optional downloads](#optional-downloads)
4. [Building from source](#building-from-source)
   - [Get the code](#get-the-code)
   - [Build](#build)
5. [Server Setup](#server-setup)
   - [1. Database](#1-database)
   - [2. Static actor data](#2-static-actor-data)
   - [3. Lua script tree](#3-lua-script-tree)
   - [4. Configure](#4-configure)
   - [5. Create an account](#5-create-an-account)
6. [Starting the servers](#starting-the-servers)
7. [Client Setup](#client-setup)
8. [Logging](#logging)
9. [Testing](#testing)
10. [Layout](#layout)
11. [Troubleshooting](#troubleshooting)
12. [References](#references)

## NOTICE

This project serves the **FINAL FANTASY XIV v1.23b** client (the original 1.0
iteration of the game), not A Realm Reborn. If your client reports
`2012.09.19.0001` as its version, you are in the right place; any other
client version will not negotiate the wire protocol correctly.

## Introduction

Garlemald is a Rust port of
[Project Meteor Server](https://bitbucket.org/Ioncannon/project-meteor-server/).
It preserves the three-tier FFXIV 1.0 architecture (lobby / world / map)
and the original wire protocol, but collapses every external service the
upstream C# project relies on - web server, PHP processor, MySQL / MariaDB
daemon, WAMP stack - into a single cargo workspace that runs anywhere
Rust does.

The reference C# tree lives alongside this repo in the parent workspace at
`../project-meteor-mirror/`. When behaviour is ambiguous, that source is
authoritative.

### What's different from Project Meteor Server

| Aspect           | Project Meteor                       | Garlemald                                         |
|------------------|--------------------------------------|---------------------------------------------------|
| Language         | C# (.NET Framework 4.5)              | Rust (2024 edition, pinned to 1.95)               |
| Platform         | Windows only, Visual Studio          | any Rust tier-1 target (macOS, Linux, Windows)    |
| Web/login server | Apache + PHP account creation        | built-in `web-server` crate (axum, argon2)        |
| Database         | MySQL 5.7 / MariaDB 10 running       | SQLite file created automatically on first boot   |
| WAMP stack       | required (or manual Apache+PHP+SQL)  | not required                                      |
| Scripting        | Lua 5.1 via NLua                     | Lua 5.4 via mlua (vendored, built from source)    |
| Config format    | `.ini`                               | `.toml` (with serde + sensible defaults)          |
| Config defaults  | none - copy from `data/` by hand     | localhost defaults committed to `configs/`        |

### Server Layout

A running Garlemald rig is four long-lived processes sharing one SQLite
file:

```
                              +--------------------+
                              |   SQLite file      |
                              |   ./data/          |
                              |   garlemald.db     |
                              +--------------------+
                              ^    ^    ^    ^
                              |    |    |    |
            +-----------------+    |    |    +-----------------+
            |                      |    |                      |
   +---------------+     +---------------+   +---------------+   +---------------+
   |   web-server  |     |  lobby-server |   |  world-server |   |   map-server  |
   |    :54993     |     |    :54994     |   |    :54992     |   |    :1989      |
   +---------------+     +---------------+   +---------------+   +---------------+
           ^                     ^                   ^                   ^
       (browser /                |                   |                   |
        webview)                 +-------------------+-------------------+
           |                                         |
           |                            +--------------------------+
           |                            | FFXIV 1.23b client       |
           |                            | (SeventhUmbral Launcher  |
           +--- login / signup -------->|  or garlemald-client)    |
                                        +--------------------------+
```

Each process has a narrow role:

1. **Web server (port 54993).** Serves the login + signup HTML forms.
   On a successful submission it inserts a row into `users` (signup) /
   `sessions` (both flows) and redirects the caller to
   `ffxiv://login_success?sessionId=<56-char>`, which the
   `garlemald-client` webview intercepts.
2. **Lobby server (port 54994).** Validates session tokens, fans out the
   character list, handles character create / rename / delete, and hands
   the client off to the world / map pair when a character is selected.
3. **World server (port 54992).** Owns per-account social state: party,
   linkshell, friend list, retainers, MOTD. Talks to the map server to
   route in-world chat.
4. **Map server (port 1989).** Runs the actual game world - zones, actors,
   NPCs, battle, inventory, events, Lua scripting, and the 100 ms tick
   loop.

Unlike the retail layout (multiple worlds, each with multiple region
servers), Garlemald ships a single world / map pair. Running multiple
worlds is possible by copying the config + data dir and bumping the ports
and `world_id`.

The web server is a single listener on :54993 and can be disabled (or
fronted by a reverse proxy for TLS) without affecting the game protocol -
the client only talks to it through its webview, and the three game
servers only read the rows it produces in `sessions` / `users`.

## Requirements

### Running the server

| Component     | Minimum                          | Notes                                                                                   |
|---------------|----------------------------------|-----------------------------------------------------------------------------------------|
| Rust toolchain| 1.95.0                           | Pinned in `rust-toolchain.toml`; `rustup` installs automatically on first build.        |
| C compiler    | any (`cc`, `clang`, `gcc`, MSVC) | `mlua` and `rusqlite` build Lua and SQLite from source via vendored / bundled features. |
| Disk          | ~500 MB                          | Compile artefacts plus SQLite data file.                                                |
| OS            | macOS, Linux, Windows            | Any tier-1 Rust target.                                                                 |

No external web server, no PHP, no MySQL / MariaDB daemon, no WAMP
install. Garlemald's own `web-server` binary handles login/signup; put a
reverse proxy in front of it if you need TLS.

### Running the client

| Component              | Version                                                                          |
|------------------------|----------------------------------------------------------------------------------|
| Final Fantasy XIV 1.23b| `2012.09.19.0001`                                                                |
| Launcher               | Seventh Umbral Launcher 1.03 (Windows), or `../garlemald-client` (cross-platform)|

If your client install is older than 1.23b, either launcher can bring it
up to date via the patch flow.

### Optional downloads

Only required for full gameplay (zones, NPCs, items, etc.):

- **Project Meteor data assets** at `../project-meteor-mirror/Data/`:
  - `scripts/` - Lua script tree the map server loads at boot. Without
    it the server still runs but NPCs have no behaviour.
  - `staticactors.bin` - the compiled actor-class table; see
    [Server Setup step 2](#2-static-actor-data).

Garlemald's SQLite schema is embedded at `common/sql/schema.sql` and
applied automatically on first run. Every `*.sql` file Project Meteor
shipped under `Data/sql/` was ported to SQLite one-shot and now lives
as a set of numbered migrations under `common/sql/seed/`
(`001_gamedata_achievements.sql`, `002_gamedata_actor_appearance.sql`,
...). So `gamedata_items`, `gamedata_actor_class`, `server_zones`,
`server_battle_commands`, `server_battlenpc_*`, etc. are populated
automatically on first boot. You do **not** need to import anything by
hand.

To add more seed data later, drop a new `NNN_<name>.sql` file into
`common/sql/seed/` (`NNN` being the next free number) and rebuild. On
the next boot, the migration runner applies it inside its own
transaction and records the filename in the `schema_migrations` table
so existing rows — and live user accounts in `users` / `sessions` /
`characters*` — are never touched.

## Building from source

### Get the code

```bash
git clone https://github.com/swstegall/Garlemald-Server.git
cd Garlemald-Server
```

### Build

The workspace builds with plain cargo:

```bash
cargo build --workspace --release
```

Or use the helper scripts under `scripts/`, which run an environment check
first (Rust toolchain, C compiler, `clippy`, `rustfmt`):

```bash
./scripts/check-env.sh         # environment check only
./scripts/build.sh             # release profile (default)
./scripts/build.sh --debug     # dev profile
./scripts/test.sh              # cargo test --workspace
```

Built binaries land at `target/{release,debug}/{lobby-server,world-server,map-server,web-server}`.

If the compile fails on Windows because of the C toolchain, install the
MSVC Build Tools, or pass `--target x86_64-pc-windows-gnu` with
MinGW-w64 on `PATH`.

## Server Setup

### 1. Database

Nothing to configure. The first time any of the four binaries boots
against a non-existent SQLite path, `common/sql/schema.sql` is applied
automatically, followed by every bundled seed migration under
`common/sql/seed/`. The default path is `./data/garlemald.db`; override
it in `configs/*.toml` or via `--db-path`.

The schema seeds a single world row in `servers` (`id=1`, `Fernehalwes`,
`127.0.0.1:54992`) so that lobby -> world handoff works out of the box
for a localhost rig. The seed pass then fills `gamedata_items`,
`gamedata_actor_*`, `server_zones`, `server_battle_commands`,
`server_battlenpc_*`, and their friends from the Project Meteor dumps.

A `schema_migrations` tracking table records which seed files have been
applied; existing databases pick up only new migrations on upgrade,
leaving user rows (`users`, `sessions`, `characters*`, `reserved_names`)
untouched.

### 2. Static actor data

The 1.23b client ships a compiled actor-class table at
`client/script/rq9q1797qvs.san`. Copy it next to the map-server binary
under the name `staticactors.bin`:

```bash
cp "/path/to/FINAL FANTASY XIV/client/script/rq9q1797qvs.san" ./staticactors.bin
```

Without it the map-server still runs; NPC class resolution will be
limited.

### 3. Lua script tree

Point `scripting.script_root` in `configs/map.toml` at Project Meteor's
`Data/scripts/` directory, or symlink it next to the binary:

```bash
ln -s ../project-meteor-mirror/Data/scripts ./scripts
```

Without scripts, the map server boots but NPC / quest / director behaviour
is absent.

### 4. Configure

Localhost defaults are committed as `configs/{web,lobby,world,map}.toml`.
They work as-is for a single-box rig. For multi-machine deployments,
edit the relevant fields.

`configs/web.toml`:

```toml
[server]
bind_ip = "0.0.0.0"          # bind on all interfaces
port = 54993
show_timestamp = true

[database]
path = "./data/garlemald.db"

[session]
hours = 24                   # session-row lifetime on successful login
```

`configs/lobby.toml`:

```toml
[server]
bind_ip = "0.0.0.0"          # bind on all interfaces
port = 54994
show_timestamp = true

[database]
path = "./data/garlemald.db"
```

`configs/world.toml`:

```toml
[server]
bind_ip = "0.0.0.0"
port = 54992
show_timestamp = true
world_id = 1                 # row id in the `servers` table

[database]
path = "./data/garlemald.db"
```

`configs/map.toml`:

```toml
[server]
bind_ip = "0.0.0.0"
port = 1989
show_timestamp = true
world_id = 1

[database]
path = "./data/garlemald.db"

[scripting]
script_root = "./scripts"
load_from_database = true
```

Also update the `servers` table so the address / port the lobby hands to
clients is reachable from the outside network:

```bash
sqlite3 ./data/garlemald.db \
  "UPDATE servers SET address='198.51.100.42', port=54992 WHERE id=1;"
```

Default CLI overrides (every binary accepts these):

```
--ip <ADDR>         bind IP
--port <PORT>       bind port
--db-path <PATH>    SQLite file path
--world-id <ID>     servers.id (world + map only)
--config <PATH>     TOML path (default ./configs/{lobby,world,map}.toml)
```

Default field values if a key (or the whole file) is missing:

| Key                             | Web                   | Lobby                 | World                 | Map                   |
|---------------------------------|-----------------------|-----------------------|-----------------------|-----------------------|
| `server.bind_ip`                | `127.0.0.1`           | `127.0.0.1`           | `127.0.0.1`           | `127.0.0.1`           |
| `server.port`                   | `54993`               | `54994`               | `54992`               | `1989`                |
| `server.world_id`               | -                     | -                     | `1`                   | `1`                   |
| `database.path`                 | `./data/garlemald.db` | `./data/garlemald.db` | `./data/garlemald.db` | `./data/garlemald.db` |
| `session.hours`                 | `24`                  | -                     | -                     | -                     |
| `scripting.script_root`         | -                     | -                     | -                     | `./scripts`           |
| `scripting.load_from_database`  | -                     | -                     | -                     | `true`                |

### 5. Create an account

Start the web server (see [Starting the servers](#starting-the-servers))
and open `http://127.0.0.1:54993/signup` — either directly in a browser
or, more usefully, by clicking the **Log in** button in
`garlemald-client`, which pops a webview onto the same URL. Filling in
the form inserts a row into `users` and a fresh 56-character row into
`sessions` in one round-trip, then 302-redirects the webview to
`ffxiv://login_success?sessionId=…`; the client intercepts that scheme
and hands the token off to the game.

Returning users hit `/login` (the same webview entry point defaults
there).

For scripted or emergency access, the old
"insert a row by hand" recipe still works against the bare database:

```bash
sqlite3 ./data/garlemald.db \
  "INSERT INTO sessions (id, userId, expiration) VALUES \
   (printf('%056d', 1), 1, datetime('now', '+1 day'));"
```

and the launcher has a developer session-id override for that case.

## Starting the servers

Run all four at once, with each process's stdout / stderr tee'd to
`logs/{web,lobby,world,map}-server.log`:

```bash
./scripts/run-all.sh
tail -f logs/*.log
```

Ctrl-C on the wrapper shuts the whole stack down cleanly.

Or start them individually:

```bash
./scripts/run-web.sh
./scripts/run-lobby.sh
./scripts/run-world.sh
./scripts/run-map.sh
```

Or invoke the binaries directly:

```bash
./target/release/web-server   --config ./configs/web.toml
./target/release/lobby-server --config ./configs/lobby.toml
./target/release/world-server --config ./configs/world.toml
./target/release/map-server   --config ./configs/map.toml
```

The map server also runs an interactive stdin console - type commands at
the running process the same way the C# server's `Console.ReadLine`
loop accepted them.

## Client Setup

If you are using the original Windows-only **Seventh Umbral Launcher**:

1. Install the Final Fantasy XIV 1.x client.
2. Install the Seventh Umbral launcher.
3. Edit `<Seventh Umbral launcher install location>/servers.xml` and add:
   ```xml
   <Server Name="Localhost" Address="127.0.0.1" LoginUrl="http://127.0.0.1:54993/login" />
   ```
   If the server is on a different machine, replace `127.0.0.1` with
   its IP.
4. Launch `Launcher.exe`, open **Game Settings**, and point at the FFXIV
   1.x install directory.
5. Select **Localhost** from the server dropdown. The launcher opens the
   web-server's `/login` page; sign in (or follow the "Create one" link
   if this is your first run) and the launcher receives the session id
   automatically.

If you are using the cross-platform **garlemald-client** rewrite:

1. `cd ../garlemald-client && cargo run --release`.
2. A `Localhost` entry is pre-seeded in the client's default server list
   pointing at `127.0.0.1`.
3. See `../garlemald-client/README.md` for platform-specific setup
   (Wine prefix on macOS / Linux, registry paths on Windows, etc.).

## Logging

Every process prefixes each line with an ASCII tag so three servers
sharing one terminal (or three log files tailed together) stay readable:

```
[WEB]   2026-04-18T03:43:34Z INFO  web_server::server:   web server listening bind_addr=127.0.0.1:54993
[LOBBY] 2026-04-18T03:43:35Z INFO  lobby_server::server: lobby server listening addr=127.0.0.1:54994
[WORLD] 2026-04-18T03:43:34Z INFO  world_server::server: world server listening addr=127.0.0.1:54992
[MAP]   2026-04-18T03:43:35Z INFO  map_server::server:   map server listening addr=127.0.0.1:1989
```

ANSI colour escapes are disabled so `logs/*.log` stays plain text.

Default filter is `info` for third-party crates and `debug` for
Garlemald's own code - you see per-packet dispatch and DB query tracing
out of the box. Override via `RUST_LOG`:

```bash
RUST_LOG=trace ./target/release/map-server
RUST_LOG=map_server=trace,info ./scripts/run-all.sh
RUST_LOG=web_server=trace,tower_http=debug,info ./scripts/run-web.sh
```

## Testing

```bash
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all -- --check
```

The map-server integration tests use a temp SQLite file per test, so
they exercise the real schema without needing any external service.

## Layout

```
garlemald-server/
|-- Cargo.toml             workspace manifest
|-- rust-toolchain.toml    pinned toolchain (1.95.0)
|-- configs/               TOML configs for each binary (localhost defaults)
|   |-- web.toml
|   |-- lobby.toml
|   |-- world.toml
|   `-- map.toml
|-- common/                shared crate (packet / crypto / db / logging)
|   |-- build.rs           gzips every seed file at compile time
|   |-- sql/schema.sql     SQLite schema, applied on first run
|   `-- sql/seed/          ported Meteor SQL dumps (one migration per file)
|-- web-server/            binary crate (login / signup HTTP forms)
|-- lobby-server/          binary crate
|-- world-server/          binary crate
|-- map-server/            binary crate
|-- scripts/               build + run wrappers (bash)
`-- logs/                  created on first run; per-server log files
```

## Troubleshooting

- **"connection refused" from the client.** Check that the relevant
  server is listening (`lsof -i :54994` or `netstat -an | grep 54994`)
  and grep `logs/lobby-server.log` for errors.
- **Webview says "unable to connect" when clicking Log in.** The web
  server isn't running or isn't reachable. `curl http://127.0.0.1:54993/healthz`
  should print `ok`; if not, check `logs/web-server.log`.
- **"Your session has expired, please login again."** No matching row
  in `sessions` for the token the client sent. Sign in again through
  the launcher — the web server will mint a fresh row.
- **Map server logs `zones loaded zones=0`.** Shouldn't happen out of
  the box — the bundled `033_server_zones.sql` migration seeds 111
  rows on first boot. If an older database was carried forward before
  the migration runner existed, delete `./data/` and reboot, or run
  `sqlite3 ./data/garlemald.db "DELETE FROM schema_migrations"` then
  boot again to force the seed pass to re-apply.
- **Build error `linker cc not found`.** Install Xcode CLI tools
  (`xcode-select --install`) on macOS, `build-essential` on
  Debian / Ubuntu, or MSVC Build Tools on Windows.
- **`duplicate column name` at DB init.** You are opening a file
  created under an older schema. Delete `./data/` and boot again to
  regenerate.
- **Windows firewall blocks incoming connections.** Add inbound rules
  for TCP 54994, 54992, and 1989 (or run the servers on different
  ports and update `configs/*.toml` accordingly).

## References

- Upstream Project Meteor Server: <https://bitbucket.org/Ioncannon/project-meteor-server/>
- Setup wiki (schema, client patching, full context):
  <http://ffxivclassic.fragmenterworks.com/wiki/index.php/Setting_up_the_project>
- Reference C# source lives at `../project-meteor-mirror/` in this workspace.
- Sibling launcher: `../garlemald-client/`.
