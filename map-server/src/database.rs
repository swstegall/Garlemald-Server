//! Map-server DB layer. Full port of `Map Server/Database.cs` (2782 lines,
//! 52 public static methods) against tokio-rusqlite with a single background
//! connection to the shared SQLite file (see `common::db::open_or_create`).
//!
//! Every method maps 1:1 to a C# counterpart; names are snake_cased. Where
//! the C# signature took a Player and mutated it, the Rust equivalent takes
//! the `chara_id` and returns a DTO so call sites can do the mutation.
#![allow(dead_code)]

use std::collections::HashMap;
use std::path::Path;

use anyhow::{Context, Result};
use rusqlite::{OptionalExtension, named_params};
use common::db::ConnCallExt;
use tokio_rusqlite::Connection;

use crate::data::{InventoryItem, ItemData, ItemTag, SeamlessBoundary, ZoneEntrance};
use crate::gamedata::{
    AppearanceFull, BattleCommand, BattleTrait, CharaBattleSave, CharaParameterSave, ChocoboData,
    EquipmentSlot, GuildleveGamedata, GuildleveLocalEntry, GuildleveRegionalEntry, HotbarEntry,
    ItemDealingInfo, ItemModifiers, LoadedPlayer, NpcLinkshellEntry, QuestScenarioEntry,
    StatusEffectDef, StatusEffectEntry, TIMER_COLUMNS, class_column,
};

/// One row of `server_zones` — the on-disk record for a zone template.
#[derive(Debug, Clone, Default)]
pub struct ZoneRow {
    pub id: u32,
    pub zone_name: String,
    pub region_id: u16,
    pub class_path: String,
    pub bgm_day: u16,
    pub bgm_night: u16,
    pub bgm_battle: u16,
    pub is_isolated: bool,
    pub is_inn: bool,
    pub can_ride_chocobo: bool,
    pub can_stealth: bool,
    pub is_instance_raid: bool,
    pub load_nav_mesh: bool,
}

/// One row of `server_zones_privateareas`.
#[derive(Debug, Clone, Default)]
pub struct PrivateAreaRow {
    pub id: u32,
    pub parent_zone_id: u32,
    pub private_area_name: String,
    pub private_area_type: u32,
    pub class_name: String,
    pub bgm_day: u16,
    pub bgm_night: u16,
    pub bgm_battle: u16,
}

pub struct Database {
    conn: Connection,
}

impl Database {
    pub async fn open(path: impl AsRef<Path>) -> Result<Self> {
        let conn = common::db::open_or_create(path).await?;
        Ok(Self { conn })
    }

    pub async fn ping(&self) -> Result<()> {
        self.conn
            .call_db(|c| {
                c.query_row("SELECT 1", [], |_| Ok(()))?;
                Ok(())
            })
            .await
            .context("ping")
    }

    // =======================================================================
    // Zone / area loaders (called at boot by WorldManager)
    // =======================================================================

    pub async fn load_zones(&self, server_ip: &str, server_port: u16) -> Result<Vec<ZoneRow>> {
        tracing::debug!(server_ip, server_port, "db: load_zones");
        let server_ip = server_ip.to_owned();
        let rows = self.conn
            .call_db(move |c| {
                let mut stmt = c.prepare(
                    r"SELECT id, zoneName, regionId, classPath,
                             dayMusic, nightMusic, battleMusic,
                             isIsolated, isInn, canRideChocobo, canStealth,
                             isInstanceRaid, loadNavMesh
                      FROM server_zones
                      WHERE zoneName IS NOT NULL AND serverIp = :ip AND serverPort = :port",
                )?;
                let rows: Vec<ZoneRow> = stmt
                    .query_map(
                        named_params! { ":ip": server_ip, ":port": server_port },
                        |r| {
                            Ok(ZoneRow {
                                id: r.get::<_, u32>(0)?,
                                zone_name: r.get::<_, String>(1).unwrap_or_default(),
                                region_id: r.get::<_, u16>(2).unwrap_or_default(),
                                class_path: r.get::<_, String>(3).unwrap_or_default(),
                                bgm_day: r.get::<_, u16>(4).unwrap_or_default(),
                                bgm_night: r.get::<_, u16>(5).unwrap_or_default(),
                                bgm_battle: r.get::<_, u16>(6).unwrap_or_default(),
                                is_isolated: r.get::<_, i64>(7).unwrap_or(0) != 0,
                                is_inn: r.get::<_, i64>(8).unwrap_or(0) != 0,
                                can_ride_chocobo: r.get::<_, i64>(9).unwrap_or(0) != 0,
                                can_stealth: r.get::<_, i64>(10).unwrap_or(0) != 0,
                                is_instance_raid: r.get::<_, i64>(11).unwrap_or(0) != 0,
                                load_nav_mesh: r.get::<_, i64>(12).unwrap_or(0) != 0,
                            })
                        },
                    )?
                    .collect::<rusqlite::Result<_>>()?;
                Ok(rows)
            })
            .await?;
        Ok(rows)
    }

    pub async fn load_private_areas(&self) -> Result<Vec<PrivateAreaRow>> {
        let rows = self.conn
            .call_db(|c| {
                let mut stmt = c.prepare(
                    r"SELECT id, parentZoneId, privateAreaName, privateAreaType,
                             className, dayMusic, nightMusic, battleMusic
                      FROM server_zones_privateareas
                      WHERE privateAreaName IS NOT NULL",
                )?;
                let rows: Vec<PrivateAreaRow> = stmt
                    .query_map([], |r| {
                        Ok(PrivateAreaRow {
                            id: r.get(0)?,
                            parent_zone_id: r.get(1)?,
                            private_area_name: r.get::<_, String>(2).unwrap_or_default(),
                            private_area_type: r.get(3)?,
                            class_name: r.get::<_, String>(4).unwrap_or_default(),
                            bgm_day: r.get::<_, u16>(5).unwrap_or_default(),
                            bgm_night: r.get::<_, u16>(6).unwrap_or_default(),
                            bgm_battle: r.get::<_, u16>(7).unwrap_or_default(),
                        })
                    })?
                    .collect::<rusqlite::Result<_>>()?;
                Ok(rows)
            })
            .await?;
        Ok(rows)
    }

    pub async fn load_zone_entrances(&self) -> Result<HashMap<u32, ZoneEntrance>> {
        let rows = self.conn
            .call_db(|c| {
                let mut stmt = c.prepare(
                    r"SELECT id, zoneId, spawnType, spawnX, spawnY, spawnZ, spawnRotation,
                             privateAreaName
                      FROM server_zones_spawnlocations",
                )?;
                let rows: Vec<(u32, ZoneEntrance)> = stmt
                    .query_map([], |r| {
                        let id: u32 = r.get(0)?;
                        Ok((
                            id,
                            ZoneEntrance {
                                id,
                                zone_id: r.get(1)?,
                                spawn_type: r.get::<_, u8>(2).unwrap_or_default(),
                                spawn_x: r.get::<_, f32>(3).unwrap_or_default(),
                                spawn_y: r.get::<_, f32>(4).unwrap_or_default(),
                                spawn_z: r.get::<_, f32>(5).unwrap_or_default(),
                                spawn_rotation: r.get::<_, f32>(6).unwrap_or_default(),
                                private_area_name: r.get::<_, Option<String>>(7).unwrap_or(None),
                                private_area_level: 1,
                            },
                        ))
                    })?
                    .collect::<rusqlite::Result<_>>()?;
                Ok(rows)
            })
            .await?;
        Ok(rows.into_iter().collect())
    }

    pub async fn load_actor_classes(&self) -> Result<HashMap<u32, crate::npc::ActorClass>> {
        tracing::debug!("db: load_actor_classes");
        let rows = self.conn
            .call_db(|c| {
                let mut stmt = c.prepare(
                    r"SELECT ac.id, ac.classPath, ac.displayNameId, ac.propertyFlags,
                             ac.eventConditions,
                             pc.pushCommand, pc.pushCommandSub, pc.pushCommandPriority
                      FROM gamedata_actor_class ac
                      LEFT JOIN gamedata_actor_pushcommand pc ON ac.id = pc.id",
                )?;
                let rows: Vec<(u32, crate::npc::ActorClass)> = stmt
                    .query_map([], |r| {
                        let id: u32 = r.get(0)?;
                        Ok((
                            id,
                            crate::npc::ActorClass::new(
                                id,
                                r.get::<_, String>(1).unwrap_or_default(),
                                r.get::<_, u32>(2).unwrap_or_default(),
                                r.get::<_, u32>(3).unwrap_or_default(),
                                r.get::<_, String>(4).unwrap_or_default(),
                                r.get::<_, u16>(5).unwrap_or_default(),
                                r.get::<_, u16>(6).unwrap_or_default(),
                                r.get::<_, u8>(7).unwrap_or_default(),
                            ),
                        ))
                    })?
                    .collect::<rusqlite::Result<_>>()?;
                Ok(rows)
            })
            .await?;
        Ok(rows.into_iter().collect())
    }

