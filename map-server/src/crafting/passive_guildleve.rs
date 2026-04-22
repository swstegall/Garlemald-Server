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

//! Per-player passive-guildleve (local crafting leve) runtime state.
//!
//! The ioncannon branch modelled this as a `PassiveGuildleve : Quest`
//! subclass carrying four extra fields (`currentDifficulty`,
//! `currentAttempt`, `currentCrafted`, `hasMaterials`) serialised to
//! the old JSON blob. Garlemald's post-redesign quest schema stores
//! three u16 counters + a 32-bit flag bitfield per slot, which is
//! enough to fit the whole PassiveGuildleve view without any schema
//! additions:
//!
//!   * `counter1` → `currentAttempt`   (u16)
//!   * `counter2` → `currentCrafted`   (u16)
//!   * `counter3` → `currentDifficulty` (u16 — band index 0..=3)
//!   * `flag bit 0` → `hasMaterials` (set once the player picks up the
//!     prop materials from the issuing NPC)
//!
//! Putting the accessors here — rather than on [`Quest`] itself —
//! keeps the leve/crafting concern localised to `src/crafting/` and
//! avoids bloating the core quest type with subsystem-specific view
//! methods. The runtime treats any quest whose id falls in the
//! 120_001..=120_452 range as a PassiveGuildleve by definition, so
//! there is no discriminator stored in the DB.

#![allow(dead_code)]

use crate::actor::quest::Quest;
use crate::crafting::PassiveGuildleveData;

/// Inclusive range of quest ids reserved for passive (local crafting)
/// leves. Meteor's `CraftCommand.lua::isLocalLeve(id)` uses exactly
/// this range; `gamedata_passivegl_craft` populates 120_001..=120_452.
pub const LOCAL_LEVE_ID_MIN: u32 = 120_001;
pub const LOCAL_LEVE_ID_MAX: u32 = 120_452;

/// `true` iff the given quest id addresses a passive-guildleve row.
/// Does **not** check whether the id is populated in the catalog —
/// callers that need that should also consult
/// [`Catalogs::passive_guildleve`].
pub fn is_local_leve_quest_id(quest_id: u32) -> bool {
    (LOCAL_LEVE_ID_MIN..=LOCAL_LEVE_ID_MAX).contains(&quest_id)
}

/// Flag bit reserved on the quest's 32-bit flag bitfield for
/// `hasMaterials`. Using a `const` so tests and Lua bindings share one
/// canonical position.
pub const HAS_MATERIALS_FLAG_BIT: u8 = 0;

/// View that pairs a mutable [`Quest`] with its static
/// [`PassiveGuildleveData`] definition. Every accessor mirrors one
/// Meteor `PassiveGuildleve` method — field access goes through the
/// quest's counter/flag API so the existing dirty-bit tracking + DB
/// persistence layer picks up the changes without extra work.
pub struct PassiveGuildleveView<'a> {
    pub quest: &'a mut Quest,
    pub data: &'a PassiveGuildleveData,
}

impl<'a> PassiveGuildleveView<'a> {
    pub fn new(quest: &'a mut Quest, data: &'a PassiveGuildleveData) -> Self {
        Self { quest, data }
    }

    // ------------------------------------------------------------------
    // Runtime-mutable state (lives in quest counters/flags)
    // ------------------------------------------------------------------

    /// `PassiveGuildleve.GetCurrentDifficulty()` — band index 0..=3.
    /// Values ≥ 4 saturate to 3 on read, matching the C# byte cast.
    pub fn current_difficulty(&self) -> u8 {
        let raw = self.quest.get_counter(2);
        raw.min(3) as u8
    }

    /// Set `currentDifficulty`. Caller is responsible for clamping —
    /// [`PassiveGuildleveData::clamp_difficulty`] is the canonical
    /// helper for values coming from Lua.
    pub fn set_current_difficulty(&mut self, difficulty: u8) {
        self.quest.set_counter(2, difficulty as u16);
    }

    /// `PassiveGuildleve.getCurrentCrafted()` — items produced so far.
    pub fn current_crafted(&self) -> u16 {
        self.quest.get_counter(1)
    }

    /// `PassiveGuildleve.GetCurrentAttempt()` — attempts consumed so
    /// far. `numberOfAttempts - currentAttempt` is the remaining
    /// material allowance, which [`Self::remaining_materials`]
    /// computes.
    pub fn current_attempt(&self) -> u16 {
        self.quest.get_counter(0)
    }

    /// `PassiveGuildleve.HasMaterials()` — whether the player picked
    /// up the props from the issuing NPC.
    pub fn has_materials(&self) -> bool {
        self.quest.get_flag(HAS_MATERIALS_FLAG_BIT)
    }

    pub fn set_has_materials(&mut self, v: bool) {
        if v {
            self.quest.set_flag(HAS_MATERIALS_FLAG_BIT);
        } else {
            self.quest.clear_flag(HAS_MATERIALS_FLAG_BIT);
        }
    }

    // ------------------------------------------------------------------
    // Derived values (from static data + runtime counters)
    // ------------------------------------------------------------------

    /// `PassiveGuildleve.GetObjectiveQuantity()` — number of items the
    /// player must produce to finish this leve's active difficulty.
    pub fn objective_quantity(&self) -> i32 {
        self.data.objective_quantity[self.current_difficulty() as usize]
    }

    /// `PassiveGuildleve.GetMaxAttempts()` — material-set size for
    /// the active difficulty band.
    pub fn max_attempts(&self) -> i32 {
        self.data.number_of_attempts[self.current_difficulty() as usize]
    }

