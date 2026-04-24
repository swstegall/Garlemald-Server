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

//! [`RegionalLeveData`] — static definition of one fieldcraft or
//! battlecraft leve. Row in `gamedata_regional_leves`.

#![allow(dead_code)]

/// Which progress pipeline a given leve plugs into. The numeric values
/// match the DB `leveType` discriminator column exactly so loads round
/// trip without a separate decoder table.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum LeveType {
    /// Gatherer leve. `objective_target_id` is an item catalog id that
    /// the player must harvest `objective_quantity` of.
    Fieldcraft = 1,
    /// Combat leve. `objective_target_id` is a BattleNpc
    /// `actor_class_id` that the player must defeat
    /// `objective_quantity` times.
    Battlecraft = 2,
}

impl LeveType {
    /// Decode from the DB column's INTEGER value. `None` on an
    /// unknown discriminator so the loader can skip the row rather
    /// than silently corrupt semantics.
    pub fn from_repr(v: i64) -> Option<Self> {
        match v {
            1 => Some(LeveType::Fieldcraft),
            2 => Some(LeveType::Battlecraft),
            _ => None,
        }
    }
}

/// Static definition of one regional leve, mirroring the field shape of
/// [`crate::crafting::PassiveGuildleveData`] but with:
///
/// * an explicit [`LeveType`] discriminator, and
/// * `objective_target_id` renamed (it's an *item catalog* id for
///   fieldcraft and a *BattleNpc actor_class* id for battlecraft —
///   reusing the same column keeps the loader code one branch wide).
///
/// Every vector is parallel (`[_; 4]` band arrays) to match the C#
/// PassiveGuildleveData convention, so a future refactor that unifies
/// the two catalogs into one common base can do so column-for-column.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegionalLeveData {
    pub id: u32,
    pub leve_type: LeveType,
    pub plate_id: u32,
    pub border_id: u32,
    pub recommended_class: u32,
    pub issuing_location: u32,
    pub leve_location: u32,
    pub delivery_display_name: u32,
    pub region: u32,

    pub objective_target_id: [i32; 4],
    pub objective_quantity: [i32; 4],
    pub recommended_level: [i32; 4],
    pub reward_item_id: [i32; 4],
    pub reward_quantity: [i32; 4],
    pub reward_gil: [i32; 4],
}

impl RegionalLeveData {
    /// Clamp a difficulty index from Lua or DB into the `0..=3`
    /// band range, matching the C# `byte` cast on the crafting-leve
    /// side.
    pub fn clamp_difficulty(d: i32) -> usize {
        d.clamp(0, 3) as usize
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn leve_type_decode_round_trips_known_values() {
        assert_eq!(LeveType::from_repr(1), Some(LeveType::Fieldcraft));
        assert_eq!(LeveType::from_repr(2), Some(LeveType::Battlecraft));
        assert_eq!(LeveType::from_repr(0), None);
        assert_eq!(LeveType::from_repr(3), None);
        assert_eq!(LeveType::from_repr(-1), None);
    }

    #[test]
    fn difficulty_clamp_saturates_identically_to_crafting_leves() {
        assert_eq!(RegionalLeveData::clamp_difficulty(-5), 0);
        assert_eq!(RegionalLeveData::clamp_difficulty(0), 0);
        assert_eq!(RegionalLeveData::clamp_difficulty(3), 3);
        assert_eq!(RegionalLeveData::clamp_difficulty(42), 3);
    }
}
