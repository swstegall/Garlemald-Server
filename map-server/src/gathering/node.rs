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

//! Gather-node record types.
//!
//! [`GatherNode`] is the template row (grade, attempts, up to eleven
//! item-pool keys). [`GatherNodeItem`] is the per-drop entry (item id,
//! aim band, sweet-spot target, remainder pool, yield).
//!
//! [`AimSlot`] is the pivoted view [`crate::gathering::resolver::GatherResolver::build_aim_slots`]
//! produces — exactly eleven entries matching the 1.x minigame's 11-slot
//! aim gauge (`+5..-5`), with the unused slots filled with the `empty`
//! sentinel. Matches the shape of the Lua `BuildHarvestNode` helper the
//! hardcoded DummyCommand.lua version was building client-side.

#![allow(dead_code)]

/// Maximum number of item keys a single [`GatherNode`] can reference.
/// Mirrors the 11-slot aim gauge on the 1.x minigame widget — each
/// aim slot is either empty or pinned to one [`GatherNodeItem`].
pub const NODE_ITEM_SLOTS: usize = 11;

/// The number of aim gauge positions the pivoted `AimSlot[]` table
/// returns. Always eleven; fixed by the client-side `_waitForTurning`
/// widget, which encodes the aim as `(slider/10)+1` (1..=11 inclusive,
/// with an extra +1 for Lua 1-indexing).
pub const AIM_SLOTS: usize = 11;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GatherNode {
    pub id: u32,
    /// 1..=5 in retail. Drives the graphic shell the client uses for
    /// the aim/strike widget; drop rates are independent of grade.
    pub grade: u8,
    /// How many times the player can run the minigame against this
    /// physical node before it exhausts and respawns.
    pub attempts: u8,
    /// Up to eleven keys into [`GatherNodeItem`]; unused slots are `0`.
    pub item_keys: [u32; NODE_ITEM_SLOTS],
}

impl GatherNode {
    /// Build from the SQL column shape (`item1..item11` are `Option<i64>` in
    /// SQLite because every slot past the populated ones is NULL). `None`
    /// slots and explicit zero both collapse to the empty sentinel.
    pub fn from_raw(id: u32, grade: u8, attempts: u8, items: [Option<i64>; NODE_ITEM_SLOTS]) -> Self {
        let mut item_keys = [0u32; NODE_ITEM_SLOTS];
        for (i, v) in items.iter().enumerate() {
            item_keys[i] = v.and_then(|x| u32::try_from(x).ok()).unwrap_or(0);
        }
        Self {
            id,
            grade,
            attempts,
            item_keys,
        }
    }

    /// Non-zero item keys the caller should resolve against the item map.
    pub fn active_item_keys(&self) -> impl Iterator<Item = u32> + '_ {
        self.item_keys.iter().copied().filter(|k| *k != 0)
    }

    pub fn num_items(&self) -> usize {
        self.item_keys.iter().filter(|k| **k != 0).count()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GatherNodeItem {
    pub id: u32,
    /// 1.x catalog id (e.g. 10001006 = Copper Ore). Written to the
    /// player's NORMAL bag on a successful strike.
    pub item_catalog_id: u32,
    /// Node-HP pool at the start of this item's strike phase. The Lua
    /// side decrements by 20 per swing; reaching 0 ends the attempt.
    pub remainder: u8,
    /// 0..=100 slider target selecting this item. Rounds down to an
    /// aim slot via `(aim / 10) + 1`.
    pub aim: u8,
    /// 0..=100 strike power target. ±10 power-units is the "hit" band.
    pub sweetspot: u8,
    /// Maximum quantity granted on a perfect strike.
    pub max_yield: u32,
}

impl GatherNodeItem {
    /// Client-visible aim slot (1..=11) this item sits in. `aim/10`
    /// truncates toward the lower edge; `+1` folds in Lua 1-indexing.
    pub fn aim_slot(&self) -> usize {
        ((self.aim as usize) / 10 + 1).min(AIM_SLOTS)
    }
}

/// One row in the pivoted aim table. Slot position (1..=11) is
/// implicit in the array index the caller stores this under. `empty`
/// is `true` when no item maps to this slot — the Lua widget still
/// wants eleven rows (it indexes by raw aim) so we pad rather than
/// compress.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct AimSlot {
    pub empty: bool,
    pub item_key: u32,
    pub item_catalog_id: u32,
    pub remainder: u8,
    pub sweetspot: u8,
    pub max_yield: u32,
}

impl AimSlot {
    pub fn empty() -> Self {
        Self {
            empty: true,
            ..Default::default()
        }
    }

    pub fn from_item(item: &GatherNodeItem) -> Self {
        Self {
            empty: false,
            item_key: item.id,
            item_catalog_id: item.item_catalog_id,
            remainder: item.remainder,
            sweetspot: item.sweetspot,
            max_yield: item.max_yield,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_raw_compacts_null_and_zero_identically() {
        let node = GatherNode::from_raw(
            1001,
            2,
            2,
            [
                Some(1),
                Some(2),
                Some(3),
                None,
                Some(0),
                None,
                None,
                None,
                None,
                None,
                None,
            ],
        );
        assert_eq!(node.item_keys[0..3], [1, 2, 3]);
        assert_eq!(node.item_keys[3], 0);
        assert_eq!(node.item_keys[4], 0);
        assert_eq!(node.num_items(), 3);
    }

    #[test]
    fn aim_slot_folds_lua_indexing() {
        let lo = GatherNodeItem {
            id: 1,
            item_catalog_id: 10001006,
            remainder: 80,
            aim: 0,
            sweetspot: 30,
            max_yield: 3,
        };
        assert_eq!(lo.aim_slot(), 1);
        let mid = GatherNodeItem { aim: 50, ..lo.clone() };
        assert_eq!(mid.aim_slot(), 6);
        let hi = GatherNodeItem { aim: 100, ..lo.clone() };
        assert_eq!(hi.aim_slot(), 11);
        let over = GatherNodeItem { aim: 120, ..lo };
        assert_eq!(over.aim_slot(), 11);
    }

    #[test]
    fn aim_slot_from_item_copies_strike_params() {
        let item = GatherNodeItem {
            id: 7,
            item_catalog_id: 123,
            remainder: 60,
            aim: 30,
            sweetspot: 40,
            max_yield: 2,
        };
        let slot = AimSlot::from_item(&item);
        assert!(!slot.empty);
        assert_eq!(slot.item_key, 7);
        assert_eq!(slot.item_catalog_id, 123);
        assert_eq!(slot.remainder, 60);
        assert_eq!(slot.sweetspot, 40);
        assert_eq!(slot.max_yield, 2);
    }
}