    pub async fn load_npc_spawn_locations(&self) -> Result<Vec<crate::zone::SpawnLocation>> {
        let rows = self.conn
            .call_db(|c| {
                let mut stmt = c.prepare(
                    r"SELECT actorClassId, uniqueId, zoneId,
                             privateAreaName, privateAreaLevel,
                             positionX, positionY, positionZ, rotation,
                             actorState, animationId
                      FROM server_spawn_locations",
                )?;
                let rows: Vec<crate::zone::SpawnLocation> = stmt
                    .query_map([], |r| {
                        Ok(crate::zone::SpawnLocation {
                            class_id: r.get(0)?,
                            unique_id: r.get::<_, String>(1).unwrap_or_default(),
                            zone_id: r.get(2)?,
                            private_area_name: r.get::<_, String>(3).unwrap_or_default(),
                            private_area_level: r.get::<_, u32>(4).unwrap_or_default(),
                            x: r.get::<_, f32>(5).unwrap_or_default(),
                            y: r.get::<_, f32>(6).unwrap_or_default(),
                            z: r.get::<_, f32>(7).unwrap_or_default(),
                            rotation: r.get::<_, f32>(8).unwrap_or_default(),
                            state: r.get::<_, u16>(9).unwrap_or_default(),
                            animation_id: r.get::<_, u32>(10).unwrap_or_default(),
                        })
                    })?
                    .collect::<rusqlite::Result<_>>()?;
                Ok(rows)
            })
            .await?;
        Ok(rows)
    }

    pub async fn load_seamless_boundaries(&self) -> Result<HashMap<u32, Vec<SeamlessBoundary>>> {
        let rows = self.conn
            .call_db(|c| {
                let mut stmt = c.prepare(
                    r"SELECT id, regionId, zoneId1, zoneId2,
                             zone1_boundingbox_x1, zone1_boundingbox_y1,
                             zone1_boundingbox_x2, zone1_boundingbox_y2,
                             zone2_boundingbox_x1, zone2_boundingbox_y1,
                             zone2_boundingbox_x2, zone2_boundingbox_y2,
                             merge_boundingbox_x1, merge_boundingbox_y1,
                             merge_boundingbox_x2, merge_boundingbox_y2
                      FROM server_seamless_zonechange_bounds",
                )?;
                let rows: Vec<SeamlessBoundary> = stmt
                    .query_map([], |r| {
                        Ok(SeamlessBoundary {
                            id: r.get(0)?,
                            region_id: r.get(1)?,
                            zone_id_1: r.get(2)?,
                            zone_id_2: r.get(3)?,
                            zone1_x1: r.get(4)?,
                            zone1_y1: r.get(5)?,
                            zone1_x2: r.get(6)?,
                            zone1_y2: r.get(7)?,
                            zone2_x1: r.get(8)?,
                            zone2_y1: r.get(9)?,
                            zone2_x2: r.get(10)?,
                            zone2_y2: r.get(11)?,
                            merge_x1: r.get(12)?,
                            merge_y1: r.get(13)?,
                            merge_x2: r.get(14)?,
                            merge_y2: r.get(15)?,
                        })
                    })?
                    .collect::<rusqlite::Result<_>>()?;
                Ok(rows)
            })
            .await?;
        let mut out: HashMap<u32, Vec<SeamlessBoundary>> = HashMap::new();
        for b in rows {
            out.entry(b.region_id).or_default().push(b);
        }
        Ok(out)
    }

    // =======================================================================
    // Session
    // =======================================================================

    pub async fn user_id_from_session(&self, session_id: &str) -> Result<u32> {
        tracing::debug!(len = session_id.len(), "db: user_id_from_session");
        let sid = session_id.to_owned();
        let id = self.conn
            .call_db(move |c| {
                let v = c
                    .query_row(
                        "SELECT userId FROM sessions WHERE id = :sid AND expiration > datetime('now')",
                        named_params! { ":sid": sid },
                        |r| r.get::<_, u32>(0),
                    )
                    .optional()?;
                Ok(v)
            })
            .await?;
        Ok(id.unwrap_or(0))
    }

    // =======================================================================
    // Gamedata loaders (called once at startup)
    // =======================================================================

    pub async fn get_item_gamedata(&self) -> Result<HashMap<u32, ItemData>> {
        let rows = self.conn
            .call_db(|c| {
                let mut stmt = c.prepare(
                    r"SELECT i.catalogID, i.name, i.singular, i.plural, i.icon, i.rarity,
                             i.itemUICategory, i.stackSize, i.itemLevel, i.equipLevel,
                             i.price, i.buyPrice, i.sellPrice
                      FROM gamedata_items i
                      LEFT JOIN gamedata_items_equipment      e ON i.catalogID = e.catalogID
                      LEFT JOIN gamedata_items_accessory      a ON i.catalogID = a.catalogID
                      LEFT JOIN gamedata_items_armor          ar ON i.catalogID = ar.catalogID
                      LEFT JOIN gamedata_items_weapon         w ON i.catalogID = w.catalogID
                      LEFT JOIN gamedata_items_graphics       g ON i.catalogID = g.catalogID
                      LEFT JOIN gamedata_items_graphics_extra gx ON i.catalogID = gx.catalogID",
                )?;
                let rows: Vec<(u32, ItemData)> = stmt
                    .query_map([], |r| {
                        let id: u32 = r.get(0)?;
                        Ok((
                            id,
                            ItemData {
                                id,
                                name: r.get::<_, String>(1).unwrap_or_default(),
                                singular: r.get::<_, String>(2).unwrap_or_default(),
                                plural: r.get::<_, String>(3).unwrap_or_default(),
                                icon: r.get::<_, u16>(4).unwrap_or_default(),
                                rarity: r.get::<_, u16>(5).unwrap_or_default(),
                                item_ui_category: r.get::<_, u16>(6).unwrap_or_default(),
                                stack_size: r.get::<_, u32>(7).unwrap_or_default(),
                                item_level: r.get::<_, u16>(8).unwrap_or_default(),
                                equip_level: r.get::<_, u16>(9).unwrap_or_default(),
                                price: r.get::<_, u32>(10).unwrap_or_default(),
                                buy_price: r.get::<_, u32>(11).unwrap_or_default(),
                                sell_price: r.get::<_, u32>(12).unwrap_or_default(),
                                ..Default::default()
                            },
                        ))
                    })?
                    .collect::<rusqlite::Result<_>>()?;
                Ok(rows)
            })
            .await?;
        Ok(rows.into_iter().collect())
    }

    pub async fn get_guildleve_gamedata(&self) -> Result<HashMap<u32, GuildleveGamedata>> {
        let rows = self.conn
            .call_db(|c| {
                let mut stmt = c.prepare(
                    r"SELECT id, zoneId, name, difficulty, leveType, rewardExp, rewardGil
                      FROM gamedata_guildleves",
                )?;
                let rows: Vec<(u32, GuildleveGamedata)> = stmt
                    .query_map([], |r| {
                        let id: u32 = r.get(0)?;
                        Ok((
                            id,
                            GuildleveGamedata {
                                id,
                                zone_id: r.get::<_, u32>(1).unwrap_or_default(),
                                name: r.get::<_, String>(2).unwrap_or_default(),
                                difficulty: r.get::<_, u8>(3).unwrap_or_default(),
                                leve_type: r.get::<_, u8>(4).unwrap_or_default(),
                                reward_exp: r.get::<_, u32>(5).unwrap_or_default(),
                                reward_gil: r.get::<_, u32>(6).unwrap_or_default(),
                            },
                        ))
                    })?
                    .collect::<rusqlite::Result<_>>()?;
                Ok(rows)
            })
            .await?;
        Ok(rows.into_iter().collect())
    }

    pub async fn load_global_status_effect_list(&self) -> Result<HashMap<u32, StatusEffectDef>> {
        let rows = self.conn
            .call_db(|c| {
                let mut stmt = c.prepare(
                    r"SELECT id, name, flags, overwrite, tickMs, hidden, silentOnGain,
                             silentOnLoss, statusGainTextId, statusLossTextId
                      FROM server_statuseffects",
                )?;
                let rows: Vec<(u32, StatusEffectDef)> = stmt
                    .query_map([], |r| {
                        let id: u32 = r.get(0)?;
                        Ok((
                            id,
                            StatusEffectDef {
                                id,
                                name: r.get::<_, String>(1).unwrap_or_default(),
                                flags: r.get::<_, u32>(2).unwrap_or_default(),
                                overwrite: r.get::<_, u8>(3).unwrap_or_default(),
                                tick_ms: r.get::<_, u32>(4).unwrap_or_default(),
                                hidden: r.get::<_, i64>(5).unwrap_or(0) != 0,
                                silent_on_gain: r.get::<_, i64>(6).unwrap_or(0) != 0,
                                silent_on_loss: r.get::<_, i64>(7).unwrap_or(0) != 0,
                                status_gain_text_id: r.get::<_, u16>(8).unwrap_or_default(),
                                status_loss_text_id: r.get::<_, u16>(9).unwrap_or_default(),
                            },
                        ))
                    })?
                    .collect::<rusqlite::Result<_>>()?;
                Ok(rows)
            })
            .await?;
        Ok(rows.into_iter().collect())
    }

