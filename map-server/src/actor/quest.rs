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

//! Quest DTOs, ported from `ioncannon/quest_system`'s
//! `Actors/Quest/{Quest,QuestData,QuestState}.cs`.
//!
//! The redesign drops the old `questData` JSON blob in favour of a
//! structured state shape:
//!
//!  * [`QuestData`] — 32-bit flag bitfield + three 16-bit counters, with
//!    a `Dirty` flag the Lua engine flips to `true` on mutation so
//!    `quest:UpdateENPCs()` / the outbox knows to resync the client and
//!    persist to the DB.
//!  * [`QuestState`] — per-sequence active-ENPC map the `onStateChange`
//!    hook populates; tracks `current` vs. `old` so the diff can be
//!    broadcast as `SetEventStatus` + `SetActorQuestGraphic` packets.
//!  * [`Quest`] — one journal slot. Owns its `QuestData`, its current
//!    `sequence`, and its `QuestState`. Mutations go through the data
//!    struct so the dirty bit is always flipped correctly.
//!
//! `QuestJournal` now keeps completed quests in a 2048-bit
//! [`Bitstream2048`] instead of a `HashSet<u32>`, matching the
//! `characters_quest_completed` VARBINARY(2048) column.

#![allow(dead_code)]

use std::collections::HashMap;

use common::bitstream::Bitstream2048;

/// The C# Quest actor lives in actor-id space with the 0xA0F0_xxxx prefix;
/// the low 20 bits are the actual quest id.
pub const QUEST_ACTOR_ID_PREFIX: u32 = 0xA0F0_0000;
pub const QUEST_ID_MASK: u32 = 0x000F_FFFF;

/// Compact bit index for a quest id inside the completion bitstream.
///
/// Meteor's quest ids live in `110_001..=112_048`; the redesign collapses
/// them to bit indices `0..=2047`. Ids outside that range clamp to the end
/// (or past it) so [`Bitstream2048::set`] no-ops — matching the C# code
/// which silently ignores out-of-range bits.
const QUEST_ID_BITSTREAM_BASE: u32 = 110_001;

/// The highest usable quest-bit index. Meteor's `Bitstream(2048)` supports
/// bits `0..=2047`.
pub const QUEST_BIT_MAX: usize = 2047;

pub fn quest_actor_id(quest_id: u32) -> u32 {
    QUEST_ACTOR_ID_PREFIX | (quest_id & QUEST_ID_MASK)
}

pub fn quest_id_from_actor(actor_id: u32) -> u32 {
    actor_id & QUEST_ID_MASK
}

/// Map a quest id (`110_001..=112_048`) to its position in the 2048-bit
/// completion bitfield. Ids below the base (or wildly above it) return
/// `None`; callers that pass them through [`Bitstream2048::set`] will
/// silently no-op, matching Meteor's behaviour.
pub fn quest_id_to_bit(quest_id: u32) -> Option<usize> {
    if quest_id < QUEST_ID_BITSTREAM_BASE {
        return None;
    }
    let bit = (quest_id - QUEST_ID_BITSTREAM_BASE) as usize;
    if bit > QUEST_BIT_MAX {
        return None;
    }
    Some(bit)
}

// ---------------------------------------------------------------------------
// QuestData — per-slot runtime flags + counters
// ---------------------------------------------------------------------------

