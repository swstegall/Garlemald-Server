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

//! [`RegionalLeveView`] — per-player runtime state for a fieldcraft or
//! battlecraft leve. Parallels [`crate::crafting::PassiveGuildleveView`]
//! but with progress counters scoped to gather / kill progression
//! instead of synthesis attempts.
//!
//! Counter / flag layout on the backing [`Quest`]:
//!
//! * `counter1` (idx 0) → `progress`   (u16, clamped by
//!   `objective_quantity[band]` when written through
//!   [`RegionalLeveView::advance_progress`])
//! * `counter2` (idx 1) → `difficulty` (u16 band index 0..=3)
//! * `counter3` (idx 2) → reserved (unused today; room for a time
//!   limit or bonus-objective tracker later)
//! * `flag bit 0` → [`ACCEPTED_FLAG_BIT`]
//! * `flag bit 1` → [`COMPLETED_FLAG_BIT`] (set once the leve clears;
//!   used to short-circuit repeat progress events that land between
//!   the completion tick and the client's rewards screen)

#![allow(dead_code)]

use crate::actor::quest::Quest;

use super::data::{LeveType, RegionalLeveData};

/// Inclusive range of quest ids reserved for fieldcraft leves. Kept
/// above the 112_048 `Bitstream2048` cap so repeatable leves don't
/// occupy completed-quest bit positions.
pub const FIELDCRAFT_LEVE_ID_MIN: u32 = 130_001;
pub const FIELDCRAFT_LEVE_ID_MAX: u32 = 130_450;

/// Inclusive range of quest ids reserved for battlecraft leves.
pub const BATTLECRAFT_LEVE_ID_MIN: u32 = 140_001;
pub const BATTLECRAFT_LEVE_ID_MAX: u32 = 140_450;

/// Flag bit — `true` once the player confirms the leve at a
/// levemete's prompt. Gates progress events so a random harvest /
/// kill doesn't tick an unaccepted leve.
pub const ACCEPTED_FLAG_BIT: u8 = 0;

/// Flag bit — `true` once `progress >= objective_quantity`. Used to
/// make [`RegionalLeveView::advance_progress`] idempotent and to
/// route the follow-up `handInLeve` RPC to the reward branch.
pub const COMPLETED_FLAG_BIT: u8 = 1;

pub fn is_fieldcraft_leve_quest_id(quest_id: u32) -> bool {
    (FIELDCRAFT_LEVE_ID_MIN..=FIELDCRAFT_LEVE_ID_MAX).contains(&quest_id)
}

pub fn is_battlecraft_leve_quest_id(quest_id: u32) -> bool {
    (BATTLECRAFT_LEVE_ID_MIN..=BATTLECRAFT_LEVE_ID_MAX).contains(&quest_id)
}

pub fn is_regional_leve_quest_id(quest_id: u32) -> bool {
    is_fieldcraft_leve_quest_id(quest_id) || is_battlecraft_leve_quest_id(quest_id)
}

/// Classify a quest id as fieldcraft / battlecraft / neither without
/// consulting the catalog. The catalog is authoritative for actual
/// content, but this function is useful when the catalog row is
/// missing (e.g. the leve was accepted but has since been retired) —
/// callers can at least tell the progress pipelines apart.
pub fn leve_type_from_quest_id(quest_id: u32) -> Option<LeveType> {
    if is_fieldcraft_leve_quest_id(quest_id) {
        Some(LeveType::Fieldcraft)
    } else if is_battlecraft_leve_quest_id(quest_id) {
        Some(LeveType::Battlecraft)
    } else {
        None
    }
}

/// View that pairs a mutable [`Quest`] with its static
/// [`RegionalLeveData`]. All accessors go through the quest's counter
/// / flag API so persistence + dirty-bit tracking continues to work.
pub struct RegionalLeveView<'a> {
    pub quest: &'a mut Quest,
    pub data: &'a RegionalLeveData,
}

impl<'a> RegionalLeveView<'a> {
    pub fn new(quest: &'a mut Quest, data: &'a RegionalLeveData) -> Self {
        Self { quest, data }
    }

    // ------------------------------------------------------------------
    // Difficulty — identical shape to the crafting-leve view.
    // ------------------------------------------------------------------

