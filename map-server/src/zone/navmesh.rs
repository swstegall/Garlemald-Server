//! Navmesh integration. Port of `Map Server/utils/NavmeshUtils.cs`.
//!
//! The retail server uses SharpNav (a C# Recast/Detour port) to load
//! `.snb` navmeshes at zone boot time and answer `GetPath` / `CanSee`
//! queries from the battle system. The Rust port keeps that interface
//! pluggable: we define a `NavmeshLoader` trait and ship a stub
//! implementation. Later a real Recast/Detour crate can slot in without
//! touching the call sites.
//!
//! The battle system's `NavmeshProvider` trait already exists in
//! `crate::battle::path_find`; this module adds the *loader* side — the
//! loading + coordinate-transform layer that sits above a raw mesh.

#![allow(dead_code)]

use common::Vector3;

use crate::battle::path_find::NavmeshProvider;

/// Opaque handle returned by a loader. The stub just holds the zone name
/// so tests can assert it was queried; a real loader would wrap the
/// deserialized mesh data.
#[derive(Debug, Clone)]
pub struct NavmeshHandle {
    pub zone_name: String,
    /// Number of polygons in the loaded mesh, or 0 for the stub.
    pub poly_count: u32,
}

impl NavmeshHandle {
    pub fn stub(zone_name: impl Into<String>) -> Self {
        Self {
            zone_name: zone_name.into(),
            poly_count: 0,
        }
    }

    pub fn is_loaded(&self) -> bool {
        self.poly_count > 0
    }
}

/// Trait for anything that can load a zone's navmesh and serve queries
/// against it. Real implementations bind a Recast/Detour crate;
/// `StubNavmeshLoader` falls back to straight-line paths for testing.
pub trait NavmeshLoader: Send + Sync {
    /// Try to load `<zone_name>.snb` from the mesh directory. Returns
    /// `None` if the file is missing or malformed.
    fn load(&self, zone_name: &str) -> Option<NavmeshHandle>;

    /// Produce a `NavmeshProvider` implementation for the battle path
    /// finder. The provider may be the straight-line fallback if the
    /// loader can't produce a real mesh.
    fn provider(&self, handle: &NavmeshHandle) -> Box<dyn NavmeshProvider>;
}

/// Stub that never actually loads a mesh — every call returns a straight
/// line. Used in tests and for zones that don't yet have mesh files.
pub struct StubNavmeshLoader;

impl NavmeshLoader for StubNavmeshLoader {
    fn load(&self, zone_name: &str) -> Option<NavmeshHandle> {
        Some(NavmeshHandle::stub(zone_name))
    }

    fn provider(&self, _handle: &NavmeshHandle) -> Box<dyn NavmeshProvider> {
        Box::new(crate::battle::path_find::StraightLineNavmesh)
    }
}

/// Coordinate-transform helpers. The retail navmesh uses a different
/// up-axis convention than the game world; this matches
/// `NavmeshUtils.GamePosToNavmeshPos` / `NavmeshPosToGamePos` from the
/// C# — swaps Y and Z.
pub struct CoordTransform;

impl CoordTransform {
    pub fn game_to_navmesh(v: Vector3) -> Vector3 {
        Vector3::new(v.x, v.z, v.y)
    }

    pub fn navmesh_to_game(v: Vector3) -> Vector3 {
        Vector3::new(v.x, v.z, v.y)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stub_loader_returns_handle() {
        let loader = StubNavmeshLoader;
        let handle = loader.load("r1f1").unwrap();
        assert_eq!(handle.zone_name, "r1f1");
        assert!(!handle.is_loaded());
    }

    #[test]
    fn coord_transform_is_self_inverse() {
        let v = Vector3::new(1.0, 2.0, 3.0);
        let round = CoordTransform::navmesh_to_game(CoordTransform::game_to_navmesh(v));
        assert_eq!(round, v);
    }
}
