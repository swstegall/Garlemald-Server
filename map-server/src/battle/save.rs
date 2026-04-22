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

//! `BattleSave` — per-character persistent battle state. Ported from
//! `Actors/Chara/BattleSave.cs`. These survive logout/login and get
//! flushed back to the DB via the character-save hooks.

#![allow(dead_code)]

/// Number of class/skill slots — mirrors `short[52]` in the C#.
pub const NUM_SKILLS: usize = 52;

#[derive(Debug, Clone)]
pub struct BattleSave {
    pub potencial: f32,
    pub skill_level: [i16; NUM_SKILLS],
    pub skill_level_cap: [i16; NUM_SKILLS],
    pub skill_point: [i32; NUM_SKILLS],

    pub physical_level: i16,
    pub physical_exp: i32,

    pub negotiation_flag: [bool; 2],
}

impl Default for BattleSave {
    fn default() -> Self {
        Self {
            potencial: 6.6,
            skill_level: [0; NUM_SKILLS],
            skill_level_cap: [0; NUM_SKILLS],
            skill_point: [0; NUM_SKILLS],
            physical_level: 0,
            physical_exp: 0,
            negotiation_flag: [false; 2],
        }
    }
}

/// Level cap for disciples-of-war / disciples-of-magic classes.
/// Matches the Meteor upstream (and FFXIV 1.23b retail) ceiling —
/// `LEVEL_THRESHOLDS` is indexed by `current_level - 1` so the last
/// meaningful entry at index 49 says "to go from 49 → 50 costs 110000
/// SP"; once a character hits 50 they stay there and `skill_point`
/// saturates without rolling over.
pub const MAX_LEVEL: i16 = 50;

/// Skill-point requirement to advance from level `i+1` to level `i+2`.
/// Ported verbatim from Project Meteor's
/// `Data/scripts/commands/gm/yolo.lua:49-100` `expTable` — that's the
/// only committed copy of the retail curve in the codebase; Meteor's
/// C# `Player.GetLevelThreshold` was never implemented.
///
/// Usage: once `skill_point[class] >= LEVEL_THRESHOLDS[current_level -
/// 1]`, roll over (`skill_point -= threshold`, `current_level += 1`).
/// Levels at or above 50 skip the check since the array ends at index
/// 49 ("50 → 51" isn't defined).
pub const LEVEL_THRESHOLDS: [i32; 50] = [
    570,     // 1 → 2
    700,     // 2 → 3
    880,     // 3 → 4
    1_100,   // 4 → 5
    1_500,   // 5 → 6
    1_800,   // 6 → 7
    2_300,   // 7 → 8
    3_200,   // 8 → 9
    4_300,   // 9 → 10
    5_000,   // 10 → 11
    5_900,   // 11 → 12
    6_800,   // 12 → 13
    7_700,   // 13 → 14
    8_700,   // 14 → 15
    9_700,   // 15 → 16
    11_000,  // 16 → 17
    12_000,  // 17 → 18
    13_000,  // 18 → 19
    15_000,  // 19 → 20
    16_000,  // 20 → 21
    20_000,  // 21 → 22
    22_000,  // 22 → 23
    23_000,  // 23 → 24
    25_000,  // 24 → 25
    27_000,  // 25 → 26
    29_000,  // 26 → 27
    31_000,  // 27 → 28
    33_000,  // 28 → 29
    35_000,  // 29 → 30
    38_000,  // 30 → 31
    45_000,  // 31 → 32
    47_000,  // 32 → 33
    50_000,  // 33 → 34
    53_000,  // 34 → 35
    56_000,  // 35 → 36
    59_000,  // 36 → 37
    62_000,  // 37 → 38
    65_000,  // 38 → 39
    68_000,  // 39 → 40
    71_000,  // 40 → 41
    74_000,  // 41 → 42
    78_000,  // 42 → 43
    81_000,  // 43 → 44
    85_000,  // 44 → 45
    89_000,  // 45 → 46
    92_000,  // 46 → 47
    96_000,  // 47 → 48
    100_000, // 48 → 49
    100_000, // 49 → 50 (Meteor intentionally repeats — the retail
             //          curve levels off at the end)
    110_000, // 50 → 51 placeholder — never consumed because the
             //          level-up loop clamps at MAX_LEVEL.
];

/// Roll over one character's `(level, skill_point)` through any
/// accumulated crossings of [`LEVEL_THRESHOLDS`]. Returns the new
/// `(level, skill_point)` pair plus the number of levels gained this
/// call (caller may want to log, emit a game message, etc.).
///
/// Clamps at [`MAX_LEVEL`] — a character who's already there keeps
/// their `skill_point` saturated rather than rolling over into
/// undefined territory.
///
/// Both inputs must be non-negative; callers are responsible for
/// clamping negative inputs (`apply_add_exp` already does `.max(0)`).
pub fn level_up_if_threshold_crossed(level: i16, skill_point: i32) -> (i16, i32, i16) {
    let mut lvl = level.max(1);
    let mut sp = skill_point.max(0);
    let mut gained: i16 = 0;

    while lvl < MAX_LEVEL {
        let idx = (lvl - 1).max(0) as usize;
        let threshold = LEVEL_THRESHOLDS[idx];
        if sp < threshold {
            break;
        }
        sp -= threshold;
        lvl += 1;
        gained += 1;
    }
    // If we hit the cap with leftover SP, clamp to the final threshold
    // so the UI doesn't flash a stuck "50% to level 51" bar — Meteor's
    // retail client treats post-cap SP as 0.
    if lvl >= MAX_LEVEL {
        sp = 0;
    }
    (lvl, sp, gained)
}

#[cfg(test)]
mod level_up_tests {
    use super::*;

    #[test]
    fn no_op_below_threshold() {
        let (lvl, sp, gained) = level_up_if_threshold_crossed(1, 569);
        assert_eq!((lvl, sp, gained), (1, 569, 0));
    }

    #[test]
    fn single_rollover() {
        let (lvl, sp, gained) = level_up_if_threshold_crossed(1, 570);
        assert_eq!((lvl, sp, gained), (2, 0, 1));
    }

    #[test]
    fn single_rollover_keeps_surplus() {
        let (lvl, sp, gained) = level_up_if_threshold_crossed(1, 600);
        assert_eq!((lvl, sp, gained), (2, 30, 1));
    }

    #[test]
    fn triple_rollover() {
        // 570 (1→2) + 700 (2→3) + 880 (3→4) = 2150 exactly.
        let (lvl, sp, gained) = level_up_if_threshold_crossed(1, 2150);
        assert_eq!((lvl, sp, gained), (4, 0, 3));
    }

    #[test]
    fn cap_at_max_level_clamps_skill_point() {
        // Already at 50 with dangling SP: should clamp SP to 0.
        let (lvl, sp, gained) = level_up_if_threshold_crossed(50, 12_345);
        assert_eq!((lvl, sp, gained), (50, 0, 0));
    }

    #[test]
    fn rollover_from_49_to_50_clamps_remainder() {
        // 100000 is the 49 → 50 threshold; 5000 surplus should be
        // dropped because 50 is the cap.
        let (lvl, sp, gained) = level_up_if_threshold_crossed(49, 105_000);
        assert_eq!((lvl, sp, gained), (50, 0, 1));
    }

    #[test]
    fn negative_inputs_saturate() {
        let (lvl, sp, gained) = level_up_if_threshold_crossed(-5, -100);
        assert_eq!((lvl, sp, gained), (1, 0, 0));
    }
}
