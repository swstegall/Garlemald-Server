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

//! Gathering runtime data. Models the node/drop pool the 1.x harvest
//! minigame (mining / logging / fishing / botany) keys off.
//!
//! The ioncannon `crafting_and_localleves` branch shipped a mining-only
//! slice of this system in Lua (hardcoded `harvestNodeContainer` +
//! `harvestNodeItems` tables inside `Data/scripts/commands/DummyCommand.lua`),
//! matching the packet/director shape the retail client expects. Garlemald
//! lifts those tables into SQL (`gamedata_gather_nodes`, `gamedata_gather_node_items`,
//! `server_gather_node_spawns`) and exposes the resolver to Lua so the
//! full harvest loop (click node → run minigame → write loot back to
//! inventory) can be driven uniformly for all four gather types.
//!
//! Per-player mutable state (the in-flight minigame's currentPower /
//! attempts-remaining / remainder counters) lives in the `DummyCommand.lua`
//! script frame — there is no Rust-side session object to persist across
//! a strike, and the minigame runs via `callClientFunction` RPCs so it
//! couldn't be driven headlessly even if there were one.

#![allow(dead_code)]

pub mod node;
pub mod resolver;
pub mod spawn;

pub use node::{GatherNode, GatherNodeItem, NODE_ITEM_SLOTS};
pub use resolver::GatherResolver;
pub use spawn::{GatherNodeSpawn, HARVEST_TYPE_MINE};

// Re-exported for tests and callers that want the full harvest-type
// namespace + the pivoted aim-slot record. Kept behind
// `#[allow(unused_imports)]` so builds that only reach the DB-loader
// entry points don't trip the unused-imports lint.
#[allow(unused_imports)]
pub use node::{AIM_SLOTS, AimSlot};
#[allow(unused_imports)]
pub use spawn::{HARVEST_TYPE_FISH, HARVEST_TYPE_LOG, is_valid_harvest_type};