    /// Band index `0..=3`. Values ≥ 4 saturate to 3 on read (C# byte
    /// cast mimicry).
    pub fn current_difficulty(&self) -> u8 {
        self.quest.get_counter(1).min(3) as u8
    }

    pub fn set_current_difficulty(&mut self, difficulty: u8) {
        self.quest.set_counter(1, difficulty as u16);
    }

    // ------------------------------------------------------------------
    // Progress + lifecycle flags.
    // ------------------------------------------------------------------

    /// How many items gathered / monsters killed so far against this
    /// leve's active difficulty.
    pub fn progress(&self) -> u16 {
        self.quest.get_counter(0)
    }

    fn set_progress(&mut self, v: u16) {
        self.quest.set_counter(0, v);
    }

    pub fn is_accepted(&self) -> bool {
        self.quest.get_flag(ACCEPTED_FLAG_BIT)
    }

    pub fn set_accepted(&mut self, v: bool) {
        if v {
            self.quest.set_flag(ACCEPTED_FLAG_BIT);
        } else {
            self.quest.clear_flag(ACCEPTED_FLAG_BIT);
        }
    }

    pub fn is_completed(&self) -> bool {
        self.quest.get_flag(COMPLETED_FLAG_BIT)
    }

    fn set_completed(&mut self, v: bool) {
        if v {
            self.quest.set_flag(COMPLETED_FLAG_BIT);
        } else {
            self.quest.clear_flag(COMPLETED_FLAG_BIT);
        }
    }

    // ------------------------------------------------------------------
    // Derived values.
    // ------------------------------------------------------------------

    /// Quantity required to complete the active difficulty band.
    pub fn objective_quantity(&self) -> i32 {
        self.data.objective_quantity[self.current_difficulty() as usize]
    }

    /// Id of the objective target (item for fieldcraft, actor class
    /// for battlecraft).
    pub fn objective_target_id(&self) -> i32 {
        self.data.objective_target_id[self.current_difficulty() as usize]
    }

    /// `objective_quantity - progress`, saturating at 0.
    pub fn remaining(&self) -> i32 {
        let rem = self.objective_quantity() - self.progress() as i32;
        rem.max(0)
    }

    // ------------------------------------------------------------------
    // Transitions.
    // ------------------------------------------------------------------

    /// Record a matching gather or kill event. Accepts the *delta*
    /// (usually 1) rather than a concrete count so a single harvest
    /// that drops 3 copper ore can advance the leve by 3. Idempotent
    /// once the leve is completed — further events are silently
    /// dropped (matching the retail behaviour where the leve's quest
    /// card greys out on completion).
    ///
    /// Returns `true` iff this call flipped the leve into the
    /// completed state (exactly once per completion). Callers use the
    /// return value to fire the "leve complete" game message / reward
    /// RPC without doing their own before/after diff.
    pub fn advance_progress(&mut self, delta: u16) -> bool {
        if !self.is_accepted() || self.is_completed() {
            return false;
        }
        let target = self.objective_quantity().max(0) as u16;
        let new = self.progress().saturating_add(delta).min(target);
        self.set_progress(new);
        if new >= target && target > 0 {
            self.set_completed(true);
            return true;
        }
        false
    }

