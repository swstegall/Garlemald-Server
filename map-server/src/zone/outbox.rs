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

//! Events emitted by Area / Zone mutations. Same pattern as
//! inventory / status / battle — mutations record intent, the game
//! loop drains and turns events into packets, DB writes, Lua calls.

#![allow(dead_code)]

#[derive(Debug, Clone)]
pub enum AreaEvent {
    /// An actor joined this area (AddActorToZone).
    ActorAdded {
        area_id: u32,
        actor_id: u32,
    },
    /// An actor left this area (RemoveActorFromZone).
    ActorRemoved {
        area_id: u32,
        actor_id: u32,
    },
    /// Actor crossed a grid cell boundary — spawn/despawn packets may be
    /// needed for players whose visibility set changed.
    ActorMoved {
        area_id: u32,
        actor_id: u32,
        old_grid: (i32, i32),
        new_grid: (i32, i32),
    },

    /// `BroadcastPacketAroundActor` — the game loop fans a packet out to
    /// every player within 50 yalms of `source_actor_id`. Payload is
    /// opaque — the caller encodes the SubPacket into `payload`.
    BroadcastAroundActor {
        area_id: u32,
        source_actor_id: u32,
        check_distance: f32,
        opcode: u16,
        payload: Vec<u8>,
    },

    /// `ChangeWeather(weather, transitionTime)` — emit a `SetWeatherPacket`
    /// to the target player (or zone-wide when `zone_wide=true`).
    WeatherChange {
        area_id: u32,
        weather_id: u16,
        transition_time: u16,
        target_actor_id: Option<u32>,
        zone_wide: bool,
    },

    /// `CreateDirector` fired — game loop instantiates a Director Lua
    /// context with the given classpath.
    DirectorCreated {
        area_id: u32,
        director_id: u32,
        class_path: String,
    },
    DirectorDeleted {
        area_id: u32,
        director_id: u32,
    },

    /// Content area created / destroyed (PrivateAreaContent lifecycle).
    ContentAreaCreated {
        parent_area_id: u32,
        area_name: String,
        private_area_type: u32,
        starter_actor_id: u32,
    },
    ContentAreaDeleted {
        parent_area_id: u32,
        area_name: String,
        private_area_type: u32,
    },

    /// An actor spawned from a SpawnLocation seed.
    SpawnActor {
        area_id: u32,
        actor_id: u32,
        class_id: u32,
        unique_id: String,
    },
}

#[derive(Debug, Default)]
pub struct AreaOutbox {
    pub events: Vec<AreaEvent>,
}

impl AreaOutbox {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn push(&mut self, event: AreaEvent) {
        self.events.push(event);
    }
    pub fn drain(&mut self) -> Vec<AreaEvent> {
        std::mem::take(&mut self.events)
    }
    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }
    pub fn len(&self) -> usize {
        self.events.len()
    }
}
