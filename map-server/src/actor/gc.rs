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

/// Sequential rank ladder the `PopulaceCompanyOfficer.lua` promotion
/// flow walks. Recruit (127) → Private Third Class (11) →
/// Private Second Class (13) → Private First Class (15) → Corporal (17)
/// → Sergeant Third Class (21) → Sergeant Second Class (23) →
/// Sergeant First Class (25) → Chief Sergeant (27) → Second Lieutenant
/// (31, the 1.23b cap). The discontinuities (17 → 21, 27 → 31) match
/// retail's tier-shift convention where the tens digit increments at
/// each promotion category boundary. Returns `None` when the input is
/// already at or past the cap, or doesn't match a known rank.
pub fn next_rank(current: u8) -> Option<u8> {
    Some(match current {
        RANK_RECRUIT => 11,
        11 => 13,
        13 => 15,
        15 => 17,
        17 => 21,
        21 => 23,
        23 => 25,
        25 => 27,
        27 => 31,
        // Lieutenants / Captains / Marshals / Champions are beyond
        // 1.23b's content gate — promotion through these tiers needs
        // the GC story-quest flag and isn't reachable from
        // `PopulaceCompanyOfficer.lua` alone.
        _ => return None,
    })
}

/// Per-rank seal cost to advance to the next rank. Values mirror the
/// retail Maelstrom NPC dialogue lines in
/// `mirke-menagerie-context.md` ("a promotion from Storm Private Third
/// Class to Storm Private Second Class will cost you 100 seals" at
/// rank 11 → 13; the next-rank quote of 2,500 seals at rank 13 → 15;
/// 25,000 at the upper Sergeant tier). The Recruit → Private Third
/// Class hop is gated at 100 seals to match the same in-game
/// conversation. Returns `0` for ranks at or past the 1.23b cap, and
/// for unknown rank codes.
pub fn gc_promotion_cost(current: u8) -> i32 {
    match current {
        RANK_RECRUIT => 100,
        11 => 100,
        13 => 1_000,
        15 => 1_500,
        17 => 2_500,
        21 => 5_000,
        23 => 10_000,
        25 => 15_000,
        27 => 25_000,
        _ => 0,
    }
}

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

    #[test]
    fn next_rank_walks_the_full_ladder_to_story_cap() {
        // Recruit → Private Third Class → … → Second Lieutenant.
        let mut path = Vec::new();
        let mut r = RANK_RECRUIT;
        path.push(r);
        while let Some(n) = next_rank(r) {
            path.push(n);
            r = n;
            assert!(
                path.len() < 16,
                "ladder should terminate within 10 steps, walked: {path:?}"
            );
        }
        assert_eq!(path, vec![127, 11, 13, 15, 17, 21, 23, 25, 27, 31]);
        // Stops at the 1.23b cap.
        assert_eq!(next_rank(31), None);
    }

    #[test]
    fn next_rank_returns_none_for_unknown_ranks() {
        assert_eq!(next_rank(0), None);
        assert_eq!(next_rank(42), None);
        assert_eq!(next_rank(99), None);
    }

    #[test]
    fn promotion_cost_table_matches_dialogue_anchors() {
        // The Recruit → Private Third Class hop is the cheapest
        // (100 seals — Storm Lieutenant Guincum dialogue).
        assert_eq!(gc_promotion_cost(RANK_RECRUIT), 100);
        // Storm Lieutenant Guincum: "from Private Third Class to
        // Private Second Class will cost you 100 seals" (mirke
        // line 16389).
        assert_eq!(gc_promotion_cost(11), 100);
        // Past-cap and unknown ranks → 0.
        assert_eq!(gc_promotion_cost(31), 0);
        assert_eq!(gc_promotion_cost(0), 0);
        assert_eq!(gc_promotion_cost(99), 0);
    }
}