/// One quest's runtime state. Mirrors Meteor's `QuestData.cs` (post-
/// redesign): a 32-bit flag bitfield the scripts address by bit index
/// `0..=31`, three 16-bit counters (the schema persists three; Meteor's
/// in-memory class has a vestigial fourth that never reaches the DB so
/// we drop it), and the `Dirty` flag the engine uses to decide whether
/// a `UpdateENPCs()` / DB write is needed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct QuestData {
    flags: u32,
    counters: [u16; 3],
    /// Per-quest NpcLs scratchpad, mirrored from C# `QuestData.npcLsFrom`.
    /// 0 = no chain active. Set by `Quest::NewNpcLsMsg`, read by
    /// `ReadNpcLsMsg` / `EndOfNpcLsMsgs` to know which NPC linkshell
    /// to flip back to ACTIVE / INACTIVE state. Persisted to DB column
    /// `npc_ls_from` (migration 050).
    npc_ls_from: u32,
    /// Per-quest message-step counter. Incremented by `ReadNpcLsMsg`
    /// between successive lines of the same NpcLs chain. Cleared on
    /// `EndOfNpcLsMsgs`. Persisted to DB column `npc_ls_msg_step`
    /// (migration 050).
    npc_ls_msg_step: u8,
    dirty: bool,
}

impl QuestData {
    /// Fresh state — all zero, not dirty.
    pub const fn new() -> Self {
        Self {
            flags: 0,
            counters: [0; 3],
            npc_ls_from: 0,
            npc_ls_msg_step: 0,
            dirty: false,
        }
    }

    /// Hydrate from DB columns (`flags`, `counter1`, `counter2`, `counter3`).
    /// Loading is not a mutation, so `Dirty` stays `false`.
    pub const fn from_parts(flags: u32, counter1: u16, counter2: u16, counter3: u16) -> Self {
        Self {
            flags,
            counters: [counter1, counter2, counter3],
            npc_ls_from: 0,
            npc_ls_msg_step: 0,
            dirty: false,
        }
    }

    /// Hydrate from the full DB row including the migration-050 NpcLs
    /// scratchpad columns.
    pub const fn from_parts_with_npc_ls(
        flags: u32,
        counter1: u16,
        counter2: u16,
        counter3: u16,
        npc_ls_from: u32,
        npc_ls_msg_step: u8,
    ) -> Self {
        Self {
            flags,
            counters: [counter1, counter2, counter3],
            npc_ls_from,
            npc_ls_msg_step,
            dirty: false,
        }
    }

    /// `GetNpcLsFrom()` — id of the NPC linkshell currently driving
    /// this quest's message chain (1..=40). 0 = no chain active.
    pub const fn npc_ls_from(self) -> u32 {
        self.npc_ls_from
    }

    /// `GetMsgStep()` — 0-based message-step counter for the active
    /// NpcLs chain.
    pub const fn npc_ls_msg_step(self) -> u8 {
        self.npc_ls_msg_step
    }

    /// `SetNpcLsFrom(from)` — flag a new NPC linkshell as driving
    /// this quest's chain.
    pub fn set_npc_ls_from(&mut self, from: u32) {
        self.npc_ls_from = from;
        self.dirty = true;
    }

    /// `IncrementNpcLsMsgStep()` — bump the per-chain message-step
    /// counter. Saturates at u8::MAX (255 lines is way past any real
    /// chain length).
    pub fn inc_npc_ls_msg_step(&mut self) -> u8 {
        self.npc_ls_msg_step = self.npc_ls_msg_step.saturating_add(1);
        self.dirty = true;
        self.npc_ls_msg_step
    }

    /// `ClearNpcLs()` — zero both scratchpad fields after a chain ends.
    pub fn clear_npc_ls(&mut self) {
        self.npc_ls_from = 0;
        self.npc_ls_msg_step = 0;
        self.dirty = true;
    }

    pub const fn flags(self) -> u32 {
        self.flags
    }

    pub const fn counter(self, idx: usize) -> u16 {
        if idx < 3 { self.counters[idx] } else { 0 }
    }

    pub const fn counter1(self) -> u16 {
        self.counters[0]
    }

    pub const fn counter2(self) -> u16 {
        self.counters[1]
    }

    pub const fn counter3(self) -> u16 {
        self.counters[2]
    }

    pub const fn is_dirty(self) -> bool {
        self.dirty
    }

    pub fn clear_dirty(&mut self) {
        self.dirty = false;
    }

    /// `GetFlag(index)` — out-of-range indices return `false`, matching C#.
    pub fn get_flag(self, index: u8) -> bool {
        if index >= 32 {
            return false;
        }
        (self.flags & (1u32 << index)) != 0
    }

