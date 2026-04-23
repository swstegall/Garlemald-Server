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

//! Inn / dream helpers.
//!
//! Port of the `GetInnCode()` + `SetSleeping()` pair from
//! `Map Server/Actors/Chara/Player/Player.cs` on `project-meteor-server`'s
//! 2019-07-27 inn rework (commit `42f0046e`). Each of the three retail
//! city-state inns (Limsa, Gridania, Ul'dah) is modelled as a single inn
//! zone whose interior has three sleeping quarters — the C# code uses a
//! trio of hardcoded `Vector3` distance checks against the known room
//! centres to map a player's XZ position to an inn code 1/2/3 (room
//! index). Code 0 means "not in an inn room".
//!
//! The `SetSleeping` counterpart snaps the player's transform to the
//! canonical bed coordinate for the detected inn code so the subsequent
//! `SetPlayerDreamPacket` lands the fade-out animation on the right
//! pillow rather than wherever the player happened to click the bed from.

#![allow(dead_code)]

/// XZ distance threshold from each inn room's centre — Meteor uses
/// 20 units and we match it verbatim.
pub const INN_ROOM_RADIUS: f32 = 20.0;

/// Inn room `1` centre (matches Meteor's `Vector3(-160, 0, -160)`).
pub const INN1_CENTRE: (f32, f32) = (-160.0, -160.0);
/// Inn room `2` centre (Meteor's `Vector3(160, 0, 160)`).
pub const INN2_CENTRE: (f32, f32) = (160.0, 160.0);
/// Inn room `3` centre (Meteor's `Vector3(0, 0, 0)`).
pub const INN3_CENTRE: (f32, f32) = (0.0, 0.0);

/// Canonical sleeping transforms — `(x, y, z, rotation)` — the player
/// gets snapped to when `SetSleeping()` runs. Lifted from Meteor's
/// `Player.SetSleeping` switch arms (same commit).
pub const INN1_BED: (f32, f32, f32, f32) = (-162.42, 0.0, -154.21, 1.56);
pub const INN2_BED: (f32, f32, f32, f32) = (157.55, 0.0, 165.05, 1.53);
pub const INN3_BED: (f32, f32, f32, f32) = (-2.65, 0.0, 3.94, 1.52);

/// Map a player position to an inn-room code (1/2/3), or `0` for
/// "outside any room". `is_inn` is the zone's `is_inn` flag; if the
/// zone isn't an inn, every position resolves to `0` so callers can
/// unconditionally feed this into the `SetPlayerDreamPacket` inn-id
/// field.
pub fn inn_code_from_position(pos: (f32, f32, f32), is_inn: bool) -> u8 {
    if !is_inn {
        return 0;
    }
    let (x, _y, z) = pos;
    // Order mirrors Meteor exactly — room 3 (origin) is the last fallback
    // even though its centre is numerically smallest, because rooms 1/2
    // are geographically offset to the far corners.
    if distance_xz((x, z), INN3_CENTRE) <= INN_ROOM_RADIUS {
        3
    } else if distance_xz((x, z), INN2_CENTRE) <= INN_ROOM_RADIUS {
        2
    } else if distance_xz((x, z), INN1_CENTRE) <= INN_ROOM_RADIUS {
        1
    } else {
        0
    }
}

/// Canonical bed transform for an inn code. Returns `None` for code 0
/// (no inn room) so the caller can skip the snap.
pub fn sleeping_position_for_inn(inn_code: u8) -> Option<(f32, f32, f32, f32)> {
    match inn_code {
        1 => Some(INN1_BED),
        2 => Some(INN2_BED),
        3 => Some(INN3_BED),
        _ => None,
    }
}

fn distance_xz(a: (f32, f32), b: (f32, f32)) -> f32 {
    let dx = a.0 - b.0;
    let dz = a.1 - b.1;
    (dx * dx + dz * dz).sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn non_inn_zone_always_returns_zero() {
        assert_eq!(inn_code_from_position((0.0, 0.0, 0.0), false), 0);
        assert_eq!(inn_code_from_position((-160.0, 0.0, -160.0), false), 0);
    }

    #[test]
    fn origin_is_inn_code_three() {
        assert_eq!(inn_code_from_position((0.0, 0.0, 0.0), true), 3);
        assert_eq!(inn_code_from_position((5.0, 0.0, -5.0), true), 3);
    }

    #[test]
    fn far_corner_inns_resolve() {
        assert_eq!(inn_code_from_position((160.0, 0.0, 160.0), true), 2);
        assert_eq!(inn_code_from_position((155.0, 0.0, 162.0), true), 2);
        assert_eq!(inn_code_from_position((-160.0, 0.0, -160.0), true), 1);
        assert_eq!(inn_code_from_position((-162.0, 0.0, -158.0), true), 1);
    }

    #[test]
    fn far_from_any_room_returns_zero() {
        assert_eq!(inn_code_from_position((500.0, 0.0, 500.0), true), 0);
        // 50 units from origin — outside 20-unit radius.
        assert_eq!(inn_code_from_position((50.0, 0.0, 50.0), true), 0);
    }

    #[test]
    fn y_axis_is_ignored() {
        assert_eq!(inn_code_from_position((0.0, 100.0, 0.0), true), 3);
        assert_eq!(inn_code_from_position((0.0, -50.0, 0.0), true), 3);
    }

    #[test]
    fn radius_edge_is_inclusive() {
        assert_eq!(inn_code_from_position((20.0, 0.0, 0.0), true), 3);
    }

    #[test]
    fn sleeping_position_maps_each_room() {
        assert_eq!(sleeping_position_for_inn(1), Some(INN1_BED));
        assert_eq!(sleeping_position_for_inn(2), Some(INN2_BED));
        assert_eq!(sleeping_position_for_inn(3), Some(INN3_BED));
        assert!(sleeping_position_for_inn(0).is_none());
        assert!(sleeping_position_for_inn(42).is_none());
    }
}
