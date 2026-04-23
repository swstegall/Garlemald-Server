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
use crate::lua::LuaEngine;
use crate::status::StatusOutbox;
use crate::world_manager::WorldManager;
use crate::zone::outbox::AreaOutbox;
use crate::zone::zone::Zone;

use super::actor_registry::ActorRegistry;
use super::dispatcher::{dispatch_area_event, dispatch_battle_event, dispatch_status_event};

/// Inn rested-XP accrual rate: 1 percentage point per N seconds while
/// the player is parked in an `is_inn` zone. 60s gives a +100% rested
/// bonus across a full inn-night sleep without being so fast that an
/// AFK lunch break maxes the bar.
pub const INN_REST_INTERVAL_SECS: u32 = 60;
/// Cap for the rested-bonus pool — 100% is retail's effective max
/// (matches the `accruerest` GM-command clamp + `consume_rested_xp`'s
/// `rested_pct.min(100)` ceiling).
pub const INN_REST_BONUS_CAP: i32 = 100;

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
    /// Optional Lua engine — when present, combat-side hooks like
    /// `onKillBNpc` can fire from the ticker's `dispatch_battle_event`
    /// path. Test harnesses pass `None`; main.rs wires the real engine.
    pub lua: Option<Arc<LuaEngine>>,
    /// Shared gamedata catalogs (items, quests, recipes, passive GL).
    /// Pulled from `lua.catalogs()` when a real `LuaEngine` is present;
    /// otherwise an empty instance so test harnesses don't need to wire
    /// up Catalogs just to run the ticker. The dispatchers consume this
    /// (gear-paramBonus summer reads `items`).
    pub catalogs: Arc<crate::lua::Catalogs>,
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
        Self::with_lua(config, world, registry, db, None)
    }

    pub fn with_lua(
        config: TickerConfig,
        world: Arc<WorldManager>,
        registry: Arc<ActorRegistry>,
        db: Arc<Database>,
        lua: Option<Arc<LuaEngine>>,
    ) -> Self {
        let catalogs = lua
            .as_ref()
            .map(|l| l.catalogs().clone())
            .unwrap_or_else(|| Arc::new(crate::lua::Catalogs::default()));
        Self {
            config,
            world,
            registry,
            db,
            lua,
            catalogs,
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
        // Hoist the zone's `is_inn` flag once per tick so the per-actor
        // rest-bonus loop doesn't have to re-acquire the read lock.
        let zone_is_inn = { zone.read().await.core.is_inn };

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
                chara
                    .ai_container
                    .update(now_ms, owner_view, &*zone_read, &mut battle_outbox);
            }

            // Chocobo rental expiry — port of Meteor commit `8687e431`'s
            // `Player.Update` arm. If the actor has an active rental
            // whose expire timestamp has elapsed, dismount and
            // restore the zone's BGM; otherwise tick down the
            // UI-visible minutes-remaining counter.
            //
            // Inn auto-accrual — Tier 4 #17 follow-up. While the
            // player is in an `is_inn` zone, accumulate `restBonus`
            // toward the +100% cap. Tracked alongside the chocobo
            // tick because both touch CharaState under the same write
            // lock; sharing the lock keeps the per-tick cost flat.
            //
            // Accrual rate: 1% per `INN_REST_INTERVAL_SECS` (default
            // 60s) — coarse enough that long inn stays visibly fill
            // the bar without a cliff. Persistence is deferred to the
            // logout / explicit-flush path so each tick stays a
            // single in-memory increment; `Database::set_rest_bonus_exp_rate`
            // already exists when we choose to flush.
            {
                let tick_utc = (now_ms / 1000) as u32;
                let mut chara = handle.character.write().await;
                if chara.chara.rental_expire_time != 0 {
                    if chara.chara.rental_expire_time <= tick_utc {
                        chara.chara.rental_expire_time = 0;
                        chara.chara.rental_min_left = 0;
                        chara.chara.mount_state = 0;
                        chara.base.current_main_state = crate::actor::MAIN_STATE_PASSIVE;
                        chara.chara.new_main_state = crate::actor::MAIN_STATE_PASSIVE;
                    } else {
                        chara.chara.rental_min_left =
                            (((chara.chara.rental_expire_time - tick_utc) / 60) as u8)
                                .min(255);
                    }
                }

                if zone_is_inn && chara.chara.rest_bonus_exp_rate < INN_REST_BONUS_CAP {
                    if chara.chara.last_rest_accrual_utc == 0 {
                        // Fresh entry — anchor the accrual window
                        // without granting points. Subsequent ticks
                        // measure elapsed time from here.
                        chara.chara.last_rest_accrual_utc = tick_utc;
                    } else {
                        let elapsed_since_last = tick_utc
                            .saturating_sub(chara.chara.last_rest_accrual_utc);
                        let earned = (elapsed_since_last / INN_REST_INTERVAL_SECS) as i32;
                        if earned > 0 {
                            chara.chara.rest_bonus_exp_rate =
                                (chara.chara.rest_bonus_exp_rate + earned).min(INN_REST_BONUS_CAP);
                            // Advance the anchor by exactly the
                            // earned amount * interval so any
                            // sub-interval remainder carries to the
                            // next tick instead of being lost.
                            chara.chara.last_rest_accrual_utc = chara
                                .chara
                                .last_rest_accrual_utc
                                .saturating_add(earned as u32 * INN_REST_INTERVAL_SECS);
                        }
                    }
                } else if !zone_is_inn {
                    // Player left the inn (or this is a non-inn
                    // tick) — reset the anchor so a future inn entry
                    // starts a fresh accrual window.
                    chara.chara.last_rest_accrual_utc = 0;
                }
            }

            for e in status_outbox.drain() {
                dispatch_status_event(
                    &e,
                    &self.registry,
                    &self.world,
                    &self.db,
                    &self.catalogs,
                )
                .await;
            }
            for e in battle_outbox.drain() {
                dispatch_battle_event(
                    &e,
                    &self.registry,
                    &self.world,
                    zone,
                    self.lua.as_ref(),
                    Some(&self.db),
                )
                .await;
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
        is_following_path: chara
            .ai_container
            .path_find
            .as_ref()
            .is_some_and(|p| p.is_following_path()),
        at_path_end: chara
            .ai_container
            .path_find
            .as_ref()
            .is_none_or(|p| !p.is_following_path()),
        most_hated_actor_id: most_hated,
        current_target_actor_id: current_target,
        has_prevent_movement: false,
        max_hp: chara.get_max_hp(),
        current_hp: chara.get_hp(),
        target_hpp: None,
        target_has_stealth: false,
        is_close_to_spawn: true,
        target_is_locked: false,
        attack_delay_ms: chara.get_attack_delay_ms(),
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
        use std::sync::atomic::{AtomicU64, Ordering};
        static SEQ: AtomicU64 = AtomicU64::new(0);
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let seq = SEQ.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir().join(format!("garlemald-ticker-{nanos}-{seq}.db"))
    }

    async fn setup_one_zone_one_actor() -> (GameTicker, Arc<RwLock<Zone>>) {
        let world = Arc::new(WorldManager::new());
        let registry = Arc::new(ActorRegistry::new());
        let db = Arc::new(Database::open(tempdb()).await.expect("database stub"));

        let mut zone = Zone::new(
            100,
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
        character
            .chara
            .mods
            .set(crate::actor::modifier::Modifier::Regen, 5.0);
        registry
            .insert(ActorHandle::new(
                1,
                ActorKindTag::BattleNpc,
                100,
                0,
                character,
            ))
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
        assert!(
            hp_after > 500,
            "regen should have bumped hp, got {hp_after}"
        );
    }

    /// Set up one zone with one Player (id=1) and one passive BattleNpc
    /// (id=2). The NPC has no controller so it won't retaliate. Shared by
    /// the auto-attack / cast-resolution tests below.
    async fn setup_player_vs_npc() -> (GameTicker, Arc<RwLock<Zone>>) {
        let world = Arc::new(WorldManager::new());
        let registry = Arc::new(ActorRegistry::new());
        let db = Arc::new(Database::open(tempdb()).await.expect("database stub"));

        let mut zone = Zone::new(
            100,
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
        );
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
                position: Vector3::new(3.0, 0.0, 0.0),
                grid: (0, 0),
                is_alive: true,
            },
            &mut ob,
        );
        world.register_zone(zone).await;

        let mut player = Character::new(1);
        player.chara.hp = 1000;
        player.chara.max_hp = 1000;
        player.chara.level = 10;
        registry
            .insert(ActorHandle::new(
                1,
                ActorKindTag::Player,
                100,
                42,
                player,
            ))
            .await;

        let mut npc = Character::new(2);
        npc.chara.hp = 1000;
        npc.chara.max_hp = 1000;
        npc.chara.level = 10;
        npc.base.position_x = 3.0;
        registry
            .insert(ActorHandle::new(
                2,
                ActorKindTag::BattleNpc,
                100,
                0,
                npc,
            ))
            .await;

        let ticker = GameTicker::new(TickerConfig::default(), world.clone(), registry, db);
        let zone_arc = world.zone(100).await.unwrap();
        (ticker, zone_arc)
    }

    #[tokio::test]
    async fn auto_attack_drops_defender_hp() {
        let (ticker, _zone) = setup_player_vs_npc().await;

        // Player engages the NPC. The default swing delay is 2500 ms.
        {
            let handle = ticker.registry.get(1).await.unwrap();
            let mut chara = handle.character.write().await;
            chara.ai_container.internal_engage(2, 0, 2500);
        }

        let npc_handle = ticker.registry.get(2).await.unwrap();
        let initial_hp = npc_handle.character.read().await.get_hp();
        assert_eq!(initial_hp, 1000);

        // Tick through ~10 swings. Base auto-attack damage is 0..=90 with
        // a ~10% chance of zero, so run enough swings that the combined
        // probability of no damage is negligible.
        for i in 1..=10 {
            ticker.tick_once((i as u64) * 2_600).await;
        }

        let final_hp = npc_handle.character.read().await.get_hp();
        assert!(
            final_hp < initial_hp,
            "npc hp should have dropped after 10 swings, got {final_hp}"
        );
    }

    #[tokio::test]
    async fn cast_completion_resolves_spell_damage() {
        let (ticker, _zone) = setup_player_vs_npc().await;

        // Give the NPC a little defense so the damage math actually has
        // something to bite into, and queue a spell against it.
        {
            let handle = ticker.registry.get(2).await.unwrap();
            let mut chara = handle.character.write().await;
            chara
                .chara
                .mods
                .set(crate::actor::modifier::Modifier::Defense, 50.0);
        }
        {
            let handle = ticker.registry.get(1).await.unwrap();
            let mut chara = handle.character.write().await;
            let mut cmd = crate::battle::BattleCommand::new(100, "stone");
            cmd.cast_time_ms = 1_000;
            cmd.action_type = crate::battle::ActionType::Magic;
            cmd.command_type = crate::battle::CommandType::SPELL;
            cmd.base_potency = 300;
            assert!(chara.ai_container.internal_cast(2, cmd, 0));
        }

        let npc_handle = ticker.registry.get(2).await.unwrap();
        let initial_hp = npc_handle.character.read().await.get_hp();

        // One tick after cast finish resolves the spell.
        ticker.tick_once(1_500).await;

        let final_hp = npc_handle.character.read().await.get_hp();
        assert!(
            final_hp < initial_hp,
            "cast should have dropped npc hp from {initial_hp}, got {final_hp}"
        );
    }
}
