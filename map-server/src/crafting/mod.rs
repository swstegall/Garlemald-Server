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

//! Crafting + local-leve data, ported from
//! `project-meteor-server` / `origin/ioncannon/crafting_and_localleves`.
//!
//! This is the read-only in-memory catalog side — DTOs for recipes and
//! passive (local) guildleve definitions, plus a [`RecipeResolver`] that
//! indexes 5 384 recipes by id and by the ordered 8-slot material
//! fingerprint the `CraftCommand.lua` minigame uses to narrow a pick from
//! the craft-start widget.
//!
//! Per-player mutable state (a [`PassiveGuildleve`](crate::actor::quest)
//! sitting in the quest journal with a difficulty + attempt counters)
//! lives alongside the quest system; this module stays value-type only.

#![allow(dead_code)]

pub mod passive_guildleve;
pub mod passive_guildleve_data;
pub mod recipe;
pub mod resolver;

#[allow(unused_imports)]
pub use passive_guildleve::{
    HAS_MATERIALS_FLAG_BIT, LOCAL_LEVE_ID_MAX, LOCAL_LEVE_ID_MIN, PassiveGuildleveView,
    is_local_leve_quest_id,
};
pub use passive_guildleve_data::PassiveGuildleveData;
pub use recipe::Recipe;
pub use resolver::RecipeResolver;
