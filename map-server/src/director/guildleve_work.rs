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

//! `GuildleveWork` — per-leve transient state. Port of
//! `Actors/Director/Work/GuildleveWork.cs`.

#![allow(dead_code)]

#[derive(Debug, Clone, Default)]
pub struct GuildleveWork {
    /// Unix timestamp when the leve started (`0` before start).
    pub start_time: u32,
    /// Target counts for up to four parallel objectives. Copied from
    /// `GuildleveGamedata.aim_num[4]` on construction.
    pub aim_num: [i8; 4],
    /// Running progress per objective.
    pub aim_num_now: [i8; 4],
    /// UI visibility per objective (0 = hidden, 1 = shown).
    pub ui_state: [i8; 4],
    /// Three waypoint markers on the map.
    pub marker_x: [f32; 3],
    pub marker_y: [f32; 3],
    pub marker_z: [f32; 3],
    /// Completion signal (`-1` on end).
    pub signal: i8,
}

impl GuildleveWork {
    pub fn new() -> Self {
        Self::default()
    }

    /// Seed from `GuildleveGamedata.aim_num[4]`. Objectives with a
    /// non-zero target are visible by default (matches the C# ctor).
    pub fn reset_from_gamedata(&mut self, aim_num: [i8; 4]) {
        self.aim_num = aim_num;
        self.aim_num_now = [0; 4];
        self.ui_state = [0; 4];
        for (i, target) in aim_num.iter().enumerate() {
            if *target != 0 {
                self.ui_state[i] = 1;
            }
        }
        self.marker_x = [0.0; 3];
        self.marker_y = [0.0; 3];
        self.marker_z = [0.0; 3];
        self.signal = 0;
    }

    pub fn clear_on_end(&mut self) {
        self.start_time = 0;
        self.signal = -1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reset_visibility_follows_target_count() {
        let mut w = GuildleveWork::new();
        w.reset_from_gamedata([3, 0, 5, 0]);
        assert_eq!(w.aim_num, [3, 0, 5, 0]);
        assert_eq!(w.ui_state, [1, 0, 1, 0]);
    }

    #[test]
    fn clear_on_end_signals_negative() {
        let mut w = GuildleveWork::new();
        w.start_time = 100;
        w.clear_on_end();
        assert_eq!(w.start_time, 0);
        assert_eq!(w.signal, -1);
    }
}
