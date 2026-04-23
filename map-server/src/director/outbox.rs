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

//! Events emitted by director mutations. Drained by the game-loop
//! dispatcher and turned into packet sends, DB writes, and Lua calls.

#![allow(dead_code)]

#[derive(Debug, Clone)]
pub enum DirectorEvent {
    // ---- Lifecycle ----------------------------------------------------
    /// Director has been constructed + `init()` succeeded. Spawn its
    /// actor + init packets to all current player members.
    DirectorStarted {
        director_id: u32,
        zone_id: u32,
        class_path: String,
        class_name: String,
        actor_name: String,
        spawn_immediate: bool,
    },
    /// Director cleanup — broadcast `RemoveActorPacket` to members,
    /// delete the content group, purge from zone registry.
    DirectorEnded {
        director_id: u32,
        zone_id: u32,
    },

    /// `main(director, contentGroup)` coroutine should start.
    MainCoroutine {
        director_id: u32,
    },
    /// Script-driven `onEventStarted` from the player or director.
    EventStarted {
        director_id: u32,
        player_actor_id: Option<u32>,
    },

    // ---- Membership ---------------------------------------------------
    MemberAdded {
        director_id: u32,
        actor_id: u32,
        is_player: bool,
    },
    MemberRemoved {
        director_id: u32,
        actor_id: u32,
        is_player: bool,
    },

    // ---- Guildleve progress ------------------------------------------
    /// Start a guildleve — emit music/text/property packets to all
    /// player members.
    GuildleveStarted {
        director_id: u32,
        guildleve_id: u32,
        difficulty: u8,
        location: u32,
        time_limit_seconds: u32,
        start_time_unix: u32,
    },
    /// End a guildleve. `was_completed=true` triggers the victory music +
    /// reward messages; `false` is a time-out.
    GuildleveEnded {
        director_id: u32,
        guildleve_id: u32,
        was_completed: bool,
        completion_time_seconds: u32,
        /// Star-rating difficulty of the leve (1..=5). Threaded onto
        /// the event so the dispatcher can size the GC seal reward
        /// without re-reading the director state.
        difficulty: u8,
    },
    GuildleveAbandoned {
        director_id: u32,
        guildleve_id: u32,
    },
    /// Property update: `guildleveWork.aimNumNow[i] = value`.
    GuildleveAimUpdated {
        director_id: u32,
        index: u8,
        value: i8,
    },
    /// Property update: `guildleveWork.uiState[i] = value`.
    GuildleveUiUpdated {
        director_id: u32,
        index: u8,
        value: i8,
    },
    /// Property update: marker `[i]` moved to `(x,y,z)`.
    GuildleveMarkerUpdated {
        director_id: u32,
        index: u8,
        x: f32,
        y: f32,
        z: f32,
    },
    /// Full property resync — used on zone-in or after big state changes.
    GuildleveSyncAll {
        director_id: u32,
    },
}

#[derive(Debug, Default)]
pub struct DirectorOutbox {
    pub events: Vec<DirectorEvent>,
}

impl DirectorOutbox {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push(&mut self, event: DirectorEvent) {
        self.events.push(event);
    }

    pub fn drain(&mut self) -> Vec<DirectorEvent> {
        std::mem::take(&mut self.events)
    }

    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }

    pub fn len(&self) -> usize {
        self.events.len()
    }
}