    pub async fn load_global_battle_command_list(
        &self,
    ) -> Result<(HashMap<u16, BattleCommand>, HashMap<(u8, i16), Vec<u16>>)> {
        let rows = self.conn
            .call_db(|c| {
                let mut stmt = c.prepare(
                    r#"SELECT id, name, classJob, lvl, requirements, mainTarget,
                              validTarget, aoeType, aoeRange, aoeMinRange, aoeConeAngle,
                              aoeRotateAngle, aoeTarget, basePotency, numHits,
                              positionBonus, procRequirement, "range", minRange,
                              rangeHeight, rangeWidth, statusId, statusDuration,
                              statusChance, castType, castTime, recastTime, mpCost,
                              tpCost, animationType, effectAnimation, modelAnimation,
                              animationDuration, battleAnimation, validUser, comboId1,
                              comboId2, comboStep, accuracyMod, worldMasterTextId,
                              commandType, actionType, actionProperty
                       FROM server_battle_commands"#,
                )?;
                let rows: Vec<BattleCommand> = stmt
                    .query_map([], |r| {
                        let recast_s: u32 = r.get::<_, u32>(26).unwrap_or_default();
                        Ok(BattleCommand {
                            id: r.get::<_, u16>(0)?,
                            name: r.get::<_, String>(1).unwrap_or_default(),
                            job: r.get::<_, u8>(2).unwrap_or_default(),
                            level: r.get::<_, u8>(3).unwrap_or_default(),
                            requirements: r.get::<_, u16>(4).unwrap_or_default(),
                            main_target: r.get::<_, u16>(5).unwrap_or_default(),
                            valid_target: r.get::<_, u16>(6).unwrap_or_default(),
                            aoe_type: r.get::<_, u8>(7).unwrap_or_default(),
                            aoe_range: r.get::<_, f32>(8).unwrap_or_default(),
                            aoe_min_range: r.get::<_, f32>(9).unwrap_or_default(),
                            aoe_cone_angle: r.get::<_, f32>(10).unwrap_or_default(),
                            aoe_rotate_angle: r.get::<_, f32>(11).unwrap_or_default(),
                            aoe_target: r.get::<_, u8>(12).unwrap_or_default(),
                            base_potency: r.get::<_, u16>(13).unwrap_or_default(),
                            num_hits: r.get::<_, u8>(14).unwrap_or_default(),
                            position_bonus: r.get::<_, u8>(15).unwrap_or_default(),
                            proc_requirement: r.get::<_, u8>(16).unwrap_or_default(),
                            range: r.get::<_, f32>(17).unwrap_or_default(),
                            min_range: r.get::<_, f32>(18).unwrap_or_default(),
                            range_height: r.get::<_, i32>(19).unwrap_or_default(),
                            range_width: r.get::<_, i32>(20).unwrap_or_default(),
                            status_id: r.get::<_, u32>(21).unwrap_or_default(),
                            status_duration: r.get::<_, u32>(22).unwrap_or_default(),
                            status_chance: r.get::<_, f32>(23).unwrap_or_default(),
                            cast_type: r.get::<_, u8>(24).unwrap_or_default(),
                            cast_time_ms: r.get::<_, u32>(25).unwrap_or_default(),
                            max_recast_time_seconds: recast_s,
                            recast_time_ms: recast_s * 1000,
                            mp_cost: r.get::<_, i16>(27).unwrap_or_default(),
                            tp_cost: r.get::<_, i16>(28).unwrap_or_default(),
                            animation_type: r.get::<_, u8>(29).unwrap_or_default(),
                            effect_animation: r.get::<_, u16>(30).unwrap_or_default(),
                            model_animation: r.get::<_, u16>(31).unwrap_or_default(),
                            animation_duration_seconds: r.get::<_, u16>(32).unwrap_or_default(),
                            battle_animation: r.get::<_, u32>(33).unwrap_or_default(),
                            valid_user: r.get::<_, u8>(34).unwrap_or_default(),
                            combo_next_command_id: [
                                r.get::<_, i32>(35).unwrap_or_default(),
                                r.get::<_, i32>(36).unwrap_or_default(),
                            ],
                            combo_step: r.get::<_, i16>(37).unwrap_or_default(),
                            accuracy_modifier: r.get::<_, f32>(38).unwrap_or_default(),
                            world_master_text_id: r.get::<_, u16>(39).unwrap_or_default(),
                            command_type: r.get::<_, i16>(40).unwrap_or_default(),
                            action_type: r.get::<_, i16>(41).unwrap_or_default(),
                            action_property: r.get::<_, i16>(42).unwrap_or_default(),
                        })
                    })?
                    .collect::<rusqlite::Result<_>>()?;
                Ok(rows)
            })
            .await?;

        let mut dict = HashMap::with_capacity(rows.len());
        let mut by_level: HashMap<(u8, i16), Vec<u16>> = HashMap::new();
        for bc in rows {
            by_level.entry((bc.job, bc.level as i16)).or_default().push(bc.id);
            dict.insert(bc.id, bc);
        }
        Ok((dict, by_level))
    }

    pub async fn load_global_battle_trait_list(
        &self,
    ) -> Result<(HashMap<u16, BattleTrait>, HashMap<u8, Vec<u16>>)> {
        let rows = self.conn
            .call_db(|c| {
                let mut stmt = c.prepare(
                    "SELECT id, name, classJob, lvl, modifier, bonus FROM server_battle_traits",
                )?;
                let rows: Vec<BattleTrait> = stmt
                    .query_map([], |r| {
                        Ok(BattleTrait {
                            id: r.get::<_, u16>(0)?,
                            name: r.get::<_, String>(1).unwrap_or_default(),
                            job: r.get::<_, u8>(2).unwrap_or_default(),
                            level: r.get::<_, u8>(3).unwrap_or_default(),
                            modifier: r.get::<_, u32>(4).unwrap_or_default(),
                            bonus: r.get::<_, i32>(5).unwrap_or_default(),
                        })
                    })?
                    .collect::<rusqlite::Result<_>>()?;
                Ok(rows)
            })
            .await?;

        let mut dict = HashMap::with_capacity(rows.len());
        let mut by_job: HashMap<u8, Vec<u16>> = HashMap::new();
        for t in rows {
            by_job.entry(t.job).or_default().push(t.id);
            dict.insert(t.id, t);
        }
        Ok((dict, by_job))
    }

    // =======================================================================
    // LoadPlayerCharacter — aggregates 10+ independent SELECTs
    // =======================================================================

    pub async fn load_player_character(&self, chara_id: u32) -> Result<Option<LoadedPlayer>> {
        tracing::debug!(chara_id, "db: load_player_character");
        let basic = self.conn
            .call_db(move |c| {
                let v = c
                    .query_row(
                        r"SELECT name, positionX, positionY, positionZ, rotation,
                                 actorState, currentZoneId, gcCurrent, gcLimsaRank,
                                 gcGridaniaRank, gcUldahRank, currentTitle, guardian,
                                 birthDay, birthMonth, initialTown, tribe, restBonus,
                                 achievementPoints, playTime, destinationZoneId,
                                 destinationSpawnType, currentPrivateArea,
                                 currentPrivateAreaType, homepoint, homepointInn
                          FROM characters WHERE id = :cid",
                        named_params! { ":cid": chara_id },
                        |r| {
                            Ok(LoadedPlayer {
                                name: r.get::<_, String>(0).unwrap_or_default(),
                                position_x: r.get::<_, f32>(1).unwrap_or_default(),
                                position_y: r.get::<_, f32>(2).unwrap_or_default(),
                                position_z: r.get::<_, f32>(3).unwrap_or_default(),
                                rotation: r.get::<_, f32>(4).unwrap_or_default(),
                                actor_state: r.get::<_, u16>(5).unwrap_or_default(),
                                current_zone_id: r.get::<_, u32>(6).unwrap_or_default(),
                                gc_current: r.get::<_, u8>(7).unwrap_or_default(),
                                gc_limsa_rank: r.get::<_, u8>(8).unwrap_or_default(),
                                gc_gridania_rank: r.get::<_, u8>(9).unwrap_or_default(),
                                gc_uldah_rank: r.get::<_, u8>(10).unwrap_or_default(),
                                current_title: r.get::<_, u32>(11).unwrap_or_default(),
                                guardian: r.get::<_, u8>(12).unwrap_or_default(),
                                birth_day: r.get::<_, u8>(13).unwrap_or_default(),
                                birth_month: r.get::<_, u8>(14).unwrap_or_default(),
                                initial_town: r.get::<_, u8>(15).unwrap_or_default(),
                                tribe: r.get::<_, u8>(16).unwrap_or_default(),
                                rest_bonus_exp_rate: r.get::<_, i32>(17).unwrap_or_default(),
                                achievement_points: r.get::<_, u32>(18).unwrap_or_default(),
                                play_time: r.get::<_, u32>(19).unwrap_or_default(),
                                destination_zone_id: r.get::<_, u32>(20).unwrap_or_default(),
                                destination_spawn_type: r.get::<_, u8>(21).unwrap_or_default(),
                                current_private_area: r.get::<_, String>(22).unwrap_or_default(),
                                current_private_area_type: r.get::<_, u32>(23).unwrap_or_default(),
                                homepoint: r.get::<_, u32>(24).unwrap_or_default(),
                                homepoint_inn: r.get::<_, u8>(25).unwrap_or_default(),
                                ..Default::default()
                            })
                        },
                    )
                    .optional()?;
                Ok(v)
            })
            .await?;

        let Some(mut player) = basic else {
            return Ok(None);
        };

        if player.destination_zone_id != 0 {
            player.current_zone_id = player.destination_zone_id;
        }

        player.class_levels = self.load_class_levels_and_exp(chara_id).await.unwrap_or_default();
        player.parameter_save = self.load_parameter_save(chara_id).await.unwrap_or_default();
        player.appearance = self.load_appearance_full(chara_id).await.unwrap_or_default();
        player.status_effects = self.load_character_status_effects(chara_id).await.unwrap_or_default();
        player.chocobo = self.load_chocobo(chara_id).await.unwrap_or_default();
        player.timers = self.load_timers(chara_id).await.unwrap_or_default();
        player.hotbar = self
            .load_hotbar(chara_id, player.parameter_save.state_main_skill[0])
            .await
            .unwrap_or_default();
        player.quest_scenario = self.load_quest_scenario(chara_id).await.unwrap_or_default();
        player.guildleves_local = self.load_guildleves_local(chara_id).await.unwrap_or_default();
        player.guildleves_regional = self.load_guildleves_regional(chara_id).await.unwrap_or_default();
        player.npc_linkshells = self.load_npc_linkshells(chara_id).await.unwrap_or_default();

        for (target, ty) in [
            (&mut player.inventory_normal, 0u32),
            (&mut player.inventory_key_items, 1),
            (&mut player.inventory_currency, 2),
            (&mut player.inventory_bazaar, 3),
            (&mut player.inventory_meldrequest, 4),
            (&mut player.inventory_loot, 5),
        ] {
            *target = self.get_item_package(chara_id, ty).await.unwrap_or_default();
        }

        player.equipment = self
            .get_equipment(chara_id, player.parameter_save.state_main_skill[0] as u16)
            .await
            .unwrap_or_default();

        Ok(Some(player))
    }