    /// Reset runtime state so a subsequent accept starts from zero.
    /// Called when a player abandons the leve at the levemete or
    /// after a successful completion's reward payout.
    pub fn reset_runtime_state(&mut self) {
        self.set_progress(0);
        self.set_accepted(false);
        self.set_completed(false);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::actor::quest::{Quest, quest_actor_id};
    use crate::leve::data::LeveType;

    fn fc_data(target_item: i32, qty_per_band: [i32; 4]) -> RegionalLeveData {
        RegionalLeveData {
            id: 130_001,
            leve_type: LeveType::Fieldcraft,
            plate_id: 0,
            border_id: 0,
            recommended_class: 30,
            issuing_location: 0,
            leve_location: 0,
            delivery_display_name: 0,
            region: 1001,
            objective_target_id: [target_item; 4],
            objective_quantity: qty_per_band,
            recommended_level: [1, 10, 20, 30],
            reward_item_id: [0; 4],
            reward_quantity: [0; 4],
            reward_gil: [200, 500, 1200, 2500],
        }
    }

    #[test]
    fn id_range_boundaries_match_roadmap() {
        assert!(is_fieldcraft_leve_quest_id(130_001));
        assert!(is_fieldcraft_leve_quest_id(130_450));
        assert!(!is_fieldcraft_leve_quest_id(130_451));
        assert!(!is_fieldcraft_leve_quest_id(130_000));
        assert!(is_battlecraft_leve_quest_id(140_001));
        assert!(is_battlecraft_leve_quest_id(140_450));
        assert!(!is_battlecraft_leve_quest_id(130_450));
        assert!(is_regional_leve_quest_id(130_200));
        assert!(is_regional_leve_quest_id(140_200));
        assert!(!is_regional_leve_quest_id(120_001));
    }

    #[test]
    fn type_classifier_splits_ranges_correctly() {
        assert_eq!(leve_type_from_quest_id(130_001), Some(LeveType::Fieldcraft));
        assert_eq!(leve_type_from_quest_id(140_001), Some(LeveType::Battlecraft));
        assert_eq!(leve_type_from_quest_id(120_001), None);
    }

    #[test]
    fn progress_does_not_advance_when_not_accepted() {
        let mut q = Quest::new(quest_actor_id(130_001), "fcl130001");
        let d = fc_data(10_001_006, [5, 10, 15, 20]);
        let mut view = RegionalLeveView::new(&mut q, &d);
        let completed = view.advance_progress(3);
        assert!(!completed);
        assert_eq!(view.progress(), 0);
    }

    #[test]
    fn accept_then_advance_ticks_progress_and_completes() {
        let mut q = Quest::new(quest_actor_id(130_001), "fcl130001");
        let d = fc_data(10_001_006, [5, 10, 15, 20]);
        let mut view = RegionalLeveView::new(&mut q, &d);
        view.set_accepted(true);
        view.set_current_difficulty(0); // objective = 5
        assert!(!view.advance_progress(3));
        assert_eq!(view.progress(), 3);
        assert!(view.advance_progress(2));
        assert!(view.is_completed());
        assert_eq!(view.progress(), 5);
        assert_eq!(view.remaining(), 0);
    }

    #[test]
    fn advance_is_idempotent_once_completed() {
        let mut q = Quest::new(quest_actor_id(130_001), "fcl130001");
        let d = fc_data(10_001_006, [5, 10, 15, 20]);
        let mut view = RegionalLeveView::new(&mut q, &d);
        view.set_accepted(true);
        assert!(view.advance_progress(10)); // first crossing flips flag
        assert!(!view.advance_progress(10)); // second call no-ops
        assert!(view.is_completed());
        assert_eq!(view.progress(), 5);
    }

    #[test]
    fn progress_saturates_at_objective_not_delta() {
        let mut q = Quest::new(quest_actor_id(130_001), "fcl130001");
        let d = fc_data(10_001_006, [5, 10, 15, 20]);
        let mut view = RegionalLeveView::new(&mut q, &d);
        view.set_accepted(true);
        view.set_current_difficulty(1); // objective = 10
        let fired = view.advance_progress(50);
        assert!(fired);
        assert_eq!(view.progress(), 10, "saturate at target, not overshoot");
    }

    #[test]
    fn reset_runtime_state_clears_progress_and_flags() {
        let mut q = Quest::new(quest_actor_id(130_001), "fcl130001");
        let d = fc_data(10_001_006, [5, 10, 15, 20]);
        let mut view = RegionalLeveView::new(&mut q, &d);
        view.set_accepted(true);
        view.advance_progress(10); // completes
        assert!(view.is_completed());
        view.reset_runtime_state();
        assert_eq!(view.progress(), 0);
        assert!(!view.is_accepted());
        assert!(!view.is_completed());
    }

    #[test]
    fn zero_objective_does_not_auto_complete_on_zero_delta() {
        let mut q = Quest::new(quest_actor_id(130_001), "fcl130001");
        let d = fc_data(10_001_006, [0, 0, 0, 0]);
        let mut view = RegionalLeveView::new(&mut q, &d);
        view.set_accepted(true);
        let fired = view.advance_progress(0);
        assert!(!fired, "zero-objective rows are malformed; don't auto-complete");
        assert!(!view.is_completed());
    }
}
