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

//! Events emitted by achievement + title mutations.

#![allow(dead_code)]

#[derive(Debug, Clone)]
pub enum AchievementEvent {
    /// Pop the in-game achievement-earned toast + add to the DB.
    Earned {
        player_actor_id: u32,
        achievement_id: u32,
    },
    /// Post-earn state sync: tell the client the new points total.
    SetPoints { player_actor_id: u32, points: u32 },
    /// Post-earn state sync: tell the client the 5 most recent ids.
    SetLatest {
        player_actor_id: u32,
        latest_ids: [u32; 5],
    },
    /// Bulk state send — on zone-in. `bits` is indexed by achievement
    /// id; `bits[id] == true` means the player has earned it.
    SetCompleted {
        player_actor_id: u32,
        bits: Vec<bool>,
    },
    /// Progress response to the client's 0x0135 request.
    SendRate {
        player_actor_id: u32,
        achievement_id: u32,
        progress_count: u32,
        progress_flags: u32,
    },
    /// Equip / clear the player's current title (0x019D).
    SetPlayerTitle { player_actor_id: u32, title_id: u32 },
}

#[derive(Debug, Default)]
pub struct AchievementOutbox {
    pub events: Vec<AchievementEvent>,
}

impl AchievementOutbox {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push(&mut self, event: AchievementEvent) {
        self.events.push(event);
    }

    pub fn drain(&mut self) -> Vec<AchievementEvent> {
        std::mem::take(&mut self.events)
    }

    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }

    pub fn len(&self) -> usize {
        self.events.len()
    }
}