    async fn load_class_levels_and_exp(&self, chara_id: u32) -> Result<CharaBattleSave> {
        let result = self.conn
            .call_db(move |c| {
                let columns: [u8; 18] = [
                    2, 3, 4, 7, 8, 22, 23, 29, 30, 31, 32, 33, 34, 35, 36, 39, 40, 41,
                ];
                let mut save = CharaBattleSave::default();
                let col_list =
                    "pug, gla, mrd, arc, lnc, thm, cnj, crp, bsm, arm, gsm, ltw, wvr, alc, cul, min, btn, fsh";

                let sql_lv = format!(
                    "SELECT {col_list} FROM characters_class_levels WHERE characterId = :cid"
                );
                let levels: Option<[i16; 18]> = c
                    .query_row(&sql_lv, named_params! { ":cid": chara_id }, |r| {
                        let mut vals = [0i16; 18];
                        for (i, v) in vals.iter_mut().enumerate() {
                            *v = r.get::<_, i16>(i).unwrap_or_default();
                        }
                        Ok(vals)
                    })
                    .optional()?;
                if let Some(vals) = levels {
                    for (i, cid) in columns.iter().enumerate() {
                        save.skill_level[(*cid - 1) as usize] = vals[i];
                    }
                }

                let sql_xp = format!(
                    "SELECT {col_list} FROM characters_class_exp WHERE characterId = :cid"
                );
                let exps: Option<[i32; 18]> = c
                    .query_row(&sql_xp, named_params! { ":cid": chara_id }, |r| {
                        let mut vals = [0i32; 18];
                        for (i, v) in vals.iter_mut().enumerate() {
                            *v = r.get::<_, i32>(i).unwrap_or_default();
                        }
                        Ok(vals)
                    })
                    .optional()?;
                if let Some(vals) = exps {
                    for (i, cid) in columns.iter().enumerate() {
                        save.skill_point[(*cid - 1) as usize] = vals[i];
                    }
                }

                Ok(save)
            })
            .await?;
        Ok(result)
    }

    async fn load_parameter_save(&self, chara_id: u32) -> Result<CharaParameterSave> {
        let v = self.conn
            .call_db(move |c| {
                let v = c
                    .query_row(
                        r"SELECT hp, hpMax, mp, mpMax, mainSkill
                          FROM characters_parametersave WHERE characterId = :cid",
                        named_params! { ":cid": chara_id },
                        |r| {
                            let mut save = CharaParameterSave::default();
                            save.hp[0] = r.get::<_, i16>(0).unwrap_or_default();
                            save.hp_max[0] = r.get::<_, i16>(1).unwrap_or_default();
                            save.mp = r.get::<_, i16>(2).unwrap_or_default();
                            save.mp_max = r.get::<_, i16>(3).unwrap_or_default();
                            save.state_main_skill[0] = r.get::<_, u8>(4).unwrap_or_default();
                            Ok(save)
                        },
                    )
                    .optional()?;
                Ok(v)
            })
            .await?;
        Ok(v.unwrap_or_default())
    }

    async fn load_appearance_full(&self, chara_id: u32) -> Result<AppearanceFull> {
        let v = self.conn
            .call_db(move |c| {
                let v = c
                    .query_row(
                        r"SELECT baseId, size, voice, skinColor, hairStyle, hairColor,
                                 hairHighlightColor, hairVariation, eyeColor, characteristics,
                                 characteristicsColor, faceType, ears, faceMouth,
                                 faceFeatures, faceNose, faceEyeShape, faceIrisSize,
                                 faceEyebrows, mainHand, offHand, head, body, legs, hands,
                                 feet, waist, neck, leftFinger, rightFinger, leftEar, rightEar
                          FROM characters_appearance WHERE characterId = :cid",
                        named_params! { ":cid": chara_id },
                        |r| {
                            Ok(AppearanceFull {
                                base_id: r.get::<_, u32>(0).unwrap_or(0xFFFFFFFF),
                                size: r.get::<_, u8>(1).unwrap_or_default(),
                                voice: r.get::<_, u8>(2).unwrap_or_default(),
                                skin_color: r.get::<_, u16>(3).unwrap_or_default(),
                                hair_style: r.get::<_, u16>(4).unwrap_or_default(),
                                hair_color: r.get::<_, u16>(5).unwrap_or_default(),
                                hair_highlight_color: r.get::<_, u16>(6).unwrap_or_default(),
                                hair_variation: r.get::<_, u16>(7).unwrap_or_default(),
                                eye_color: r.get::<_, u16>(8).unwrap_or_default(),
                                characteristics: r.get::<_, u8>(9).unwrap_or_default(),
                                characteristics_color: r.get::<_, u8>(10).unwrap_or_default(),
                                face_type: r.get::<_, u8>(11).unwrap_or_default(),
                                ears: r.get::<_, u8>(12).unwrap_or_default(),
                                face_mouth: r.get::<_, u8>(13).unwrap_or_default(),
                                face_features: r.get::<_, u8>(14).unwrap_or_default(),
                                face_nose: r.get::<_, u8>(15).unwrap_or_default(),
                                face_eye_shape: r.get::<_, u8>(16).unwrap_or_default(),
                                face_iris_size: r.get::<_, u8>(17).unwrap_or_default(),
                                face_eyebrows: r.get::<_, u8>(18).unwrap_or_default(),
                                main_hand: r.get::<_, u32>(19).unwrap_or_default(),
                                off_hand: r.get::<_, u32>(20).unwrap_or_default(),
                                head: r.get::<_, u32>(21).unwrap_or_default(),
                                body: r.get::<_, u32>(22).unwrap_or_default(),
                                legs: r.get::<_, u32>(23).unwrap_or_default(),
                                hands: r.get::<_, u32>(24).unwrap_or_default(),
                                feet: r.get::<_, u32>(25).unwrap_or_default(),
                                waist: r.get::<_, u32>(26).unwrap_or_default(),
                                neck: r.get::<_, u32>(27).unwrap_or_default(),
                                left_finger: r.get::<_, u32>(28).unwrap_or_default(),
                                right_finger: r.get::<_, u32>(29).unwrap_or_default(),
                                left_ear: r.get::<_, u32>(30).unwrap_or_default(),
                                right_ear: r.get::<_, u32>(31).unwrap_or_default(),
                            })
                        },
                    )
                    .optional()?;
                Ok(v)
            })
            .await?;
        Ok(v.unwrap_or_default())
    }

    async fn load_character_status_effects(
        &self,
        chara_id: u32,
    ) -> Result<Vec<StatusEffectEntry>> {
        let rows = self.conn
            .call_db(move |c| {
                let mut stmt = c.prepare(
                    r"SELECT statusId, duration, magnitude, tick, tier, extra
                      FROM characters_statuseffect WHERE characterId = :cid",
                )?;
                let rows: Vec<StatusEffectEntry> = stmt
                    .query_map(named_params! { ":cid": chara_id }, |r| {
                        Ok(StatusEffectEntry {
                            status_id: r.get::<_, u32>(0).unwrap_or_default(),
                            duration: r.get::<_, u32>(1).unwrap_or_default(),
                            magnitude: r.get::<_, u64>(2).unwrap_or_default(),
                            tick: r.get::<_, u32>(3).unwrap_or_default(),
                            tier: r.get::<_, u8>(4).unwrap_or_default(),
                            extra: r.get::<_, u64>(5).unwrap_or_default(),
                        })
                    })?
                    .collect::<rusqlite::Result<_>>()?;
                Ok(rows)
            })
            .await?;
        Ok(rows)
    }

    async fn load_chocobo(&self, chara_id: u32) -> Result<ChocoboData> {
        let v = self.conn
            .call_db(move |c| {
                let v = c
                    .query_row(
                        r"SELECT hasChocobo, hasGoobbue, chocoboAppearance, chocoboName
                          FROM characters_chocobo WHERE characterId = :cid",
                        named_params! { ":cid": chara_id },
                        |r| {
                            Ok(ChocoboData {
                                has_chocobo: r.get::<_, i64>(0).unwrap_or(0) != 0,
                                has_goobbue: r.get::<_, i64>(1).unwrap_or(0) != 0,
                                chocobo_appearance: r.get::<_, u8>(2).unwrap_or_default(),
                                chocobo_name: r.get::<_, String>(3).unwrap_or_default(),
                            })
                        },
                    )
                    .optional()?;
                Ok(v)
            })
            .await?;
        Ok(v.unwrap_or_default())
    }

