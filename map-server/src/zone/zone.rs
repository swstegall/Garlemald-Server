//! `Zone` — the top-level area type. Port of `Actors/Area/Zone.cs`.
//!
//! Compared to a plain `Area`:
//!
//! * Holds a map of `PrivateArea`s keyed by `(name, level)`.
//! * Holds a list of runtime `PrivateAreaContent` instances keyed by
//!   `area_name`.
//! * Owns a `NavmeshHandle` + exposes a `NavmeshProvider` for the battle
//!   path finder.
//! * `find_actor_in_zone` searches the zone root + all private areas
//!   + all content areas.

#![allow(dead_code)]

use std::collections::HashMap;

use super::area::{ActorKind, AreaCore, AreaKind, StoredActor};
use super::navmesh::{NavmeshHandle, NavmeshLoader, StubNavmeshLoader};
use super::outbox::{AreaEvent, AreaOutbox};
use super::private_area::{PrivateArea, PrivateAreaContent};
use super::spawn_location::SpawnLocation;

use crate::battle::path_find::NavmeshProvider;
use crate::battle::target_find::{ActorArena, ActorView};
use common::Vector3;

#[derive(Debug)]
pub struct Zone {
    pub core: AreaCore,
    /// `privateAreas[name][level] = PrivateArea`.
    pub private_areas: HashMap<String, HashMap<u32, PrivateArea>>,
    /// `contentAreas[area_name] = [content]`.
    pub content_areas: HashMap<String, Vec<PrivateAreaContent>>,

    /// Actors seeded at boot time from the spawn DB.
    pub spawn_locations: Vec<SpawnLocation>,

    pub navmesh: Option<NavmeshHandle>,
    /// Pathing-call telemetry (matches the C# `pathCalls`/`pathCallTime`).
    pub path_calls: u64,
    pub path_call_time_ms: u64,
}

