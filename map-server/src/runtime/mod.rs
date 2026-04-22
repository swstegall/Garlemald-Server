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

//! Per-tick game-loop runtime.
//!
//! This module bridges the typed event outboxes (inventory / status / battle
//! / zone) and the real world: packet sends, database writes, Lua calls,
//! broadcast fan-out. The `GameTicker` owns the scheduler; the
//! `ActorRegistry` holds live `Character` state for every player/npc/mob;
//! the `dispatcher` submodule turns individual events into side effects.

#![allow(dead_code, unused_imports)]

pub mod actor_registry;
pub mod broadcast;
pub mod dispatcher;
pub mod ticker;

#[cfg(test)]
mod integration_tests;

pub use actor_registry::{ActorHandle, ActorKindTag, ActorRegistry};
pub use broadcast::broadcast_around_actor;
pub use dispatcher::{
    dispatch_area_event, dispatch_battle_event, dispatch_inventory_event, dispatch_status_event,
};
pub use ticker::{GameTicker, TickerConfig};
