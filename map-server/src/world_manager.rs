//! Zone + session registry and transition orchestrator. Port of the
//! `WorldManager.cs` surface that deals with zones + boundaries +
//! session handoff. Heavy sub-systems (Director, Group, Party) live in
//! their own modules.
//!
//! Character state (Players, Npcs, BattleNpcs) lives in
//! `runtime::actor_registry::ActorRegistry`. WorldManager only owns:
//!
//! * `zones` — canonical `zone::Zone` instances keyed by zone id.
//! * `zone_entrances` — named warp points keyed by entrance id.
//! * `seamless_boundaries` — per-region boundary boxes for seamless
//!   zone transitions.
//! * `sessions` / `clients` — per-socket state.
//!
//! All `RwLock<HashMap>` so independent zones / sessions don't contend.
#![allow(dead_code)]

use std::collections::HashMap;
use std::sync::Arc;

use anyhow::Result;
use tokio::sync::RwLock;

use common::Vector3;

use crate::data::{ClientHandle, SeamlessBoundary, Session, ZoneEntrance, check_pos_in_bounds};
use crate::database::{Database, PrivateAreaRow, ZoneRow};
use crate::packets::send as tx;
use crate::runtime::actor_registry::ActorRegistry;
use crate::zone::navmesh::StubNavmeshLoader;
use crate::zone::private_area::PrivateArea;
use crate::zone::zone::Zone;

/// Mirror of C# `Director.GenerateActorName` zone-name abbreviation:
/// `Field→Fld, Dungeon→Dgn, Town→Twn, Battle→Btl, Test→Tes,
/// Event→Evt, Ship→Shp, Office→Ofc`. Used when building the director's
/// actor-name field so it matches the format the client expects (e.g.
/// `ocn0Battle02` → `ocn0Btl02`).
fn shorten_zone_name(zone_name: &str) -> String {
    zone_name
        .replace("Field", "Fld")
        .replace("Dungeon", "Dgn")
        .replace("Town", "Twn")
        .replace("Battle", "Btl")
        .replace("Test", "Tes")
        .replace("Event", "Evt")
        .replace("Ship", "Shp")
        .replace("Office", "Ofc")
}

/// Append the full 7-packet spawn sequence for a zone-resident "master"
/// actor (area master, debug, or world master) — matches C# `Actor.
/// GetSpawnPackets`: AddActor(0), Speed, SpawnPosition(spawnType=1),
/// Name, State(passive), IsZoning(false), ScriptBind. All three master
/// actors share this shape; the only thing that varies is the actor id,
/// name, class name, and the LuaParam list that goes into the
/// `ActorInstantiate` script-bind packet.
///
/// **Currently unused.** Emitting any of the three master spawns to the
/// 1.23b client triggers STATUS_INVALID_PARAMETER on a deferred callback
/// ~5s post-login — most likely a failed script/resource load driven by
/// a ScriptBind LuaParam the client can't resolve against its local
/// data archive. Kept intact so the implementation can be revived once
/// we know which specific param path fails.
#[allow(dead_code)]
fn push_master_spawn(
    subpackets: &mut Vec<common::subpacket::SubPacket>,
    actor_id: u32,
    actor_name: String,
    class_name: String,
    script_bind_params: Vec<common::luaparam::LuaParam>,
) {
    subpackets.push(tx::actor::build_add_actor(actor_id, 0));
    subpackets.push(tx::actor::build_set_actor_speed_default(actor_id));
    // C# `Actor.CreateSpawnPositonPacket` passes `actorId` as the second
    // (target) arg for plain actors. The `-1` sentinel is player-self only
    // — using it for NPCs trips STATUS_INVALID_PARAMETER inside the client's
    // actor-resolve path and raises 0xc000000d a couple seconds after the
    // zone-in packets are consumed.
    subpackets.push(tx::actor::build_set_actor_position(
        actor_id, actor_id as i32, 0.0, 0.0, 0.0, 0.0, 0x1, false,
    ));
    // C# `CreateNamePacket` uses displayNameId=0 when a customDisplayName
    // is set; all three masters ship with names ("debug", "worldMaster",
    // "_areaMaster@…"). The area master's displayNameId is technically
    // 0xFFFFFFFF in C# but the packet skips that branch when a custom
    // name is present, so 0 here is fine.
    subpackets.push(tx::actor::build_set_actor_name(actor_id, 0, &actor_name));
    subpackets.push(tx::actor::build_set_actor_state(actor_id, 0, 0));
    subpackets.push(tx::actor::build_set_actor_is_zoning(actor_id, false));
    subpackets.push(tx::actor::build_actor_instantiate(
        actor_id,
        0,
        0x3040,
        &actor_name,
        &class_name,
        &script_bind_params,
    ));
}

/// Outcome of a single `seamless_check` call. Describes what, if anything,
/// the player's position change triggered.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SeamlessResult {
    /// Inside the main zone-1 box, not near a boundary — nothing to do.
    InsideZoneOne,
    /// Inside the main zone-2 box, not near a boundary — nothing to do.
    InsideZoneTwo,
    /// The player crossed into a zone they weren't tracked in — we fired
    /// a seamless zone change. The `u32` is the new primary zone id.
    ZoneChanged(u32),
    /// Player entered the merge strip; a secondary zone was merged in.
    /// The `u32` is the merged (secondary) zone id.
    ZoneMerged(u32),
    /// Position isn't inside any boundary — fully inside a single zone.
    None,
}

