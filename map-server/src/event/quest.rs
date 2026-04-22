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

//! `Quest` — per-player quest runtime. Port of
//! `Actors/Quest/Quest.cs`.
//!
//! Each row carries a 32-bit flag bitfield, a current phase, and a
//! free-form JSON data blob. Mutations emit events on a shared
//! `EventOutbox` — the ticker drains these and the dispatcher turns
//! them into DB writes + Lua callbacks + game messages.

#![allow(dead_code)]

use super::outbox::{EventEvent, EventOutbox};

/// World-master text ids used by the C# quest flow.
pub const TEXT_NEXT_PHASE: u16 = 25116;
pub const TEXT_OBJECTIVES_COMPLETE: u16 = 25225;
pub const TEXT_ABANDON: u16 = 25236;

/// Thin wrapper around the 32-bit `quest_flags` bitfield. Matches
/// the C# `SetQuestFlag(bitIndex, value)` behaviour.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct QuestFlags(pub u32);

impl QuestFlags {
    pub const fn bits(self) -> u32 {
        self.0
    }

    pub fn get(self, bit: u8) -> bool {
        debug_assert!(bit < 32);
        (self.0 & (1u32 << bit)) != 0
    }

    pub fn set(&mut self, bit: u8, v: bool) {
        debug_assert!(bit < 32);
        if v {
            self.0 |= 1u32 << bit;
        } else {
            self.0 &= !(1u32 << bit);
        }
    }

    pub fn clear(&mut self) {
        self.0 = 0;
    }
}

impl From<u32> for QuestFlags {
    fn from(v: u32) -> Self {
        Self(v)
    }
}

impl From<QuestFlags> for u32 {
    fn from(f: QuestFlags) -> u32 {
        f.0
    }
}

// ---------------------------------------------------------------------------
// Quest runtime
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct Quest {
    /// Encodes `(quest_id | 0xA0F00000)` — matches the C# actor-id layout
    /// so `quest_id = actor_id & 0xFFFFF`.
    pub actor_id: u32,
    pub name: String,
    pub owner_actor_id: u32,
    pub current_phase: u32,
    pub quest_flags: QuestFlags,
    /// Free-form JSON data blob — matches the C# `questData` Dict
    /// serialised with Json.NET. Persisted verbatim.
    pub quest_data: String,
}

impl Quest {
    pub fn new(owner_actor_id: u32, actor_id: u32, name: impl Into<String>) -> Self {
        Self {
            actor_id,
            name: name.into(),
            owner_actor_id,
            current_phase: 0,
            quest_flags: QuestFlags::default(),
            quest_data: "{}".to_string(),
        }
    }

    /// `Quest(player, actorId, name, questDataJson, flags, phase)` —
    /// constructor for rows hydrated from the DB.
    pub fn from_db_row(
        owner_actor_id: u32,
        actor_id: u32,
        name: impl Into<String>,
        data_json: Option<String>,
        flags: u32,
        phase: u32,
    ) -> Self {
        Self {
            actor_id,
            name: name.into(),
            owner_actor_id,
            current_phase: phase,
            quest_flags: QuestFlags(flags),
            quest_data: data_json.unwrap_or_else(|| "{}".to_string()),
        }
    }

    /// `GetQuestId()` — last 20 bits of the actor id.
    pub fn quest_id(&self) -> u32 {
        self.actor_id & 0xF_FFFF
    }

    pub fn phase(&self) -> u32 {
        self.current_phase
    }

    pub fn flags(&self) -> u32 {
        self.quest_flags.bits()
    }

    pub fn flag(&self, bit: u8) -> bool {
        self.quest_flags.get(bit)
    }

    /// `SetQuestFlag(bitIndex, value)` — toggles the bit, enqueues a
    /// completion check, and a DB save.
    pub fn set_flag(&mut self, bit: u8, value: bool, outbox: &mut EventOutbox) {
        if bit >= 32 {
            tracing::warn!(quest = self.quest_id(), bit, "quest flag out of range");
            return;
        }
        self.quest_flags.set(bit, value);
        self.enqueue_save(outbox);
        outbox.push(EventEvent::QuestCheckCompletion {
            player_actor_id: self.owner_actor_id,
            quest_id: self.quest_id(),
            quest_name: self.name.clone(),
        });
    }

