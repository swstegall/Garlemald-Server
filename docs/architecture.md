# Architecture

This document explains **what each Garlemald Server binary owns**, **how the
binaries talk to each other**, and **how they talk to the game client**. It is
the map you want before reading any single crate.

If you just want to build and run the stack, start with
[`dev-environment.md`](dev-environment.md). For the Lua content system that
`map-server` hosts, see [`lua-runtime.md`](lua-runtime.md).

> **Scope.** Garlemald Server emulates **FINAL FANTASY XIV v1.23b** — the last
> patch of the original 2010 1.0 release, *not* A Realm Reborn. It is a Rust
> port of the C# [Project Meteor Server](https://bitbucket.org/Ioncannon/project-meteor-server/);
> where useful, this doc names the upstream C# analogue so you can cross-read.

---

## The shape of the system

Garlemald is a **single Cargo workspace** (`Cargo.toml`, `resolver = "3"`) with
five members: one shared library crate and four service binaries.

| Crate          | Binary?        | Default port | One-line responsibility                                              |
|----------------|----------------|--------------|---------------------------------------------------------------------|
| `common`       | library        | —            | Shared wire primitives, Blowfish, packet logging, tracing, SQLite   |
| `web-server`   | `web-server`   | **54993**    | axum HTTP signup/login; mints session tokens                        |
| `lobby-server` | `lobby-server` | **54994**    | Account/session handoff, character list + creation, world select    |
| `world-server` | `world-server` | **54992**    | World master / session ownership, party & group hierarchy, routing  |
| `map-server`   | `map-server`   | **1989**     | Zone simulation: actors, battle, directors, quests, the Lua runtime |

All four binaries share **one SQLite database** (`./data/garlemald.db` by
default) and `map-server` additionally loads the **shared Lua script root**
(`./scripts/lua/`).

### Topology diagram

```
                          shared SQLite: ./data/garlemald.db
        ┌───────────────────────────────────────────────────────────┐
        │                                                             │
        ▼                                                             ▼
  ┌─────────────┐   (1) HTTP signup/login          ┌──────────────────────────┐
  │ web-server  │◄──────────────────────────────── │                          │
  │  :54993     │   303 → ffxiv://login_success?    │                          │
  │  (axum)     │        sessionId=<56 hex chars>   │                          │
  └─────────────┘ ─────────────────────────────────►          CLIENT          │
        │  writes sessions row                      │   (patched 1.23b game    │
        │                                           │    + Garlemald Client)   │
        ▼                                           │                          │
  ┌─────────────┐   (2) TCP + Blowfish handshake    │                          │
  │ lobby-server│◄──────────────────────────────── │                          │
  │  :54994     │   character list, world select    │                          │
  └─────────────┘ ─────────────────────────────────►                          │
        │  (3) select character →                   └──────────────────────────┘
        │      confirm packet embeds world ip:port            ▲   ▲
        │                                                      │   │
        ▼                                                      │   │
  ┌─────────────┐   (4) TCP: client connects to world         │   │
  │ world-server│◄──────────────────────────────────────────-─┘   │
  │  :54992     │   session ownership, party/linkshell groups      │
  └──────┬──────┘                                                  │
         │ (5) world dials OUT to every advertised zone server     │
         │     (retry-forever supervisor), then relays per-zone    │
         ▼                                                         │
  ┌─────────────┐   (6) zone-in, movement, combat, events,         │
  │  map-server │   directors, quests, Lua — relayed via world ────┘
  │  :1989      │
  └─────────────┘
```