    /// `SetFlag(index)` — bits `32..` are ignored (C# also no-ops).
    pub fn set_flag(&mut self, index: u8) {
        if index >= 32 {
            return;
        }
        self.flags |= 1u32 << index;
        self.dirty = true;
    }

    /// `ClearFlag(index)` — bits `32..` are ignored.
    pub fn clear_flag(&mut self, index: u8) {
        if index >= 32 {
            return;
        }
        self.flags &= !(1u32 << index);
        self.dirty = true;
    }

    /// `IncCounter(num)` → new value. Matches Meteor's wrap-on-overflow
    /// behaviour (ushort++ in C# wraps at 65_536 without panicking).
    pub fn inc_counter(&mut self, idx: usize) -> u16 {
        if idx >= 3 {
            return 0;
        }
        self.counters[idx] = self.counters[idx].wrapping_add(1);
        self.dirty = true;
        self.counters[idx]
    }

    /// `DecCounter(num)` → new value. Wraps at 0 like the C#.
    pub fn dec_counter(&mut self, idx: usize) -> u16 {
        if idx >= 3 {
            return 0;
        }
        self.counters[idx] = self.counters[idx].wrapping_sub(1);
        self.dirty = true;
        self.counters[idx]
    }

    /// `SetCounter(num, value)`.
    pub fn set_counter(&mut self, idx: usize, value: u16) {
        if idx >= 3 {
            return;
        }
        self.counters[idx] = value;
        self.dirty = true;
    }

    /// `ClearData()` — zero every counter + flag, and flip dirty so the
    /// next UpdateENPCs rebroadcasts.
    pub fn clear(&mut self) {
        self.flags = 0;
        self.counters = [0; 3];
        self.npc_ls_from = 0;
        self.npc_ls_msg_step = 0;
        self.dirty = true;
    }
}

// ---------------------------------------------------------------------------
// QuestState — active ENPCs per sequence
// ---------------------------------------------------------------------------

/// Meteor's `QuestState.QuestFlag` enum. Controls the coloured marker on
/// the quest-active NPC (`SetActorQuestGraphicPacket`).
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum QuestFlagGraphic {
    #[default]
    None = 0,
    Map = 1,
    Plate = 2,
}

impl QuestFlagGraphic {
    pub const fn as_u8(self) -> u8 {
        self as u8
    }

    /// Inverse of `as_u8`, for Lua values coming across the FFI boundary.
    pub const fn from_u8(v: u8) -> Self {
        match v {
            1 => Self::Map,
            2 => Self::Plate,
            _ => Self::None,
        }
    }
}

/// One entry in `QuestState::current` / `::old`. Mirrors the `QuestENpc`
/// nested class.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QuestEnpc {
    pub actor_class_id: u32,
    pub quest_flag_type: u8,
    pub is_spawned: bool,
    pub is_talk_enabled: bool,
    pub is_emote_enabled: bool,
    pub is_push_enabled: bool,
}

impl QuestEnpc {
    pub const fn new(
        actor_class_id: u32,
        quest_flag_type: u8,
        is_spawned: bool,
        is_talk_enabled: bool,
        is_emote_enabled: bool,
        is_push_enabled: bool,
    ) -> Self {
        Self {
            actor_class_id,
            quest_flag_type,
            is_spawned,
            is_talk_enabled,
            is_emote_enabled,
            is_push_enabled,
        }
    }

    /// `IsChanged(...)` — `true` iff any of the broadcastable fields
    /// differ from `other`. Used by `QuestState::add_enpc` to decide
    /// whether to queue a resync packet.
    pub const fn is_changed(&self, other: &Self) -> bool {
        self.quest_flag_type != other.quest_flag_type
            || self.is_spawned != other.is_spawned
            || self.is_talk_enabled != other.is_talk_enabled
            || self.is_emote_enabled != other.is_emote_enabled
            || self.is_push_enabled != other.is_push_enabled
    }
}