    /// `NextPhase(phase)` — bumps the phase, sends the advance game
    /// message, runs a completion check, persists.
    pub fn next_phase(&mut self, phase: u32, outbox: &mut EventOutbox) {
        self.current_phase = phase;
        outbox.push(EventEvent::QuestGameMessage {
            player_actor_id: self.owner_actor_id,
            text_id: TEXT_NEXT_PHASE,
            quest_id: self.quest_id(),
        });
        self.enqueue_save(outbox);
        outbox.push(EventEvent::QuestCheckCompletion {
            player_actor_id: self.owner_actor_id,
            quest_id: self.quest_id(),
            quest_name: self.name.clone(),
        });
    }

    /// `DoAbandon()` — fires the Lua hook + abandon game message.
    pub fn abandon(&self, outbox: &mut EventOutbox) {
        outbox.push(EventEvent::QuestAbandonHook {
            player_actor_id: self.owner_actor_id,
            quest_id: self.quest_id(),
            quest_name: self.name.clone(),
        });
        outbox.push(EventEvent::QuestGameMessage {
            player_actor_id: self.owner_actor_id,
            text_id: TEXT_ABANDON,
            quest_id: self.quest_id(),
        });
    }

    /// `ClearQuestData()`.
    pub fn clear_data(&mut self) {
        self.quest_data = "{}".to_string();
    }

    /// `ClearQuestFlags()`.
    pub fn clear_flags(&mut self) {
        self.quest_flags.clear();
    }

    /// Replace the JSON data blob wholesale. Callers that only want to
    /// set a single key should parse + rebuild externally (matches the
    /// C#, which uses Json.NET round-tripping).
    pub fn set_data_json(&mut self, json: impl Into<String>) {
        self.quest_data = json.into();
    }

    fn enqueue_save(&self, outbox: &mut EventOutbox) {
        outbox.push(EventEvent::QuestSaveToDb {
            player_actor_id: self.owner_actor_id,
            quest_id: self.quest_id(),
            phase: self.current_phase,
            flags: self.quest_flags.bits(),
            data: self.quest_data.clone(),
        });
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn new_quest() -> Quest {
        Quest::new(42, 0xA0F0_0100, "man0l0")
    }

    #[test]
    fn quest_id_masks_actor_id() {
        let q = new_quest();
        assert_eq!(q.quest_id(), 0x00F0_0100 & 0xF_FFFF);
    }

    #[test]
    fn set_flag_emits_save_and_check() {
        let mut q = new_quest();
        let mut ob = EventOutbox::new();
        q.set_flag(3, true, &mut ob);
        assert!(q.flag(3));
        assert!(
            ob.events
                .iter()
                .any(|e| matches!(e, EventEvent::QuestSaveToDb { .. }))
        );
        assert!(
            ob.events
                .iter()
                .any(|e| matches!(e, EventEvent::QuestCheckCompletion { .. }))
        );
    }

    #[test]
    fn set_flag_out_of_range_is_noop() {
        let mut q = new_quest();
        let mut ob = EventOutbox::new();
        q.set_flag(40, true, &mut ob);
        assert!(ob.is_empty());
        assert_eq!(q.flags(), 0);
    }

    #[test]
    fn next_phase_emits_advance_message() {
        let mut q = new_quest();
        let mut ob = EventOutbox::new();
        q.next_phase(3, &mut ob);
        assert_eq!(q.phase(), 3);
        let found = ob.events.iter().any(|e| {
            matches!(
                e,
                EventEvent::QuestGameMessage {
                    text_id: TEXT_NEXT_PHASE,
                    ..
                }
            )
        });
        assert!(found);
    }

    #[test]
    fn abandon_emits_hook_and_game_message() {
        let q = new_quest();
        let mut ob = EventOutbox::new();
        q.abandon(&mut ob);
        assert!(
            ob.events
                .iter()
                .any(|e| matches!(e, EventEvent::QuestAbandonHook { .. }))
        );
        assert!(ob.events.iter().any(|e| matches!(
            e,
            EventEvent::QuestGameMessage {
                text_id: TEXT_ABANDON,
                ..
            }
        )));
    }
}