impl Zone {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        actor_id: u32,
        zone_name: impl Into<String>,
        region_id: u16,
        class_path: impl Into<String>,
        bgm_day: u16,
        bgm_night: u16,
        bgm_battle: u16,
        is_isolated: bool,
        is_inn: bool,
        can_ride_chocobo: bool,
        can_stealth: bool,
        is_instance_raid: bool,
        navmesh_loader: Option<&dyn NavmeshLoader>,
    ) -> Self {
        let zone_name: String = zone_name.into();
        let navmesh = navmesh_loader.and_then(|l| l.load(&zone_name));
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
                is_instance_raid,
                AreaKind::Zone,
            ),
            private_areas: HashMap::new(),
            content_areas: HashMap::new(),
            spawn_locations: Vec::new(),
            navmesh,
            path_calls: 0,
            path_call_time_ms: 0,
        }
    }

    // -----------------------------------------------------------------
    // Private-area registry
    // -----------------------------------------------------------------

    pub fn add_private_area(&mut self, area: PrivateArea) {
        let name = area.private_area_name.clone();
        let level = area.private_area_level;
        self.private_areas
            .entry(name)
            .or_default()
            .insert(level, area);
    }

    pub fn get_private_area(&self, name: &str, level: u32) -> Option<&PrivateArea> {
        self.private_areas.get(name).and_then(|m| m.get(&level))
    }

    pub fn get_private_area_mut(&mut self, name: &str, level: u32) -> Option<&mut PrivateArea> {
        self.private_areas
            .get_mut(name)
            .and_then(|m| m.get_mut(&level))
    }

    // -----------------------------------------------------------------
    // Content areas (dynamic instances)
    // -----------------------------------------------------------------

    pub fn create_content_area(
        &mut self,
        class_path: impl Into<String>,
        area_name: impl Into<String>,
        director_id: u32,
        starter_actor_id: u32,
        outbox: &mut AreaOutbox,
    ) -> usize {
        let area_name: String = area_name.into();
        let pa = PrivateArea::new(
            self.core.actor_id,
            self.core.zone_name.clone(),
            self.core.region_id,
            self.core.actor_id,
            class_path,
            area_name.clone(),
            1,
            0,
            0,
            0,
            self.core.is_isolated,
            self.core.is_inn,
            self.core.can_ride_chocobo,
            self.core.can_stealth,
        );
        let content = PrivateAreaContent::new(pa, director_id, starter_actor_id);
        outbox.push(AreaEvent::ContentAreaCreated {
            parent_area_id: self.core.actor_id,
            area_name: area_name.clone(),
            private_area_type: 1,
            starter_actor_id,
        });
        let list = self.content_areas.entry(area_name).or_default();
        list.push(content);
        list.len() - 1
    }

    pub fn delete_content_area(
        &mut self,
        area_name: &str,
        index: usize,
        outbox: &mut AreaOutbox,
    ) -> bool {
        let Some(list) = self.content_areas.get_mut(area_name) else {
            return false;
        };
        if index >= list.len() {
            return false;
        }
        let removed = list.remove(index);
        removed.emit_delete(outbox);
        if list.is_empty() {
            self.content_areas.remove(area_name);
        }
        true
    }

    /// Walk every content area and drop the ones whose content finished
    /// and has no players. Mirrors the periodic `CheckDestroy` sweep.
    pub fn sweep_finished_content(&mut self, outbox: &mut AreaOutbox) {
        let names: Vec<String> = self.content_areas.keys().cloned().collect();
        for name in names {
            let Some(list) = self.content_areas.get_mut(&name) else {
                continue;
            };
            let mut i = 0;
            while i < list.len() {
                if list[i].should_destroy() {
                    let removed = list.remove(i);
                    removed.emit_delete(outbox);
                } else {
                    i += 1;
                }
            }
            if let Some(list) = self.content_areas.get(&name)
                && list.is_empty()
            {
                self.content_areas.remove(&name);
            }
        }
    }

    // -----------------------------------------------------------------
    // Spawn seeds
    // -----------------------------------------------------------------

    /// `AddSpawnLocation(spawn)` — routes to the zone root or into the
    /// right private area if the seed names one. Returns `Err` if the
    /// seed points at a missing private area.
    pub fn add_spawn_location(&mut self, spawn: SpawnLocation) -> Result<(), String> {
        if spawn.is_in_private_area() {
            let name = spawn.private_area_name.clone();
            let level = spawn.private_area_level;
            let Some(pa) = self.get_private_area_mut(&name, level) else {
                return Err(format!(
                    "private area '{}' level {} missing in zone '{}'",
                    name, level, self.core.zone_name
                ));
            };
            pa.add_spawn_location(spawn);
        } else {
            self.spawn_locations.push(spawn);
        }
        Ok(())
    }

    /// Iterate every spawn seed — root + every private area. Used by
    /// the game loop to construct real NPCs at boot time.
    pub fn all_spawn_locations(&self) -> impl Iterator<Item = (AreaKind, u32, &SpawnLocation)> {
        let root = self
            .spawn_locations
            .iter()
            .map(|s| (AreaKind::Zone, self.core.actor_id, s));
        let private = self.private_areas.values().flat_map(|levels| {
            levels.values().flat_map(|pa| {
                let id = pa.core.actor_id;
                pa.spawn_locations
                    .iter()
                    .map(move |s| (AreaKind::PrivateArea, id, s))
            })
        });
        root.chain(private)
    }

    // -----------------------------------------------------------------
    // Cross-area lookups
    // -----------------------------------------------------------------

    /// `FindActorInZone` — search zone root, every private area, and
    /// every content area.
    pub fn find_actor_in_zone(&self, actor_id: u32) -> Option<StoredActor> {
        if let Some(a) = self.core.find_actor(actor_id) {
            return Some(a);
        }
        for levels in self.private_areas.values() {
            for pa in levels.values() {
                if let Some(a) = pa.find_actor(actor_id) {
                    return Some(a);
                }
            }
        }
        for list in self.content_areas.values() {
            for ca in list {
                if let Some(a) = ca.area.find_actor(actor_id) {
                    return Some(a);
                }
            }
        }
        None
    }

    // -----------------------------------------------------------------
    // Navmesh hand-off
    // -----------------------------------------------------------------

    pub fn navmesh_provider<'a>(
        &'a self,
        loader: &'a dyn NavmeshLoader,
    ) -> Box<dyn NavmeshProvider> {
        let Some(handle) = self.navmesh.as_ref() else {
            return Box::new(crate::battle::path_find::StraightLineNavmesh);
        };
        loader.provider(handle)
    }

    /// Convenience — a zone with no mesh yet.
    pub fn with_stub_navmesh(mut self) -> Self {
        self.navmesh = Some(NavmeshHandle::stub(self.core.zone_name.clone()));
        let _ = &StubNavmeshLoader;
        self
    }

    // -----------------------------------------------------------------
    // Path telemetry (matches C# Zone.Update spam)
    // -----------------------------------------------------------------

    pub fn record_path_call(&mut self, elapsed_ms: u64) {
        self.path_calls += 1;
        self.path_call_time_ms += elapsed_ms;
    }
}

// ---------------------------------------------------------------------------
// battle::ActorArena adapter — the whole point of the port. The battle
// system's TargetFind + Controller can now query a live Zone to resolve
// "who's around me" using the 50-yalm spatial grid.
// ---------------------------------------------------------------------------