    /// `PassiveGuildleve.GetRemainingMaterials()` = `maxAttempts -
    /// currentAttempt`. Clamped to `0` rather than going negative so
    /// the Lua UI never sees a nonsensical count when a mistuned
    /// synth overshoots.
    pub fn remaining_materials(&self) -> i32 {
        (self.max_attempts() - self.current_attempt() as i32).max(0)
    }

    /// Item id of the crafting objective (fed into
    /// `RecipeResolver::by_item_id`).
    pub fn objective_item_id(&self) -> i32 {
        self.data.objective_item_id[self.current_difficulty() as usize]
    }

    /// `PassiveGuildleve.isCraftPassiveGuildleve()` — a quest is a
    /// PassiveGuildleve iff its id is in the reserved range. Kept
    /// here because Lua calls it on the quest userdata.
    pub fn is_craft_passive_guildleve(&self) -> bool {
        is_local_leve_quest_id(self.quest.quest_id())
    }

    // ------------------------------------------------------------------
    // Transitions
    // ------------------------------------------------------------------

    /// `PassiveGuildleve.CraftSuccess()` — one attempt consumed, N
    /// items added to the crafted count.
    pub fn craft_success(&mut self, result_quantity: u16) {
        let new_crafted = self.current_crafted().saturating_add(result_quantity);
        self.quest.set_counter(1, new_crafted);
        self.quest.set_counter(0, self.current_attempt().saturating_add(1));
    }

    /// `PassiveGuildleve.CraftFail()` — one attempt consumed, nothing
    /// produced.
    pub fn craft_fail(&mut self) {
        self.quest.set_counter(0, self.current_attempt().saturating_add(1));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::actor::quest::Quest;

    fn test_data(band0_item: i32, band0_qty: i32, band0_attempts: i32) -> PassiveGuildleveData {
        PassiveGuildleveData {
            id: 120_001,
            plate_id: 0,
            border_id: 0,
            recommended_class: 0,
            issuing_location: 0,
            leve_location: 0,
            delivery_display_name: 0,
            objective_item_id: [band0_item, 0, 0, 0],
            objective_quantity: [band0_qty, 0, 0, 0],
            number_of_attempts: [band0_attempts, 0, 0, 0],
            recommended_level: [0; 4],
            reward_item_id: [0; 4],
            reward_quantity: [0; 4],
        }
    }

    #[test]
    fn local_leve_id_range_bounds() {
        assert!(is_local_leve_quest_id(120_001));
        assert!(is_local_leve_quest_id(120_452));
        assert!(!is_local_leve_quest_id(120_000));
        assert!(!is_local_leve_quest_id(120_453));
    }

    #[test]
    fn craft_success_increments_both_counters() {
        let mut q = Quest::new(crate::actor::quest::quest_actor_id(120_001), "plg120001");
        let d = test_data(3000001, 5, 4);
        let mut view = PassiveGuildleveView::new(&mut q, &d);
        assert_eq!(view.current_crafted(), 0);
        assert_eq!(view.current_attempt(), 0);
        view.craft_success(2);
        assert_eq!(view.current_crafted(), 2);
        assert_eq!(view.current_attempt(), 1);
        view.craft_success(1);
        assert_eq!(view.current_crafted(), 3);
        assert_eq!(view.current_attempt(), 2);
    }

    #[test]
    fn craft_fail_only_bumps_attempt() {
        let mut q = Quest::new(crate::actor::quest::quest_actor_id(120_001), "plg120001");
        let d = test_data(3000001, 5, 4);
        let mut view = PassiveGuildleveView::new(&mut q, &d);
        view.craft_success(1);
        view.craft_fail();
        assert_eq!(view.current_crafted(), 1);
        assert_eq!(view.current_attempt(), 2);
    }

    #[test]
    fn remaining_materials_clamps_to_zero() {
        let mut q = Quest::new(crate::actor::quest::quest_actor_id(120_001), "plg120001");
        let d = test_data(3000001, 5, 3);
        let mut view = PassiveGuildleveView::new(&mut q, &d);
        view.craft_success(1);
        view.craft_success(1);
        view.craft_success(1);
        view.craft_success(1); // one more than allowed
        assert_eq!(view.remaining_materials(), 0);
    }

    #[test]
    fn has_materials_toggles_the_reserved_flag_bit() {
        let mut q = Quest::new(crate::actor::quest::quest_actor_id(120_001), "plg120001");
        let d = test_data(3000001, 5, 4);
        let mut view = PassiveGuildleveView::new(&mut q, &d);
        assert!(!view.has_materials());
        view.set_has_materials(true);
        assert!(view.has_materials());
        view.set_has_materials(false);
        assert!(!view.has_materials());
    }

    #[test]
    fn difficulty_is_read_as_clamped_band_index() {
        let mut q = Quest::new(crate::actor::quest::quest_actor_id(120_001), "plg120001");
        let d = test_data(3000001, 5, 4);
        let mut view = PassiveGuildleveView::new(&mut q, &d);
        view.set_current_difficulty(0);
        assert_eq!(view.current_difficulty(), 0);
        view.set_current_difficulty(3);
        assert_eq!(view.current_difficulty(), 3);
        // Out-of-range values saturate on read (C# byte cast mimicry).
        view.quest.set_counter(2, 42);
        assert_eq!(view.current_difficulty(), 3);
    }

    #[test]
    fn quest_in_leve_range_is_craft_passive_guildleve() {
        let mut q = Quest::new(crate::actor::quest::quest_actor_id(120_001), "plg120001");
        let d = test_data(3000001, 5, 4);
        let view = PassiveGuildleveView::new(&mut q, &d);
        assert!(view.is_craft_passive_guildleve());
    }
}