/// Tracks which ENPCs the current quest sequence has "activated". The
/// Lua `onStateChange` hook calls `quest:SetENpc(...)` repeatedly to
/// populate [`current`]; the engine diffs against [`old`] (which holds
/// the previous sequence's ENPCs) to emit the right `SetEventStatus` +
/// `SetActorQuestGraphic` packets.
///
/// [`current`]: QuestState::current
/// [`old`]: QuestState::old
#[derive(Debug, Clone, Default)]
pub struct QuestState {
    /// ENPCs the current sequence wants active. Keyed by actorClassId.
    pub current: HashMap<u32, QuestEnpc>,
    /// ENPCs the previous sequence had active. Populated by
    /// [`begin_sequence_swap`]; drained by the engine after the hook
    /// finishes so stale entries get a "clear" broadcast.
    pub old: HashMap<u32, QuestEnpc>,
    /// `actorClassId`s that have already fired their `onPush` hook this
    /// sequence — proximity-push detection runs on every position update,
    /// so without this dedupe we'd re-fire the hook every ~350ms while
    /// the player was inside the trigger radius. Cleared on every
    /// `begin_sequence_swap` (so a new sequence can re-trigger pushes
    /// for the same NPCs) and never persisted to disk (in-memory only;
    /// re-login resets it).
    pub recently_pushed: std::collections::HashSet<u32>,
}

impl QuestState {
    pub fn new() -> Self {
        Self::default()
    }

    /// `AddENpc` — register/replace an ENPC in the current sequence.
    ///
    /// Returns `AddEnpcOutcome::Unchanged` when the previous sequence
    /// already had this ENPC with identical state (no packet needed),
    /// `::Updated(snapshot)` when the state differs (caller emits a
    /// resync packet), or `::New(snapshot)` when the ENPC is entering
    /// fresh.
    pub fn add_enpc(&mut self, enpc: QuestEnpc) -> AddEnpcOutcome {
        let class_id = enpc.actor_class_id;

        // Recycle from `old` if the previous sequence also had this NPC.
        if let Some(mut existing) = self.old.remove(&class_id) {
            let changed = existing.is_changed(&enpc);
            existing = enpc;
            self.current.insert(class_id, existing);
            if changed {
                AddEnpcOutcome::Updated(existing)
            } else {
                AddEnpcOutcome::Unchanged
            }
        } else {
            self.current.insert(class_id, enpc);
            AddEnpcOutcome::New(enpc)
        }
    }

    pub fn get_enpc(&self, class_id: u32) -> Option<&QuestEnpc> {
        self.current.get(&class_id)
    }

    pub fn has_enpc(&self, class_id: u32) -> bool {
        self.current.contains_key(&class_id)
    }

    /// Prepare for a new sequence: move `current` → `old`, leaving
    /// `current` empty for the next `onStateChange` call to repopulate.
    /// The caller must drain `old` after the hook finishes to emit
    /// clear-broadcasts for ENPCs the new sequence didn't re-activate.
    pub fn begin_sequence_swap(&mut self) {
        self.old = std::mem::take(&mut self.current);
        // A fresh sequence may legitimately want to re-fire push hooks
        // (e.g. SEQ_010 re-uses ROSTNSTHAL with a new push trigger).
        self.recently_pushed.clear();
    }

    /// Iterate and drain every ENPC left in `old` after the hook ran —
    /// these are the NPCs the new sequence dropped.
    pub fn drain_stale_enpcs(&mut self) -> impl Iterator<Item = QuestEnpc> + '_ {
        self.old.drain().map(|(_, enpc)| enpc)
    }

    /// Drop every active ENPC. Used by `DeleteState()` on quest complete.
    pub fn clear(&mut self) {
        self.current.clear();
        self.old.clear();
    }
}