/// Top-level zone + session registry.
pub struct WorldManager {
    zones: RwLock<HashMap<u32, Arc<RwLock<Zone>>>>,

    /// Named entrance points (`server_zones_spawnlocations`) keyed by id.
    zone_entrances: RwLock<HashMap<u32, ZoneEntrance>>,

    /// Seamless boundary boxes keyed by region id.
    seamless_boundaries: RwLock<HashMap<u32, Vec<SeamlessBoundary>>>,

    /// Player state indexed by session id — zone membership, player
    /// position snapshot, etc. Updated by movement handlers.
    sessions: RwLock<HashMap<u32, Session>>,

    /// Live socket handles. Used by packet dispatchers to fan outbound
    /// SubPackets to the right clients.
    clients: RwLock<HashMap<u32, ClientHandle>>,
}

impl WorldManager {
    pub fn new() -> Self {
        Self {
            zones: RwLock::new(HashMap::new()),
            zone_entrances: RwLock::new(HashMap::new()),
            seamless_boundaries: RwLock::new(HashMap::new()),
            sessions: RwLock::new(HashMap::new()),
            clients: RwLock::new(HashMap::new()),
        }
    }

    // -----------------------------------------------------------------
    // Boot-time loaders — pull from DB, populate in-memory registries.
    // -----------------------------------------------------------------

    /// Full boot-time zone load — port of
    /// `WorldManager.LoadZoneList + LoadZoneEntranceList + LoadSeamlessBoundryList`.
    pub async fn load_from_database(
        &self,
        db: &Database,
        server_ip: &str,
        server_port: u16,
    ) -> Result<()> {
        tracing::info!(server_ip, server_port, "world boot-load starting");
        // 1. Zones
        let zone_rows = db.load_zones(server_ip, server_port).await?;
        tracing::info!(count = zone_rows.len(), "zones fetched from DB");
        for row in zone_rows {
            self.install_zone(row).await;
        }
        // 2. Private areas — attach to already-loaded zones.
        let private_area_rows = db.load_private_areas().await?;
        tracing::info!(count = private_area_rows.len(), "private areas fetched");
        for row in private_area_rows {
            self.install_private_area(row).await;
        }
        // 3. Zone entrances.
        let entrances = db.load_zone_entrances().await?;
        tracing::info!(count = entrances.len(), "zone entrances loaded");
        *self.zone_entrances.write().await = entrances;
        // 4. Seamless boundaries.
        let seamless = db.load_seamless_boundaries().await?;
        let total: usize = seamless.values().map(|v| v.len()).sum();
        tracing::info!(
            regions = seamless.len(),
            boundaries = total,
            "seamless boundaries loaded"
        );
        *self.seamless_boundaries.write().await = seamless;
        tracing::info!("world boot-load complete");
        Ok(())
    }

    async fn install_zone(&self, row: ZoneRow) {
        tracing::debug!(
            id = row.id,
            name = %row.zone_name,
            region = row.region_id,
            navmesh = row.load_nav_mesh,
            "installing zone"
        );
        let navmesh_loader = if row.load_nav_mesh {
            Some(&StubNavmeshLoader as &dyn crate::zone::navmesh::NavmeshLoader)
        } else {
            None
        };
        let zone = Zone::new(
            row.id,
            row.zone_name,
            row.region_id,
            row.class_path,
            row.bgm_day,
            row.bgm_night,
            row.bgm_battle,
            row.is_isolated,
            row.is_inn,
            row.can_ride_chocobo,
            row.can_stealth,
            row.is_instance_raid,
            navmesh_loader,
        );
        self.register_zone(zone).await;
    }

    async fn install_private_area(&self, row: PrivateAreaRow) {
        let Some(parent_arc) = self.zone(row.parent_zone_id).await else {
            tracing::warn!(
                parent = row.parent_zone_id,
                name = %row.private_area_name,
                "private area references missing parent zone"
            );
            return;
        };
        let (zone_name, region_id, is_isolated, is_inn, can_ride_chocobo, can_stealth) = {
            let parent = parent_arc.read().await;
            (
                parent.core.zone_name.clone(),
                parent.core.region_id,
                parent.core.is_isolated,
                parent.core.is_inn,
                parent.core.can_ride_chocobo,
                parent.core.can_stealth,
            )
        };
        let pa = PrivateArea::new(
            row.parent_zone_id,
            zone_name,
            region_id,
            row.id,
            row.class_name,
            row.private_area_name,
            row.private_area_type,
            row.bgm_day,
            row.bgm_night,
            row.bgm_battle,
            is_isolated,
            is_inn,
            can_ride_chocobo,
            can_stealth,
        );
        let mut parent = parent_arc.write().await;
        parent.add_private_area(pa);
    }

    // -----------------------------------------------------------------
    // Zone registry
    // -----------------------------------------------------------------