The four ports above are the **localhost defaults** in `configs/*.toml`; every
one is overridable. The exact handoff steps are detailed in
[Inter-server communication](#inter-server-communication) below.

---

## Per-binary responsibilities

### `web-server` (:54993) — HTTP auth frontend

An [axum](https://github.com/tokio-rs/axum) HTTP service that replaces the
upstream PHP/WAMP login stack. It is the *only* binary a player's browser
touches.

- **Routes** (`web-server/src/server.rs::run`): `GET /` (root), `GET|POST
  /login`, `GET|POST /signup`, `GET /healthz`.
- **Handlers** live in `web-server/src/handlers.rs`; account passwords are
  **Argon2**-hashed.
- **Sessions** (`web-server/src/session.rs::generate`): on a successful
  login/signup the server mints a **56-character hex session token** and writes
  a `sessions` row, then redirects to `ffxiv://login_success?sessionId=<token>`
  (HTTP 303). The token's lifetime is the `[session].hours` value in
  `configs/web.toml` (default 24h); the lobby later rejects rows whose
  `expiration` is in the past.
- **Shared state** is an `AppState { db: Arc<Database>, session_hours }` handed
  to axum via `.with_state(...)`.

> The token is the bridge between HTTP-world and TCP-world: the client carries
> it into the lobby handshake (step 2 below), and the lobby validates it against
> the same `sessions` table.

### `lobby-server` (:54994) — character list & world handoff

Anchors: `lobby-server/src/{processor.rs,database.rs,character_creator.rs,hardcoded.rs}`,
`lobby-server/src/packets/`.

- **`processor.rs`** holds the per-connection state (`LobbySession`: a
  per-connection `Blowfish` cipher, the current user id, and the session token)
  and dispatches lobby opcodes — `0x03` get-characters, `0x04` select-character,
  `0x05` session-acknowledge, `0x0B` modify-character.
- **`database.rs`** runs the lobby's SQLite queries: resolve a user id from a
  session token and list characters/retainers/reserved names. The destination
  world list itself is config, not DB — `configs/servers.toml`, loaded at
  startup (`common::server_list`).
- **`character_creator.rs`** holds the per-class starting equipment layout used
  during character creation.
- **`hardcoded.rs`** carries the pre-built `SECURE_CONNECTION_ACKNOWLEDGMENT`
  blob — a fully-formed handshake packet the server encrypts and returns.
- **Handoff** (`processor.rs`, the select-character path): on opcode `0x04` the
  lobby resolves the character's world from the server list and replies with
  a **select-character confirm packet** (opcode `0x0F`) that embeds the
  character id, the session token, and the **world server's IP and port**. The
  client reads those and opens a new TCP connection to `world-server`.

### `world-server` (:54992) — session ownership & routing

Anchors: `world-server/src/{server.rs,world_master.rs,group.rs,database.rs}`.

- **`world_master.rs`** is the `WorldMaster` — the registry that owns live
  sessions and routes traffic. It holds the four group managers
  (`PartyManager`, `LinkshellManager`, `RelationGroupManager`,
  `RetainerGroupManager`). This is the analogue of the C#
  `WorldManager`/`WorldMaster`.
- **`group.rs`** is the group hierarchy: **Party** (max 8), **Linkshell**
  (max 128), **Retainer**, and **Relation** groups, each with a distinct group
  type id, allocated from a monotonic group-id space.
- **`server.rs`** accepts client connections and maintains two per-client
  channels — a **Zone** channel (movement, combat, interaction) and a **Chat**
  channel — tracked in a session registry. Outbound `SubPacket`s are wrapped in
  a `BasePacket` frame before they hit the wire.
- **`database.rs::get_server_zones`** reads the `server_zones` table to discover
  which `(zone_id, ip, port)` map endpoints exist.
- **Zone-server supervision** (`server.rs::connect_zone_servers` →
  `supervise_zone_endpoint` → `run_zone_connection`): on startup the world dials
  **out** to every advertised map/zone endpoint (grouped by unique `ip:port`),
  registers a `ZoneServerHandle` for each zone the endpoint owns, and supervises
  the connection **forever** — on disconnect it deregisters those zones and
  retries with backoff. This is the analogue of C#
  `WorldMaster.ConnectToZoneServers()`. The retry-forever design decouples boot
  order: `map-server` can start, crash, or restart without taking the world down.

### `map-server` (:1989) — the zone simulation

Anchors: `map-server/src/{server.rs,processor.rs,world_manager.rs,command_processor.rs}`,
`map-server/src/{actor/,battle/,lua/,runtime/,packets/}`.

This is the largest crate — the actual game world.

- **`server.rs`** accepts the world-server's TCP connection (same
  `BasePacket`/`SubPacket` framing) and runs per-connection reader/writer tasks.
- **`processor.rs`** dispatches inbound game opcodes (session begin/end,
  zone-in-complete, position updates, chat, events, battle, social).
- **`world_manager.rs`** is the zone + session registry: zone/entrance loaders
  from the DB, boundary boxes, and director init packets.
- **`actor/`** defines the actor types (player `Character`, NPC, `BattleNpc`)
  managed by the actor registry; **`battle/`** is the combat system.
- **`lua/`** is the Lua content engine and **`runtime/`** is the command-apply
  pipeline that turns Lua side effects into wire packets — both documented in
  [`lua-runtime.md`](lua-runtime.md).
- **`command_processor.rs`** bridges inbound game events to the Lua hooks.
- Startup is multi-phase: load config → open DB → build the Lua engine and load
  the content catalogs (quests, items, battle commands, recipes, leves, gather
  data) → build the world manager and actor registry → spawn the game-loop
  ticker → accept connections.

### `common` — shared primitives

Anchors: `common/src/{blowfish.rs,bitstream.rs,bitfield.rs,packet.rs,subpacket.rs,packet_log.rs,logging.rs,db.rs,migrations.rs}`.

Everything the four binaries agree on lives here:

- **`blowfish.rs`** — the 1.x Blowfish cipher (with the original
  implementation's quirks; see [Wire protocol](#how-the-client-talks-to-the-servers)).
- **`bitstream.rs` / `bitfield.rs`** — bit/byte packing primitives for wire
  fields.
- **`packet.rs` / `subpacket.rs`** — the `BasePacket` and `SubPacket` frame
  types (the two-layer envelope every TCP message uses).
- **`packet_log.rs`** — the env-gated hex-dump packet logger (see
  [`dev-environment.md`](dev-environment.md)).
- **`logging.rs`** — the `tracing` setup and `RUST_LOG` filter.
- **`db.rs`** — the shared SQLite layer (`open_or_create`): WAL mode, foreign
  keys on, a 60-second busy timeout, and a `schema_migrations` table so multiple
  binaries booting against the same file don't double-apply the schema.
- **`migrations.rs`** — the bundled schema SQL applied on first run.

---

## Inter-server communication

### The lobby → world → map handoff

A player reaches the game world through a chain of handoffs, each one telling the
client where to connect next:

1. **HTTP login** (`web-server` :54993). The browser posts credentials; the
   server mints a 56-hex-char session token, writes a `sessions` row, and
   redirects to `ffxiv://login_success?sessionId=<token>`. The launcher captures
   the token.
2. **Lobby handshake** (`lobby-server` :54994). The client opens a TCP
   connection and sends an unencrypted hello; the lobby derives a Blowfish key
   and returns the encrypted acknowledgment (see
   [the wire protocol](#how-the-client-talks-to-the-servers)). The client then
   sends a session-acknowledge packet carrying the token, which the lobby
   validates against the `sessions` table.
3. **Character list & select.** The client requests its characters (opcode
   `0x03`); the lobby returns chunked world/account/retainer/character list
   packets. On select (opcode `0x04`), the lobby looks up the character's world
   in the server list (`configs/servers.toml`) and replies with a confirm
   packet (opcode `0x0F`)
   **embedding the world server's IP and port** plus the session token.
4. **World connect** (`world-server` :54992). The client opens a new TCP
   connection to the world address it was just handed, presenting the token. The
   world registers the session and owns it from here on.
5. **Zone routing → map.** The world has already dialed out to every advertised
   zone endpoint (next section). When the client zones in, the world relays its
   traffic to the `map-server` that owns that zone, by zone id.

### How world finds and supervises the map servers

The world does **not** wait for map servers to connect to it — it connects to
**them**:

- `world-server/src/database.rs::get_server_zones` reads the `server_zones`
  table:

  ```sql
  SELECT id, serverIp, serverPort
  FROM   server_zones
  WHERE  serverIp IS NOT NULL AND serverIp <> '' AND serverPort > 0
  ```

  yielding `(zone_id, ip, port)` rows. In the default single-machine setup every
  zone points at `127.0.0.1:1989`.
- `world-server/src/server.rs::connect_zone_servers` groups those rows by unique
  `(ip, port)` endpoint and spawns one **supervisor task per endpoint**
  (`supervise_zone_endpoint`).
- Each supervisor loops forever: connect → register a `ZoneServerHandle` for
  every zone id that endpoint owns (`WorldMaster::register_zone_server`) → run
  the reader/writer (`run_zone_connection`) → on disconnect, deregister those
  zones and retry with backoff. A map server can therefore come and go without
  crashing the world; it simply re-registers on reconnect.

### The packet envelope (world ↔ map and client ↔ server)

Every TCP message — client-facing or server-to-server — uses the same two-layer
envelope, defined in `common`:

- A **`BasePacket`** = a 16-byte header (`common/src/packet.rs`) + a body
  holding one or more `SubPacket`s. The header carries the total size, the
  subpacket count, an "authenticated" (encrypted) flag, a "compressed" (zlib via
  `flate2`) flag, and a timestamp.
- A **`SubPacket`** = a 16-byte header (`common/src/subpacket.rs`) + payload. The
  header carries the subpacket size, a `type`, and a **`source_id` / `target_id`**
  pair. Those ids are the routing hint the world uses to fan a map server's
  messages back to the right client session.
- A **game-message** subpacket additionally prefixes a 16-byte game-message
  header carrying the **opcode** before its body.

The world relays by reading inbound `BasePacket`s from a zone server, extracting
the `SubPacket`s, and routing each one to a session by `target_id` (Zone channel
first, Chat channel as fallback).

---

## How the client talks to the servers

The 1.x client speaks a TCP-framed, Blowfish-encrypted protocol. There is no
single "RUDP" socket — the **Lobby / Zone / Chat** split people refer to is a
logical one: the lobby connection is its own TCP stream, and the world
connection multiplexes a **Zone** channel and a **Chat** channel distinguished
by routing metadata in the subpacket headers.

### The Blowfish-keyed handshake (lobby)

1. **Hello (unencrypted).** The client's first lobby packet is sent in the clear
   and carries a *ticket phrase* and a *client number*.
2. **Key derivation** (`lobby-server`, then `common/src/blowfish.rs`). The server
   builds a fixed-layout buffer from the ticket phrase and client number,
   MD5-hashes it, and uses the digest as the 16-byte Blowfish key. The 1.x
   implementation has a **sign-extension quirk** (key bytes ≥ `0x80` are treated
   as signed before being folded into the key schedule) that diverges from stock
   OpenSSL Blowfish — `common/src/blowfish.rs` reproduces it deliberately so the
   keystream matches the retail client bit-for-bit.
3. **Acknowledgment (encrypted).** The server encrypts and returns the
   `SECURE_CONNECTION_ACKNOWLEDGMENT` blob (`lobby-server/src/hardcoded.rs`).
4. **Session-acknowledge (encrypted).** The client sends opcode `0x05` carrying
   the 56-char session token (and a version string). The lobby resolves the user
   from the token; an unknown/expired token returns an error and the connection
   is refused.

From the acknowledgment onward, lobby traffic is Blowfish-encrypted; the
`map-server` connection is authenticated by the already-validated session id
rather than by re-deriving a Blowfish key.

### Encryption and framing details

- Encryption operates on **8-byte Blowfish blocks**, applied to each subpacket
  body (after its 16-byte header); a misaligned length is a hard error.
- The wire is read by peeking a `BasePacket`'s size field and waiting for the
  full frame before decoding (`common/src/packet.rs`).
- Optional zlib compression (the `is_compressed` header flag) is decompressed
  before subpackets are extracted.

### A note on the client version `2012.09.19.0001`

`2012.09.19.0001` is the **client build the wire protocol targets** — the final
1.23b patch. The companion
[Garlemald Client](https://github.com/swstegall/Garlemald-Client) patches a
stock 1.x install forward to that build so its opcodes and packet layouts match
this server.

The session-acknowledge packet *carries* a version string, and the lobby
currently **logs** it rather than hard-rejecting a mismatch — so a wrong client
build typically fails downstream (mismatched opcodes/layouts) rather than at an
explicit version gate. If you add a strict version check, the lobby session path
(opcode `0x05` handling) is the place to put it.

---

## Where to read next

- [`dev-environment.md`](dev-environment.md) — build the workspace, run the four
  binaries, enable tracing and packet capture, and reset save state.
- [`lua-runtime.md`](lua-runtime.md) — the `map-server` Lua engine: script
  layout, the coroutine scheduler, the hook surface, and the
  command-processor → `LuaCommand` → wire-packet pipeline.
- [`../CONTRIBUTING.md`](../CONTRIBUTING.md) — how to pick up an issue and open a
  PR.
- `porting-progress-context.md` (parent workspace) — the subsystem status matrix
  and roadmap for what is and isn't implemented yet.