/// Outcome of [`QuestState::add_enpc`] — tells the engine what packet
/// (if any) to emit for this registration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AddEnpcOutcome {
    /// Same ENPC state as the previous sequence; nothing to broadcast.
    Unchanged,
    /// ENPC already existed but state changed; emit a resync packet.
    Updated(QuestEnpc),
    /// Fresh entry; emit a full activation packet.
    New(QuestEnpc),
}

// ---------------------------------------------------------------------------
// Quest — one journal slot
// ---------------------------------------------------------------------------

/// Per-slot quest runtime. Owns its [`QuestData`] (flags + counters +
/// dirty bit) and its [`QuestState`] (active ENPC map).
#[derive(Debug, Clone)]
pub struct Quest {
    pub actor_id: u32,
    pub name: String,
    pub sequence: u32,
    pub data: QuestData,
    pub state: QuestState,
}

impl Quest {
    pub fn new(actor_id: u32, name: impl Into<String>) -> Self {
        Self {
            actor_id,
            name: name.into(),
            sequence: 0,
            data: QuestData::new(),
            state: QuestState::new(),
        }
    }

    /// Hydrate from a DB row (matches `characters_quest_scenario`'s new
    /// column layout). `QuestState` starts empty — it gets populated on
    /// first `onStateChange` call after login.
    pub fn from_db_row(
        actor_id: u32,
        name: impl Into<String>,
        sequence: u32,
        flags: u32,
        counter1: u16,
        counter2: u16,
        counter3: u16,
    ) -> Self {
        Self::from_db_row_with_npc_ls(
            actor_id, name, sequence, flags, counter1, counter2, counter3, 0, 0,
        )
    }

    /// Migration-050 hydrate: includes the per-quest NpcLs scratchpad
    /// (`npc_ls_from`, `npc_ls_msg_step`).
    #[allow(clippy::too_many_arguments)]
    pub fn from_db_row_with_npc_ls(
        actor_id: u32,
        name: impl Into<String>,
        sequence: u32,
        flags: u32,
        counter1: u16,
        counter2: u16,
        counter3: u16,
        npc_ls_from: u32,
        npc_ls_msg_step: u8,
    ) -> Self {
        Self {
            actor_id,
            name: name.into(),
            sequence,
            data: QuestData::from_parts_with_npc_ls(
                flags,
                counter1,
                counter2,
                counter3,
                npc_ls_from,
                npc_ls_msg_step,
            ),
            state: QuestState::new(),
        }
    }

    pub fn quest_id(&self) -> u32 {
        quest_id_from_actor(self.actor_id)
    }

    pub fn get_sequence(&self) -> u32 {
        self.sequence
    }

    /// `StartSequence(sequence)` — bumps the sequence number and flags
    /// the quest dirty. Actual ENPC resync runs out of the event
    /// dispatcher after this returns (Phase C wiring).
    pub fn start_sequence(&mut self, sequence: u32) {
        self.sequence = sequence;
        self.data.dirty = true;
    }

    // Convenience pass-throughs to the inner QuestData. Kept thin so
    // callers can go through either the raw data accessor or these
    // methods interchangeably.

    pub fn set_flag(&mut self, index: u8) {
        self.data.set_flag(index);
    }

    pub fn clear_flag(&mut self, index: u8) {
        self.data.clear_flag(index);
    }

    pub fn get_flag(&self, index: u8) -> bool {
        self.data.get_flag(index)
    }

    pub fn get_flags(&self) -> u32 {
        self.data.flags()
    }

    pub fn get_counter(&self, idx: usize) -> u16 {
        self.data.counter(idx)
    }

    pub fn set_counter(&mut self, idx: usize, value: u16) {
        self.data.set_counter(idx, value);
    }

    pub fn inc_counter(&mut self, idx: usize) -> u16 {
        self.data.inc_counter(idx)
    }

    pub fn dec_counter(&mut self, idx: usize) -> u16 {
        self.data.dec_counter(idx)
    }