    /// Register (or replace) a zone. Called once per zone during startup.
    pub async fn register_zone(&self, zone: Zone) {
        let id = zone.core.actor_id;
        self.zones
            .write()
            .await
            .insert(id, Arc::new(RwLock::new(zone)));
    }

    pub async fn zone(&self, zone_id: u32) -> Option<Arc<RwLock<Zone>>> {
        self.zones.read().await.get(&zone_id).cloned()
    }

    /// Snapshot of all zone ids — used by the game ticker.
    pub async fn zone_ids(&self) -> Vec<u32> {
        self.zones.read().await.keys().copied().collect()
    }

    pub async fn zone_count(&self) -> usize {
        self.zones.read().await.len()
    }

    // -----------------------------------------------------------------
    // Zone entrances + seamless boundaries
    // -----------------------------------------------------------------

    pub async fn zone_entrance(&self, id: u32) -> Option<ZoneEntrance> {
        self.zone_entrances.read().await.get(&id).cloned()
    }

    /// Returns every boundary for `region_id`. Empty if the region has
    /// none.
    pub async fn seamless_boundaries_for(&self, region_id: u32) -> Vec<SeamlessBoundary> {
        self.seamless_boundaries
            .read()
            .await
            .get(&region_id)
            .cloned()
            .unwrap_or_default()
    }

    // -----------------------------------------------------------------
    // Session + client registries
    // -----------------------------------------------------------------

    pub async fn upsert_session(&self, session: Session) {
        self.sessions.write().await.insert(session.id, session);
    }

    pub async fn session(&self, id: u32) -> Option<Session> {
        self.sessions.read().await.get(&id).cloned()
    }

    pub async fn remove_session(&self, id: u32) -> Option<Session> {
        self.clients.write().await.remove(&id);
        self.sessions.write().await.remove(&id)
    }

    pub async fn register_client(&self, id: u32, handle: ClientHandle) {
        self.clients.write().await.insert(id, handle);
    }

    pub async fn client(&self, id: u32) -> Option<ClientHandle> {
        self.clients.read().await.get(&id).cloned()
    }

    pub async fn all_clients(&self) -> Vec<ClientHandle> {
        self.clients.read().await.values().cloned().collect()
    }

    // -----------------------------------------------------------------
    // Zone-change orchestration — port of WorldManager.DoZoneChange /
    // DoSeamlessZoneChange / MergeZones / SeamlessCheck.
    // -----------------------------------------------------------------

    /// Whole-cloth zone transition — removes the player from their old
    /// zone (if any), places them in the new one, updates their session
    /// state. Callers must follow this with `send_zone_in_bundle` once
    /// they are ready to fan the first-render packets to the client;
    /// `do_zone_change` only settles registries.
    pub async fn do_zone_change(
        &self,
        actor_id: u32,
        session_id: u32,
        destination_zone_id: u32,
        spawn: Vector3,
        rotation: f32,
    ) -> Result<()> {
        // 1. Look up the destination zone.
        let Some(dest_zone) = self.zone(destination_zone_id).await else {
            tracing::warn!(
                zone = destination_zone_id,
                "do_zone_change: destination not on this map server"
            );
            return Ok(());
        };

        // 2. Lock the session for updates + update its zone/dest fields.
        {
            let mut sessions = self.sessions.write().await;
            let session = sessions
                .entry(session_id)
                .or_insert_with(|| Session::new(session_id));
            session.is_updates_locked = true;
            let old_zone_id = session.current_zone_id;
            session.current_zone_id = destination_zone_id;
            session.destination_zone_id = destination_zone_id;
            session.destination_x = spawn.x;
            session.destination_y = spawn.y;
            session.destination_z = spawn.z;
            session.destination_rot = rotation;

            // 3. Detach from old zone's spatial grid if different.
            if old_zone_id != 0
                && old_zone_id != destination_zone_id
                && let Some(old_zone) = self.zones.read().await.get(&old_zone_id).cloned()
            {
                let mut old = old_zone.write().await;
                let mut ob = crate::zone::outbox::AreaOutbox::new();
                old.core.remove_actor(actor_id, &mut ob);
            }

            // 4. Attach to new zone.
            let mut dest = dest_zone.write().await;
            let mut ob = crate::zone::outbox::AreaOutbox::new();
            dest.core.add_actor(
                crate::zone::area::StoredActor {
                    actor_id,
                    kind: crate::zone::area::ActorKind::Player,
                    position: spawn,
                    grid: (0, 0),
                    is_alive: true,
                },
                &mut ob,
            );

            // Unlock updates.
            session.is_updates_locked = false;
        }
        Ok(())
    }

