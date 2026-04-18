//! Game-loop ticker.
//!
//! Spawned from `main.rs` alongside `server::run`. Ticks at a configurable
//! cadence (default 100ms); each tick:
//!
//! 1. Advances the global millisecond clock.
//! 2. Walks every zone and, within each zone, every actor whose `zone_id`
//!    matches. For each actor, drives:
//!    - `StatusEffectContainer::update` → emits `StatusEvent`s.
//!    - `AIContainer::update`           → emits `BattleEvent`s.
//! 3. Drains and dispatches all typed outboxes (status / battle / area /
//!    inventory). Events route through the existing dispatcher functions,
//!    which turn them into real packets on session queues, DB writes, and
//!    Lua calls.
//!
//! The ticker holds `Arc` references to the `Database`, `WorldManager`,
//! `ActorRegistry`, and (later) `LuaEngine` — shareable and cheap to
//! clone into spawned tasks.

#![allow(dead_code)]

use std::sync::Arc;
use std::time::Duration;

use tokio::sync::RwLock;
use tokio::time::{Instant, interval};

use crate::actor::Character;
use crate::actor::modifier::ModifierMap;
use crate::battle::controller::ControllerOwnerView;
use crate::battle::outbox::BattleOutbox;
use crate::battle::target_find::{ActorArena, ActorView};
use crate::database::Database;
use crate::status::StatusOutbox;
use crate::world_manager::WorldManager;
use crate::zone::outbox::AreaOutbox;
use crate::zone::zone::Zone;

use super::actor_registry::ActorRegistry;
use super::dispatcher::{dispatch_area_event, dispatch_battle_event, dispatch_status_event};

#[derive(Debug, Clone, Copy)]
pub struct TickerConfig {
    /// Tick period. Retail runs the zone thread ~every 333 ms, but 100 ms
    /// keeps combat + regen crisper without adding much load.
    pub tick_interval: Duration,
}

impl Default for TickerConfig {
    fn default() -> Self {
        Self {
            tick_interval: Duration::from_millis(100),
        }
    }
}

pub struct GameTicker {
    pub config: TickerConfig,
    pub world: Arc<WorldManager>,
    pub registry: Arc<ActorRegistry>,
    pub db: Arc<Database>,
    /// Server-start wall-clock — `now_ms` on each tick is relative to this.
    start: Instant,
}

impl GameTicker {
    pub fn new(
        config: TickerConfig,
        world: Arc<WorldManager>,
        registry: Arc<ActorRegistry>,
        db: Arc<Database>,
    ) -> Self {
        Self {
            config,
            world,
            registry,
            db,
            start: Instant::now(),
        }
    }

    /// Run forever — suitable for `tokio::spawn`. Returns only on error.
    pub async fn run(self) -> ! {
        let mut int = interval(self.config.tick_interval);
        // The first tick fires immediately; we want the period to apply
        // between ticks.
        int.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        loop {
            int.tick().await;
            let now_ms = self.start.elapsed().as_millis() as u64;
            self.tick_once(now_ms).await;
        }
    }

    /// One pass of the tick loop. Exposed separately so tests can call it
    /// without waiting on the interval.
    pub async fn tick_once(&self, now_ms: u64) {
        let zone_ids = self.world.zone_ids().await;
        for zone_id in zone_ids {
            let Some(zone_arc) = self.world.zone(zone_id).await else {
                continue;
            };
            self.tick_zone(now_ms, zone_id, &zone_arc).await;
        }
    }

    async fn tick_zone(&self, now_ms: u64, zone_id: u32, zone: &Arc<RwLock<Zone>>) {
        let actors = self.registry.actors_in_zone(zone_id).await;

        for handle in actors {
            let mut status_outbox = StatusOutbox::new();
            let mut battle_outbox = BattleOutbox::new();

            // Drive status effects + AI while holding the character write lock.
            let owner_view = {
                let mut chara = handle.character.write().await;
                tick_status(&mut chara, now_ms, &mut status_outbox);
                build_owner_view(&chara, handle.actor_id, zone_id)
            };

            // AIContainer::update needs an ActorArena — the zone itself.
            {
                let zone_read = zone.read().await;
                let mut chara = handle.character.write().await;
                chara.ai_container.update(now_ms, owner_view, &*zone_read, &mut battle_outbox);
            }

            for e in status_outbox.drain() {
                dispatch_status_event(&e, &self.registry, &self.world, &self.db).await;
            }
            for e in battle_outbox.drain() {
                dispatch_battle_event(&e, &self.registry, &self.world, zone).await;
            }
        }

        // Area-level events (weather sweeps, broadcasts queued by scripts).
        let mut area_outbox = AreaOutbox::new();
        {
            let mut zone_write = zone.write().await;
            zone_write.sweep_finished_content(&mut area_outbox);
        }
        for e in area_outbox.drain() {
            dispatch_area_event(&e, &self.registry, &self.world, zone).await;
        }
    }
}

