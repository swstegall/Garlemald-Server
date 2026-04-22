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

## Acknowledgments

Thanks to Ioncannon, Jean-Philip Desjardins, and every contributor to
Project Meteor Server, Seventh Umbral, the FFXIV Classic wiki
(<http://ffxivclassic.fragmenterworks.com/wiki/>), the Project Meteor
Discord, the Mirke Menagerie archive, Gamer Escape's Loremonger
namespace, and the wider community of 1.0 preservationists whose
notes, spreadsheets, and packet captures made this port feasible.

Any bugs in the Rust port are the fault of this project, not of the
upstream work it builds on.
