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

//! Minimal Quest and guildleve DTOs, ported from `Actors/Quest/Quest.cs`.
//!
//! The full C# Quest is a Lua-scriptable object with phase-state + flag
//! bitfield + arbitrary JSON payload. We model the data shape so Player
//! helpers like `has_quest` / `get_quest_slot` / `complete_quest_slot`
//! can work without the Lua scripting machinery attached.

#![allow(dead_code)]

use std::collections::HashSet;

/// The C# Quest actor lives in actor-id space with the 0xA0F0_xxxx prefix;
/// the low 20 bits are the actual quest id. Helpers below mask appropriately.
pub const QUEST_ACTOR_ID_PREFIX: u32 = 0xA0F0_0000;
pub const QUEST_ID_MASK: u32 = 0x000F_FFFF;

pub fn quest_actor_id(quest_id: u32) -> u32 {
    QUEST_ACTOR_ID_PREFIX | (quest_id & QUEST_ID_MASK)
}

pub fn quest_id_from_actor(actor_id: u32) -> u32 {
    actor_id & QUEST_ID_MASK
}

/// Per-slot active quest record.
#[derive(Debug, Clone)]
pub struct Quest {
    pub actor_id: u32,
    pub name: String,
    pub phase: u32,
    pub flags: u32,
    /// JSON blob (the C# server hands out arbitrary data keyed by field name).
    pub data: String,
}

impl Quest {
    pub fn new(actor_id: u32, name: impl Into<String>) -> Self {
        Self {
            actor_id,
            name: name.into(),
            phase: 0,
            flags: 0,
            data: "{}".to_string(),
        }
    }

    pub fn quest_id(&self) -> u32 {
        quest_id_from_actor(self.actor_id)
    }

    pub fn get_phase(&self) -> u32 {
        self.phase
    }

    pub fn set_phase(&mut self, phase: u32) {
        self.phase = phase;
    }

    pub fn get_quest_flags(&self) -> u32 {
        self.flags
    }

    pub fn set_flag(&mut self, bit: u32) {
        self.flags |= 1u32 << bit;
    }

    pub fn clear_flag(&mut self, bit: u32) {
        self.flags &= !(1u32 << bit);
    }

    pub fn has_flag(&self, bit: u32) -> bool {
        self.flags & (1u32 << bit) != 0
    }

    pub fn clear_data(&mut self) {
        self.data = "{}".to_string();
    }

    pub fn clear_flags(&mut self) {
        self.flags = 0;
    }
}

/// Slot-tracked active quest book (scenario quests, 16 slots).
#[derive(Debug, Clone, Default)]
pub struct QuestJournal {
    pub slots: [Option<Quest>; 16],
    pub completed: HashSet<u32>,
}

impl QuestJournal {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn get_free_slot(&self) -> Option<usize> {
        self.slots.iter().position(|s| s.is_none())
    }

    pub fn has(&self, quest_id: u32) -> bool {
        self.slots
            .iter()
            .any(|slot| matches!(slot, Some(q) if q.quest_id() == quest_id))
    }

    pub fn has_by_name(&self, name: &str) -> bool {
        self.slots
            .iter()
            .any(|slot| matches!(slot, Some(q) if q.name == name))
    }

    pub fn slot_of(&self, quest_id: u32) -> Option<usize> {
        self.slots
            .iter()
            .position(|slot| matches!(slot, Some(q) if q.quest_id() == quest_id))
    }

    pub fn get(&self, quest_id: u32) -> Option<&Quest> {
        self.slots
            .iter()
            .flatten()
            .find(|q| q.quest_id() == quest_id)
    }

    pub fn get_mut(&mut self, quest_id: u32) -> Option<&mut Quest> {
        self.slots
            .iter_mut()
            .flatten()
            .find(|q| q.quest_id() == quest_id)
    }

    pub fn add(&mut self, quest: Quest) -> Option<usize> {
        let slot = self.get_free_slot()?;
        self.slots[slot] = Some(quest);
        Some(slot)
    }

    pub fn remove(&mut self, quest_id: u32) -> Option<Quest> {
        let slot = self.slot_of(quest_id)?;
        self.slots[slot].take()
    }

    pub fn complete(&mut self, quest_id: u32) {
        self.remove(quest_id);
        self.completed.insert(quest_id);
    }

    pub fn is_completed(&self, quest_id: u32) -> bool {
        self.completed.contains(&quest_id)
    }

    pub fn can_accept(&self, quest_id: u32) -> bool {
        !self.has(quest_id) && !self.is_completed(quest_id) && self.get_free_slot().is_some()
    }
}

/// 8-slot guildleve journal, one u16 id per slot (0 = empty).
#[derive(Debug, Clone, Default)]
pub struct GuildleveJournal {
    pub ids: [u16; 8],
}

impl GuildleveJournal {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn get_free_slot(&self) -> Option<usize> {
        self.ids.iter().position(|&id| id == 0)
    }

    pub fn has(&self, id: u32) -> bool {
        let id = id as u16;
        self.ids.contains(&id)
    }

    pub fn add(&mut self, id: u16) -> Option<usize> {
        let slot = self.get_free_slot()?;
        self.ids[slot] = id;
        Some(slot)
    }

    pub fn remove(&mut self, id: u32) -> bool {
        let id = id as u16;
        if let Some(slot) = self.ids.iter().position(|&s| s == id) {
            self.ids[slot] = 0;
            true
        } else {
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quest_actor_id_roundtrip() {
        let id = 110_001u32;
        let aid = quest_actor_id(id);
        assert_eq!(quest_id_from_actor(aid), id);
    }

    #[test]
    fn journal_add_remove_complete() {
        let mut j = QuestJournal::new();
        let quest = Quest::new(quest_actor_id(110_001), "man0l0");
        let slot = j.add(quest).expect("slot");
        assert_eq!(slot, 0);
        assert!(j.has(110_001));
        j.complete(110_001);
        assert!(!j.has(110_001));
        assert!(j.is_completed(110_001));
        assert!(!j.can_accept(110_001));
    }

    #[test]
    fn flags_are_bit_indexed() {
        let mut q = Quest::new(0, "test");
        q.set_flag(3);
        q.set_flag(10);
        assert!(q.has_flag(3));
        assert!(q.has_flag(10));
        q.clear_flag(3);
        assert!(!q.has_flag(3));
    }
}
