# NOTICE

Garlemald Server is a Rust port of a FINAL FANTASY XIV v1.23b server
emulator (lobby / world / map). The port was made possible by, and
derives directly from, the following upstream projects:

## Project Meteor Server

- Source: <https://bitbucket.org/Ioncannon/project-meteor-server>
- License: GNU Affero General Public License v3.0 (AGPL-3.0)

Project Meteor Server is the C# FFXIV 1.23b server emulator maintained
by Ioncannon and contributors, spanning a Lobby Server, World Server,
Map Server, and Common Class Library, with Lua-driven content scripts
for quests, NPCs, directors, private areas, and commands.

`garlemald-server` is a reimplementation of that design in Rust:

- The lobby / world / map split, session handoff, and Blowfish-keyed
  packet framing follow Project Meteor's wire protocol.
- Opcodes, packet layouts, actor-spawn fields (AddActor, SetActorIcon,
  SetSpeed, SetLookNew, SetMainStats, motion packs, FACEINFO bitfield,
  SPAWNTYPE values, Murmur2 tail, etc.), and the Director / ContentArea
  / PrivateArea / ZoneDirector / WeatherDirector state machines
  mirror the behavior of the C# server.
- The SQL schema, zone / NPC / warp / spawn-location data, Lua quest
  and populace scripts, shop packs, and levequest categories are
  ported from the Project Meteor data set.

Credit for the reverse-engineering of the FFXIV 1.x protocol, data
model, and content scripting system belongs to the Project Meteor
Server authors. Where this Rust port substitutes SQLite for MySQL,
TOML for INI, and `tokio-rusqlite` / `mlua` for ADO.NET / NLua, the
underlying game-server semantics are faithful to the C# reference
implementation.

## Seventh Umbral

- Source: <https://github.com/Meteor-Project/SeventhUmbral>
- License: 2-clause BSD-style (see upstream `License.txt`)

Seventh Umbral is the companion Windows-only C++ launcher, renderer,
and client-side research suite that drove the 1.23b client against
Project Meteor Server. While `garlemald-server` does not port Seventh
Umbral's code directly, the project's notes on the login / patch
handshake, server-address injection, and 1.x client quirks informed
the server-side handling of early-session flows (lobby handshake,
patch advertisement, world list, character-create round trip).

## LandSandBoat (FFXI reference)

- Source: <https://github.com/LandSandBoat/server>
- License: GNU General Public License v3.0 (GPL-3.0) — a copy of the
  upstream tree is held at
  [`../land-sand-boat-server/`](../land-sand-boat-server/) in the
  workspace for offline reference
- Upstream lineage: LandSandBoat is a community fork of the original
  **DarkStar Project** FFXI server emulator (2010 –), and the source
  headers of most C++ files in the repository still carry
  `Copyright (c) 2010-2015 Darkstar Dev Teams`; both lineages are
  credited here

LandSandBoat is an actively-maintained open-source **Final Fantasy XI**
private server (C++ core + Lua scripts + MariaDB schema). It is **not**
an FFXIV 1.x server and does not speak the 1.x wire protocol. It is
credited here because FINAL FANTASY XIV 1.x was directed by Nobuaki
Komoto and produced by Hiromichi Tanaka, both coming straight off the
FFXI team, and 1.x reused XI's design grammar for stats
(STR/DEX/VIT/INT/MND/CHR), TP-and-Weaponskill combat, the hit/crit/
level-difference roll ladder, the six-element wheel with weather/day
amplification, enmity (Cumulative + Volatile), claim, aggro
bitflags (sight/sound/magic/blood/low-HP), mob stat generation from
pool × family × spawn, skillchain / Battle Regimen resolution, and
the flat `Mod` stat-shelf idiom.

LandSandBoat is the most reverse-engineered, retail-cross-referenced,
BG-wiki- / Studio-Gobli-annotated public source of those XI-side
formulas. Where `garlemald-server` implements the XI-inherited pieces
of 1.x's combat and mob-AI systems, the **structural** derivation
(which variables enter the formula, in what order, with what caps and
rolls) is drawn from LandSandBoat's `src/map/utils/battleutils.cpp`,
`src/map/attack.cpp`, `src/map/modifier.h`, `src/map/ai/controllers/`,
`src/map/ai/states/`, `src/map/enmity_container.cpp`,
`src/map/status_effect_container.cpp`, `src/map/utils/mobutils.cpp`,
and the Lua files under `scripts/globals/combat/`. The *numbers*
(constants, caps, breakpoints) are then cross-checked and replaced
with 1.x-specific values from the mozk-tabetai FFXIV 1.x database
dump, the FFXIV 1.x battle-command table, the FFXIV Classic wiki,
the Project Meteor Discord archive, and the other 1.x-specific
sources listed in the parent workspace's `CLAUDE.md`.

A full mining guide for the LandSandBoat tree is kept at
[`../land-sand-boat-server/xi-private-server.md`](../land-sand-boat-server/xi-private-server.md).
Contributors porting XI-inherited logic into this project are expected
to consult that guide, prefer **re-derive-and-cite** over verbatim
translation, and leave a breadcrumb comment in Rust
(`// LSB: <relative/path/to/file.lua>::<function_name>`) so a future
reader can replay the derivation.

**Licensing note.** LandSandBoat is **GPL-3.0** while
`garlemald-server` is **AGPL-3.0-or-later**. Section 13 of GPLv3
expressly authorises combining a GPL-3 work with an AGPLv3 work; the
resulting combined work must be distributed under AGPLv3. Any
**verbatim** translation of LandSandBoat code (C++ → Rust or Lua →
Lua) into `garlemald-server` therefore triggers that combined-work
rule and must be explicitly listed in this NOTICE with the specific
file it derives from. Re-derivations from a formula *description*
(reading the LandSandBoat source + the wiki citations its comments
point at, then typing a fresh Rust implementation) are treated as
clean-room and do not need per-file listing, but the general
acknowledgment above still applies.

## Acknowledgments

Thanks to Ioncannon, Jean-Philip Desjardins, and every contributor to
Project Meteor Server, Seventh Umbral, the LandSandBoat and DarkStar
Project developer teams (past and present), the FFXIV Classic wiki
(<http://ffxivclassic.fragmenterworks.com/wiki/>), the Project Meteor
Discord, the Mirke Menagerie archive, Gamer Escape's Loremonger
namespace, and the wider community of 1.0 preservationists whose
notes, spreadsheets, and packet captures made this port feasible.

Any bugs in the Rust port are the fault of this project, not of the
upstream work it builds on.
