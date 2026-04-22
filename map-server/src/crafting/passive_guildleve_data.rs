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

//! [`PassiveGuildleveData`] — static definition of one "local leve"
//! (crafting levequest), ported from
//! `Map Server/DataObjects/PassiveGuildleveData.cs` on
//! `origin/ioncannon/crafting_and_localleves`.
//!
//! Each row in `gamedata_passivegl_craft` describes one leve with four
//! difficulty bands (index `0..=3`): objective item + quantity, number
//! of material allowances, recommended crafting-class level, and the
//! reward item + quantity. Meteor's C# PassiveGuildleve quest actor
//! picks which band is active at runtime from the player's menu choice
//! — garlemald mirrors that with [`objective_item_id`] etc. indexed by
//! the quest's `difficulty` counter.
//!
//! [`objective_item_id`]: PassiveGuildleveData::objective_item_id

#![allow(dead_code)]

/// Every field mirrors the DB column name verbatim (snake-cased). The
/// four difficulty bands are stored in parallel arrays rather than a
/// `struct Band` so the loader maps straight from `objectiveItemIdN`
/// columns without an intermediate struct — matches the C# layout.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PassiveGuildleveData {
    pub id: u32,
    pub plate_id: u32,
    pub border_id: u32,
    pub recommended_class: u32,
    pub issuing_location: u32,
    pub leve_location: u32,
    pub delivery_display_name: u32,

    pub objective_item_id: [i32; 4],
    pub objective_quantity: [i32; 4],
    pub reward_item_id: [i32; 4],
    pub reward_quantity: [i32; 4],
    pub number_of_attempts: [i32; 4],
    pub recommended_level: [i32; 4],
}

impl PassiveGuildleveData {
    /// Clamp a difficulty from Lua (which may come in 1-indexed from the
    /// UI) to a valid band `0..=3`. Out-of-range values saturate rather
    /// than panic — the C# `byte` cast also silently wraps.
    pub fn clamp_difficulty(d: i32) -> usize {
        d.clamp(0, 3) as usize
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn difficulty_clamp_saturates() {
        assert_eq!(PassiveGuildleveData::clamp_difficulty(-5), 0);
        assert_eq!(PassiveGuildleveData::clamp_difficulty(0), 0);
        assert_eq!(PassiveGuildleveData::clamp_difficulty(3), 3);
        assert_eq!(PassiveGuildleveData::clamp_difficulty(42), 3);
    }
}
