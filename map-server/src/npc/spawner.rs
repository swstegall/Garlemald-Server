//! Boot-time spawn pipeline. Port of the portions of
//! `WorldManager.SpawnAllActors` + `Area.SpawnActor` that instantiate
//! live actors from the database seeds.
//!
//! Flow:
//!
//! 1. Each Zone has a list of `SpawnLocation` seeds (populated by
//!    `Database::load_npc_spawn_locations` + `Zone::add_spawn_location`).
//! 2. `spawn_all_actors` walks every zone, looks up the `ActorClass` for
//!    each seed, instantiates an `Npc` (or a `BattleNpc` when the class
//!    flags request it), inserts a `StoredActor` projection into the
//!    zone's spatial grid, and registers an `ActorHandle` in the
//!    global `ActorRegistry`.
//! 3. The game-loop ticker then drives each actor's
//!    `StatusEffectContainer::update` + `AIContainer::update` per frame.

#![allow(dead_code)]

use std::collections::HashMap;
use std::sync::Arc;

use crate::runtime::actor_registry::{ActorHandle, ActorKindTag, ActorRegistry};
use crate::world_manager::WorldManager;
use crate::zone::area::{ActorKind, StoredActor};
use crate::zone::spawn_location::SpawnLocation;
use common::Vector3;

use super::actor_class::ActorClass;
use super::battle_npc::BattleNpc;
use super::npc::Npc;

/// Read-only context held while spawning. The `WorldManager` is here so
/// we can drop actor projections into the right zone's spatial grid,
/// and the `ActorRegistry` so the game loop picks them up.
pub struct SpawnContext<'a> {
    pub world: &'a WorldManager,
    pub registry: &'a ActorRegistry,
    pub actor_classes: &'a HashMap<u32, ActorClass>,
    /// Class ids in this set are treated as combat mobs (routed through
    /// `BattleNpc::new`). Anything else becomes a plain `Npc`.
    pub battle_class_ids: &'a std::collections::HashSet<u32>,
    /// Per-actor-class appearance rows from `gamedata_actor_appearance`.
    /// Used by `spawn_one` to stamp the 0x00D6 model_id + appearance
    /// table onto each NPC at spawn time. Empty map = no appearance
    /// data loaded (Phase-2 server flag); NPCs then spawn with
    /// model_id=0 and the client's DepictionJudge falls back to the
    /// actor-class default (but populace avatars won't look right).
    pub npc_appearances: &'a HashMap<u32, crate::database::NpcAppearance>,
}

impl SpawnContext<'_> {
    /// Instantiate every seeded actor across every loaded zone. Returns
    /// the list of newly-spawned actor ids.
    pub async fn spawn_all_actors(&self) -> Vec<u32> {
        let mut spawned = Vec::new();
        let zone_ids = self.world.zone_ids().await;
        for zone_id in zone_ids {
            let ids = self.spawn_zone(zone_id).await;
            spawned.extend(ids);
        }
        spawned
    }

    /// Walk the seeds for one zone + its private areas. Each seed turns
    /// into a live `Npc` or `BattleNpc` registered in the game loop.
    pub async fn spawn_zone(&self, zone_id: u32) -> Vec<u32> {
        let Some(zone_arc) = self.world.zone(zone_id).await else {
            return Vec::new();
        };

        let mut spawned = Vec::new();

        // Snapshot the seed list + an initial actor-number counter. We
        // hold the zone's write lock only for grid inserts so the spawn
        // call itself (which needs other locks) doesn't deadlock.
        let (seeds, mut next_number) = {
            let z = zone_arc.read().await;
            let seeds: Vec<SpawnLocation> = z.spawn_locations.clone();
            let counter = z.core.actor_count() as u32 + 1;
            (seeds, counter)
        };

        for seed in seeds {
            let Some(class) = self.actor_classes.get(&seed.class_id) else {
                continue;
            };
            let actor_number = next_number;
            next_number += 1;
            let is_battle = self.battle_class_ids.contains(&seed.class_id);
            let actor_id = spawn_one(
                self,
                &zone_arc,
                zone_id,
                actor_number,
                class,
                &seed,
                is_battle,
            )
            .await;
            if let Some(id) = actor_id {
                spawned.push(id);
            }
        }
        spawned
    }
}

async fn spawn_one(
    ctx: &SpawnContext<'_>,
    zone_arc: &Arc<tokio::sync::RwLock<crate::zone::Zone>>,
    zone_id: u32,
    actor_number: u32,
    class: &ActorClass,
    seed: &SpawnLocation,
    is_battle: bool,
) -> Option<u32> {
    // 1. Build the actor-specific struct.
    let (actor_id, character, kind_tag, grid_kind) = if is_battle {
        let bnpc = BattleNpc::new(
            actor_number,
            class,
            seed.unique_id.clone(),
            zone_id,
            seed.x,
            seed.y,
            seed.z,
            seed.rotation,
            seed.state,
            seed.animation_id,
            None,
        );
        let id = bnpc.actor_id();
        (
            id,
            bnpc.npc.character,
            ActorKindTag::BattleNpc,
            ActorKind::BattleNpc,
        )
    } else {
        let mut npc = Npc::new(
            actor_number,
            class,
            seed.unique_id.clone(),
            zone_id,
            seed.x,
            seed.y,
            seed.z,
            seed.rotation,
            seed.state,
            seed.animation_id,
            None,
        );
        // Stamp per-actor-class model_id + packed appearance slots
        // from `gamedata_actor_appearance`. Populace NPCs ship with
        // real model/gear ids in that table (e.g. class 1000438 →
        // modelId=1, body=1091, legs=9248). Without this the 0x00D6
        // goes out all zeros and the Wine client derefs nil on the
        // model-lookup path.
        if let Some(app) = ctx.npc_appearances.get(&class.actor_class_id) {
            let (model_id, slots) = app.pack();
            npc.character.chara.model_id = model_id;
            npc.character.chara.appearance_ids = slots;
        }
        let id = npc.actor_id();
        (id, npc.character, ActorKindTag::Npc, ActorKind::Npc)
    };

    // 2. Push a spatial projection into the zone.
    {
        let mut zone = zone_arc.write().await;
        let mut ob = crate::zone::outbox::AreaOutbox::new();
        zone.core.add_actor(
            StoredActor {
                actor_id,
                kind: grid_kind,
                position: Vector3::new(seed.x, seed.y, seed.z),
                grid: (0, 0),
                is_alive: true,
            },
            &mut ob,
        );
    }

    // 3. Register the live Character in the ActorRegistry.
    ctx.registry
        .insert(ActorHandle::new(
            actor_id, kind_tag, zone_id, /* session */ 0, character,
        ))
        .await;
    Some(actor_id)
}