impl ActorArena for Zone {
    fn get(&self, actor_id: u32) -> Option<ActorView> {
        self.find_actor_in_zone(actor_id)
            .map(|a| stored_to_view(a, self.core.actor_id))
    }

    fn actors_around(&self, center: u32, radius: f32) -> Vec<ActorView> {
        self.core
            .actors_around(center, radius)
            .into_iter()
            .map(|a| stored_to_view(a, self.core.actor_id))
            .collect()
    }
}

fn stored_to_view(a: StoredActor, zone_id: u32) -> ActorView {
    ActorView {
        actor_id: a.actor_id,
        position: a.position,
        rotation: 0.0,
        is_alive: a.is_alive,
        is_static: matches!(a.kind, ActorKind::Npc),
        // Allegiance heuristic: players = 1, battle npcs/pets = 2, allies
        // = 3, static/other = 0. Battle code treats "same allegiance" as
        // "ally" and "different" as "enemy".
        allegiance: match a.kind {
            ActorKind::Player => 1,
            ActorKind::Npc | ActorKind::Other => 0,
            ActorKind::BattleNpc | ActorKind::Pet => 2,
            ActorKind::Ally => 3,
        },
        party_id: 0,
        zone_id,
        is_updates_locked: false,
        is_player: a.kind == ActorKind::Player,
        is_battle_npc: a.kind == ActorKind::BattleNpc,
    }
}