    /// Port of `Player.SendZoneInPackets(world, spawnType)`. This is the
    /// bundle the client waits on before leaving "Now loading…": zoning
    /// clear, music/weather/map, the player's self-spawn, an empty
    /// inventory bracket, and the `/_init` property flags. Without it the
    /// client has no way to know the server is done placing the actor.
    ///
    /// Inventory dump and area-master/director spawns are intentionally
    /// stubbed — the minimum viable login flow doesn't need them and they
    /// depend on plumbing that's still in progress (item_packages live on
    /// the `Player` shape, the registry only holds `Character`).
    pub async fn send_zone_in_bundle(
        &self,
        registry: &ActorRegistry,
        session_id: u32,
        spawn_type: u16,
    ) {
        let Some(session) = self.session(session_id).await else {
            tracing::warn!(session = session_id, "send_zone_in_bundle: no session");
            return;
        };
        let Some(client) = self.client(session_id).await else {
            tracing::warn!(session = session_id, "send_zone_in_bundle: no client");
            return;
        };
        let Some(actor_handle) = registry.by_session(session_id).await else {
            tracing::warn!(session = session_id, "send_zone_in_bundle: no actor");
            return;
        };
        let Some(zone_arc) = self.zone(session.current_zone_id).await else {
            tracing::warn!(
                session = session_id,
                zone = session.current_zone_id,
                "send_zone_in_bundle: no zone",
            );
            return;
        };

        let actor_id = actor_handle.actor_id;
        let (
            actor_name,
            display_name_id,
            main_state,
            position,
            rotation,
            model_id,
            appearance_ids,
            hp,
            hp_max,
            mp,
            mp_max,
            tp,
            class_slot,
            tribe,
            guardian,
            birthday_day,
            birthday_month,
            initial_town,
            rest_bonus_exp_rate,
            current_job,
            login_director_actor_id,
        ) = {
            let c = actor_handle.character.read().await;
            (
                c.base.display_name().to_string(),
                c.base.display_name_id,
                c.base.current_main_state as u8,
                c.base.position(),
                c.base.rotation,
                c.chara.model_id,
                c.chara.appearance_ids,
                c.chara.hp.max(0) as u16,
                c.chara.max_hp.max(0) as u16,
                c.chara.mp.max(0) as u16,
                c.chara.max_mp.max(0) as u16,
                c.chara.tp,
                c.chara.class.max(0) as u8,
                c.chara.tribe,
                c.chara.guardian,
                c.chara.birthday_day,
                c.chara.birthday_month,
                c.chara.initial_town,
                c.chara.rest_bonus_exp_rate,
                c.chara.current_job,
                c.chara.login_director_actor_id,
            )
        };
        let has_login_director = login_director_actor_id != 0;
        let login_director_spec = session.login_director.clone();
        let (zone_actor_id, region_id, bgm_day, zone_name, zone_class_path, zone_class_name) = {
            let z = zone_arc.read().await;
            (
                z.core.actor_id,
                z.core.region_id as u32,
                z.core.bgm_day,
                z.core.zone_name.clone(),
                z.core.class_path.clone(),
                z.core.class_name.clone(),
            )
        };

        // The "script-bind" for the player — mirrors
        // `Map Server/Actors/Chara/Player/Player.cs` `CreateScriptBindPacket`
        // for the self-view. Two variants depending on whether the
        // `player.lua:onBeginLogin` hook attached a login director (via
        // `player:SetLoginDirector(director)` — fires on the tutorial
        // path in zones 193/166/184).
        //
        // - No director (default): `[classPath, true, false, false, true,
        //   Int(0), false, timers[20], true]` — 28 params.
        // - With director: `[classPath, false, false, true, Actor(dirId),
        //   true, Int(0), false, timers[20], true]` — 29 params, one
        //   extra `Actor` reference for the director.
        //
        // Director spawning is still stubbed, so we send `Actor(0)` as a
        // placeholder; the client's Lua binder receives it as a null
        // actor reference, and the tutorial script-bind path still sees
        // the "Is Init Director = true" flag.
        let player_actor_name = format!("_pc{:08}", actor_id);
        let player_class_name = "Player";
        let mut script_bind_params: Vec<common::luaparam::LuaParam> = if has_login_director {
            vec![
                common::luaparam::LuaParam::String("/Chara/Player/Player_work".to_string()),
                common::luaparam::LuaParam::False,
                common::luaparam::LuaParam::False,
                common::luaparam::LuaParam::True, // "Is Init Director"
                common::luaparam::LuaParam::Actor(login_director_actor_id),
                common::luaparam::LuaParam::True,
                common::luaparam::LuaParam::Int32(0),
                common::luaparam::LuaParam::False,
            ]
        } else {
            vec![
                common::luaparam::LuaParam::String("/Chara/Player/Player_work".to_string()),
                common::luaparam::LuaParam::True,
                common::luaparam::LuaParam::False,
                common::luaparam::LuaParam::False,
                common::luaparam::LuaParam::True,
                common::luaparam::LuaParam::Int32(0),
                common::luaparam::LuaParam::False,
            ]
        };
        script_bind_params.extend((0..20).map(|_| common::luaparam::LuaParam::UInt32(0)));
        script_bind_params.push(common::luaparam::LuaParam::True);

        // Every subpacket crosses the world-server proxy; its reader
        // (`world-server/src/server.rs`) drops subpackets whose
        // `target_id == 0`, so tag each one with the session id before
        // serialising.
        //
        // The 8 `_0x132` packets register the client's command / widget /
        // macro system. They fire only for the self-view — mirrors the C#
        // `Player.Create0x132Packets()`. The client needs `widgetCreate`
        // in particular to instantiate the in-game UI; without these the
        // player sits on "Now Loading" indefinitely after an otherwise
        // clean zone-in bundle.
        //
        // The login director (if any) is spawned **first** — C#
        // `Director.StartDirector(spawnImmediate=true)` emits the
        // director's 7-packet spawn sequence during `onBeginLogin`
        // BEFORE `DoZoneIn` runs `SendZoneInPackets`. That ordering
        // matters: the player's `ActorInstantiate` references the
        // director via `Actor(login_director_actor_id)` inside its
        // ScriptBind LuaParams, and the client needs to have seen the
        // director's `AddActor` before it can resolve that reference.
        let mut subpackets: Vec<common::subpacket::SubPacket> = Vec::new();
        if let Some(spec) = &login_director_spec {
            let zone_short = shorten_zone_name(&zone_name);
            let mut class_lower = spec.class_name.clone();
            if let Some(first) = class_lower.chars().next() {
                let mut lowered = first.to_lowercase().to_string();
                lowered.push_str(&class_lower[first.len_utf8()..]);
                class_lower = lowered;
            }
            let max_class_len = 20usize.saturating_sub(zone_short.len());
            if class_lower.len() > max_class_len {
                class_lower.truncate(max_class_len);
            }
            let director_actor_name = format!(
                "{class_lower}_{zone_short}_0@{zone_actor_id:03X}00",
                zone_actor_id = spec.zone_actor_id
            );
            let director_bind_params = vec![
                common::luaparam::LuaParam::String(spec.class_path.clone()),
                common::luaparam::LuaParam::False,
                common::luaparam::LuaParam::False,
                common::luaparam::LuaParam::False,
                common::luaparam::LuaParam::False,
                common::luaparam::LuaParam::False,
            ];
            subpackets.push(tx::actor::build_add_actor(spec.actor_id, 0));
            subpackets.push(tx::actor::build_set_actor_speed_default(spec.actor_id));
            subpackets.push(tx::actor::build_set_actor_position(
                spec.actor_id,
                spec.actor_id as i32,
                0.0,
                0.0,
                0.0,
                0.0,
                0x0,
                false,
            ));
            subpackets.push(tx::actor::build_set_actor_name(
                spec.actor_id,
                0,
                &director_actor_name,
            ));
            subpackets.push(tx::actor::build_set_actor_state(spec.actor_id, 0, 0));
            subpackets.push(tx::actor::build_set_actor_is_zoning(spec.actor_id, false));
            subpackets.push(tx::actor::build_actor_instantiate(
                spec.actor_id,
                0,
                0x3040,
                &director_actor_name,
                &spec.class_name,
                &director_bind_params,
            ));
            tracing::info!(
                director = spec.actor_id,
                class_path = %spec.class_path,
                name = %director_actor_name,
                "login director spawn packets prepended"
            );
        }
        subpackets.extend(vec![
            tx::actor::build_set_actor_is_zoning(actor_id, false),
            tx::misc::build_set_dalamud(actor_id, 0),
            tx::misc::build_set_music(actor_id, bgm_day, 0x01),
            tx::misc::build_set_weather(actor_id, 1, 1),
            tx::misc::build_set_map(actor_id, region_id, zone_actor_id),
            tx::actor::build_add_actor(actor_id, 8),
            tx::actor::build_0x132(actor_id, 0x0B, "commandForced"),
            tx::actor::build_0x132(actor_id, 0x0A, "commandDefault"),
            tx::actor::build_0x132(actor_id, 0x06, "commandWeak"),
            tx::actor::build_0x132(actor_id, 0x04, "commandContent"),
            tx::actor::build_0x132(actor_id, 0x06, "commandJudgeMode"),
            tx::actor::build_0x132(actor_id, 0x100, "commandRequest"),
            tx::actor::build_0x132(actor_id, 0x100, "widgetCreate"),
            tx::actor::build_0x132(actor_id, 0x100, "macroRequest"),
            tx::actor::build_set_actor_speed_default(actor_id),
            tx::actor::build_set_actor_position(
                actor_id, -1, position.x, position.y, position.z, rotation, spawn_type, true,
            ),
            tx::actor::build_set_actor_appearance(actor_id, model_id, &appearance_ids),
            tx::actor::build_set_actor_name(actor_id, display_name_id, &actor_name),
            tx::handshake::build_0xf(actor_id),
            tx::actor::build_set_actor_state(actor_id, main_state, 0),
            tx::actor::build_set_actor_sub_state(actor_id, 0, 0, 0, 0, 0, 0),
            tx::actor::build_set_actor_status_all(actor_id, &[0u16; 20]),
            tx::actor::build_set_actor_icon(actor_id, 0),
            tx::actor::build_set_actor_is_zoning(actor_id, false),
            tx::actor::build_actor_instantiate(
                actor_id,
                0,
                0x3040,
                &player_actor_name,
                player_class_name,
                &script_bind_params,
            ),
            tx::actor_inventory::build_inventory_begin_change(actor_id, true),
            // Empty-package brackets for the 6 item packages + equipment,
            // matching the C# `Player.SendZoneInPackets` sequence that
            // calls `itemPackages[...].SendFullPackage()` for each of
            // NORMAL/CURRENCY_CRYSTALS/KEYITEMS/BAZAAR/MELDREQUEST/LOOT
            // followed by `equipment.SendUpdate()`. For a fresh
            // character each package is empty — the client still needs
            // to see the (SetBegin, SetEnd) pair to know the package
            // exists and is empty.
            //
            // `code` values from C# `ItemPackage.cs`, `size` from the
            // `MAXSIZE_*` constants: NORMAL=0/200, CURRENCY_CRYSTALS=99/320,
            // KEYITEMS=100/500, BAZAAR=7/10, MELDREQUEST=5/4, LOOT=4/10,
            // EQUIPMENT=0x00FE/35.
            tx::actor_inventory::build_inventory_set_begin(actor_id, 200, 0),
            tx::actor_inventory::build_inventory_set_end(actor_id),
            tx::actor_inventory::build_inventory_set_begin(actor_id, 320, 99),
            tx::actor_inventory::build_inventory_set_end(actor_id),
            tx::actor_inventory::build_inventory_set_begin(actor_id, 500, 100),
            tx::actor_inventory::build_inventory_set_end(actor_id),
            tx::actor_inventory::build_inventory_set_begin(actor_id, 10, 7),
            tx::actor_inventory::build_inventory_set_end(actor_id),
            tx::actor_inventory::build_inventory_set_begin(actor_id, 4, 5),
            tx::actor_inventory::build_inventory_set_end(actor_id),
            tx::actor_inventory::build_inventory_set_begin(actor_id, 10, 4),
            tx::actor_inventory::build_inventory_set_end(actor_id),
            // `equipment.SendUpdate` — ReferencedItemPackage shape with
            // code=0x00FE, size=35. Empty for a fresh character.
            tx::actor_inventory::build_inventory_set_begin(actor_id, 35, 0x00FE),
            tx::actor_inventory::build_inventory_set_end(actor_id),
            tx::actor_inventory::build_inventory_end_change(actor_id),
        ]);
        // `Player.GetInitPackets` can span multiple `SetActorProperty`
        // subpackets when the byte budget is exceeded; the builder emits
        // them in order with the right continuation markers (0x60+len on
        // every packet except the last, which gets 0x82+len). Extend the
        // zone-in bundle with whatever the builder returned.
        //
        // C# `Player.CreatePlayerRelatedPackets` only emits SetCurrentJob
        // when `currentJob != 0`. Advanced job is 0 by default for a
        // fresh character — sending SetCurrentJob(0) unconditionally was
        // a port bug (we were also passing `class_slot` here, which is
        // the active *class*, not the advanced *job* — distinct fields
        // in the 1.0 data model).
        if current_job != 0 {
            subpackets.push(tx::player::build_set_current_job(
                actor_id,
                current_job as u32,
            ));
        }
        subpackets.extend(tx::actor::build_player_property_init(
            actor_id,
            hp,
            hp_max,
            mp,
            mp_max,
            tp,
            class_slot,
            1,
            0x20, // commandBorder: C# CharaWork default is 0x20
            tribe,
            guardian,
            birthday_day,
            birthday_month,
            initial_town,
            rest_bonus_exp_rate,
        ));
        // Master-actor spawns (area master / debug / world master) are
        // still disabled — they crashed the client last round with
        // STATUS_INVALID_PARAMETER. The login director spawn is
        // prepended earlier in the bundle (above the player's own
        // packets) so the client sees the director's `AddActor` before
        // the player's `ActorInstantiate` references it by actor id.
        let _ = (&zone_name, &zone_class_path, &zone_class_name);
        let _ = &main_state;
        let _ = &login_director_spec;

        for mut sub in subpackets {
            sub.set_target_id(session_id);
            client.send_bytes(sub.to_bytes()).await;
        }
        tracing::info!(
            session = session_id,
            actor = actor_id,
            zone = zone_actor_id,
            "zone-in bundle dispatched",
        );
    }