    /// Per-quest NpcLs scratchpad accessors — proxy through QuestData.
    /// See `QuestData::set_npc_ls_from` etc. for semantics.
    pub fn get_npc_ls_from(&self) -> u32 {
        self.data.npc_ls_from()
    }

    pub fn get_npc_ls_msg_step(&self) -> u8 {
        self.data.npc_ls_msg_step()
    }

    pub fn set_npc_ls_from(&mut self, from: u32) {
        self.data.set_npc_ls_from(from);
    }

    pub fn inc_npc_ls_msg_step(&mut self) -> u8 {
        self.data.inc_npc_ls_msg_step()
    }

    pub fn clear_npc_ls(&mut self) {
        self.data.clear_npc_ls();
    }

    pub fn clear_data(&mut self) {
        self.data.clear();
    }

    pub fn clear_flags(&mut self) {
        self.data.flags = 0;
        self.data.dirty = true;
    }

    pub fn is_dirty(&self) -> bool {
        self.data.is_dirty()
    }

    pub fn clear_dirty(&mut self) {
        self.data.clear_dirty();
    }
}

// ---------------------------------------------------------------------------
// QuestJournal — 16 active slots + 2048-bit completion bitfield
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default)]
pub struct QuestJournal {
    pub slots: [Option<Quest>; 16],
    /// Replaces the previous `HashSet<u32>` — matches the
    /// `characters_quest_completed.completedQuests` VARBINARY(2048) column.
    pub completed: Bitstream2048,
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

    /// `CompleteQuest(id)` — remove from active slots (if present) and
    /// set the completion bit.
    pub fn complete(&mut self, quest_id: u32) {
        self.remove(quest_id);
        if let Some(bit) = quest_id_to_bit(quest_id) {
            self.completed.set(bit);
        }
    }

    pub fn set_completed(&mut self, quest_id: u32, done: bool) {
        if let Some(bit) = quest_id_to_bit(quest_id) {
            if done {
                self.completed.set(bit);
            } else {
                self.completed.clear(bit);
            }
        }
    }

    pub fn is_completed(&self, quest_id: u32) -> bool {
        quest_id_to_bit(quest_id)
            .map(|bit| self.completed.get(bit))
            .unwrap_or(false)
    }

    pub fn can_accept(&self, quest_id: u32) -> bool {
        !self.has(quest_id) && !self.is_completed(quest_id) && self.get_free_slot().is_some()
    }

    /// Iterate every completed quest id (expanded from the bitfield).
    pub fn iter_completed(&self) -> impl Iterator<Item = u32> + '_ {
        self.completed
            .iter_set()
            .map(|bit| QUEST_ID_BITSTREAM_BASE + bit as u32)
    }
}

