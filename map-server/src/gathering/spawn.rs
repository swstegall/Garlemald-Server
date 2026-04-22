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

//! World-placement record for gathering nodes. Parallel to
//! [`crate::zone::SpawnLocation`] for BattleNpcs/ENPCs, but carries the
//! `harvest_node_id` + `harvest_type` fields the minigame needs to
//! resolve a physical node back to its template pool.

#![allow(dead_code)]

/// `!mine` / Quarry — ore, clay, salt outcrops.
pub const HARVEST_TYPE_MINE: u32 = 22002;
/// `!log` — tree stumps, bushes.
pub const HARVEST_TYPE_LOG: u32 = 22003;
/// `!fish` — schools of fish, squid beds, coral.
pub const HARVEST_TYPE_FISH: u32 = 22004;

pub fn is_valid_harvest_type(ty: u32) -> bool {
    matches!(ty, HARVEST_TYPE_MINE | HARVEST_TYPE_LOG | HARVEST_TYPE_FISH)
}

#[derive(Debug, Clone, PartialEq)]
pub struct GatherNodeSpawn {
    pub id: u32,
    pub actor_class_id: u32,
    pub unique_id: String,
    pub zone_id: u32,
    pub private_area_name: String,
    pub private_area_level: i32,
    pub position: (f32, f32, f32),
    pub rotation: f32,
    /// FK into `gamedata_gather_nodes.id` — selects the template
    /// (grade/attempts/item pool) the DummyCommand minigame will run
    /// against this physical placement.
    pub harvest_node_id: u32,
    /// Which harvest command opens the minigame. See
    /// [`HARVEST_TYPE_MINE`] / [`HARVEST_TYPE_LOG`] / [`HARVEST_TYPE_FISH`].
    pub harvest_type: u32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn harvest_type_validation_accepts_three_commands() {
        assert!(is_valid_harvest_type(HARVEST_TYPE_MINE));
        assert!(is_valid_harvest_type(HARVEST_TYPE_LOG));
        assert!(is_valid_harvest_type(HARVEST_TYPE_FISH));
        assert!(!is_valid_harvest_type(0));
        assert!(!is_valid_harvest_type(22001));
        assert!(!is_valid_harvest_type(22005));
    }
}