    async fn load_timers(&self, chara_id: u32) -> Result<[u32; 20]> {
        let cols = TIMER_COLUMNS.join(", ");
        let sql = format!("SELECT {cols} FROM characters_timers WHERE characterId = :cid");
        let v = self.conn
            .call_db(move |c| {
                let v = c
                    .query_row(&sql, named_params! { ":cid": chara_id }, |r| {
                        let mut out = [0u32; 20];
                        for i in 0..20 {
                            out[i] = r.get::<_, u32>(i).unwrap_or_default();
                        }
                        Ok(out)
                    })
                    .optional()?;
                Ok(v)
            })
            .await?;
        Ok(v.unwrap_or([0u32; 20]))
    }

    async fn load_quest_scenario(&self, chara_id: u32) -> Result<Vec<QuestScenarioEntry>> {
        let rows = self.conn
            .call_db(move |c| {
                let mut stmt = c.prepare(
                    r"SELECT slot, questId, questData, questFlags, currentPhase
                      FROM characters_quest_scenario WHERE characterId = :cid",
                )?;
                let rows: Vec<QuestScenarioEntry> = stmt
                    .query_map(named_params! { ":cid": chara_id }, |r| {
                        Ok(QuestScenarioEntry {
                            slot: r.get::<_, u16>(0).unwrap_or_default(),
                            quest_id: r.get::<_, u32>(1).unwrap_or_default(),
                            quest_data: r
                                .get::<_, Option<String>>(2)
                                .unwrap_or(None)
                                .unwrap_or_else(|| "{}".to_string()),
                            quest_flags: r.get::<_, u32>(3).unwrap_or_default(),
                            current_phase: r.get::<_, u32>(4).unwrap_or_default(),
                        })
                    })?
                    .collect::<rusqlite::Result<_>>()?;
                Ok(rows)
            })
            .await?;
        Ok(rows)
    }

    async fn load_guildleves_local(&self, chara_id: u32) -> Result<Vec<GuildleveLocalEntry>> {
        let rows = self.conn
            .call_db(move |c| {
                let mut stmt = c.prepare(
                    r"SELECT slot, questId, abandoned, completed
                      FROM characters_quest_guildleve_local WHERE characterId = :cid",
                )?;
                let rows: Vec<GuildleveLocalEntry> = stmt
                    .query_map(named_params! { ":cid": chara_id }, |r| {
                        Ok(GuildleveLocalEntry {
                            slot: r.get::<_, u16>(0).unwrap_or_default(),
                            quest_id: r.get::<_, u32>(1).unwrap_or_default(),
                            abandoned: r.get::<_, i64>(2).unwrap_or(0) != 0,
                            completed: r.get::<_, i64>(3).unwrap_or(0) != 0,
                        })
                    })?
                    .collect::<rusqlite::Result<_>>()?;
                Ok(rows)
            })
            .await?;
        Ok(rows)
    }

    async fn load_guildleves_regional(
        &self,
        chara_id: u32,
    ) -> Result<Vec<GuildleveRegionalEntry>> {
        let rows = self.conn
            .call_db(move |c| {
                let mut stmt = c.prepare(
                    r"SELECT slot, guildleveId, abandoned, completed
                      FROM characters_quest_guildleve_regional WHERE characterId = :cid",
                )?;
                let rows: Vec<GuildleveRegionalEntry> = stmt
                    .query_map(named_params! { ":cid": chara_id }, |r| {
                        Ok(GuildleveRegionalEntry {
                            slot: r.get::<_, u16>(0).unwrap_or_default(),
                            guildleve_id: r.get::<_, u16>(1).unwrap_or_default(),
                            abandoned: r.get::<_, i64>(2).unwrap_or(0) != 0,
                            completed: r.get::<_, i64>(3).unwrap_or(0) != 0,
                        })
                    })?
                    .collect::<rusqlite::Result<_>>()?;
                Ok(rows)
            })
            .await?;
        Ok(rows)
    }

    async fn load_npc_linkshells(&self, chara_id: u32) -> Result<Vec<NpcLinkshellEntry>> {
        let rows = self.conn
            .call_db(move |c| {
                let mut stmt = c.prepare(
                    r"SELECT npcLinkshellId, isCalling, isExtra
                      FROM characters_npclinkshell WHERE characterId = :cid",
                )?;
                let rows: Vec<NpcLinkshellEntry> = stmt
                    .query_map(named_params! { ":cid": chara_id }, |r| {
                        Ok(NpcLinkshellEntry {
                            npc_ls_id: r.get::<_, u16>(0).unwrap_or_default(),
                            is_calling: r.get::<_, i64>(1).unwrap_or(0) != 0,
                            is_extra: r.get::<_, i64>(2).unwrap_or(0) != 0,
                        })
                    })?
                    .collect::<rusqlite::Result<_>>()?;
                Ok(rows)
            })
            .await?;
        Ok(rows)
    }

    // =======================================================================
    // Character saves (flat SQL updates)
    // =======================================================================

    pub async fn save_player_appearance(&self, chara_id: u32, ids: &[u32; 28]) -> Result<()> {
        const MAINHAND: usize = 5;
        const OFFHAND: usize = 6;
        const HEADGEAR: usize = 12;
        const BODYGEAR: usize = 13;
        const LEGSGEAR: usize = 14;
        const HANDSGEAR: usize = 15;
        const FEETGEAR: usize = 16;
        const WAISTGEAR: usize = 17;
        const NECKGEAR: usize = 18;
        const L_EAR: usize = 19;
        const R_EAR: usize = 20;
        const R_RINGFINGER: usize = 23;
        const L_RINGFINGER: usize = 24;

        let ids = *ids;
        self.conn
            .call_db(move |c| {
                c.execute(
                    r"UPDATE characters_appearance SET
                          mainHand = :mh, offHand = :oh, head = :head, body = :body,
                          legs = :legs, hands = :hands, feet = :feet, waist = :waist,
                          neck = :neck, leftFinger = :lf, rightFinger = :rf,
                          leftEar = :le, rightEar = :re
                       WHERE characterId = :cid",
                    named_params! {
                        ":mh": ids[MAINHAND],
                        ":oh": ids[OFFHAND],
                        ":head": ids[HEADGEAR],
                        ":body": ids[BODYGEAR],
                        ":legs": ids[LEGSGEAR],
                        ":hands": ids[HANDSGEAR],
                        ":feet": ids[FEETGEAR],
                        ":waist": ids[WAISTGEAR],
                        ":neck": ids[NECKGEAR],
                        ":lf": ids[L_RINGFINGER],
                        ":rf": ids[R_RINGFINGER],
                        ":le": ids[L_EAR],
                        ":re": ids[R_EAR],
                        ":cid": chara_id,
                    },
                )?;
                Ok(())
            })
            .await?;
        Ok(())
    }

