// garlemald-server — Rust port of a FINAL FANTASY XIV v1.23b server emulator (lobby/world/map)
// Copyright (C) 2026  Samuel Stegall
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU Affero General Public License as published
// by the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU Affero General Public License for more details.
//
// You should have received a copy of the GNU Affero General Public License
// along with this program.  If not, see <https://www.gnu.org/licenses/>.
//
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Event + quest runtime. Port of `Map Server/Actors/Quest/*` +
//! the event-dispatch half of `PacketProcessor.cs`.
//!
//! Layout:
//!
//! * `quest` — the per-player `Quest` runtime (phase, flag bits, JSON
//!   data blob, completion/abandon hooks).
//! * `outbox` — typed events emitted by event/quest mutations. Same
//!   pattern as inventory / status / battle / area: the game loop
//!   drains the outbox each tick and turns events into packet sends,
//!   DB writes, and Lua calls.
//! * `session` — per-player "currently running event" state
//!   (`current_event_owner`, `current_event_name`, `current_event_type`).
//! * `dispatcher` — packet + DB + Lua side-effect resolver.

#![allow(dead_code, unused_imports)]

pub mod dispatcher;
pub mod lua_bridge;
pub mod outbox;
pub mod quest;
pub mod session;

pub use dispatcher::dispatch_event_event;
pub use lua_bridge::translate_lua_commands_into_outbox;
pub use outbox::{EventEvent, EventOutbox};
pub use quest::{Quest, QuestFlags};
pub use session::EventSession;