fn tick_status(chara: &mut Character, now_ms: u64, outbox: &mut StatusOutbox) {
    // Clone the ModifierMap so we can hand it in without aliasing — the
    // underlying HashMap is small enough that this is essentially free.
    let mods_snapshot: ModifierMap = chara.chara.mods.clone();
    chara.status_effects.update(now_ms, &mods_snapshot, outbox);
}

fn build_owner_view(chara: &Character, actor_id: u32, zone_id: u32) -> ControllerOwnerView {
    let is_engaged = chara.ai_container.is_engaged();
    let current_target = chara
        .ai_container
        .current_state()
        .map(|s| s.target_actor_id)
        .filter(|id| *id != 0);
    let most_hated = chara.hate.most_hated();
    ControllerOwnerView {
        actor: ActorView {
            actor_id,
            position: chara.base.position(),
            rotation: chara.base.rotation,
            is_alive: chara.is_alive(),
            is_static: false,
            allegiance: actor_id_to_allegiance(actor_id, chara),
            party_id: 0,
            zone_id,
            is_updates_locked: false,
            is_player: false,
            is_battle_npc: true,
        },
        is_engaged,
        is_spawned: true,
        is_following_path: chara.ai_container.path_find.as_ref().is_some_and(|p| p.is_following_path()),
        at_path_end: chara.ai_container.path_find.as_ref().is_none_or(|p| !p.is_following_path()),
        most_hated_actor_id: most_hated,
        current_target_actor_id: current_target,
        has_prevent_movement: false,
        max_hp: chara.get_max_hp(),
        current_hp: chara.get_hp(),
        target_hpp: None,
        target_has_stealth: false,
        is_close_to_spawn: true,
        target_is_locked: false,
    }
}

fn actor_id_to_allegiance(_actor_id: u32, chara: &Character) -> u32 {
    // Allegiance is on BattleSave/Character in retail; Phase 1 treats
    // anyone with an AI controller as "BattleNpc allegiance = 2" and the
    // rest as "Player allegiance = 1". Refined in Phase 3 once the Npc
    // types carry explicit allegiance fields.
    if chara.ai_container.controller.is_some() {
        2
    } else {
        1
    }
}

// ActorArena is implemented on Zone (crate::zone::zone). We re-export it
// here as a sanity check so downstream code knows the trait is in scope.
#[allow(dead_code)]
fn _zone_is_actor_arena(z: &Zone) -> &dyn ActorArena {
    z
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::actor::Character;
    use crate::runtime::actor_registry::{ActorHandle, ActorKindTag};
    use crate::zone::area::{ActorKind, StoredActor};
    use crate::zone::navmesh::StubNavmeshLoader;
    use common::Vector3;

    fn tempdb() -> std::path::PathBuf {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("garlemald-ticker-{nanos}.db"))
    }

    async fn setup_one_zone_one_actor() -> (GameTicker, Arc<RwLock<Zone>>) {
        let world = Arc::new(WorldManager::new());
        let registry = Arc::new(ActorRegistry::new());
        let db = Arc::new(
            Database::open(tempdb()).await.expect("database stub"),
        );

        let mut zone = Zone::new(
            100, "test", 1, "/Area/Zone/Test", 0, 0, 0, false, false, false, false, false,
            Some(&StubNavmeshLoader),
        );
        let mut ob = AreaOutbox::new();
        zone.core.add_actor(
            StoredActor {
                actor_id: 1,
                kind: ActorKind::BattleNpc,
                position: Vector3::ZERO,
                grid: (0, 0),
                is_alive: true,
            },
            &mut ob,
        );
        world.register_zone(zone).await;

        let mut character = Character::new(1);
        character.chara.hp = 1000;
        character.chara.max_hp = 1000;
        character.chara.mods.set(crate::actor::modifier::Modifier::Regen, 5.0);
        registry
            .insert(ActorHandle::new(1, ActorKindTag::BattleNpc, 100, 0, character))
            .await;

        let ticker = GameTicker::new(TickerConfig::default(), world.clone(), registry, db);
        let zone_arc = world.zone(100).await.unwrap();
        (ticker, zone_arc)
    }

    #[tokio::test]
    async fn one_tick_runs_without_panic() {
        let (ticker, _zone) = setup_one_zone_one_actor().await;
        ticker.tick_once(1_000).await;
    }

    #[tokio::test]
    async fn regen_tick_applies_hp_delta() {
        let (ticker, _zone) = setup_one_zone_one_actor().await;

        // Drop the actor's HP first, then tick far enough to cross the 3s
        // regen cadence.
        let handle = ticker.registry.get(1).await.unwrap();
        {
            let mut chara = handle.character.write().await;
            chara.chara.hp = 500;
        }
        ticker.tick_once(5_000).await;

        let hp_after = handle.character.read().await.chara.hp;
        assert!(hp_after > 500, "regen should have bumped hp, got {hp_after}");
    }
}