    /// Lightweight port of `DoSeamlessZoneChange`. Used when the player
    /// crosses a seamless boundary into an adjacent zone — the client
    /// doesn't see a full zone-in cutscene; we just move their projection.
    pub async fn do_seamless_zone_change(
        &self,
        actor_id: u32,
        session_id: u32,
        destination_zone_id: u32,
        position: Vector3,
    ) -> Result<()> {
        let Some(_dest_zone) = self.zone(destination_zone_id).await else {
            return Ok(());
        };

        // Pop the actor projection out of whatever zone held it, add it
        // to the new one, clear any merged-secondary-zone reference.
        let old_zone_id = {
            let sessions = self.sessions.read().await;
            sessions
                .get(&session_id)
                .map(|s| s.current_zone_id)
                .unwrap_or(0)
        };
        if let Some(old) = self.zone(old_zone_id).await {
            let mut ob = crate::zone::outbox::AreaOutbox::new();
            old.write().await.core.remove_actor(actor_id, &mut ob);
        }
        if let Some(dest) = self.zone(destination_zone_id).await {
            let mut ob = crate::zone::outbox::AreaOutbox::new();
            dest.write().await.core.add_actor(
                crate::zone::area::StoredActor {
                    actor_id,
                    kind: crate::zone::area::ActorKind::Player,
                    position,
                    grid: (0, 0),
                    is_alive: true,
                },
                &mut ob,
            );
        }

        // Update session bookkeeping.
        let mut sessions = self.sessions.write().await;
        if let Some(session) = sessions.get_mut(&session_id) {
            session.current_zone_id = destination_zone_id;
        }
        Ok(())
    }