// ---------------------------------------------------------------------------
// GuildleveJournal — unchanged by the quest-engine redesign
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

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
    fn quest_id_bit_mapping() {
        assert_eq!(quest_id_to_bit(110_001), Some(0));
        assert_eq!(quest_id_to_bit(110_002), Some(1));
        assert_eq!(quest_id_to_bit(112_048), Some(2047));
        // Out of range.
        assert_eq!(quest_id_to_bit(110_000), None);
        assert_eq!(quest_id_to_bit(112_049), None);
    }

    #[test]
    fn questdata_flag_set_clear_flips_dirty() {
        let mut d = QuestData::new();
        assert!(!d.is_dirty());
        d.set_flag(3);
        assert!(d.get_flag(3));
        assert!(d.is_dirty());

        d.clear_dirty();
        d.clear_flag(3);
        assert!(!d.get_flag(3));
        assert!(d.is_dirty());
    }

    #[test]
    fn questdata_out_of_range_flag_is_noop() {
        let mut d = QuestData::new();
        d.set_flag(40);
        assert_eq!(d.flags(), 0);
        assert!(!d.is_dirty());
    }

    #[test]
    fn questdata_counter_operations() {
        let mut d = QuestData::new();
        assert_eq!(d.inc_counter(0), 1);
        assert_eq!(d.inc_counter(0), 2);
        assert_eq!(d.inc_counter(2), 1);
        d.set_counter(1, 99);
        assert_eq!(d.counter(0), 2);
        assert_eq!(d.counter(1), 99);
        assert_eq!(d.counter(2), 1);
        assert!(d.is_dirty());
    }

    #[test]
    fn questdata_counter_wraps_on_overflow() {
        let mut d = QuestData::from_parts(0, 0xFFFF, 0, 0);
        assert_eq!(d.inc_counter(0), 0);
        assert_eq!(d.dec_counter(1), 0xFFFF);
    }

    #[test]
    fn questdata_counter_out_of_range_is_noop() {
        let mut d = QuestData::new();
        assert_eq!(d.inc_counter(3), 0);
        d.set_counter(5, 42);
        assert_eq!(d.counter(0), 0);
        assert!(!d.is_dirty());
    }

    #[test]
    fn queststate_add_enpc_reports_outcome() {
        let mut s = QuestState::new();
        let a = QuestEnpc::new(1001, 0, true, true, false, false);
        assert_eq!(s.add_enpc(a), AddEnpcOutcome::New(a));
        assert!(s.has_enpc(1001));

        // Bridge to a new sequence: current → old.
        s.begin_sequence_swap();
        assert!(!s.has_enpc(1001));

        // Re-register with the same state: unchanged.
        assert_eq!(s.add_enpc(a), AddEnpcOutcome::Unchanged);

        // Move to another sequence again, re-register with a new flag.
        s.begin_sequence_swap();
        let b = QuestEnpc::new(1001, 1, true, true, false, false);
        assert_eq!(s.add_enpc(b), AddEnpcOutcome::Updated(b));
    }

    #[test]
    fn queststate_stale_enpcs_drainable_after_swap() {
        let mut s = QuestState::new();
        s.add_enpc(QuestEnpc::new(1001, 0, true, true, false, false));
        s.add_enpc(QuestEnpc::new(1002, 0, true, true, false, false));

        s.begin_sequence_swap();
        // New sequence only keeps 1001; 1002 is stale.
        s.add_enpc(QuestEnpc::new(1001, 0, true, true, false, false));

        let stale: Vec<u32> = s.drain_stale_enpcs().map(|e| e.actor_class_id).collect();
        assert_eq!(stale, vec![1002]);
    }

    #[test]
    fn quest_start_sequence_flips_dirty() {
        let mut q = Quest::new(quest_actor_id(110_001), "man0l0");
        assert!(!q.is_dirty());
        q.start_sequence(10);
        assert_eq!(q.get_sequence(), 10);
        assert!(q.is_dirty());
    }

    #[test]
    fn journal_add_remove_complete_via_bitstream() {
        let mut j = QuestJournal::new();
        let quest = Quest::new(quest_actor_id(110_001), "man0l0");
        let slot = j.add(quest).expect("slot");
        assert_eq!(slot, 0);
        assert!(j.has(110_001));

        j.complete(110_001);
        assert!(!j.has(110_001));
        assert!(j.is_completed(110_001));
        assert!(!j.can_accept(110_001));

        // Bitstream should have exactly one bit set, at position 0.
        assert_eq!(j.completed.count_ones(), 1);
        assert!(j.completed.get(0));

        let ids: Vec<u32> = j.iter_completed().collect();
        assert_eq!(ids, vec![110_001]);
    }

    #[test]
    fn journal_set_completed_idempotent() {
        let mut j = QuestJournal::new();
        j.set_completed(110_050, true);
        j.set_completed(110_050, true);
        assert_eq!(j.completed.count_ones(), 1);
        j.set_completed(110_050, false);
        assert_eq!(j.completed.count_ones(), 0);
    }

    #[test]
    fn journal_completed_ignores_out_of_range_ids() {
        let mut j = QuestJournal::new();
        j.set_completed(50_000, true);
        assert!(!j.is_completed(50_000));
        assert_eq!(j.completed.count_ones(), 0);
    }
}