    pub async fn save_player_current_class(
        &self,
        chara_id: u32,
        class_id: u8,
        class_level: i16,
    ) -> Result<()> {
        self.conn
            .call_db(move |c| {
                c.execute(
                    r"UPDATE characters_parametersave SET mainSkill = :cid, mainSkillLevel = :lvl
                      WHERE characterId = :charaId",
                    named_params! { ":cid": class_id, ":lvl": class_level, ":charaId": chara_id },
                )?;
                Ok(())
            })
            .await?;
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn save_player_position(
        &self,
        chara_id: u32,
        zone_id: u32,
        private_area: &str,
        private_area_type: u32,
        dest_zone: u32,
        dest_spawn: u8,
        x: f32,
        y: f32,
        z: f32,
        rotation: f32,
    ) -> Result<()> {
        let private_area = private_area.to_owned();
        self.conn
            .call_db(move |c| {
                c.execute(
                    r"UPDATE characters SET
                          positionX = :x, positionY = :y, positionZ = :z, rotation = :rot,
                          destinationZoneId = :dz, destinationSpawnType = :ds,
                          currentZoneId = :zid, currentPrivateArea = :pa,
                          currentPrivateAreaType = :pat
                       WHERE id = :cid",
                    named_params! {
                        ":x": x, ":y": y, ":z": z, ":rot": rotation,
                        ":dz": dest_zone, ":ds": dest_spawn,
                        ":zid": zone_id, ":pa": private_area, ":pat": private_area_type,
                        ":cid": chara_id,
                    },
                )?;
                Ok(())
            })
            .await?;
        Ok(())
    }

    pub async fn save_player_play_time(&self, chara_id: u32, play_time: u32) -> Result<()> {
        self.conn
            .call_db(move |c| {
                c.execute(
                    "UPDATE characters SET playTime = :pt WHERE id = :cid",
                    named_params! { ":pt": play_time, ":cid": chara_id },
                )?;
                Ok(())
            })
            .await?;
        Ok(())
    }

    pub async fn save_player_home_points(
        &self,
        chara_id: u32,
        homepoint: u32,
        homepoint_inn: u8,
    ) -> Result<()> {
        self.conn
            .call_db(move |c| {
                c.execute(
                    "UPDATE characters SET homepoint = :hp, homepointInn = :hpi WHERE id = :cid",
                    named_params! { ":hp": homepoint, ":hpi": homepoint_inn, ":cid": chara_id },
                )?;
                Ok(())
            })
            .await?;
        Ok(())
    }

    // =======================================================================
    // Quests / guildleves
    // =======================================================================

    pub async fn save_quest(
        &self,
        chara_id: u32,
        slot: i32,
        quest_actor_id: u32,
        phase: u32,
        quest_data: &str,
        quest_flags: u32,
    ) -> Result<()> {
        let quest_data = quest_data.to_owned();
        let qid = 0xF_FFFFu32 & quest_actor_id;
        self.conn
            .call_db(move |c| {
                c.execute(
                    r"INSERT INTO characters_quest_scenario
                        (characterId, slot, questId, currentPhase, questData, questFlags)
                      VALUES (:cid, :slot, :qid, :phase, :data, :flags)
                      ON CONFLICT(characterId, slot) DO UPDATE SET
                        questId = excluded.questId,
                        currentPhase = excluded.currentPhase,
                        questData = excluded.questData,
                        questFlags = excluded.questFlags",
                    named_params! {
                        ":cid": chara_id,
                        ":slot": slot,
                        ":qid": qid,
                        ":phase": phase,
                        ":data": quest_data,
                        ":flags": quest_flags,
                    },
                )?;
                Ok(())
            })
            .await?;
        Ok(())
    }

    pub async fn mark_guildleve(
        &self,
        chara_id: u32,
        gl_id: u32,
        is_abandoned: bool,
        is_completed: bool,
    ) -> Result<()> {
        self.conn
            .call_db(move |c| {
                c.execute(
                    r"UPDATE characters_quest_guildleve_regional
                      SET abandoned = :ab, completed = :cmp
                      WHERE characterId = :cid AND guildleveId = :gid",
                    named_params! {
                        ":ab": is_abandoned as i64,
                        ":cmp": is_completed as i64,
                        ":cid": chara_id,
                        ":gid": gl_id,
                    },
                )?;
                Ok(())
            })
            .await?;
        Ok(())
    }

    pub async fn save_guildleve(&self, chara_id: u32, gl_id: u32, slot: i32) -> Result<()> {
        self.conn
            .call_db(move |c| {
                c.execute(
                    r"INSERT INTO characters_quest_guildleve_regional
                        (characterId, slot, guildleveId, abandoned, completed)
                      VALUES (:cid, :slot, :gid, 0, 0)
                      ON CONFLICT(characterId, guildleveId) DO UPDATE SET
                        guildleveId = excluded.guildleveId,
                        abandoned = 0, completed = 0",
                    named_params! { ":cid": chara_id, ":slot": slot, ":gid": gl_id },
                )?;
                Ok(())
            })
            .await?;
        Ok(())
    }

    pub async fn remove_guildleve(&self, chara_id: u32, gl_id: u32) -> Result<()> {
        self.conn
            .call_db(move |c| {
                c.execute(
                    "DELETE FROM characters_quest_guildleve_regional
                     WHERE characterId = :cid AND guildleveId = :gid",
                    named_params! { ":cid": chara_id, ":gid": gl_id },
                )?;
                Ok(())
            })
            .await?;
        Ok(())
    }

    pub async fn remove_quest(&self, chara_id: u32, quest_id: u32) -> Result<()> {
        let qid = 0xF_FFFFu32 & quest_id;
        self.conn
            .call_db(move |c| {
                c.execute(
                    "DELETE FROM characters_quest_scenario WHERE characterId = :cid AND questId = :qid",
                    named_params! { ":cid": chara_id, ":qid": qid },
                )?;
                Ok(())
            })
            .await?;
        Ok(())
    }

    pub async fn complete_quest(&self, chara_id: u32, quest_id: u32) -> Result<()> {
        let qid = 0xF_FFFFu32 & quest_id;
        self.conn
            .call_db(move |c| {
                c.execute(
                    r"INSERT OR IGNORE INTO characters_quest_completed (characterId, questId)
                      VALUES (:cid, :qid)",
                    named_params! { ":cid": chara_id, ":qid": qid },
                )?;
                Ok(())
            })
            .await?;
        Ok(())
    }

    pub async fn is_quest_completed(&self, chara_id: u32, quest_id: u32) -> Result<bool> {
        let found = self.conn
            .call_db(move |c| {
                let v: Option<u32> = c
                    .query_row(
                        "SELECT questId FROM characters_quest_completed
                         WHERE characterId = :cid AND questId = :qid",
                        named_params! { ":cid": chara_id, ":qid": quest_id },
                        |r| r.get(0),
                    )
                    .optional()?;
                Ok(v.is_some())
            })
            .await?;
        Ok(found)
    }

    // =======================================================================
    // Equipment / hotbar
    // =======================================================================

    pub async fn get_equipment(&self, chara_id: u32, class_id: u16) -> Result<Vec<EquipmentSlot>> {
        let rows = self.conn
            .call_db(move |c| {
                let mut stmt = c.prepare(
                    r"SELECT equipSlot, itemId FROM characters_inventory_equipment
                      WHERE characterId = :cid AND (classId = :class OR classId = 0)
                      ORDER BY equipSlot",
                )?;
                let rows: Vec<EquipmentSlot> = stmt
                    .query_map(named_params! { ":cid": chara_id, ":class": class_id }, |r| {
                        Ok(EquipmentSlot {
                            equip_slot: r.get::<_, u16>(0).unwrap_or_default(),
                            item_id: r.get::<_, u64>(1).unwrap_or_default(),
                        })
                    })?
                    .collect::<rusqlite::Result<_>>()?;
                Ok(rows)
            })
            .await?;
        Ok(rows)
    }

    pub async fn equip_item(
        &self,
        chara_id: u32,
        class_id: u8,
        equip_slot: u16,
        unique_item_id: u64,
        is_undergarment: bool,
    ) -> Result<()> {
        let effective_class: u8 = if is_undergarment { 0 } else { class_id };
        self.conn
            .call_db(move |c| {
                c.execute(
                    r"INSERT INTO characters_inventory_equipment (characterId, classId, equipSlot, itemId)
                      VALUES (:cid, :class, :slot, :iid)
                      ON CONFLICT(characterId, classId, equipSlot) DO UPDATE SET itemId = excluded.itemId",
                    named_params! {
                        ":cid": chara_id,
                        ":class": effective_class,
                        ":slot": equip_slot,
                        ":iid": unique_item_id as i64,
                    },
                )?;
                Ok(())
            })
            .await?;
        Ok(())
    }

    pub async fn unequip_item(&self, chara_id: u32, class_id: u8, equip_slot: u16) -> Result<()> {
        self.conn
            .call_db(move |c| {
                c.execute(
                    r"DELETE FROM characters_inventory_equipment
                      WHERE characterId = :cid AND classId = :class AND equipSlot = :slot",
                    named_params! {
                        ":cid": chara_id,
                        ":class": class_id,
                        ":slot": equip_slot,
                    },
                )?;
                Ok(())
            })
            .await?;
        Ok(())
    }

    pub async fn equip_ability(
        &self,
        chara_id: u32,
        class_id: u8,
        hotbar_slot: u16,
        command_id: u32,
        recast_time: u32,
    ) -> Result<()> {
        let command_id = command_id & 0xFFFF;
        self.conn
            .call_db(move |c| {
                c.execute(
                    r"INSERT INTO characters_hotbar (characterId, classId, hotbarSlot, commandId, recastTime)
                      VALUES (:cid, :class, :slot, :cmd, :rcst)
                      ON CONFLICT(characterId, classId, hotbarSlot) DO UPDATE SET
                        commandId = excluded.commandId, recastTime = excluded.recastTime",
                    named_params! {
                        ":cid": chara_id, ":class": class_id, ":slot": hotbar_slot,
                        ":cmd": command_id, ":rcst": recast_time,
                    },
                )?;
                Ok(())
            })
            .await?;
        Ok(())
    }

    pub async fn unequip_ability(
        &self,
        chara_id: u32,
        class_id: u8,
        hotbar_slot: u16,
    ) -> Result<()> {
        self.conn
            .call_db(move |c| {
                c.execute(
                    r"DELETE FROM characters_hotbar
                      WHERE characterId = :cid AND classId = :class AND hotbarSlot = :slot",
                    named_params! { ":cid": chara_id, ":class": class_id, ":slot": hotbar_slot },
                )?;
                Ok(())
            })
            .await?;
        Ok(())
    }

    pub async fn load_hotbar(&self, chara_id: u32, class_id: u8) -> Result<Vec<HotbarEntry>> {
        let rows = self.conn
            .call_db(move |c| {
                let mut stmt = c.prepare(
                    r"SELECT hotbarSlot, commandId, recastTime
                      FROM characters_hotbar
                      WHERE characterId = :cid AND classId = :class
                      ORDER BY hotbarSlot",
                )?;
                let rows: Vec<HotbarEntry> = stmt
                    .query_map(named_params! { ":cid": chara_id, ":class": class_id }, |r| {
                        Ok(HotbarEntry {
                            hotbar_slot: r.get::<_, u16>(0).unwrap_or_default(),
                            command_id: r.get::<_, u32>(1).unwrap_or_default(),
                            recast_time: r.get::<_, u32>(2).unwrap_or_default(),
                        })
                    })?
                    .collect::<rusqlite::Result<_>>()?;
                Ok(rows)
            })
            .await?;
        Ok(rows)
    }

    pub async fn find_first_command_slot(&self, chara_id: u32, class_id: u8) -> Result<u16> {
        let slots = self.conn
            .call_db(move |c| {
                let mut stmt = c.prepare(
                    r"SELECT hotbarSlot FROM characters_hotbar
                      WHERE characterId = :cid AND classId = :class
                      ORDER BY hotbarSlot",
                )?;
                let rows: Vec<u16> = stmt
                    .query_map(
                        named_params! { ":cid": chara_id, ":class": class_id },
                        |r| r.get::<_, u16>(0),
                    )?
                    .collect::<rusqlite::Result<_>>()?;
                Ok(rows)
            })
            .await?;
        let mut expected: u16 = 0;
        for slot in slots {
            if slot != expected {
                return Ok(expected);
            }
            expected += 1;
        }
        Ok(expected)
    }

    // =======================================================================
    // Inventory
    // =======================================================================

    pub async fn get_item_package(
        &self,
        owner_id: u32,
        item_package: u32,
    ) -> Result<Vec<InventoryItem>> {
        let rows = self.conn
            .call_db(move |c| {
                let mut stmt = c.prepare(
                    r"SELECT
                        ci.serverItemId, si.itemId, sm.id AS modifierId,
                        si.quantity, si.quality,
                        sd.dealingValue, sd.dealingMode, sd.dealingAttached1,
                        sd.dealingAttached2, sd.dealingAttached3, sd.dealingTag, sd.bazaarMode,
                        sm.durability, sm.mainQuality, sm.subQuality1, sm.subQuality2, sm.subQuality3,
                        sm.param1, sm.param2, sm.param3, sm.spiritbind,
                        sm.materia1, sm.materia2, sm.materia3, sm.materia4, sm.materia5
                      FROM characters_inventory ci
                      INNER JOIN server_items si ON ci.serverItemId = si.id
                      LEFT JOIN server_items_modifiers sm ON si.id = sm.id
                      LEFT JOIN server_items_dealing   sd ON si.id = sd.id
                      WHERE ci.characterId = :cid AND ci.itemPackage = :pkg
                      ORDER BY ci.slot ASC",
                )?;
                let rows: Vec<InventoryItem> = stmt
                    .query_map(named_params! { ":cid": owner_id, ":pkg": item_package }, |r| {
                        Ok(InventoryItem {
                            unique_id: r.get::<_, u64>(0).unwrap_or_default(),
                            item_id: r.get::<_, u32>(1).unwrap_or_default(),
                            quantity: r.get::<_, i32>(3).unwrap_or(1),
                            quality: r.get::<_, u8>(4).unwrap_or(1),
                            tag: ItemTag {
                                durability: r.get::<_, u32>(12).unwrap_or_default(),
                                main_quality: r.get::<_, u8>(13).unwrap_or_default(),
                                param1: r.get::<_, u32>(17).unwrap_or_default(),
                                param2: r.get::<_, u32>(18).unwrap_or_default(),
                                param3: r.get::<_, u32>(19).unwrap_or_default(),
                                spiritbind: r.get::<_, u16>(20).unwrap_or_default(),
                                materia_id: r.get::<_, u32>(21).unwrap_or_default(),
                                ..Default::default()
                            },
                            ..Default::default()
                        })
                    })?
                    .collect::<rusqlite::Result<_>>()?;
                Ok(rows)
            })
            .await?;
        Ok(rows)
    }

    pub async fn create_item(
        &self,
        item_id: u32,
        quantity: i32,
        quality: u8,
        modifiers: Option<&ItemModifiers>,
    ) -> Result<InventoryItem> {
        let modifiers = modifiers.cloned();
        let item = self.conn
            .call_db(move |c| {
                c.execute(
                    r"INSERT INTO server_items (itemId, quantity, quality)
                      VALUES (:iid, :qty, :qual)",
                    named_params! { ":iid": item_id, ":qty": quantity, ":qual": quality },
                )?;
                let unique_id = c.last_insert_rowid() as u64;

                let mut item = InventoryItem {
                    unique_id,
                    item_id,
                    quantity,
                    quality,
                    ..Default::default()
                };

                if let Some(m) = modifiers {
                    c.execute(
                        r"INSERT INTO server_items_modifiers (id, durability) VALUES (:id, :d)",
                        named_params! { ":id": unique_id as i64, ":d": m.durability },
                    )?;
                    item.tag = ItemTag {
                        durability: m.durability,
                        main_quality: m.main_quality,
                        param1: m.param[0],
                        param2: m.param[1],
                        param3: m.param[2],
                        spiritbind: m.spiritbind,
                        materia_id: m.materia[0],
                        ..Default::default()
                    };
                }

                Ok(item)
            })
            .await?;
        Ok(item)
    }

    pub async fn add_item(
        &self,
        owner_id: u32,
        server_item_id: u64,
        item_package: u16,
        slot: u16,
    ) -> Result<()> {
        self.conn
            .call_db(move |c| {
                c.execute(
                    r"INSERT INTO characters_inventory (characterId, itemPackage, serverItemId, slot)
                      VALUES (:cid, :pkg, :iid, :slot)",
                    named_params! {
                        ":cid": owner_id, ":pkg": item_package,
                        ":iid": server_item_id as i64, ":slot": slot,
                    },
                )?;
                Ok(())
            })
            .await?;
        Ok(())
    }

    pub async fn remove_item(&self, owner_id: u32, server_item_id: u64) -> Result<()> {
        self.conn
            .call_db(move |c| {
                c.execute(
                    "DELETE FROM characters_inventory WHERE characterId = :cid AND serverItemId = :iid",
                    named_params! { ":cid": owner_id, ":iid": server_item_id as i64 },
                )?;
                Ok(())
            })
            .await?;
        Ok(())
    }

    pub async fn update_item_positions(&self, updates: &[InventoryItem]) -> Result<()> {
        let updates: Vec<(u16, u64)> = updates.iter().map(|i| (i.slot, i.unique_id)).collect();
        self.conn
            .call_db(move |c| {
                let tx = c.transaction()?;
                {
                    let mut stmt = tx.prepare(
                        "UPDATE characters_inventory SET slot = :slot WHERE serverItemId = :iid",
                    )?;
                    for (slot, iid) in updates {
                        stmt.execute(
                            named_params! { ":slot": slot, ":iid": iid as i64 },
                        )?;
                    }
                }
                tx.commit()?;
                Ok(())
            })
            .await?;
        Ok(())
    }

    pub async fn set_quantity(&self, server_item_id: u64, quantity: i32) -> Result<()> {
        self.conn
            .call_db(move |c| {
                c.execute(
                    "UPDATE server_items SET quantity = :q WHERE id = :iid",
                    named_params! { ":q": quantity, ":iid": server_item_id as i64 },
                )?;
                Ok(())
            })
            .await?;
        Ok(())
    }

    pub async fn set_dealing_info(
        &self,
        server_item_id: u64,
        info: &ItemDealingInfo,
    ) -> Result<()> {
        let info = info.clone();
        self.conn
            .call_db(move |c| {
                c.execute(
                    r"REPLACE INTO server_items_dealing
                        (id, dealingValue, dealingMode, dealingAttached1, dealingAttached2,
                         dealingAttached3, dealingTag, bazaarMode)
                      VALUES
                        (:iid, :dv, :dm, :da1, :da2, :da3, :dt, :bm)",
                    named_params! {
                        ":iid": server_item_id as i64,
                        ":dv": info.dealing_value,
                        ":dm": info.dealing_mode,
                        ":da1": info.dealing_attached[0] as i64,
                        ":da2": info.dealing_attached[1] as i64,
                        ":da3": info.dealing_attached[2] as i64,
                        ":dt": info.dealing_tag as i64,
                        ":bm": info.bazaar_mode,
                    },
                )?;
                Ok(())
            })
            .await?;
        Ok(())
    }

    pub async fn clear_dealing_info(&self, server_item_id: u64) -> Result<()> {
        self.conn
            .call_db(move |c| {
                c.execute(
                    "DELETE FROM server_items_dealing WHERE id = :iid",
                    named_params! { ":iid": server_item_id as i64 },
                )?;
                Ok(())
            })
            .await?;
        Ok(())
    }

    // =======================================================================
    // Achievements
    // =======================================================================

    pub async fn get_latest_achievements(&self, chara_id: u32) -> Result<[u32; 5]> {
        let rows = self.conn
            .call_db(move |c| {
                let mut stmt = c.prepare(
                    r"SELECT ca.achievementId
                      FROM characters_achievements ca
                      INNER JOIN gamedata_achievements ga
                          ON ca.achievementId = ga.achievementId
                      WHERE ca.characterId = :cid AND ga.rewardPoints <> 0
                        AND ca.timeDone IS NOT NULL
                      ORDER BY ca.timeDone LIMIT 5",
                )?;
                let rows: Vec<u32> = stmt
                    .query_map(named_params! { ":cid": chara_id }, |r| r.get::<_, u32>(0))?
                    .collect::<rusqlite::Result<_>>()?;
                Ok(rows)
            })
            .await?;
        let mut out = [0u32; 5];
        for (i, v) in rows.into_iter().take(5).enumerate() {
            out[i] = v;
        }
        Ok(out)
    }

    pub async fn get_achievements(&self, chara_id: u32) -> Result<Vec<u32>> {
        let rows = self.conn
            .call_db(move |c| {
                let mut stmt = c.prepare(
                    r"SELECT ga.packetOffsetId
                      FROM characters_achievements ca
                      INNER JOIN gamedata_achievements ga
                          ON ca.achievementId = ga.achievementId
                      WHERE ca.characterId = :cid AND ca.timeDone IS NOT NULL",
                )?;
                let rows: Vec<u32> = stmt
                    .query_map(named_params! { ":cid": chara_id }, |r| r.get::<_, u32>(0))?
                    .collect::<rusqlite::Result<_>>()?;
                Ok(rows)
            })
            .await?;
        Ok(rows)
    }

    pub async fn get_achievement_progress(
        &self,
        chara_id: u32,
        achievement_id: u32,
    ) -> Result<(u32, u32)> {
        let v = self.conn
            .call_db(move |c| {
                let v = c
                    .query_row(
                        "SELECT progress, progressFlags FROM characters_achievements
                         WHERE characterId = :cid AND achievementId = :aid",
                        named_params! { ":cid": chara_id, ":aid": achievement_id },
                        |r| Ok((r.get::<_, u32>(0)?, r.get::<_, u32>(1)?)),
                    )
                    .optional()?;
                Ok(v)
            })
            .await?;
        Ok(v.unwrap_or((0, 0)))
    }

    // =======================================================================
    // Linkshells, support tickets, FAQ, chocobo, status save
    // =======================================================================

    pub async fn create_linkshell(
        &self,
        chara_id: u32,
        ls_name: &str,
        ls_crest: u16,
    ) -> Result<bool> {
        let ls_name = ls_name.to_owned();
        let ok = self.conn
            .call_db(move |c| {
                let r = c.execute(
                    r"INSERT INTO server_linkshells (name, master, crest)
                      VALUES (:name, :master, :crest)",
                    named_params! { ":name": ls_name, ":master": chara_id, ":crest": ls_crest },
                );
                Ok(r.is_ok())
            })
            .await?;
        Ok(ok)
    }

    pub async fn save_npc_ls(
        &self,
        chara_id: u32,
        npc_ls_id: u32,
        is_calling: bool,
        is_extra: bool,
    ) -> Result<()> {
        self.conn
            .call_db(move |c| {
                c.execute(
                    r"INSERT INTO characters_npclinkshell (characterId, npcLinkshellId, isCalling, isExtra)
                      VALUES (:cid, :lsid, :c, :e)
                      ON CONFLICT(characterId, npcLinkshellId) DO UPDATE SET
                        isCalling = excluded.isCalling, isExtra = excluded.isExtra",
                    named_params! {
                        ":cid": chara_id, ":lsid": npc_ls_id,
                        ":c": is_calling as i64, ":e": is_extra as i64,
                    },
                )?;
                Ok(())
            })
            .await?;
        Ok(())
    }

    /// Returns `true` on error (matches the C# convention).
    pub async fn save_support_ticket(
        &self,
        player_name: &str,
        title: &str,
        body: &str,
        lang_code: u32,
    ) -> Result<bool> {
        let player_name = player_name.to_owned();
        let title = title.to_owned();
        let body = body.to_owned();
        let err = self.conn
            .call_db(move |c| {
                let r = c.execute(
                    r"INSERT INTO supportdesk_tickets (name, title, body, langCode)
                      VALUES (:name, :title, :body, :lang)",
                    named_params! { ":name": player_name, ":title": title, ":body": body, ":lang": lang_code },
                );
                Ok(r.is_err())
            })
            .await?;
        Ok(err)
    }

    pub async fn is_ticket_open(&self, player_name: &str) -> Result<bool> {
        let player_name = player_name.to_owned();
        let v = self.conn
            .call_db(move |c| {
                let v: Option<i64> = c
                    .query_row(
                        "SELECT isOpen FROM supportdesk_tickets WHERE name = :n",
                        named_params! { ":n": player_name },
                        |r| r.get(0),
                    )
                    .optional()?;
                Ok(v.unwrap_or(0) != 0)
            })
            .await?;
        Ok(v)
    }

    pub async fn close_ticket(&self, player_name: &str) -> Result<()> {
        let player_name = player_name.to_owned();
        self.conn
            .call_db(move |c| {
                c.execute(
                    "UPDATE supportdesk_tickets SET isOpen = 0 WHERE name = :n",
                    named_params! { ":n": player_name },
                )?;
                Ok(())
            })
            .await?;
        Ok(())
    }

    pub async fn get_faq_names(&self, lang_code: u32) -> Result<Vec<String>> {
        let rows = self.conn
            .call_db(move |c| {
                let mut stmt = c.prepare(
                    "SELECT title FROM supportdesk_faqs WHERE languageCode = :l ORDER BY slot",
                )?;
                let rows: Vec<String> = stmt
                    .query_map(named_params! { ":l": lang_code }, |r| r.get::<_, String>(0))?
                    .collect::<rusqlite::Result<_>>()?;
                Ok(rows)
            })
            .await?;
        Ok(rows)
    }

    pub async fn get_faq_body(&self, slot: u32, lang_code: u32) -> Result<String> {
        let v = self.conn
            .call_db(move |c| {
                let v = c
                    .query_row(
                        "SELECT body FROM supportdesk_faqs WHERE slot = :s AND languageCode = :l",
                        named_params! { ":s": slot, ":l": lang_code },
                        |r| r.get::<_, String>(0),
                    )
                    .optional()?;
                Ok(v)
            })
            .await?;
        Ok(v.unwrap_or_default())
    }

    pub async fn get_issues(&self, _lang_code: u32) -> Result<Vec<String>> {
        let rows = self.conn
            .call_db(|c| {
                let mut stmt = c.prepare("SELECT title FROM supportdesk_issues ORDER BY slot")?;
                let rows: Vec<String> = stmt
                    .query_map([], |r| r.get::<_, String>(0))?
                    .collect::<rusqlite::Result<_>>()?;
                Ok(rows)
            })
            .await?;
        Ok(rows)
    }

    pub async fn issue_player_chocobo(
        &self,
        chara_id: u32,
        appearance_id: u8,
        name: &str,
    ) -> Result<()> {
        let name = name.to_owned();
        self.conn
            .call_db(move |c| {
                c.execute(
                    r"INSERT INTO characters_chocobo (characterId, hasChocobo, chocoboAppearance, chocoboName)
                      VALUES (:cid, 1, :app, :name)
                      ON CONFLICT(characterId) DO UPDATE SET
                        hasChocobo = 1,
                        chocoboAppearance = excluded.chocoboAppearance,
                        chocoboName = excluded.chocoboName",
                    named_params! { ":cid": chara_id, ":app": appearance_id, ":name": name },
                )?;
                Ok(())
            })
            .await?;
        Ok(())
    }

    pub async fn change_player_chocobo_appearance(
        &self,
        chara_id: u32,
        appearance_id: u8,
    ) -> Result<()> {
        self.conn
            .call_db(move |c| {
                c.execute(
                    "UPDATE characters_chocobo SET chocoboAppearance = :app WHERE characterId = :cid",
                    named_params! { ":app": appearance_id, ":cid": chara_id },
                )?;
                Ok(())
            })
            .await?;
        Ok(())
    }

    pub async fn save_player_status_effects(
        &self,
        chara_id: u32,
        effects: &[StatusEffectEntry],
    ) -> Result<()> {
        let effects = effects.to_vec();
        self.conn
            .call_db(move |c| {
                let tx = c.transaction()?;
                {
                    let mut stmt = tx.prepare(
                        r"REPLACE INTO characters_statuseffect
                            (characterId, statusId, magnitude, duration, tick, tier, extra)
                          VALUES (:cid, :sid, :mag, :dur, :tick, :tier, :extra)",
                    )?;
                    for eff in effects {
                        stmt.execute(named_params! {
                            ":cid": chara_id,
                            ":sid": eff.status_id,
                            ":mag": eff.magnitude as i64,
                            ":dur": eff.duration,
                            ":tick": eff.tick,
                            ":tier": eff.tier,
                            ":extra": eff.extra as i64,
                        })?;
                    }
                }
                tx.commit()?;
                Ok(())
            })
            .await?;
        Ok(())
    }

    // =======================================================================
    // XP / level / retainer
    // =======================================================================

    pub async fn set_exp(&self, chara_id: u32, class_id: u8, exp: i32) -> Result<()> {
        let Some(col) = class_column(class_id) else {
            return Ok(());
        };
        let col = col.to_owned();
        self.conn
            .call_db(move |c| {
                let sql = format!(
                    "UPDATE characters_class_exp SET {col} = :exp WHERE characterId = :cid"
                );
                c.execute(&sql, named_params! { ":exp": exp, ":cid": chara_id })?;
                Ok(())
            })
            .await?;
        Ok(())
    }

    pub async fn set_level(&self, chara_id: u32, class_id: u8, level: i16) -> Result<()> {
        let Some(col) = class_column(class_id) else {
            return Ok(());
        };
        let col = col.to_owned();
        self.conn
            .call_db(move |c| {
                let sql = format!(
                    "UPDATE characters_class_levels SET {col} = :lvl WHERE characterId = :cid"
                );
                c.execute(&sql, named_params! { ":lvl": level, ":cid": chara_id })?;
                Ok(())
            })
            .await?;
        Ok(())
    }

    /// Returns `(retainerId, name, actorClassId)`.
    pub async fn load_retainer(
        &self,
        chara_id: u32,
        retainer_index: i32,
    ) -> Result<Option<(u32, String, u32)>> {
        let offset = (retainer_index - 1).max(0);
        let v = self.conn
            .call_db(move |c| {
                let v = c
                    .query_row(
                        r"SELECT sr.id, sr.name, sr.actorClassId
                          FROM characters_retainers cr
                          INNER JOIN server_retainers sr ON cr.retainerId = sr.id
                          WHERE cr.characterId = :cid
                          ORDER BY sr.id
                          LIMIT 1 OFFSET :off",
                        named_params! { ":cid": chara_id, ":off": offset },
                        |r| {
                            Ok((
                                r.get::<_, u32>(0)?,
                                r.get::<_, String>(1)?,
                                r.get::<_, u32>(2)?,
                            ))
                        },
                    )
                    .optional()?;
                Ok(v)
            })
            .await?;
        Ok(v)
    }

    pub async fn player_character_update_class_level(
        &self,
        chara_id: u32,
        class_id: u8,
        level: i16,
    ) -> Result<()> {
        self.set_level(chara_id, class_id, level).await
    }
}