    /// Lightweight port of `MergeZones`. Pulls actors from `mergedZoneId`
    /// into the player's view (logically — the session carries `zoneId2`
    /// so range queries expand to include the secondary zone). No
    /// primary-zone change happens.
    pub async fn merge_zones(
        &self,
        actor_id: u32,
        _session_id: u32,
        merged_zone_id: u32,
        position: Vector3,
    ) -> Result<()> {
        let Some(merged) = self.zone(merged_zone_id).await else {
            return Ok(());
        };
        // Add a projection of the player into the merged zone too. The
        // game loop then broadcasts to the merged zone's grid as well.
        let mut ob = crate::zone::outbox::AreaOutbox::new();
        merged.write().await.core.add_actor(
            crate::zone::area::StoredActor {
                actor_id,
                kind: crate::zone::area::ActorKind::Player,
                position,
                grid: (0, 0),
                is_alive: true,
            },
            &mut ob,
        );
        Ok(())
    }

    /// `SeamlessCheck(player)` port. Drives all three possible outcomes:
    ///
    /// * Inside zone-1 box but primary zone isn't zone 1 → fire
    ///   `do_seamless_zone_change` to zone 1.
    /// * Inside zone-2 box but primary zone isn't zone 2 → fire to zone 2.
    /// * Inside the merge strip → fire `merge_zones` with whichever zone
    ///   isn't already primary.
    pub async fn seamless_check(
        &self,
        actor_id: u32,
        session_id: u32,
        position: Vector3,
    ) -> SeamlessResult {
        // Which region is this player in?
        let (region_id, current_zone_id) = match self.session(session_id).await {
            Some(s) => {
                let zone = self.zone(s.current_zone_id).await;
                let region = match zone {
                    Some(z) => z.read().await.core.region_id as u32,
                    None => return SeamlessResult::None,
                };
                (region, s.current_zone_id)
            }
            None => return SeamlessResult::None,
        };

        let bounds = self.seamless_boundaries_for(region_id).await;
        for b in &bounds {
            if check_pos_in_bounds(
                position.x, position.z, b.zone1_x1, b.zone1_y1, b.zone1_x2, b.zone1_y2,
            ) {
                if current_zone_id == b.zone_id_1 {
                    return SeamlessResult::InsideZoneOne;
                }
                let _ = self
                    .do_seamless_zone_change(actor_id, session_id, b.zone_id_1, position)
                    .await;
                return SeamlessResult::ZoneChanged(b.zone_id_1);
            }
            if check_pos_in_bounds(
                position.x, position.z, b.zone2_x1, b.zone2_y1, b.zone2_x2, b.zone2_y2,
            ) {
                if current_zone_id == b.zone_id_2 {
                    return SeamlessResult::InsideZoneTwo;
                }
                let _ = self
                    .do_seamless_zone_change(actor_id, session_id, b.zone_id_2, position)
                    .await;
                return SeamlessResult::ZoneChanged(b.zone_id_2);
            }
            if check_pos_in_bounds(
                position.x, position.z, b.merge_x1, b.merge_y1, b.merge_x2, b.merge_y2,
            ) {
                let merged = if current_zone_id == b.zone_id_1 {
                    b.zone_id_2
                } else {
                    b.zone_id_1
                };
                let _ = self
                    .merge_zones(actor_id, session_id, merged, position)
                    .await;
                return SeamlessResult::ZoneMerged(merged);
            }
        }
        SeamlessResult::None
    }