// Shut up the unused `Vector3` import when the file only uses it
// through `ActorView`.
#[allow(dead_code)]
fn _assert_vector3_used() -> Vector3 {
    Vector3::ZERO
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn mk_zone() -> Zone {
        Zone::new(
            100,
            "r1f1",
            1,
            "/Area/Zone/R1F1",
            0,
            0,
            0,
            false,
            false,
            false,
            false,
            false,
            Some(&StubNavmeshLoader),
        )
    }

    #[test]
    fn add_private_area_keyed_by_name_and_level() {
        let mut zone = mk_zone();
        let pa = PrivateArea::new(
            100,
            "r1f1",
            1,
            200,
            "/Area/Zone/R1F1/Private",
            "office",
            1,
            0,
            0,
            0,
            false,
            false,
            false,
            false,
        );
        zone.add_private_area(pa);
        assert!(zone.get_private_area("office", 1).is_some());
        assert!(zone.get_private_area("office", 2).is_none());
    }

    #[test]
    fn add_spawn_location_routes_to_private_area() {
        let mut zone = mk_zone();
        zone.add_private_area(PrivateArea::new(
            100,
            "r1f1",
            1,
            200,
            "/Area/Zone/R1F1/Private",
            "office",
            1,
            0,
            0,
            0,
            false,
            false,
            false,
            false,
        ));
        zone.add_spawn_location(SpawnLocation::new(
            1001,
            "aetheryte",
            100,
            "office",
            1,
            0.0,
            0.0,
            0.0,
            0.0,
            0,
            0,
        ))
        .unwrap();
        assert_eq!(zone.spawn_locations.len(), 0);
        assert_eq!(
            zone.get_private_area("office", 1)
                .unwrap()
                .spawn_locations
                .len(),
            1
        );
    }

    #[test]
    fn add_spawn_location_errors_on_missing_private_area() {
        let mut zone = mk_zone();
        let err = zone
            .add_spawn_location(SpawnLocation::new(
                1001,
                "ghost",
                100,
                "does_not_exist",
                1,
                0.0,
                0.0,
                0.0,
                0.0,
                0,
                0,
            ))
            .unwrap_err();
        assert!(err.contains("does_not_exist"));
    }

    #[test]
    fn find_actor_searches_across_private_areas() {
        let mut zone = mk_zone();
        let mut pa = PrivateArea::new(
            100,
            "r1f1",
            1,
            200,
            "/Area/Zone/R1F1/Private",
            "office",
            1,
            0,
            0,
            0,
            false,
            false,
            false,
            false,
        );
        let mut ob = AreaOutbox::new();
        pa.core.add_actor(
            StoredActor {
                actor_id: 500,
                kind: ActorKind::Npc,
                position: Vector3::ZERO,
                grid: (0, 0),
                is_alive: true,
            },
            &mut ob,
        );
        zone.add_private_area(pa);
        assert!(zone.find_actor_in_zone(500).is_some());
    }

    #[test]
    fn create_content_area_returns_index_and_emits() {
        let mut zone = mk_zone();
        let mut ob = AreaOutbox::new();
        let idx = zone.create_content_area("/Area/Content/Guildleve1", "gl1", 0, 42, &mut ob);
        assert_eq!(idx, 0);
        assert_eq!(zone.content_areas.get("gl1").unwrap().len(), 1);
        assert!(matches!(ob.events[0], AreaEvent::ContentAreaCreated { .. }));
    }

    #[test]
    fn sweep_finished_content_drops_empty_finished_instances() {
        let mut zone = mk_zone();
        let mut ob = AreaOutbox::new();
        zone.create_content_area("/Area/Content/Gl1", "gl1", 0, 42, &mut ob);
        ob.drain();

        // Mark finished with no players inside — sweep drops it.
        zone.content_areas.get_mut("gl1").unwrap()[0].mark_finished();
        zone.sweep_finished_content(&mut ob);
        assert!(!zone.content_areas.contains_key("gl1"));
        assert!(
            ob.events
                .iter()
                .any(|e| matches!(e, AreaEvent::ContentAreaDeleted { .. }))
        );
    }

    #[test]
    fn actor_arena_trait_answers_range_query() {
        let mut zone = mk_zone();
        let mut ob = AreaOutbox::new();
        zone.core.add_actor(
            StoredActor {
                actor_id: 1,
                kind: ActorKind::Player,
                position: Vector3::ZERO,
                grid: (0, 0),
                is_alive: true,
            },
            &mut ob,
        );
        zone.core.add_actor(
            StoredActor {
                actor_id: 2,
                kind: ActorKind::BattleNpc,
                position: Vector3::new(5.0, 0.0, 5.0),
                grid: (0, 0),
                is_alive: true,
            },
            &mut ob,
        );
        // Use the trait impl directly.
        let near = ActorArena::actors_around(&zone, 1, 20.0);
        let ids: Vec<u32> = near.iter().map(|v| v.actor_id).collect();
        assert!(ids.contains(&2));
    }

    #[test]
    fn battle_npc_controller_aggros_player_via_live_zone() {
        use crate::battle::controller::{
            BattleNpcController, ControllerDecision, ControllerOwnerView, DetectionType,
        };

        let mut zone = mk_zone();
        let mut ob = AreaOutbox::new();

        // Seed the BattleNpc in the grid...
        zone.core.add_actor(
            StoredActor {
                actor_id: 1,
                kind: ActorKind::BattleNpc,
                position: Vector3::new(0.0, 0.0, 0.0),
                grid: (0, 0),
                is_alive: true,
            },
            &mut ob,
        );
        // ...and a Player nearby (different allegiance from the mob).
        zone.core.add_actor(
            StoredActor {
                actor_id: 10,
                kind: ActorKind::Player,
                position: Vector3::new(3.0, 0.0, 0.0),
                grid: (0, 0),
                is_alive: true,
            },
            &mut ob,
        );
        ob.drain();

        let mut controller = BattleNpcController::new_for(1);
        controller.battle.detection_type = DetectionType::SIGHT;
        controller.battle.sight_range = 50.0;

        let owner_view = ControllerOwnerView {
            actor: zone_actor_view(&zone, 1),
            is_engaged: false,
            is_spawned: true,
            is_following_path: false,
            at_path_end: true,
            most_hated_actor_id: None,
            current_target_actor_id: None,
            has_prevent_movement: false,
            max_hp: 1000,
            current_hp: 1000,
            target_hpp: None,
            target_has_stealth: false,
            is_close_to_spawn: true,
            target_is_locked: false,
        };

        let decision = controller.tick(1_000, owner_view, &zone);
        match decision {
            ControllerDecision::Engage { target_actor_id } => assert_eq!(target_actor_id, 10),
            other => panic!("expected Engage, got {:?}", other),
        }
    }

    fn zone_actor_view(zone: &Zone, id: u32) -> ActorView {
        ActorArena::get(zone, id).expect("actor present")
    }

    #[test]
    fn all_spawn_locations_iterates_root_and_private() {
        let mut zone = mk_zone();
        zone.add_private_area(PrivateArea::new(
            100,
            "r1f1",
            1,
            200,
            "/Area/Zone/R1F1/Private",
            "office",
            1,
            0,
            0,
            0,
            false,
            false,
            false,
            false,
        ));
        zone.add_spawn_location(SpawnLocation::new(
            1001,
            "aetheryte",
            100,
            "",
            0,
            0.0,
            0.0,
            0.0,
            0.0,
            0,
            0,
        ))
        .unwrap();
        zone.add_spawn_location(SpawnLocation::new(
            1002, "clerk", 100, "office", 1, 0.0, 0.0, 0.0, 0.0, 0, 0,
        ))
        .unwrap();
        let all: Vec<_> = zone.all_spawn_locations().collect();
        assert_eq!(all.len(), 2);
    }
}
