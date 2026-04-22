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
use std::sync::RwLock;

use crate::data::ItemData;
use crate::gamedata::{BattleCommand, GuildleveGamedata, QuestMeta, StatusEffectDef};

#[derive(Default)]
pub struct Catalogs {
    pub items: RwLock<HashMap<u32, ItemData>>,
    pub guildleves: RwLock<HashMap<u32, GuildleveGamedata>>,
    pub status_effects: RwLock<HashMap<u32, StatusEffectDef>>,
    pub battle_commands: RwLock<HashMap<u16, BattleCommand>>,
    /// Maps static-actor name (e.g. `"DftFst"`) to its fixed actor id.
    pub static_actors: RwLock<HashMap<String, u32>>,
    /// Quest id → metadata (className drives Lua script-path resolution).
    /// Loaded from `gamedata_quests` at startup. ~524 rows from Meteor's
    /// `origin/ioncannon/quest_system` seed.
    pub quests: RwLock<HashMap<u32, QuestMeta>>,
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