    /// Move an actor *within* its current zone — updates the spatial
    /// grid so broadcast fan-out stays accurate. Called from the
    /// packet-processor's `UpdatePlayerPosition` handler.
    pub async fn update_actor_position(
        &self,
        actor_id: u32,
        session_id: u32,
        new_position: Vector3,
    ) {
        let zone_id = match self.session(session_id).await {
            Some(s) => s.current_zone_id,
            None => return,
        };
        let Some(zone_arc) = self.zone(zone_id).await else {
            return;
        };
        let mut zone = zone_arc.write().await;
        let mut ob = crate::zone::outbox::AreaOutbox::new();
        zone.core
            .update_actor_position(actor_id, new_position, &mut ob);
    }
}

impl Default for WorldManager {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::zone::navmesh::StubNavmeshLoader;

    fn mk_zone(id: u32, name: &str, region: u16) -> Zone {
        Zone::new(
            id,
            name,
            region,
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

    #[tokio::test]
    async fn seamless_check_zone_1_no_change_when_already_primary() {
        let wm = WorldManager::new();
        wm.register_zone(mk_zone(1, "east_thanalan", 103)).await;
        wm.register_zone(mk_zone(2, "central_thanalan", 103)).await;

        // Install a session that's already primary to zone 1.
        let mut s = Session::new(42);
        s.current_zone_id = 1;
        wm.upsert_session(s).await;

        // Install a boundary that wraps (−10..10, −10..10) around origin for
        // zone 1; zone 2 box is elsewhere; merge box is a tiny strip.
        let boundary = SeamlessBoundary {
            id: 1,
            region_id: 103,
            zone_id_1: 1,
            zone_id_2: 2,
            zone1_x1: -10.0,
            zone1_y1: -10.0,
            zone1_x2: 10.0,
            zone1_y2: 10.0,
            zone2_x1: 100.0,
            zone2_y1: 100.0,
            zone2_x2: 110.0,
            zone2_y2: 110.0,
            merge_x1: 20.0,
            merge_y1: 20.0,
            merge_x2: 30.0,
            merge_y2: 30.0,
        };
        wm.seamless_boundaries
            .write()
            .await
            .entry(103)
            .or_default()
            .push(boundary);

        let result = wm
            .seamless_check(100, 42, Vector3::new(0.0, 0.0, 0.0))
            .await;
        assert_eq!(result, SeamlessResult::InsideZoneOne);
    }

    #[tokio::test]
    async fn seamless_check_fires_zone_change_when_entering_zone_2_box() {
        let wm = WorldManager::new();
        wm.register_zone(mk_zone(1, "east", 103)).await;
        wm.register_zone(mk_zone(2, "central", 103)).await;

        let mut s = Session::new(42);
        s.current_zone_id = 1;
        wm.upsert_session(s).await;

        let boundary = SeamlessBoundary {
            id: 1,
            region_id: 103,
            zone_id_1: 1,
            zone_id_2: 2,
            zone1_x1: -10.0,
            zone1_y1: -10.0,
            zone1_x2: 10.0,
            zone1_y2: 10.0,
            zone2_x1: 100.0,
            zone2_y1: 100.0,
            zone2_x2: 110.0,
            zone2_y2: 110.0,
            merge_x1: 20.0,
            merge_y1: 20.0,
            merge_x2: 30.0,
            merge_y2: 30.0,
        };
        wm.seamless_boundaries
            .write()
            .await
            .entry(103)
            .or_default()
            .push(boundary);

        let result = wm
            .seamless_check(100, 42, Vector3::new(105.0, 0.0, 105.0))
            .await;
        assert_eq!(result, SeamlessResult::ZoneChanged(2));
        // And the session now reflects the new primary zone.
        let updated = wm.session(42).await.unwrap();
        assert_eq!(updated.current_zone_id, 2);
    }

    #[tokio::test]
    async fn seamless_check_merges_in_merge_strip() {
        let wm = WorldManager::new();
        wm.register_zone(mk_zone(1, "east", 103)).await;
        wm.register_zone(mk_zone(2, "central", 103)).await;

        let mut s = Session::new(42);
        s.current_zone_id = 1;
        wm.upsert_session(s).await;

        let boundary = SeamlessBoundary {
            id: 1,
            region_id: 103,
            zone_id_1: 1,
            zone_id_2: 2,
            zone1_x1: -10.0,
            zone1_y1: -10.0,
            zone1_x2: 10.0,
            zone1_y2: 10.0,
            zone2_x1: 100.0,
            zone2_y1: 100.0,
            zone2_x2: 110.0,
            zone2_y2: 110.0,
            merge_x1: 20.0,
            merge_y1: 20.0,
            merge_x2: 30.0,
            merge_y2: 30.0,
        };
        wm.seamless_boundaries
            .write()
            .await
            .entry(103)
            .or_default()
            .push(boundary);

        let result = wm
            .seamless_check(100, 42, Vector3::new(25.0, 0.0, 25.0))
            .await;
        assert_eq!(result, SeamlessResult::ZoneMerged(2));
        // Session's primary zone is unchanged; the secondary is merged.
        assert_eq!(wm.session(42).await.unwrap().current_zone_id, 1);
    }

    #[tokio::test]
    async fn do_zone_change_moves_actor_between_zones() {
        let wm = WorldManager::new();
        wm.register_zone(mk_zone(1, "east", 103)).await;
        wm.register_zone(mk_zone(2, "central", 103)).await;
        let mut s = Session::new(42);
        s.current_zone_id = 1;
        wm.upsert_session(s).await;

        // Pre-populate zone 1 with the player projection.
        {
            let z = wm.zone(1).await.unwrap();
            let mut z = z.write().await;
            let mut ob = crate::zone::outbox::AreaOutbox::new();
            z.core.add_actor(
                crate::zone::area::StoredActor {
                    actor_id: 100,
                    kind: crate::zone::area::ActorKind::Player,
                    position: Vector3::ZERO,
                    grid: (0, 0),
                    is_alive: true,
                },
                &mut ob,
            );
        }

        wm.do_zone_change(100, 42, 2, Vector3::new(50.0, 0.0, 50.0), 0.0)
            .await
            .unwrap();

        assert!(!wm.zone(1).await.unwrap().read().await.core.contains(100));
        assert!(wm.zone(2).await.unwrap().read().await.core.contains(100));
    }
}
