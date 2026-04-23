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

//! Grand Company helpers — the canonical mapping between GC id
//! (`gc_current` 1/2/3), GC seal item ids, and rank-based seal caps.
//!
//! Values match the 1.x in-game tables as recorded in
//! `scripts/lua/gcseals.lua` and cross-checked against retail packet
//! captures. Seal item ids live in the currency bag
//! (`PKG_CURRENCY_CRYSTALS`, package code 99) — exactly like gil
//! (1_000_001).
//!
//! The three GCs are:
//!
//!   1 — Maelstrom                 (Limsa Lominsa; "Storm" seals)
//!   2 — Order of the Twin Adder   (Gridania; "Serpent" seals)
//!   3 — Immortal Flames           (Ul'dah; "Flame" seals)
//!
//! Rank codes don't run 0..9 — they use the retail "two-digit"
//! form where each promotion category increments the ones digit and
//! each tier shift increments the tens digit (11 = Private Third
//! Class, 13 = Second, 15 = First, 17 = Corporal, 21..=27 =
//! Sergeants, 31..=35 = Lieutenant..Captain, etc.). `127` is the
//! sentinel "not yet promoted past Recruit".

#![allow(dead_code)]

pub const GC_NONE: u8 = 0;
pub const GC_MAELSTROM: u8 = 1;
pub const GC_TWIN_ADDER: u8 = 2;
pub const GC_IMMORTAL_FLAMES: u8 = 3;

/// Retail seal item ids. Kept in index-1 alignment with `GC_*` so
/// `seal_item_id(GC_MAELSTROM) == 1_000_201`.
pub const SEAL_STORM: u32 = 1_000_201;
pub const SEAL_SERPENT: u32 = 1_000_202;
pub const SEAL_FLAME: u32 = 1_000_203;

/// Sentinel rank for "never promoted past Recruit" — matches the
/// retail default on fresh characters.
pub const RANK_RECRUIT: u8 = 127;

pub fn is_valid_gc(gc: u8) -> bool {
    matches!(gc, GC_MAELSTROM | GC_TWIN_ADDER | GC_IMMORTAL_FLAMES)
}

/// Seal catalog id for a GC. Returns `None` for GC_NONE / invalid ids.
pub fn seal_item_id(gc: u8) -> Option<u32> {
    match gc {
        GC_MAELSTROM => Some(SEAL_STORM),
        GC_TWIN_ADDER => Some(SEAL_SERPENT),
        GC_IMMORTAL_FLAMES => Some(SEAL_FLAME),
        _ => None,
    }
}

/// Per-rank seal cap — lifted from `scripts/lua/gcseals.lua`. Returns
/// `0` for unrecognised ranks so the caller falls through to the "no
/// seals accumulated yet" branch rather than accepting an arbitrary
/// deposit.
pub fn rank_seal_cap(rank: u8) -> i32 {
    match rank {
        0 => 0,           // None
        11 => 10_000,     // Private Third Class
        13 => 15_000,     // Private Second Class
        15 => 20_000,     // Private First Class
        17 => 25_000,     // Corporal
        21 => 30_000,     // Sergeant Third Class
        23 => 35_000,     // Sergeant Second Class
        25 => 40_000,     // Sergeant First Class
        27 => 45_000,     // Chief Sergeant
        31 => 50_000,     // Second Lieutenant
        33 => 50_000,     // First Lieutenant
        35 => 50_000,     // Captain
        41 => 60_000,     // Second Commander
        43 => 60_000,     // First Commander
        45 => 60_000,     // High Commander
        51 => 70_000,     // Rear Marshal
        53 => 70_000,     // Vice Marshal
        55 => 70_000,     // Marshal
        57 => 70_000,     // Grand Marshal
        100 => 100_000,   // Champion
        111 => 0,         // Chief Admiral / Elder Seedseer / General (0-cap in source)
        127 => 10_000,    // Recruit
        _ => 0,
    }
}

/// 1.23b rank cap — the "highest rank the GC officer will promote
/// you to without a story-quest gate" is Second Lieutenant (code 31),
/// which is also retail's content cap for 1.x. Promotion past this
/// needs the flag set at quest-completion time.
pub const STORY_RANK_CAP: u8 = 31;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seal_id_maps_each_gc() {
        assert_eq!(seal_item_id(GC_MAELSTROM), Some(SEAL_STORM));
        assert_eq!(seal_item_id(GC_TWIN_ADDER), Some(SEAL_SERPENT));
        assert_eq!(seal_item_id(GC_IMMORTAL_FLAMES), Some(SEAL_FLAME));
        assert_eq!(seal_item_id(GC_NONE), None);
        assert_eq!(seal_item_id(99), None);
    }

    #[test]
    fn is_valid_gc_accepts_only_three() {
        for gc in [1u8, 2, 3] {
            assert!(is_valid_gc(gc));
        }
        for gc in [0u8, 4, 5, 127, 255] {
            assert!(!is_valid_gc(gc));
        }
    }

    #[test]
    fn rank_cap_known_ranks() {
        assert_eq!(rank_seal_cap(11), 10_000);
        assert_eq!(rank_seal_cap(31), 50_000);
        assert_eq!(rank_seal_cap(127), 10_000);
        assert_eq!(rank_seal_cap(0), 0);
        assert_eq!(rank_seal_cap(111), 0);
        assert_eq!(rank_seal_cap(42), 0);
    }
}
