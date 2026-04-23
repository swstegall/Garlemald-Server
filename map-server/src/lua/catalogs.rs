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

//! Shared read-only gamedata catalogs. Loaded once at startup (from
//! `Database::get_item_gamedata` etc.) and handed out to every Lua VM.
//!
//! Using `std::sync::RwLock` rather than `tokio::sync::RwLock` because Lua
//! globals are called from synchronous script contexts and must not `await`.

#![allow(dead_code)]

use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use crate::crafting::{PassiveGuildleveData, RecipeResolver};
use crate::data::ItemData;
use crate::gamedata::{BattleCommand, GuildleveGamedata, QuestMeta, StatusEffectDef};
use crate::gathering::GatherResolver;

#[derive(Default)]
pub struct Catalogs {
    pub items: RwLock<HashMap<u32, ItemData>>,
    pub guildleves: RwLock<HashMap<u32, GuildleveGamedata>>,
    pub status_effects: RwLock<HashMap<u32, StatusEffectDef>>,
    pub battle_commands: RwLock<HashMap<u16, BattleCommand>>,
    /// Side-index of `battle_commands` grouped by `(class_id, level)`.
    /// Lets the level-up path look up "which abilities unlock on the
    /// transition 1→2 for Gladiator?" in constant time. Populated
    /// alongside the flat `battle_commands` map.
    pub battle_commands_by_level: RwLock<HashMap<(u8, i16), Vec<u16>>>,
    /// Maps static-actor name (e.g. `"DftFst"`) to its fixed actor id.
    pub static_actors: RwLock<HashMap<String, u32>>,
    /// Quest id → metadata (className drives Lua script-path resolution).
    /// Loaded from `gamedata_quests` at startup. ~524 rows from Meteor's
    /// `origin/ioncannon/quest_system` seed.
    pub quests: RwLock<HashMap<u32, QuestMeta>>,
    /// Shared recipe resolver. `Arc` so `GetRecipeResolver()` in Lua
    /// can hand back a userdata wrapper without copying the 5 000-row
    /// catalog — the VM holds a clone of the Arc for its lifetime.
    pub recipes: RwLock<Option<Arc<RecipeResolver>>>,
    /// Static passive-guildleve (local leve) definitions keyed by leve
    /// id (120001..=120452). Read-only after boot; the runtime-mutable
    /// per-player state lives on the quest journal.
    pub passive_guildleves: RwLock<HashMap<u32, PassiveGuildleveData>>,
    /// Shared gathering resolver. Same Arc-under-lock shape as
    /// `recipes` — every Lua VM clones the Arc into its local frame
    /// for `GetGatherResolver():GetNode(...)` lookups.
    pub gather_nodes: RwLock<Option<Arc<GatherResolver>>>,
}

impl Catalogs {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn install_items(&self, items: HashMap<u32, ItemData>) {
        if let Ok(mut w) = self.items.write() {
            *w = items;
        }
    }

    pub fn install_guildleves(&self, guildleves: HashMap<u32, GuildleveGamedata>) {
        if let Ok(mut w) = self.guildleves.write() {
            *w = guildleves;
        }
    }

    pub fn install_status_effects(&self, effects: HashMap<u32, StatusEffectDef>) {
        if let Ok(mut w) = self.status_effects.write() {
            *w = effects;
        }
    }

    pub fn install_battle_commands(&self, commands: HashMap<u16, BattleCommand>) {
        if let Ok(mut w) = self.battle_commands.write() {
            *w = commands;
        }
    }

    /// Install both the flat command map and the `(class, level)`
    /// side index at once. Used at boot from `load_global_battle_command_list`
    /// which builds both in a single DB pass.
    pub fn install_battle_commands_with_level_index(
        &self,
        commands: HashMap<u16, BattleCommand>,
        by_level: HashMap<(u8, i16), Vec<u16>>,
    ) {
        if let Ok(mut w) = self.battle_commands.write() {
            *w = commands;
        }
        if let Ok(mut w) = self.battle_commands_by_level.write() {
            *w = by_level;
        }
    }

    /// Command ids a `class_id` unlocks when they reach `level`.
    /// Returns an empty Vec when nothing unlocks at that threshold —
    /// most levels don't have a new ability. Caller iterates the
    /// result to emit "You learn X" lines.
    pub fn commands_unlocked_at(&self, class_id: u8, level: i16) -> Vec<u16> {
        let Ok(w) = self.battle_commands_by_level.read() else {
            return Vec::new();
        };
        w.get(&(class_id, level)).cloned().unwrap_or_default()
    }

    pub fn register_static_actor(&self, name: impl Into<String>, actor_id: u32) {
        if let Ok(mut w) = self.static_actors.write() {
            w.insert(name.into(), actor_id);
        }
    }

    pub fn install_quests(&self, quests: HashMap<u32, QuestMeta>) {
        if let Ok(mut w) = self.quests.write() {
            *w = quests;
        }
    }

    pub fn install_recipes(&self, resolver: RecipeResolver) {
        if let Ok(mut w) = self.recipes.write() {
            *w = Some(Arc::new(resolver));
        }
    }

    pub fn install_passive_guildleves(&self, data: HashMap<u32, PassiveGuildleveData>) {
        if let Ok(mut w) = self.passive_guildleves.write() {
            *w = data;
        }
    }

    pub fn install_gather_resolver(&self, resolver: GatherResolver) {
        if let Ok(mut w) = self.gather_nodes.write() {
            *w = Some(Arc::new(resolver));
        }
    }

    /// Cheap `Arc` clone of the installed gathering resolver, or
    /// `None` if `install_gather_resolver` hasn't run yet (fresh DB /
    /// startup race).
    pub fn gather_resolver(&self) -> Option<Arc<GatherResolver>> {
        self.gather_nodes.read().ok().and_then(|w| w.clone())
    }

    /// Return a cheap `Arc` clone of the installed resolver, or `None`
    /// if `install_recipes` hasn't run yet (fresh DB / startup race).
    pub fn recipe_resolver(&self) -> Option<Arc<RecipeResolver>> {
        self.recipes.read().ok().and_then(|w| w.clone())
    }

    /// Look up one passive-guildleve definition. Cheap clone (~200 B);
    /// callers that need to read many rows should hold the RwLock
    /// themselves.
    pub fn passive_guildleve(&self, leve_id: u32) -> Option<PassiveGuildleveData> {
        self.passive_guildleves
            .read()
            .ok()
            .and_then(|w| w.get(&leve_id).cloned())
    }

    /// Resolve a quest id to its lowercase script-name (`"man0l0"`) the
    /// Lua dispatcher uses as `scripts/lua/quests/<prefix>/<name>.lua`.
    /// Returns `None` if the quest id isn't in the catalog.
    pub fn quest_script_name(&self, quest_id: u32) -> Option<String> {
        self.quests
            .read()
            .ok()
            .and_then(|m| m.get(&quest_id).map(|q| q.class_name.to_lowercase()))
    }
}
