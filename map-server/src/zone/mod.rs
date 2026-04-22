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

//! Zone / Area runtime. Port of `Map Server/Actors/Area/*`.
//!
//! Layout:
//!
//! * `Area` — the spatial-grid container all zone-like actors derive from.
//! * `Zone` — `Area` + navmesh + private-area/content-area bookkeeping.
//! * `PrivateArea` + `PrivateAreaContent` — instanced sub-zones.
//! * `SpawnLocation` — pure data struct describing a single NPC seed.
//!
//! The C# has an inheritance hierarchy (`PrivateArea : Area`,
//! `PrivateAreaContent : PrivateArea`, `Zone : Area`). Rust doesn't do
//! inheritance, so we model the shared fields as a `AreaCore` struct that
//! `Zone`/`PrivateArea` compose. Behaviour that diverges (`FindActor…`,
//! `CreateScriptBindPacket`) becomes methods on the enclosing type.

#![allow(dead_code, unused_imports, clippy::module_inception)]

pub mod area;
pub mod navmesh;
pub mod outbox;
pub mod private_area;
pub mod spawn_location;
pub mod zone;

pub use area::{
    AREA_MAX, AREA_MIN, ActorKind, Area, AreaCore, AreaKind, BOUNDING_GRID_SIZE, StoredActor,
};
pub use navmesh::{CoordTransform, NavmeshHandle, NavmeshLoader, StubNavmeshLoader};
pub use outbox::{AreaEvent, AreaOutbox};
pub use private_area::{PrivateArea, PrivateAreaContent};
pub use spawn_location::SpawnLocation;
pub use zone::Zone;
