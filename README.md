# Garlemald Server

Garlemald Server is a Rust port of the original
[Project Meteor Server](https://bitbucket.org/Ioncannon/project-meteor-server/),
a private-server emulator for **FINAL FANTASY XIV v1.23b** — the final
patch of the original 1.0 release, not *A Realm Reborn*. The lobby,
world, and map services, the 1.x wire protocol, and the Lua-driven
content system are preserved; the external stack (MySQL, PHP, WAMP)
collapses into a single Cargo workspace that runs on any platform Rust
targets.

Only a client reporting version `2012.09.19.0001` will negotiate the
wire protocol correctly.

> Created with [Claude](https://claude.ai/).

## Attribution

This port stands on the shoulders of Project Meteor Server and
Seventh Umbral, plus the wider 1.0 preservation community. See
[`NOTICE.md`](NOTICE.md) for the full attribution list and
[`LICENSE.md`](LICENSE.md) for license terms (AGPL-3.0-or-later).

## Sister projects

- **[Garlemald Client](https://github.com/swstegall/Garlemald-Client)** —
  cross-platform Rust launcher that speaks this server's lobby / patch
  handshake.
- **[XIV 1.0 Apple Silicon Installer](https://github.com/swstegall/XIV-1.0-Apple-Silicon-Installer)** —
  tooling to install and run the 1.23b client on Apple Silicon Macs.

## Community

Development discussion, bug reports, and questions for the maintainer
happen on the Garlemald Server Discord:
<https://discord.gg/CVjwWs6jnX>.
