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

//! NPC actor types + spawning pipeline. Port of `Map Server/Actors/Chara/Npc/*`.
//!
//! Layout:
//!
//! * `actor_class` — `ActorClass` metadata + push-command hints.
//! * `mob_modifier` — `MobModifier` enum + per-NPC tuning values.
//! * `npc_work` — transient NPC state (push command + hate type).
//! * `npc` — the `Npc` struct (wraps `Character` + class metadata).
//! * `battle_npc` — `BattleNpc` (wraps `Npc` + modifier layers + respawn).
//! * `ally`, `pet`, `retainer` — thin specialisations.
//! * `spawner` — the boot-time spawn pipeline that turns
//!   `SpawnLocation` seeds into live actors in the registry.

#![allow(dead_code, unused_imports, clippy::module_inception)]

pub mod actor_class;
pub mod ally;
pub mod battle_npc;
pub mod mob_modifier;
pub mod npc;
pub mod npc_work;
pub mod pet;
pub mod retainer;
pub mod spawner;

pub use actor_class::ActorClass;
pub use ally::Ally;
pub use battle_npc::{BattleNpc, DetectionType, KindredType, ModifierLayer};
pub use mob_modifier::{MobModifier, MobModifierMap};
pub use crate::actor::event_conditions::{
    EmoteCondition, EventConditionList, NoticeCondition, PushBoxCondition, PushCircleCondition,
    PushFanCondition, TalkCondition,
};
pub use npc::{EventConditionMap, Npc};
pub use npc_work::{HATE_TYPE_ENGAGED, HATE_TYPE_ENGAGED_PARTY, HATE_TYPE_NONE, NpcWork};
pub use pet::Pet;
pub use retainer::Retainer;
pub use spawner::{SpawnContext, spawn_all_actors, spawn_from_location};
