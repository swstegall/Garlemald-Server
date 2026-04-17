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

pub use area::{Area, AreaCore, AreaKind, ActorKind, StoredActor, BOUNDING_GRID_SIZE, AREA_MIN, AREA_MAX};
pub use navmesh::{CoordTransform, NavmeshHandle, NavmeshLoader, StubNavmeshLoader};
pub use outbox::{AreaEvent, AreaOutbox};
pub use private_area::{PrivateArea, PrivateAreaContent};
pub use spawn_location::SpawnLocation;
pub use zone::Zone;