/// Convenience entry point matching the Phase 3 task name.
pub async fn spawn_all_actors(ctx: &SpawnContext<'_>) -> Vec<u32> {
    ctx.spawn_all_actors().await
}

/// Spawn one actor outside the bulk-boot path. Useful for Lua-triggered
/// spawns from event scripts and for unit tests.
#[allow(clippy::too_many_arguments)]
pub async fn spawn_from_location(
    ctx: &SpawnContext<'_>,
    zone_id: u32,
    actor_number: u32,
    seed: &SpawnLocation,
) -> Option<u32> {
    let zone_arc = ctx.world.zone(zone_id).await?;
    let class = ctx.actor_classes.get(&seed.class_id)?;
    let is_battle = ctx.battle_class_ids.contains(&seed.class_id);
    spawn_one(
        ctx,
        &zone_arc,
        zone_id,
        actor_number,
        class,
        seed,
        is_battle,
    )
    .await
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    use crate::zone::Zone;
    use crate::zone::navmesh::StubNavmeshLoader;

    fn mk_zone(id: u32) -> Zone {
        Zone::new(
            id,
            "test",
            1,
            "/Area/Zone/Test",
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

    fn class(id: u32) -> ActorClass {
        ActorClass::new(id, "/Chara/Npc/Populace/Generic", 0, 0, "", 0, 0, 0)
    }

    fn seed(class_id: u32, unique: &str, x: f32, z: f32) -> SpawnLocation {
        SpawnLocation::new(class_id, unique, 100, "", 0, x, 0.0, z, 0.0, 0, 0)
    }

    #[tokio::test]
    async fn spawn_zone_materialises_npcs() {
        let world = WorldManager::new();
        let registry = ActorRegistry::new();

        let mut zone = mk_zone(100);
        zone.add_spawn_location(seed(10_001, "greeter", 5.0, 0.0))
            .unwrap();
        zone.add_spawn_location(seed(20_002, "dodo", 10.0, 10.0))
            .unwrap();
        world.register_zone(zone).await;

        let mut classes = HashMap::new();
        classes.insert(10_001, class(10_001));
        classes.insert(20_002, class(20_002));

        let mut battle_ids = HashSet::new();
        battle_ids.insert(20_002);

        let ctx = SpawnContext {
            world: &world,
            registry: &registry,
            actor_classes: &classes,
            battle_class_ids: &battle_ids,
            npc_appearances: &std::collections::HashMap::new(),
        };
        let spawned = ctx.spawn_zone(100).await;
        assert_eq!(spawned.len(), 2);

        // The registry should now hold two actors with the right kinds.
        assert_eq!(registry.len().await, 2);
        let in_zone = registry.actors_in_zone(100).await;
        assert_eq!(in_zone.len(), 2);
        assert!(in_zone.iter().any(|h| h.kind == ActorKindTag::BattleNpc));
        assert!(in_zone.iter().any(|h| h.kind == ActorKindTag::Npc));

        // The zone's spatial grid should carry matching projections.
        let z = world.zone(100).await.unwrap();
        let g = z.read().await;
        assert_eq!(g.core.actor_count(), 2);
    }

    #[tokio::test]
    async fn spawn_from_location_routes_one() {
        let world = WorldManager::new();
        let registry = ActorRegistry::new();
        let zone = mk_zone(100);
        world.register_zone(zone).await;

        let mut classes = HashMap::new();
        classes.insert(42, class(42));
        let battle_ids = HashSet::new();

        let ctx = SpawnContext {
            world: &world,
            registry: &registry,
            actor_classes: &classes,
            battle_class_ids: &battle_ids,
            npc_appearances: &std::collections::HashMap::new(),
        };
        let s = seed(42, "lone_npc", 0.0, 0.0);
        let actor_id = spawn_from_location(&ctx, 100, 1, &s).await;
        assert!(actor_id.is_some());
        assert_eq!(registry.len().await, 1);
    }

    #[tokio::test]
    async fn unknown_class_id_skips_spawn() {
        let world = WorldManager::new();
        let registry = ActorRegistry::new();
        let mut zone = mk_zone(100);
        zone.add_spawn_location(seed(99, "ghost", 0.0, 0.0))
            .unwrap();
        world.register_zone(zone).await;

        let classes: HashMap<u32, ActorClass> = HashMap::new();
        let battle_ids = HashSet::new();
        let ctx = SpawnContext {
            world: &world,
            registry: &registry,
            actor_classes: &classes,
            battle_class_ids: &battle_ids,
            npc_appearances: &std::collections::HashMap::new(),
        };
        let spawned = ctx.spawn_zone(100).await;
        assert!(spawned.is_empty());
        assert_eq!(registry.len().await, 0);
    }
}
