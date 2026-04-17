//! `PrivateArea` + `PrivateAreaContent`. Port of
//! `Actors/Area/PrivateArea.cs` and `PrivateAreaContent.cs`.
//!
//! A `PrivateArea` is an instanced sub-zone of its parent `Zone`. It
//! carries its own actor grid (inherited from `AreaCore`) but looks up
//! its parent for weather, region, and navmesh. `PrivateAreaContent`
//! adds a content-director handle + a finished flag + cleanup trigger.

#![allow(dead_code)]

use super::area::{ActorKind, AreaCore, AreaKind, StoredActor};
use super::outbox::{AreaEvent, AreaOutbox};
use super::spawn_location::SpawnLocation;

#[derive(Debug, Clone)]
pub struct PrivateArea {
    pub core: AreaCore,
    pub parent_zone_id: u32,
    pub private_area_name: String,
    pub private_area_level: u32,

    pub spawn_locations: Vec<SpawnLocation>,
}

impl PrivateArea {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        parent_zone_id: u32,
        zone_name: impl Into<String>,
        region_id: u16,
        actor_id: u32,
        class_path: impl Into<String>,
        private_area_name: impl Into<String>,
        private_area_level: u32,
        bgm_day: u16,
        bgm_night: u16,
        bgm_battle: u16,
        is_isolated: bool,
        is_inn: bool,
        can_ride_chocobo: bool,
        can_stealth: bool,
    ) -> Self {
        let zone_name: String = zone_name.into();
        Self {
            core: AreaCore::new(
                actor_id,
                zone_name,
                region_id,
                class_path,
                bgm_day,
                bgm_night,
                bgm_battle,
                is_isolated,
                is_inn,
                can_ride_chocobo,
                can_stealth,
                /* is_instance_raid */ true,
                AreaKind::PrivateArea,
            ),
            parent_zone_id,
            private_area_name: private_area_name.into(),
            private_area_level,
            spawn_locations: Vec::new(),
        }
    }

    pub fn add_spawn_location(&mut self, loc: SpawnLocation) {
        self.spawn_locations.push(loc);
    }

    /// Find an actor in this instance only. Use `Zone::find_actor_in_zone`
    /// when you want a cross-instance lookup.
    pub fn find_actor(&self, actor_id: u32) -> Option<StoredActor> {
        self.core.find_actor(actor_id)
    }
}

// ---------------------------------------------------------------------------
// PrivateAreaContent — dynamic content instance (guildleve, trial, …).
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct PrivateAreaContent {
    pub area: PrivateArea,
    /// Director actor id (0 when no director is attached).
    pub director_id: u32,
    pub starter_actor_id: u32,
    pub is_content_finished: bool,
}

impl PrivateAreaContent {
    pub fn new(
        area: PrivateArea,
        director_id: u32,
        starter_actor_id: u32,
    ) -> Self {
        Self {
            area,
            director_id,
            starter_actor_id,
            is_content_finished: false,
        }
    }

    pub fn mark_finished(&mut self) {
        self.is_content_finished = true;
    }

    /// Port of `CheckDestroy` — should the parent zone drop this content
    /// area from its registry? True when the content is flagged finished
    /// AND no players remain inside.
    pub fn should_destroy(&self) -> bool {
        if !self.is_content_finished {
            return false;
        }
        !self.area.core.iter().any(|a| a.kind == ActorKind::Player)
    }

    /// Emit a `ContentAreaDeleted` event — the parent Zone calls this
    /// right before removing the content from its list.
    pub fn emit_delete(&self, outbox: &mut AreaOutbox) {
        outbox.push(AreaEvent::ContentAreaDeleted {
            parent_area_id: self.area.parent_zone_id,
            area_name: self.area.private_area_name.clone(),
            private_area_type: self.area.private_area_level,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use common::Vector3;

    fn mk_private_area() -> PrivateArea {
        PrivateArea::new(
            1, "parent_zone", 1, 100, "/Area/Zone/TestContent", "private1", 1,
            0, 0, 0, false, false, false, false,
        )
    }

    #[test]
    fn private_area_tracks_parent_zone() {
        let pa = mk_private_area();
        assert_eq!(pa.parent_zone_id, 1);
        assert_eq!(pa.private_area_name, "private1");
        assert_eq!(pa.private_area_level, 1);
    }

    #[test]
    fn content_should_destroy_when_finished_and_empty() {
        let mut c = PrivateAreaContent::new(mk_private_area(), 0, 42);
        assert!(!c.should_destroy());
        c.mark_finished();
        assert!(c.should_destroy());
    }

    #[test]
    fn content_not_destroyed_while_players_present() {
        let mut c = PrivateAreaContent::new(mk_private_area(), 0, 42);
        let mut ob = AreaOutbox::new();
        c.area.core.add_actor(
            StoredActor {
                actor_id: 100,
                kind: ActorKind::Player,
                position: Vector3::ZERO,
                grid: (0, 0),
                is_alive: true,
            },
            &mut ob,
        );
        c.mark_finished();
        assert!(!c.should_destroy(), "players still inside");
    }

    #[test]
    fn add_spawn_location_accumulates() {
        let mut pa = mk_private_area();
        pa.add_spawn_location(SpawnLocation::new(
            1001, "npc_a", 100, "private1", 1, 0.0, 0.0, 0.0, 0.0, 0, 0,
        ));
        pa.add_spawn_location(SpawnLocation::new(
            1002, "npc_b", 100, "private1", 1, 5.0, 0.0, 5.0, 0.0, 0, 0,
        ));
        assert_eq!(pa.spawn_locations.len(), 2);
    }
}
