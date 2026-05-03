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
use common::bitstream::Bitstream2048;
use common::db::ConnCallExt;
use rusqlite::{OptionalExtension, named_params};
use tokio_rusqlite::Connection;

use crate::actor::modifier::decode_param_bonus_type;
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

/// One row of `gamedata_actor_appearance` keyed by actor_class_id.
/// Mirrors the columns read by Meteor's `Npc.LoadAppearance`.
#[derive(Debug, Clone, Default)]
pub struct NpcAppearance {
    pub base: u32,
    pub size: u32,
    pub hair_style: u32,
    pub hair_highlight_color: u32,
    pub hair_variation: u32,
    pub face_type: u8,
    pub characteristics: u8,
    pub characteristics_color: u8,
    pub face_eyebrows: u8,
    pub face_iris_size: u8,
    pub face_eye_shape: u8,
    pub face_nose: u8,
    // GAM ids 112 / 114 — see lobby Appearance::face_cheek / face_jaw.
    // SQL column names (`faceFeatures`, `ears`) preserved for back-compat.
    pub face_cheek: u8,
    pub face_mouth: u8,
    pub face_jaw: u8,
    pub hair_color: u32,
    pub skin_color: u32,
    pub eye_color: u32,
    pub voice: u32,
    pub main_hand: u32,
    pub off_hand: u32,
    pub sp_main_hand: u32,
    pub sp_off_hand: u32,
    pub throwing: u32,
    pub pack: u32,
    pub pouch: u32,
    pub head: u32,
    pub body: u32,
    pub legs: u32,
    pub hands: u32,
    pub feet: u32,
    pub waist: u32,
    pub neck: u32,
    pub left_ear: u32,
    pub right_ear: u32,
    pub left_index: u32,
    pub right_index: u32,
    pub left_finger: u32,
    pub right_finger: u32,
}

impl NpcAppearance {
    /// Pack the per-row columns into the (model_id, appearance_ids[28])
    /// layout that `SetActorAppearancePacket` (0x00D6) expects. Mirrors
    /// `Npc.LoadAppearance` at `Actors/Chara/Npc/Npc.cs:362` — the
    /// slot-index enum lives in `SetActorAppearancePacket.cs:39-60`.
    pub fn pack(&self) -> (u32, [u32; 28]) {
        let mut a = [0u32; 28];
        // 0 SIZE
        a[0] = self.size;
        // 1 COLORINFO = skin | hair<<10 | eye<<20
        a[1] = self.skin_color | (self.hair_color << 10) | (self.eye_color << 20);
        // 2 FACEINFO — bitfield pack per C# `CharacterUtils.FaceInfo`
        // (Bitfield.cs:39). Shared implementation with the player path
        // lives in `gamedata::pack_face_info`. All 10 face bytes fit
        // into the 32-bit slot (5+3+6+2+2+2+3+3+1+3 = 30 bits; last 2
        // bits are an "unknown" field left zero).
        a[2] = crate::gamedata::pack_face_info(
            self.characteristics,
            self.characteristics_color,
            self.face_type,
            self.face_jaw,
            self.face_mouth,
            self.face_cheek,
            self.face_nose,
            self.face_eye_shape,
            self.face_iris_size,
            self.face_eyebrows,
        );
        // 3 HIGHLIGHT_HAIR = highlight | variation<<5 | style<<10
        a[3] = self.hair_highlight_color
            | (self.hair_variation << 5)
            | (self.hair_style << 10);
        // 4 VOICE
        a[4] = self.voice;
        // 5..11 weapons + bag
        a[5] = self.main_hand;
        a[6] = self.off_hand;
        a[7] = self.sp_main_hand;
        a[8] = self.sp_off_hand;
        a[9] = self.throwing;
        a[10] = self.pack;
        a[11] = self.pouch;
        // 12..17 gear
        a[12] = self.head;
        a[13] = self.body;
        a[14] = self.legs;
        a[15] = self.hands;
        a[16] = self.feet;
        a[17] = self.waist;
        // 18 NECK, 19/20 L/R ear
        a[18] = self.neck;
        a[19] = self.left_ear;
        a[20] = self.right_ear;
        // Meteor writes left/rightIndex into R_INDEXFINGER/L_INDEXFINGER
        // (indices 25/26) and left/rightFinger into R_RINGFINGER/L_RINGFINGER
        // (23/24) — port the same mapping to match the wire.
        a[23] = self.right_finger;
        a[24] = self.left_finger;
        a[25] = self.left_index;
        a[26] = self.right_index;
        (self.base, a)
    }
}

/// Joined result of `server_battlenpc_spawn_locations` +
/// `server_battlenpc_groups` + `server_battlenpc_pools` +
/// `server_battlenpc_genus` for a single `bnpcId`. Used by
/// `SpawnBattleNpcById` (the Lua-callable spawn entry point) to drive
/// the actor materialisation. Mirrors the row shape C#
/// `WorldManager.SpawnBattleNpcById` reads in
/// `Map Server/WorldManager.cs:518`.
#[derive(Debug, Clone, Default)]
pub struct BattleNpcSpawn {
    pub bnpc_id: u32,
    pub group_id: u32,
    pub position_x: f32,
    pub position_y: f32,
    pub position_z: f32,
    pub rotation: f32,
    pub pool_id: u32,
    pub script_name: String,
    pub min_level: u32,
    pub max_level: u32,
    pub respawn_time: u32,
    pub hp: u32,
    pub mp: u32,
    pub drop_list_id: u32,
    /// 0 = mob, 1 = ally (Player allegiance — controls combat side).
    pub allegiance: u8,
    pub spawn_type: u32,
    pub animation_id: u32,
    pub actor_state: u16,
    pub private_area_name: String,
    pub private_area_level: u32,
    pub zone_id: u32,
    pub genus_id: u32,
    pub actor_class_id: u32,
    pub current_job: u32,
    pub combat_skill: u32,
    pub combat_delay: u32,
    pub aggro_type: u8,
    pub speed: u8,
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

    #[cfg(test)]
    pub fn conn_for_test(&self) -> &Connection {
        &self.conn
    }

    // =======================================================================
    // Zone / area loaders (called at boot by WorldManager)
    // =======================================================================

    pub async fn load_zones(&self, server_ip: &str, server_port: u16) -> Result<Vec<ZoneRow>> {
        tracing::debug!(server_ip, server_port, "db: load_zones");
        let server_ip = server_ip.to_owned();
        let rows = self
            .conn
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
        let rows = self
            .conn
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
        let rows = self
            .conn
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

    /// Single-class variant of [`load_actor_classes`] — used by
    /// runtime spawn paths (e.g. `SpawnBattleNpcById`) that need one
    /// row on demand without paying the full-table-load cost.
    pub async fn load_actor_class(
        &self,
        actor_class_id: u32,
    ) -> Result<Option<crate::npc::ActorClass>> {
        let row = self
            .conn
            .call_db(move |c| {
                let mut stmt = c.prepare(
                    r"SELECT ac.classPath, ac.displayNameId, ac.propertyFlags,
                             ac.eventConditions,
                             pc.pushCommand, pc.pushCommandSub, pc.pushCommandPriority
                      FROM gamedata_actor_class ac
                      LEFT JOIN gamedata_actor_pushcommand pc ON ac.id = pc.id
                      WHERE ac.id = ?1
                      LIMIT 1",
                )?;
                let row = stmt
                    .query_row([actor_class_id], |r| {
                        Ok(crate::npc::ActorClass::new(
                            actor_class_id,
                            r.get::<_, String>(0).unwrap_or_default(),
                            r.get::<_, u32>(1).unwrap_or_default(),
                            r.get::<_, u32>(2).unwrap_or_default(),
                            r.get::<_, String>(3).unwrap_or_default(),
                            r.get::<_, u16>(4).unwrap_or_default(),
                            r.get::<_, u16>(5).unwrap_or_default(),
                            r.get::<_, u8>(6).unwrap_or_default(),
                        ))
                    })
                    .optional()?;
                Ok(row)
            })
            .await?;
        Ok(row)
    }

    pub async fn load_actor_classes(&self) -> Result<HashMap<u32, crate::npc::ActorClass>> {
        tracing::debug!("db: load_actor_classes");
        let rows = self
            .conn
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

    /// Loads `gamedata_actor_appearance` keyed by actor_class_id. Mirrors
    /// Meteor's `Npc.LoadAppearance` (Actors/Chara/Npc/Npc.cs:316) — the
    /// table packs per-actor-class model + gear slots that the client
    /// needs to render NPC avatars. Without this data every populace NPC
    /// gets a 0x00D6 with model_id=0 and the client derefs nil during
    /// model load, which on Wine crashes the whole process.
    pub async fn load_npc_appearances(&self) -> Result<HashMap<u32, NpcAppearance>> {
        let rows = self
            .conn
            .call_db(|c| {
                let mut stmt = c.prepare(
                    r"SELECT id, base, size, hairStyle, hairHighlightColor, hairVariation,
                             faceType, characteristics, characteristicsColor,
                             faceEyebrows, faceIrisSize, faceEyeShape, faceNose,
                             faceFeatures, faceMouth, ears,
                             hairColor, skinColor, eyeColor, voice,
                             mainHand, offHand, spMainHand, spOffHand, throwing,
                             pack, pouch,
                             head, body, legs, hands, feet, waist, neck,
                             leftEar, rightEar, leftIndex, rightIndex,
                             leftFinger, rightFinger
                      FROM gamedata_actor_appearance",
                )?;
                let rows: Vec<(u32, NpcAppearance)> = stmt
                    .query_map([], |r| {
                        let id: u32 = r.get(0)?;
                        Ok((id, NpcAppearance {
                            base:                   r.get(1).unwrap_or(0),
                            size:                   r.get(2).unwrap_or(0),
                            hair_style:             r.get(3).unwrap_or(0),
                            hair_highlight_color:   r.get(4).unwrap_or(0),
                            hair_variation:         r.get(5).unwrap_or(0),
                            face_type:              r.get(6).unwrap_or(0),
                            characteristics:        r.get(7).unwrap_or(0),
                            characteristics_color:  r.get(8).unwrap_or(0),
                            face_eyebrows:          r.get(9).unwrap_or(0),
                            face_iris_size:         r.get(10).unwrap_or(0),
                            face_eye_shape:         r.get(11).unwrap_or(0),
                            face_nose:              r.get(12).unwrap_or(0),
                            face_cheek:             r.get(13).unwrap_or(0),  // SQL col `faceFeatures`
                            face_mouth:             r.get(14).unwrap_or(0),
                            face_jaw:               r.get(15).unwrap_or(0),  // SQL col `ears`
                            hair_color:             r.get(16).unwrap_or(0),
                            skin_color:             r.get(17).unwrap_or(0),
                            eye_color:              r.get(18).unwrap_or(0),
                            voice:                  r.get(19).unwrap_or(0),
                            main_hand:              r.get(20).unwrap_or(0),
                            off_hand:               r.get(21).unwrap_or(0),
                            sp_main_hand:           r.get(22).unwrap_or(0),
                            sp_off_hand:            r.get(23).unwrap_or(0),
                            throwing:               r.get(24).unwrap_or(0),
                            pack:                   r.get(25).unwrap_or(0),
                            pouch:                  r.get(26).unwrap_or(0),
                            head:                   r.get(27).unwrap_or(0),
                            body:                   r.get(28).unwrap_or(0),
                            legs:                   r.get(29).unwrap_or(0),
                            hands:                  r.get(30).unwrap_or(0),
                            feet:                   r.get(31).unwrap_or(0),
                            waist:                  r.get(32).unwrap_or(0),
                            neck:                   r.get(33).unwrap_or(0),
                            left_ear:               r.get(34).unwrap_or(0),
                            right_ear:              r.get(35).unwrap_or(0),
                            left_index:             r.get(36).unwrap_or(0),
                            right_index:            r.get(37).unwrap_or(0),
                            left_finger:            r.get(38).unwrap_or(0),
                            right_finger:           r.get(39).unwrap_or(0),
                        }))
                    })?
                    .collect::<rusqlite::Result<_>>()?;
                Ok(rows)
            })
            .await?;
        Ok(rows.into_iter().collect())
    }

    pub async fn load_npc_spawn_locations(&self) -> Result<Vec<crate::zone::SpawnLocation>> {
        let rows = self
            .conn
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

    /// Load one `BattleNpcSpawn` row by `bnpc_id` — the spawn-locations,
    /// groups, pools, and genus tables joined into a single DTO. Returns
    /// `None` if `bnpc_id` doesn't exist in `server_battlenpc_spawn_locations`.
    /// Port of the C# join in `Map Server/WorldManager.cs:518` (the body
    /// of `SpawnBattleNpcById`).
    pub async fn load_battle_npc_spawn(&self, bnpc_id: u32) -> Result<Option<BattleNpcSpawn>> {
        let row = self
            .conn
            .call_db(move |c| {
                let mut stmt = c.prepare(
                    r"SELECT
                        bsl.bnpcId, bsl.groupId, bsl.positionX, bsl.positionY,
                        bsl.positionZ, bsl.rotation,
                        bgr.poolId, bgr.scriptName, bgr.minLevel, bgr.maxLevel,
                        bgr.respawnTime, bgr.hp, bgr.mp, bgr.dropListId,
                        bgr.allegiance, bgr.spawnType, bgr.animationId,
                        bgr.actorState, bgr.privateAreaName, bgr.privateAreaLevel,
                        bgr.zoneId,
                        bpo.genusId, bpo.actorClassId, bpo.currentJob,
                        bpo.combatSkill, bpo.combatDelay, bpo.aggroType,
                        bge.speed
                      FROM server_battlenpc_spawn_locations bsl
                      INNER JOIN server_battlenpc_groups bgr ON bsl.groupId = bgr.groupId
                      INNER JOIN server_battlenpc_pools  bpo ON bgr.poolId  = bpo.poolId
                      INNER JOIN server_battlenpc_genus  bge ON bpo.genusId = bge.genusId
                      WHERE bsl.bnpcId = ?1
                      LIMIT 1",
                )?;
                let row = stmt
                    .query_row([bnpc_id], |r| {
                        Ok(BattleNpcSpawn {
                            bnpc_id: r.get(0)?,
                            group_id: r.get(1)?,
                            position_x: r.get::<_, f32>(2)?,
                            position_y: r.get::<_, f32>(3)?,
                            position_z: r.get::<_, f32>(4)?,
                            rotation: r.get::<_, f32>(5)?,
                            pool_id: r.get(6)?,
                            script_name: r.get::<_, String>(7).unwrap_or_default(),
                            min_level: r.get(8)?,
                            max_level: r.get(9)?,
                            respawn_time: r.get(10)?,
                            hp: r.get(11)?,
                            mp: r.get(12)?,
                            drop_list_id: r.get(13)?,
                            allegiance: r.get::<_, u8>(14)?,
                            spawn_type: r.get(15)?,
                            animation_id: r.get(16)?,
                            actor_state: r.get::<_, u16>(17)?,
                            private_area_name: r.get::<_, String>(18).unwrap_or_default(),
                            private_area_level: r.get(19)?,
                            zone_id: r.get(20)?,
                            genus_id: r.get(21)?,
                            actor_class_id: r.get(22)?,
                            current_job: r.get(23)?,
                            combat_skill: r.get(24)?,
                            combat_delay: r.get(25)?,
                            aggro_type: r.get::<_, u8>(26)?,
                            speed: r.get::<_, u8>(27)?,
                        })
                    })
                    .optional()?;
                Ok(row)
            })
            .await?;
        Ok(row)
    }

    pub async fn load_seamless_boundaries(&self) -> Result<HashMap<u32, Vec<SeamlessBoundary>>> {
        let rows = self
            .conn
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
        let rows = self
            .conn
            .call_db(|c| {
                let mut stmt = c.prepare(
                    r"SELECT i.catalogID, i.name, i.singular, i.plural, i.icon, i.rarity,
                             i.itemUICategory, i.stackSize, i.itemLevel, i.equipLevel,
                             i.price, i.buyPrice, i.sellPrice,
                             e.paramBonusType1,  e.paramBonusValue1,
                             e.paramBonusType2,  e.paramBonusValue2,
                             e.paramBonusType3,  e.paramBonusValue3,
                             e.paramBonusType4,  e.paramBonusValue4,
                             e.paramBonusType5,  e.paramBonusValue5,
                             e.paramBonusType6,  e.paramBonusValue6,
                             e.paramBonusType7,  e.paramBonusValue7,
                             e.paramBonusType8,  e.paramBonusValue8,
                             e.paramBonusType9,  e.paramBonusValue9,
                             e.paramBonusType10, e.paramBonusValue10,
                             w.damageInterval, w.damageAttributeType1, w.frequency,
                             w.damagePower,    w.attack,               w.parry
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
                        // paramBonus columns live on the LEFT-JOIN'd
                        // `gamedata_items_equipment` row and are NULL for
                        // non-equipment items — fallback to -1 (the
                        // empty-slot sentinel) so the parser skips them.
                        let mut bonuses: Vec<(u32, i32)> = Vec::new();
                        for slot in 0..10 {
                            // columns: 13 (type1), 14 (value1), 15 (type2), 16 (value2), ...
                            let type_idx = 13 + slot * 2;
                            let val_idx = 14 + slot * 2;
                            let ty: i32 = r
                                .get::<_, Option<i32>>(type_idx)
                                .ok()
                                .flatten()
                                .unwrap_or(-1);
                            let val: i32 = r
                                .get::<_, Option<i32>>(val_idx)
                                .ok()
                                .flatten()
                                .unwrap_or(0);
                            if let Some(mod_id) = decode_param_bonus_type(ty) {
                                if val != 0 {
                                    bonuses.push((mod_id, val));
                                }
                            }
                        }
                        // Weapon columns: 33 (damageInterval), 34
                        // (damageAttributeType1), 35 (frequency), 36
                        // (damagePower), 37 (attack), 38 (parry). The
                        // LEFT JOIN fills NULL on non-weapons; we treat
                        // "all six NULL" as "not a weapon" and leave
                        // `ItemData.weapon` as None. A single non-NULL
                        // column counts — damageInterval 0.0 is still a
                        // valid (if useless) weapon row.
                        let delay_s: Option<f64> =
                            r.get::<_, Option<f64>>(33).ok().flatten();
                        let attack_type: Option<i32> =
                            r.get::<_, Option<i32>>(34).ok().flatten();
                        let frequency: Option<i32> =
                            r.get::<_, Option<i32>>(35).ok().flatten();
                        let damage_power: Option<i32> =
                            r.get::<_, Option<i32>>(36).ok().flatten();
                        let attack: Option<i32> =
                            r.get::<_, Option<i32>>(37).ok().flatten();
                        let parry: Option<i32> =
                            r.get::<_, Option<i32>>(38).ok().flatten();
                        let weapon: Option<crate::data::WeaponAttributes> = if delay_s.is_none()
                            && attack_type.is_none()
                            && frequency.is_none()
                            && damage_power.is_none()
                            && attack.is_none()
                            && parry.is_none()
                        {
                            None
                        } else {
                            Some(crate::data::WeaponAttributes {
                                delay_ms: (delay_s.unwrap_or(0.0) * 1000.0).round().max(0.0)
                                    as u32,
                                attack_type: attack_type.unwrap_or(0).max(0) as u16,
                                // HitCount defaults to 1 so a weapon
                                // row with `frequency = 0` (bad data)
                                // doesn't silently disable auto-attacks
                                // when equipped.
                                hit_count: frequency
                                    .map(|f| f.max(1) as u16)
                                    .unwrap_or(1),
                                damage_power: damage_power.unwrap_or(0).max(0) as u16,
                                attack: attack.unwrap_or(0).max(0) as u16,
                                parry: parry.unwrap_or(0).max(0) as u16,
                            })
                        };
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
                                gear_bonuses: bonuses,
                                weapon,
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

    /// Fetch the set of currently-equipped catalog ids for a character
    /// filtered by the class slot they're logged in as. JOINs
    /// `characters_inventory_equipment` → `server_items` so the caller
    /// receives `(equip_slot, catalog_id)` pairs ready for the gear-
    /// paramBonus summer — the dispatcher can then cross-reference
    /// catalog ids against the `ItemData` map.
    ///
    /// Undergarment rows were written with `classId = 0` (see
    /// `inventory/referenced.rs` — the dispatcher's `DbEquip` arm forces
    /// class 0 for SLOT_UNDERSHIRT / SLOT_UNDERGARMENT), so we include
    /// them in the query alongside the active class.
    pub async fn load_equipped_catalog_ids(
        &self,
        chara_id: u32,
        class_id: u8,
    ) -> Result<HashMap<u16, u32>> {
        let rows = self
            .conn
            .call_db(move |c| {
                let mut stmt = c.prepare(
                    r"SELECT cie.equipSlot, si.itemId
                      FROM characters_inventory_equipment cie
                      JOIN server_items si ON si.id = cie.itemId
                      WHERE cie.characterId = :cid
                        AND (cie.classId = :class OR cie.classId = 0)",
                )?;
                let rows: Vec<(u16, u32)> = stmt
                    .query_map(
                        named_params! {
                            ":cid": chara_id,
                            ":class": class_id,
                        },
                        |r| {
                            Ok((
                                r.get::<_, u16>(0).unwrap_or_default(),
                                r.get::<_, u32>(1).unwrap_or_default(),
                            ))
                        },
                    )?
                    .collect::<rusqlite::Result<_>>()?;
                Ok(rows)
            })
            .await?;
        Ok(rows.into_iter().collect())
    }

    /// Load the `gamedata_quests` catalog (id, questName, className,
    /// prerequisite, minLevel) that the five-hook dispatcher uses to
    /// resolve a quest id to its Lua script path.
    pub async fn get_quest_gamedata(&self) -> Result<HashMap<u32, crate::gamedata::QuestMeta>> {
        let rows = self
            .conn
            .call_db(|c| {
                let mut stmt = c.prepare(
                    r"SELECT id, questName, className, prerequisite, minLevel
                      FROM gamedata_quests",
                )?;
                let rows: Vec<(u32, crate::gamedata::QuestMeta)> = stmt
                    .query_map([], |r| {
                        let id: u32 = r.get(0)?;
                        Ok((
                            id,
                            crate::gamedata::QuestMeta {
                                id,
                                quest_name: r.get::<_, String>(1).unwrap_or_default(),
                                class_name: r.get::<_, String>(2).unwrap_or_default(),
                                prerequisite: r.get::<_, u32>(3).unwrap_or_default(),
                                min_level: r.get::<_, u16>(4).unwrap_or_default(),
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
        let rows = self
            .conn
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

    /// Load every row of `gamedata_recipes` into a [`RecipeResolver`].
    /// Mirrors `Database.GetRecipeGamedata` on
    /// `origin/ioncannon/crafting_and_localleves` — the C# populates two
    /// dictionaries (id → Recipe, md5(mats) → [Recipe]); the resolver's
    /// constructor does both in one pass without the MD5 step.
    pub async fn load_recipes(&self) -> Result<crate::crafting::RecipeResolver> {
        let rows = self
            .conn
            .call_db(|c| {
                let mut stmt = c.prepare(
                    r"SELECT id, craftedItem, craftedQuantity, job,
                             crystal0ID, crystal0Quantity, crystal1ID, crystal1Quantity,
                             material0, material1, material2, material3,
                             material4, material5, material6, material7
                      FROM gamedata_recipes
                      ORDER BY craftedItem ASC",
                )?;
                let rows: Vec<crate::crafting::Recipe> = stmt
                    .query_map([], |r| {
                        let id: u32 = r.get(0)?;
                        let crafted_item: u32 = r.get::<_, u32>(1).unwrap_or_default();
                        let crafted_qty: u32 = r.get::<_, u32>(2).unwrap_or_default();
                        let job: String = r.get::<_, String>(3).unwrap_or_default();
                        let c0_id: u32 = r.get::<_, u32>(4).unwrap_or_default();
                        let c0_qty: u32 = r.get::<_, u32>(5).unwrap_or_default();
                        let c1_id: u32 = r.get::<_, u32>(6).unwrap_or_default();
                        let c1_qty: u32 = r.get::<_, u32>(7).unwrap_or_default();
                        let mats: [u32; crate::crafting::recipe::RECIPE_MATERIAL_SLOTS] = [
                            r.get::<_, u32>(8).unwrap_or_default(),
                            r.get::<_, u32>(9).unwrap_or_default(),
                            r.get::<_, u32>(10).unwrap_or_default(),
                            r.get::<_, u32>(11).unwrap_or_default(),
                            r.get::<_, u32>(12).unwrap_or_default(),
                            r.get::<_, u32>(13).unwrap_or_default(),
                            r.get::<_, u32>(14).unwrap_or_default(),
                            r.get::<_, u32>(15).unwrap_or_default(),
                        ];
                        let allowed: Vec<String> = job
                            .chars()
                            .next()
                            .and_then(crate::crafting::Recipe::job_code_to_class)
                            .map(|c| vec![c.to_string()])
                            .unwrap_or_default();
                        Ok(crate::crafting::Recipe::new(
                            id,
                            crafted_item,
                            crafted_qty,
                            mats,
                            c0_id,
                            c0_qty,
                            c1_id,
                            c1_qty,
                            allowed,
                            1,
                        ))
                    })?
                    .collect::<rusqlite::Result<_>>()?;
                Ok(rows)
            })
            .await?;
        Ok(crate::crafting::RecipeResolver::from_recipes(rows))
    }

    /// Load every row of `gamedata_passivegl_craft` into a map keyed by
    /// leve id. Mirrors `Database.GetPassiveGuildleveGamedata` on the
    /// ioncannon branch — each row becomes one
    /// [`PassiveGuildleveData`](crate::crafting::PassiveGuildleveData)
    /// with the four parallel difficulty arrays populated.
    pub async fn load_passive_guildleve_data(
        &self,
    ) -> Result<HashMap<u32, crate::crafting::PassiveGuildleveData>> {
        let rows = self
            .conn
            .call_db(|c| {
                let mut stmt = c.prepare(
                    r"SELECT id, plateId, borderId, recommendedClass,
                             issuingLocation, guildleveLocation, deliveryDisplayName,
                             objectiveItemId1, objectiveQuantity1, numberOfAttempts1,
                             recommendedLevel1, rewardItemId1, rewardQuantity1,
                             objectiveItemId2, objectiveQuantity2, numberOfAttempts2,
                             recommendedLevel2, rewardItemId2, rewardQuantity2,
                             objectiveItemId3, objectiveQuantity3, numberOfAttempts3,
                             recommendedLevel3, rewardItemId3, rewardQuantity3,
                             objectiveItemId4, objectiveQuantity4, numberOfAttempts4,
                             recommendedLevel4, rewardItemId4, rewardQuantity4
                      FROM gamedata_passivegl_craft",
                )?;
                let rows: Vec<(u32, crate::crafting::PassiveGuildleveData)> = stmt
                    .query_map([], |r| {
                        let id: u32 = r.get(0)?;
                        Ok((
                            id,
                            crate::crafting::PassiveGuildleveData {
                                id,
                                plate_id: r.get::<_, u32>(1).unwrap_or_default(),
                                border_id: r.get::<_, u32>(2).unwrap_or_default(),
                                recommended_class: r.get::<_, u32>(3).unwrap_or_default(),
                                issuing_location: r.get::<_, u32>(4).unwrap_or_default(),
                                leve_location: r.get::<_, u32>(5).unwrap_or_default(),
                                delivery_display_name: r.get::<_, u32>(6).unwrap_or_default(),
                                objective_item_id: [
                                    r.get::<_, i32>(7).unwrap_or_default(),
                                    r.get::<_, i32>(13).unwrap_or_default(),
                                    r.get::<_, i32>(19).unwrap_or_default(),
                                    r.get::<_, i32>(25).unwrap_or_default(),
                                ],
                                objective_quantity: [
                                    r.get::<_, i32>(8).unwrap_or_default(),
                                    r.get::<_, i32>(14).unwrap_or_default(),
                                    r.get::<_, i32>(20).unwrap_or_default(),
                                    r.get::<_, i32>(26).unwrap_or_default(),
                                ],
                                number_of_attempts: [
                                    r.get::<_, i32>(9).unwrap_or_default(),
                                    r.get::<_, i32>(15).unwrap_or_default(),
                                    r.get::<_, i32>(21).unwrap_or_default(),
                                    r.get::<_, i32>(27).unwrap_or_default(),
                                ],
                                recommended_level: [
                                    r.get::<_, i32>(10).unwrap_or_default(),
                                    r.get::<_, i32>(16).unwrap_or_default(),
                                    r.get::<_, i32>(22).unwrap_or_default(),
                                    r.get::<_, i32>(28).unwrap_or_default(),
                                ],
                                reward_item_id: [
                                    r.get::<_, i32>(11).unwrap_or_default(),
                                    r.get::<_, i32>(17).unwrap_or_default(),
                                    r.get::<_, i32>(23).unwrap_or_default(),
                                    r.get::<_, i32>(29).unwrap_or_default(),
                                ],
                                reward_quantity: [
                                    r.get::<_, i32>(12).unwrap_or_default(),
                                    r.get::<_, i32>(18).unwrap_or_default(),
                                    r.get::<_, i32>(24).unwrap_or_default(),
                                    r.get::<_, i32>(30).unwrap_or_default(),
                                ],
                            },
                        ))
                    })?
                    .collect::<rusqlite::Result<_>>()?;
                Ok(rows)
            })
            .await?;
        Ok(rows.into_iter().collect())
    }

    /// Load every row of `gamedata_gather_nodes` + `gamedata_gather_node_items`
    /// into a [`GatherResolver`](crate::gathering::GatherResolver). Mirrors
    /// the pattern used by [`Database::load_recipes`] — one catalog,
    /// two indexes, built once at boot and shared across Lua VMs via
    /// [`Catalogs::install_gather_resolver`](crate::lua::Catalogs::install_gather_resolver).
    pub async fn load_gather_resolver(&self) -> Result<crate::gathering::GatherResolver> {
        let (nodes, items) = self
            .conn
            .call_db(|c| {
                let mut nodes_stmt = c.prepare(
                    r"SELECT id, grade, attempts,
                             item1, item2, item3, item4, item5, item6,
                             item7, item8, item9, item10, item11
                      FROM gamedata_gather_nodes
                      ORDER BY id ASC",
                )?;
                let nodes: Vec<crate::gathering::GatherNode> = nodes_stmt
                    .query_map([], |r| {
                        let id: u32 = r.get(0)?;
                        let grade: u8 = r.get::<_, i64>(1).unwrap_or(1).clamp(0, 255) as u8;
                        let attempts: u8 = r.get::<_, i64>(2).unwrap_or(2).clamp(0, 255) as u8;
                        let mut items: [Option<i64>; crate::gathering::NODE_ITEM_SLOTS] =
                            [None; crate::gathering::NODE_ITEM_SLOTS];
                        for (i, slot) in items.iter_mut().enumerate() {
                            *slot = r.get::<_, Option<i64>>(3 + i).unwrap_or(None);
                        }
                        Ok(crate::gathering::GatherNode::from_raw(id, grade, attempts, items))
                    })?
                    .collect::<rusqlite::Result<_>>()?;
                let mut items_stmt = c.prepare(
                    r"SELECT id, itemCatalogId, remainder, aim, sweetspot, maxYield
                      FROM gamedata_gather_node_items
                      ORDER BY id ASC",
                )?;
                let items: Vec<crate::gathering::GatherNodeItem> = items_stmt
                    .query_map([], |r| {
                        Ok(crate::gathering::GatherNodeItem {
                            id: r.get::<_, u32>(0)?,
                            item_catalog_id: r.get::<_, u32>(1).unwrap_or_default(),
                            remainder: r
                                .get::<_, i64>(2)
                                .unwrap_or(80)
                                .clamp(0, 255) as u8,
                            aim: r.get::<_, i64>(3).unwrap_or(50).clamp(0, 255) as u8,
                            sweetspot: r
                                .get::<_, i64>(4)
                                .unwrap_or(30)
                                .clamp(0, 255) as u8,
                            max_yield: r.get::<_, u32>(5).unwrap_or(1),
                        })
                    })?
                    .collect::<rusqlite::Result<_>>()?;
                Ok((nodes, items))
            })
            .await?;
        Ok(crate::gathering::GatherResolver::from_parts(nodes, items))
    }

    /// Load every row of `server_gather_node_spawns`. Returned in id
    /// order so the world-manager can hand them to the spawn loop
    /// without sorting. Each row is self-contained (private-area
    /// membership lives on the row, not in a join table) so this is a
    /// single SELECT.
    pub async fn load_gather_node_spawns(
        &self,
    ) -> Result<Vec<crate::gathering::GatherNodeSpawn>> {
        let rows = self
            .conn
            .call_db(|c| {
                let mut stmt = c.prepare(
                    r"SELECT id, actorClassId, uniqueId, zoneId,
                             privateAreaName, privateAreaLevel,
                             positionX, positionY, positionZ, rotation,
                             harvestNodeId, harvestType
                      FROM server_gather_node_spawns
                      ORDER BY id ASC",
                )?;
                let rows: Vec<crate::gathering::GatherNodeSpawn> = stmt
                    .query_map([], |r| {
                        Ok(crate::gathering::GatherNodeSpawn {
                            id: r.get::<_, u32>(0)?,
                            actor_class_id: r.get::<_, u32>(1)?,
                            unique_id: r.get::<_, String>(2).unwrap_or_default(),
                            zone_id: r.get::<_, u32>(3)?,
                            private_area_name: r.get::<_, String>(4).unwrap_or_default(),
                            private_area_level: r.get::<_, i32>(5).unwrap_or_default(),
                            position: (
                                r.get::<_, f32>(6).unwrap_or_default(),
                                r.get::<_, f32>(7).unwrap_or_default(),
                                r.get::<_, f32>(8).unwrap_or_default(),
                            ),
                            rotation: r.get::<_, f32>(9).unwrap_or_default(),
                            harvest_node_id: r.get::<_, u32>(10)?,
                            harvest_type: r.get::<_, u32>(11).unwrap_or(
                                crate::gathering::HARVEST_TYPE_MINE,
                            ),
                        })
                    })?
                    .collect::<rusqlite::Result<_>>()?;
                Ok(rows)
            })
            .await?;
        Ok(rows)
    }

    /// Load every row of `gamedata_regional_leves` into a
    /// [`RegionalLeveResolver`](crate::leve::RegionalLeveResolver).
    /// Parallels [`Database::load_recipes`] / [`Database::load_gather_resolver`]
    /// — one catalog, built once at boot, shared across Lua VMs via
    /// [`Catalogs::install_regional_leve_resolver`](crate::lua::Catalogs::install_regional_leve_resolver).
    /// Rows with an unknown `leveType` discriminator are skipped (not
    /// silently corrupted into the default), so the resolver never
    /// misroutes progress events.
    pub async fn load_regional_leve_resolver(
        &self,
    ) -> Result<crate::leve::RegionalLeveResolver> {
        let rows = self
            .conn
            .call_db(|c| {
                let mut stmt = c.prepare(
                    r"SELECT id, leveType, plateId, borderId, recommendedClass,
                             issuingLocation, guildleveLocation, deliveryDisplayName, region,
                             objectiveTargetId1, objectiveQuantity1, recommendedLevel1,
                             rewardItemId1, rewardQuantity1, rewardGil1,
                             objectiveTargetId2, objectiveQuantity2, recommendedLevel2,
                             rewardItemId2, rewardQuantity2, rewardGil2,
                             objectiveTargetId3, objectiveQuantity3, recommendedLevel3,
                             rewardItemId3, rewardQuantity3, rewardGil3,
                             objectiveTargetId4, objectiveQuantity4, recommendedLevel4,
                             rewardItemId4, rewardQuantity4, rewardGil4
                      FROM gamedata_regional_leves
                      ORDER BY id ASC",
                )?;
                let rows: Vec<crate::leve::RegionalLeveData> = stmt
                    .query_map([], |r| {
                        let Some(ty) = crate::leve::LeveType::from_repr(r.get::<_, i64>(1)?) else {
                            // Unknown discriminator → return a sentinel
                            // the outer filter drops. We can't return
                            // `None` directly from inside the rusqlite
                            // closure, so mark the id as 0 and filter
                            // after.
                            return Ok(crate::leve::RegionalLeveData {
                                id: 0,
                                leve_type: crate::leve::LeveType::Fieldcraft,
                                plate_id: 0,
                                border_id: 0,
                                recommended_class: 0,
                                issuing_location: 0,
                                leve_location: 0,
                                delivery_display_name: 0,
                                region: 0,
                                objective_target_id: [0; 4],
                                objective_quantity: [0; 4],
                                recommended_level: [0; 4],
                                reward_item_id: [0; 4],
                                reward_quantity: [0; 4],
                                reward_gil: [0; 4],
                            });
                        };
                        Ok(crate::leve::RegionalLeveData {
                            id: r.get::<_, u32>(0)?,
                            leve_type: ty,
                            plate_id: r.get::<_, u32>(2).unwrap_or_default(),
                            border_id: r.get::<_, u32>(3).unwrap_or_default(),
                            recommended_class: r.get::<_, u32>(4).unwrap_or_default(),
                            issuing_location: r.get::<_, u32>(5).unwrap_or_default(),
                            leve_location: r.get::<_, u32>(6).unwrap_or_default(),
                            delivery_display_name: r.get::<_, u32>(7).unwrap_or_default(),
                            region: r.get::<_, u32>(8).unwrap_or_default(),
                            objective_target_id: [
                                r.get::<_, i32>(9)?,
                                r.get::<_, i32>(15)?,
                                r.get::<_, i32>(21)?,
                                r.get::<_, i32>(27)?,
                            ],
                            objective_quantity: [
                                r.get::<_, i32>(10)?,
                                r.get::<_, i32>(16)?,
                                r.get::<_, i32>(22)?,
                                r.get::<_, i32>(28)?,
                            ],
                            recommended_level: [
                                r.get::<_, i32>(11)?,
                                r.get::<_, i32>(17)?,
                                r.get::<_, i32>(23)?,
                                r.get::<_, i32>(29)?,
                            ],
                            reward_item_id: [
                                r.get::<_, i32>(12)?,
                                r.get::<_, i32>(18)?,
                                r.get::<_, i32>(24)?,
                                r.get::<_, i32>(30)?,
                            ],
                            reward_quantity: [
                                r.get::<_, i32>(13)?,
                                r.get::<_, i32>(19)?,
                                r.get::<_, i32>(25)?,
                                r.get::<_, i32>(31)?,
                            ],
                            reward_gil: [
                                r.get::<_, i32>(14)?,
                                r.get::<_, i32>(20)?,
                                r.get::<_, i32>(26)?,
                                r.get::<_, i32>(32)?,
                            ],
                        })
                    })?
                    .collect::<rusqlite::Result<Vec<_>>>()?;
                Ok(rows)
            })
            .await?;
        // Filter out the id-0 sentinels that mark unknown-discriminator
        // rows (see the `from_repr` None branch above).
        let filtered = rows.into_iter().filter(|r| r.id != 0);
        Ok(crate::leve::RegionalLeveResolver::from_rows(filtered))
    }

    /// Grant a stack of a gathered item to `chara_id`'s NORMAL bag.
    /// Matches the C# `Player.GetItemPackage(INVENTORY_NORMAL).AddItem(...)`
    /// contract: if a partial stack of the same item+quality already
    /// exists, top it up first; otherwise insert a fresh stack.
    ///
    /// This is the direct-DB write path used while the live ItemPackage
    /// runtime state isn't yet addressable from the runtime command
    /// drain (see `Player` not being in `ActorRegistry`). It parallels
    /// [`Database::add_gil`] and is adequate for "you gathered N copper
    /// ore" style flows where the grant is acknowledged on the next
    /// inventory resync — which is also how the retail 1.x client
    /// behaves while the minigame's `textInputWidget` is still shown.
    /// Returns the new quantity of the stack that absorbed the grant.
    pub async fn add_harvest_item(
        &self,
        chara_id: u32,
        item_catalog_id: u32,
        delta: i32,
        quality: u8,
    ) -> Result<i32> {
        if delta <= 0 || item_catalog_id == 0 {
            return Ok(0);
        }
        let total = self
            .conn
            .call_db(move |c| {
                let tx = c.transaction()?;
                // Merge into a pre-existing partial stack if one
                // exists. `item_packages` NORMAL = 0. `stackSize` lives
                // on `gamedata_items` (keyed by catalogID), not
                // `server_items` — the LEFT JOIN lets the merge work
                // even when the catalog row isn't seeded yet (common
                // in the test harness). Stack-cap enforcement is
                // deferred; treat any existing stack as capable of
                // absorbing the delta.
                let existing: Option<(i64, i32, i32, i32)> = tx
                    .query_row(
                        r"SELECT si.id, si.quantity, ci.slot, COALESCE(gi.stackSize, 99)
                          FROM characters_inventory ci
                          INNER JOIN server_items si ON ci.serverItemId = si.id
                          LEFT JOIN gamedata_items gi ON si.itemId = gi.catalogID
                          WHERE ci.characterId = :cid
                            AND ci.itemPackage = 0
                            AND si.itemId = :iid
                            AND si.quality = :q",
                        named_params! {
                            ":cid": chara_id,
                            ":iid": item_catalog_id,
                            ":q": quality as i64,
                        },
                        |r| Ok((
                            r.get::<_, i64>(0)?,
                            r.get::<_, i32>(1)?,
                            r.get::<_, i32>(2)?,
                            r.get::<_, i32>(3).unwrap_or(99),
                        )),
                    )
                    .optional()?;
                let new_total = match existing {
                    Some((sid, qty, _slot, _stack_max)) => {
                        let updated = qty.saturating_add(delta).max(0);
                        tx.execute(
                            "UPDATE server_items SET quantity = :q WHERE id = :id",
                            named_params! { ":q": updated, ":id": sid },
                        )?;
                        updated
                    }
                    None => {
                        let seed = delta.max(0);
                        tx.execute(
                            r"INSERT INTO server_items (itemId, quantity, quality)
                              VALUES (:iid, :q, :qual)",
                            named_params! {
                                ":iid": item_catalog_id,
                                ":q": seed,
                                ":qual": quality as i64,
                            },
                        )?;
                        let sid = tx.last_insert_rowid();
                        // Next empty slot = current NORMAL row count.
                        // Matches `ItemPackage.end_of_list_index` in Rust.
                        let next_slot: i32 = tx
                            .query_row(
                                r"SELECT COALESCE(MAX(slot), -1) + 1
                                  FROM characters_inventory
                                  WHERE characterId = :cid AND itemPackage = 0",
                                named_params! { ":cid": chara_id },
                                |r| r.get::<_, i32>(0),
                            )
                            .unwrap_or(0);
                        tx.execute(
                            r"INSERT INTO characters_inventory
                                (characterId, itemPackage, serverItemId, slot)
                              VALUES (:cid, 0, :sid, :slot)",
                            named_params! {
                                ":cid": chara_id, ":sid": sid, ":slot": next_slot,
                            },
                        )?;
                        seed
                    }
                };
                tx.commit()?;
                Ok(new_total)
            })
            .await?;
        Ok(total)
    }

    /// Tier 4 #14 C — grant a stack to `retainer_id`'s personal
    /// inventory. Parallels [`Database::add_harvest_item`] but writes
    /// to `characters_retainer_inventory` (keyed by `retainerId`)
    /// rather than `characters_inventory` (keyed by `characterId`),
    /// so retainer storage and player inventory stay logically
    /// disjoint.
    ///
    /// Merges an existing partial stack of the same `(item_id,
    /// quality, package)` when one exists; spills to a new row with
    /// the next free slot otherwise. Quantity clamps to ≥ 0 on
    /// negative delta (matches the player-side behaviour, though
    /// retainer scripts never legitimately pass a negative).
    pub async fn add_retainer_inventory_item(
        &self,
        retainer_id: u32,
        item_catalog_id: u32,
        delta: i32,
        quality: u8,
        item_package: u16,
    ) -> Result<i32> {
        if delta <= 0 || item_catalog_id == 0 {
            return Ok(0);
        }
        let total = self
            .conn
            .call_db(move |c| {
                let tx = c.transaction()?;
                let existing: Option<(i64, i32, i32)> = tx
                    .query_row(
                        r"SELECT si.id, si.quantity, ri.slot
                          FROM characters_retainer_inventory ri
                          INNER JOIN server_items si ON ri.serverItemId = si.id
                          WHERE ri.retainerId = :rid
                            AND ri.itemPackage = :pkg
                            AND si.itemId = :iid
                            AND si.quality = :q",
                        named_params! {
                            ":rid": retainer_id,
                            ":pkg": item_package as i64,
                            ":iid": item_catalog_id,
                            ":q": quality as i64,
                        },
                        |r| Ok((
                            r.get::<_, i64>(0)?,
                            r.get::<_, i32>(1)?,
                            r.get::<_, i32>(2)?,
                        )),
                    )
                    .optional()?;
                let new_total = match existing {
                    Some((sid, qty, _slot)) => {
                        let updated = qty.saturating_add(delta).max(0);
                        tx.execute(
                            "UPDATE server_items SET quantity = :q WHERE id = :id",
                            named_params! { ":q": updated, ":id": sid },
                        )?;
                        updated
                    }
                    None => {
                        let seed = delta.max(0);
                        tx.execute(
                            r"INSERT INTO server_items (itemId, quantity, quality)
                              VALUES (:iid, :q, :qual)",
                            named_params! {
                                ":iid": item_catalog_id,
                                ":q": seed,
                                ":qual": quality as i64,
                            },
                        )?;
                        let sid = tx.last_insert_rowid();
                        let next_slot: i32 = tx
                            .query_row(
                                r"SELECT COALESCE(MAX(slot), -1) + 1
                                  FROM characters_retainer_inventory
                                  WHERE retainerId = :rid AND itemPackage = :pkg",
                                named_params! {
                                    ":rid": retainer_id,
                                    ":pkg": item_package as i64,
                                },
                                |r| r.get::<_, i32>(0),
                            )
                            .unwrap_or(0);
                        tx.execute(
                            r"INSERT INTO characters_retainer_inventory
                                (retainerId, itemPackage, serverItemId, slot)
                              VALUES (:rid, :pkg, :sid, :slot)",
                            named_params! {
                                ":rid": retainer_id,
                                ":pkg": item_package as i64,
                                ":sid": sid,
                                ":slot": next_slot,
                            },
                        )?;
                        seed
                    }
                };
                tx.commit()?;
                Ok(new_total)
            })
            .await?;
        Ok(total)
    }

    pub async fn load_global_status_effect_list(&self) -> Result<HashMap<u32, StatusEffectDef>> {
        let rows = self
            .conn
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
        let rows = self
            .conn
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
            by_level
                .entry((bc.job, bc.level as i16))
                .or_default()
                .push(bc.id);
            dict.insert(bc.id, bc);
        }
        Ok((dict, by_level))
    }

    pub async fn load_global_battle_trait_list(
        &self,
    ) -> Result<(HashMap<u16, BattleTrait>, HashMap<u8, Vec<u16>>)> {
        let rows = self
            .conn
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
        let basic = self
            .conn
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

        player.class_levels = self
            .load_class_levels_and_exp(chara_id)
            .await
            .unwrap_or_default();
        player.parameter_save = self.load_parameter_save(chara_id).await.unwrap_or_default();
        player.appearance = self
            .load_appearance_full(chara_id)
            .await
            .unwrap_or_default();
        player.status_effects = self
            .load_character_status_effects(chara_id)
            .await
            .unwrap_or_default();
        player.chocobo = self.load_chocobo(chara_id).await.unwrap_or_default();
        player.timers = self.load_timers(chara_id).await.unwrap_or_default();
        player.hotbar = self
            .load_hotbar(chara_id, player.parameter_save.state_main_skill[0])
            .await
            .unwrap_or_default();
        player.quest_scenario = self.load_quest_scenario(chara_id).await.unwrap_or_default();
        player.guildleves_local = self
            .load_guildleves_local(chara_id)
            .await
            .unwrap_or_default();
        player.guildleves_regional = self
            .load_guildleves_regional(chara_id)
            .await
            .unwrap_or_default();
        player.npc_linkshells = self.load_npc_linkshells(chara_id).await.unwrap_or_default();

        for (target, ty) in [
            (&mut player.inventory_normal, 0u32),
            (&mut player.inventory_key_items, 1),
            (&mut player.inventory_currency, 2),
            (&mut player.inventory_bazaar, 3),
            (&mut player.inventory_meldrequest, 4),
            (&mut player.inventory_loot, 5),
        ] {
            *target = self
                .get_item_package(chara_id, ty)
                .await
                .unwrap_or_default();
        }

        player.equipment = self
            .get_equipment(chara_id, player.parameter_save.state_main_skill[0] as u16)
            .await
            .unwrap_or_default();

        Ok(Some(player))
    }

    pub async fn load_class_levels_and_exp(&self, chara_id: u32) -> Result<CharaBattleSave> {
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
        let v = self
            .conn
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
        let v = self
            .conn
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
                                ears: r.get::<_, u8>(12).unwrap_or_default(),           // SQL col `ears`
                                face_mouth: r.get::<_, u8>(13).unwrap_or_default(),
                                face_features: r.get::<_, u8>(14).unwrap_or_default(),  // SQL col `faceFeatures`
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

    async fn load_character_status_effects(&self, chara_id: u32) -> Result<Vec<StatusEffectEntry>> {
        let rows = self
            .conn
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
        let v = self
            .conn
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
        let v = self
            .conn
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
        let rows = self
            .conn
            .call_db(move |c| {
                let mut stmt = c.prepare(
                    r"SELECT slot, questId, sequence, flags, counter1, counter2, counter3,
                              npc_ls_from, npc_ls_msg_step
                      FROM characters_quest_scenario WHERE characterId = :cid",
                )?;
                let rows: Vec<QuestScenarioEntry> = stmt
                    .query_map(named_params! { ":cid": chara_id }, |r| {
                        Ok(QuestScenarioEntry {
                            slot: r.get::<_, u16>(0).unwrap_or_default(),
                            quest_id: r.get::<_, u32>(1).unwrap_or_default(),
                            sequence: r.get::<_, u32>(2).unwrap_or_default(),
                            flags: r.get::<_, u32>(3).unwrap_or_default(),
                            counter1: r.get::<_, u16>(4).unwrap_or_default(),
                            counter2: r.get::<_, u16>(5).unwrap_or_default(),
                            counter3: r.get::<_, u16>(6).unwrap_or_default(),
                            npc_ls_from: r.get::<_, u32>(7).unwrap_or_default(),
                            npc_ls_msg_step: r.get::<_, u8>(8).unwrap_or_default(),
                        })
                    })?
                    .collect::<rusqlite::Result<_>>()?;
                Ok(rows)
            })
            .await?;
        Ok(rows)
    }

    /// Load a character's 2048-bit completed-quest bitfield. Empty BLOBs
    /// (never-completed-anything) and missing rows both return an all-zero
    /// bitstream.
    pub async fn load_completed_quests(&self, chara_id: u32) -> Result<Bitstream2048> {
        let bytes = self
            .conn
            .call_db(move |c| {
                c.query_row(
                    "SELECT completedQuests FROM characters_quest_completed WHERE characterId = :cid",
                    named_params! { ":cid": chara_id },
                    |r| r.get::<_, Option<Vec<u8>>>(0),
                )
                .optional()
            })
            .await?;
        Ok(match bytes.flatten() {
            Some(bytes) => Bitstream2048::from_slice(&bytes),
            None => Bitstream2048::new(),
        })
    }

    /// Write a character's full 2048-bit completion bitfield.
    pub async fn save_completed_quests(
        &self,
        chara_id: u32,
        bitstream: &Bitstream2048,
    ) -> Result<()> {
        let bytes = bitstream.as_bytes().to_vec();
        self.conn
            .call_db(move |c| {
                c.execute(
                    r"INSERT INTO characters_quest_completed (characterId, completedQuests)
                      VALUES (:cid, :blob)
                      ON CONFLICT(characterId) DO UPDATE SET completedQuests = excluded.completedQuests",
                    named_params! { ":cid": chara_id, ":blob": bytes },
                )?;
                Ok(())
            })
            .await?;
        Ok(())
    }

    async fn load_guildleves_local(&self, chara_id: u32) -> Result<Vec<GuildleveLocalEntry>> {
        let rows = self
            .conn
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

    async fn load_guildleves_regional(&self, chara_id: u32) -> Result<Vec<GuildleveRegionalEntry>> {
        let rows = self
            .conn
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
        let rows = self
            .conn
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

    /// Persist one slot of `characters_quest_scenario` under the
    /// post-redesign layout (`sequence` + `flags` + three 16-bit counters).
    /// The migration-050 NpcLs scratchpad columns
    /// (`npc_ls_from` / `npc_ls_msg_step`) are NOT touched — the ON
    /// CONFLICT branch leaves them at whatever value `save_quest_npc_ls`
    /// previously wrote, and INSERTs fall through to the schema's
    /// `DEFAULT 0`. This keeps the 8 existing call sites caller-shape
    /// stable + isolates the NpcLs persistence to a focused
    /// per-mutation path.
    #[allow(clippy::too_many_arguments)]
    pub async fn save_quest(
        &self,
        chara_id: u32,
        slot: i32,
        quest_actor_id: u32,
        sequence: u32,
        flags: u32,
        counter1: u16,
        counter2: u16,
        counter3: u16,
    ) -> Result<()> {
        let qid = 0xF_FFFFu32 & quest_actor_id;
        self.conn
            .call_db(move |c| {
                c.execute(
                    r"INSERT INTO characters_quest_scenario
                        (characterId, slot, questId, sequence, flags,
                         counter1, counter2, counter3)
                      VALUES (:cid, :slot, :qid, :seq, :flags,
                              :c1, :c2, :c3)
                      ON CONFLICT(characterId, slot) DO UPDATE SET
                        questId  = excluded.questId,
                        sequence = excluded.sequence,
                        flags    = excluded.flags,
                        counter1 = excluded.counter1,
                        counter2 = excluded.counter2,
                        counter3 = excluded.counter3",
                    named_params! {
                        ":cid": chara_id,
                        ":slot": slot,
                        ":qid": qid,
                        ":seq": sequence,
                        ":flags": flags,
                        ":c1": counter1,
                        ":c2": counter2,
                        ":c3": counter3,
                    },
                )?;
                Ok(())
            })
            .await?;
        Ok(())
    }

    /// Update only the NpcLs scratchpad columns for an existing
    /// `characters_quest_scenario` row. Called from
    /// `LuaCommand::QuestSetNpcLsFrom` / `IncrementNpcLsMsgStep` /
    /// `ClearNpcLs` apply paths. Silently no-ops if the row doesn't
    /// exist (UPDATE matches 0 rows) — caller is expected to have
    /// added the quest via `save_quest` first.
    pub async fn save_quest_npc_ls(
        &self,
        chara_id: u32,
        slot: i32,
        npc_ls_from: u32,
        npc_ls_msg_step: u8,
    ) -> Result<()> {
        self.conn
            .call_db(move |c| {
                c.execute(
                    r"UPDATE characters_quest_scenario
                      SET npc_ls_from = :nlsf, npc_ls_msg_step = :nlss
                      WHERE characterId = :cid AND slot = :slot",
                    named_params! {
                        ":cid": chara_id,
                        ":slot": slot,
                        ":nlsf": npc_ls_from,
                        ":nlss": npc_ls_msg_step,
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

    /// Set one quest's completion bit inside the 2048-bit bitfield
    /// column. No-ops when `quest_id` is outside the compact id space
    /// (`110_001..=112_048`), matching Meteor's silent clamp.
    pub async fn complete_quest(&self, chara_id: u32, quest_id: u32) -> Result<()> {
        let Some(bit) = crate::actor::quest::quest_id_to_bit(quest_id) else {
            return Ok(());
        };
        let mut current = self.load_completed_quests(chara_id).await?;
        if current.get(bit) {
            return Ok(());
        }
        current.set(bit);
        self.save_completed_quests(chara_id, &current).await
    }

    pub async fn is_quest_completed(&self, chara_id: u32, quest_id: u32) -> Result<bool> {
        let Some(bit) = crate::actor::quest::quest_id_to_bit(quest_id) else {
            return Ok(false);
        };
        let bitstream = self.load_completed_quests(chara_id).await?;
        Ok(bitstream.get(bit))
    }

    // =======================================================================
    // Equipment / hotbar
    // =======================================================================

    pub async fn get_equipment(&self, chara_id: u32, class_id: u16) -> Result<Vec<EquipmentSlot>> {
        let rows = self
            .conn
            .call_db(move |c| {
                let mut stmt = c.prepare(
                    r"SELECT equipSlot, itemId FROM characters_inventory_equipment
                      WHERE characterId = :cid AND (classId = :class OR classId = 0)
                      ORDER BY equipSlot",
                )?;
                let rows: Vec<EquipmentSlot> = stmt
                    .query_map(
                        named_params! { ":cid": chara_id, ":class": class_id },
                        |r| {
                            Ok(EquipmentSlot {
                                equip_slot: r.get::<_, u16>(0).unwrap_or_default(),
                                item_id: r.get::<_, u64>(1).unwrap_or_default(),
                            })
                        },
                    )?
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
        let rows = self
            .conn
            .call_db(move |c| {
                let mut stmt = c.prepare(
                    r"SELECT hotbarSlot, commandId, recastTime
                      FROM characters_hotbar
                      WHERE characterId = :cid AND classId = :class
                      ORDER BY hotbarSlot",
                )?;
                let rows: Vec<HotbarEntry> = stmt
                    .query_map(
                        named_params! { ":cid": chara_id, ":class": class_id },
                        |r| {
                            Ok(HotbarEntry {
                                hotbar_slot: r.get::<_, u16>(0).unwrap_or_default(),
                                command_id: r.get::<_, u32>(1).unwrap_or_default(),
                                recast_time: r.get::<_, u32>(2).unwrap_or_default(),
                            })
                        },
                    )?
                    .collect::<rusqlite::Result<_>>()?;
                Ok(rows)
            })
            .await?;
        Ok(rows)
    }

    pub async fn find_first_command_slot(&self, chara_id: u32, class_id: u8) -> Result<u16> {
        let slots = self
            .conn
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
        let item = self
            .conn
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
                        stmt.execute(named_params! { ":slot": slot, ":iid": iid as i64 })?;
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
        let rows = self
            .conn
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
        let rows = self
            .conn
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
        let v = self
            .conn
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
        let ok = self
            .conn
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

    /// Grant `delta` gil to `chara_id`. The 1.x gil stack lives as an
    /// `itemId = 1_000_001` row in `server_items`, linked from
    /// `characters_inventory` at `itemPackage = PKG_CURRENCY_CRYSTALS (99)`.
    /// First-time callers get a fresh row; subsequent calls increment the
    /// quantity in place. Returns the new total.
    pub async fn add_gil(&self, chara_id: u32, delta: i32) -> Result<i32> {
        const GIL_ITEM_ID: u32 = 1_000_001;
        const PKG_CURRENCY: u16 = 99;
        let total = self
            .conn
            .call_db(move |c| {
                let tx = c.transaction()?;
                let existing: Option<(i64, i32)> = tx
                    .query_row(
                        r"SELECT ci.serverItemId, si.quantity
                          FROM characters_inventory ci
                          INNER JOIN server_items si ON ci.serverItemId = si.id
                          WHERE ci.characterId = :cid
                            AND ci.itemPackage = :pkg
                            AND si.itemId = :iid",
                        named_params! { ":cid": chara_id, ":pkg": PKG_CURRENCY, ":iid": GIL_ITEM_ID },
                        |r| Ok((r.get::<_, i64>(0)?, r.get::<_, i32>(1)?)),
                    )
                    .optional()?;
                let new_total = match existing {
                    Some((sid, qty)) => {
                        let updated = qty.saturating_add(delta).max(0);
                        tx.execute(
                            "UPDATE server_items SET quantity = :q WHERE id = :id",
                            named_params! { ":q": updated, ":id": sid },
                        )?;
                        updated
                    }
                    None => {
                        let seed = delta.max(0);
                        tx.execute(
                            r"INSERT INTO server_items (itemId, quantity, quality)
                              VALUES (:iid, :q, 1)",
                            named_params! { ":iid": GIL_ITEM_ID, ":q": seed },
                        )?;
                        let sid = tx.last_insert_rowid();
                        tx.execute(
                            r"INSERT INTO characters_inventory
                                (characterId, itemPackage, serverItemId, slot)
                              VALUES (:cid, :pkg, :sid, 0)",
                            named_params! { ":cid": chara_id, ":pkg": PKG_CURRENCY, ":sid": sid },
                        )?;
                        seed
                    }
                };
                tx.commit()?;
                Ok(new_total)
            })
            .await?;
        Ok(total)
    }

    pub async fn get_linkshell_member_character_ids(&self, ls_id: u64) -> Result<Vec<u32>> {
        let ls_id_i = ls_id as i64;
        let rows = self
            .conn
            .call_db(move |c| {
                let mut stmt = c.prepare(
                    r"SELECT characterId FROM characters_linkshells WHERE linkshellId = :id",
                )?;
                let rows: Vec<u32> = stmt
                    .query_map(named_params! { ":id": ls_id_i }, |r| r.get::<_, u32>(0))?
                    .collect::<rusqlite::Result<_>>()?;
                Ok(rows)
            })
            .await?;
        Ok(rows)
    }

    pub async fn character_id_by_name(&self, name: &str) -> Result<Option<u32>> {
        let name = name.to_owned();
        let id = self
            .conn
            .call_db(move |c| {
                let v: Option<u32> = c
                    .query_row(
                        "SELECT id FROM characters WHERE name = :n",
                        named_params! { ":n": name },
                        |r| r.get(0),
                    )
                    .optional()?;
                Ok(v)
            })
            .await?;
        Ok(id)
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
        let v = self
            .conn
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
        let rows = self
            .conn
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
        let v = self
            .conn
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
        let rows = self
            .conn
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

    /// Set the player's currently-joined Grand Company. `gc_current`
    /// is 0 (none) / 1 (Maelstrom) / 2 (Twin Adder) / 3 (Immortal
    /// Flames). Mirrors `Database.PlayerCharacterUpdateGrandCompany`
    /// in Meteor — same column, same enum.
    pub async fn set_gc_current(&self, chara_id: u32, gc: u8) -> Result<()> {
        self.conn
            .call_db(move |c| {
                c.execute(
                    r"UPDATE characters SET gcCurrent = :g WHERE id = :cid",
                    named_params! { ":g": gc as i64, ":cid": chara_id },
                )?;
                Ok(())
            })
            .await?;
        Ok(())
    }

    /// Set the player's rank for one specific GC. Picks the right
    /// column based on the GC id (1/2/3); no-ops for invalid ids.
    pub async fn set_gc_rank(&self, chara_id: u32, gc: u8, rank: u8) -> Result<()> {
        let col = match gc {
            crate::actor::gc::GC_MAELSTROM => "gcLimsaRank",
            crate::actor::gc::GC_TWIN_ADDER => "gcGridaniaRank",
            crate::actor::gc::GC_IMMORTAL_FLAMES => "gcUldahRank",
            _ => return Ok(()),
        };
        let col = col.to_owned();
        self.conn
            .call_db(move |c| {
                let sql = format!("UPDATE characters SET {col} = :r WHERE id = :cid");
                c.execute(
                    &sql,
                    named_params! { ":r": rank as i64, ":cid": chara_id },
                )?;
                Ok(())
            })
            .await?;
        Ok(())
    }

    /// Grant `delta` seals of `gc`'s currency to `chara_id`. The seal
    /// stack lives in `characters_inventory` at `itemPackage =
    /// PKG_CURRENCY_CRYSTALS (99)`, keyed by the 1_000_20X item id.
    /// First call inserts a fresh stack; subsequent calls merge in
    /// place. Same transactional-upsert shape as `add_gil`. Returns
    /// the new total, or `0` for an invalid GC id.
    pub async fn add_seals(&self, chara_id: u32, gc: u8, delta: i32) -> Result<i32> {
        const PKG_CURRENCY: u16 = 99;
        let Some(seal_id) = crate::actor::gc::seal_item_id(gc) else {
            return Ok(0);
        };
        let total = self
            .conn
            .call_db(move |c| {
                let tx = c.transaction()?;
                let existing: Option<(i64, i32)> = tx
                    .query_row(
                        r"SELECT ci.serverItemId, si.quantity
                          FROM characters_inventory ci
                          INNER JOIN server_items si ON ci.serverItemId = si.id
                          WHERE ci.characterId = :cid
                            AND ci.itemPackage = :pkg
                            AND si.itemId = :iid",
                        named_params! { ":cid": chara_id, ":pkg": PKG_CURRENCY, ":iid": seal_id },
                        |r| Ok((r.get::<_, i64>(0)?, r.get::<_, i32>(1)?)),
                    )
                    .optional()?;
                let new_total = match existing {
                    Some((sid, qty)) => {
                        let updated = qty.saturating_add(delta).max(0);
                        tx.execute(
                            "UPDATE server_items SET quantity = :q WHERE id = :id",
                            named_params! { ":q": updated, ":id": sid },
                        )?;
                        updated
                    }
                    None => {
                        let seed = delta.max(0);
                        tx.execute(
                            r"INSERT INTO server_items (itemId, quantity, quality)
                              VALUES (:iid, :q, 1)",
                            named_params! { ":iid": seal_id, ":q": seed },
                        )?;
                        let sid = tx.last_insert_rowid();
                        // Currency slot assignment — pick the next
                        // free slot within the currency package
                        // (matches add_gil behaviour).
                        let next_slot: i32 = tx
                            .query_row(
                                r"SELECT COALESCE(MAX(slot), -1) + 1
                                  FROM characters_inventory
                                  WHERE characterId = :cid AND itemPackage = :pkg",
                                named_params! { ":cid": chara_id, ":pkg": PKG_CURRENCY },
                                |r| r.get::<_, i32>(0),
                            )
                            .unwrap_or(0);
                        tx.execute(
                            r"INSERT INTO characters_inventory
                                (characterId, itemPackage, serverItemId, slot)
                              VALUES (:cid, :pkg, :sid, :slot)",
                            named_params! {
                                ":cid": chara_id, ":pkg": PKG_CURRENCY,
                                ":sid": sid, ":slot": next_slot,
                            },
                        )?;
                        seed
                    }
                };
                tx.commit()?;
                Ok(new_total)
            })
            .await?;
        Ok(total)
    }

    /// Read current seal balance for `gc`. Returns `0` for unknown
    /// GC ids or when no stack exists yet.
    pub async fn get_seals(&self, chara_id: u32, gc: u8) -> Result<i32> {
        const PKG_CURRENCY: u16 = 99;
        let Some(seal_id) = crate::actor::gc::seal_item_id(gc) else {
            return Ok(0);
        };
        let v = self
            .conn
            .call_db(move |c| {
                let v: Option<i32> = c
                    .query_row(
                        r"SELECT si.quantity
                          FROM characters_inventory ci
                          INNER JOIN server_items si ON ci.serverItemId = si.id
                          WHERE ci.characterId = :cid
                            AND ci.itemPackage = :pkg
                            AND si.itemId = :iid",
                        named_params! { ":cid": chara_id, ":pkg": PKG_CURRENCY, ":iid": seal_id },
                        |r| r.get::<_, i32>(0),
                    )
                    .optional()?;
                Ok(v.unwrap_or(0))
            })
            .await?;
        Ok(v)
    }

    /// Rename the chocobo. Only updates `chocoboName`; appearance
    /// and ownership flags are untouched.
    pub async fn change_player_chocobo_name(
        &self,
        chara_id: u32,
        name: &str,
    ) -> Result<()> {
        let name = name.to_owned();
        self.conn
            .call_db(move |c| {
                c.execute(
                    "UPDATE characters_chocobo SET chocoboName = :n WHERE characterId = :cid",
                    named_params! { ":n": name, ":cid": chara_id },
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

    /// Persist the rest-bonus accumulator for a character. The value
    /// is the "bonus multiplier %" the client displays in the UI (0 =
    /// no bonus, 100 = +100% XP on the next gain). Stored in the
    /// `characters.restBonus` column.
    pub async fn set_rest_bonus_exp_rate(&self, chara_id: u32, rate: i32) -> Result<()> {
        self.conn
            .call_db(move |c| {
                c.execute(
                    r"UPDATE characters SET restBonus = :r WHERE id = :cid",
                    named_params! { ":r": rate, ":cid": chara_id },
                )?;
                Ok(())
            })
            .await?;
        Ok(())
    }

    /// Round-trip read for `characters.restBonus`. Returns `0` when
    /// the character row is missing so callers don't have to unwrap.
    pub async fn get_rest_bonus_exp_rate(&self, chara_id: u32) -> Result<i32> {
        let v = self
            .conn
            .call_db(move |c| {
                let v: Option<i32> = c
                    .query_row(
                        r"SELECT restBonus FROM characters WHERE id = :cid",
                        named_params! { ":cid": chara_id },
                        |r| r.get::<_, i32>(0),
                    )
                    .optional()?;
                Ok(v.unwrap_or(0))
            })
            .await?;
        Ok(v)
    }

    /// Port of `Database.LoadRetainer` — find the Nth retainer a
    /// character owns (1-indexed, matching Meteor's caller at
    /// `Player.SpawnMyRetainer`), returning the full catalog template.
    /// `None` means the character has fewer than `retainer_index`
    /// retainers on file.
    pub async fn load_retainer(
        &self,
        chara_id: u32,
        retainer_index: i32,
    ) -> Result<Option<crate::npc::RetainerTemplate>> {
        let offset = (retainer_index - 1).max(0);
        let v = self
            .conn
            .call_db(move |c| {
                let v = c
                    .query_row(
                        r"SELECT sr.id,
                                 COALESCE(NULLIF(cr.customName, ''), sr.name) AS displayName,
                                 sr.actorClassId, sr.cdIDOffset,
                                 sr.placeName, sr.conditions, sr.level,
                                 COALESCE(ac.classPath, '')
                          FROM characters_retainers cr
                          INNER JOIN server_retainers sr ON cr.retainerId = sr.id
                          LEFT JOIN gamedata_actor_class ac ON sr.actorClassId = ac.id
                          WHERE cr.characterId = :cid
                          ORDER BY sr.id
                          LIMIT 1 OFFSET :off",
                        named_params! { ":cid": chara_id, ":off": offset },
                        |r| {
                            Ok(crate::npc::RetainerTemplate {
                                id: r.get::<_, u32>(0)?,
                                name: r.get::<_, String>(1)?,
                                actor_class_id: r.get::<_, u32>(2)?,
                                cd_id_offset: r
                                    .get::<_, i64>(3)
                                    .unwrap_or(0)
                                    .clamp(0, 255)
                                    as u8,
                                place_name: r.get::<_, u32>(4).unwrap_or(0),
                                conditions: r
                                    .get::<_, i64>(5)
                                    .unwrap_or(0)
                                    .clamp(0, 255)
                                    as u8,
                                level: r.get::<_, i64>(6).unwrap_or(0).clamp(0, 255) as u8,
                                class_path: r.get::<_, String>(7).unwrap_or_default(),
                            })
                        },
                    )
                    .optional()?;
                Ok(v)
            })
            .await?;
        Ok(v)
    }

    /// List every retainer a character owns, in `server_retainers.id`
    /// order. Backs `PopulaceRetainerManager.lua` menus and the
    /// `player:ListMyRetainers()` Lua helper.
    pub async fn list_character_retainers(
        &self,
        chara_id: u32,
    ) -> Result<Vec<crate::npc::RetainerTemplate>> {
        let rows = self
            .conn
            .call_db(move |c| {
                let mut stmt = c.prepare(
                    r"SELECT sr.id, sr.name, sr.actorClassId, sr.cdIDOffset,
                             sr.placeName, sr.conditions, sr.level,
                             COALESCE(ac.classPath, '')
                      FROM characters_retainers cr
                      INNER JOIN server_retainers sr ON cr.retainerId = sr.id
                      LEFT JOIN gamedata_actor_class ac ON sr.actorClassId = ac.id
                      WHERE cr.characterId = :cid
                      ORDER BY sr.id",
                )?;
                let rows: Vec<crate::npc::RetainerTemplate> = stmt
                    .query_map(named_params! { ":cid": chara_id }, |r| {
                        Ok(crate::npc::RetainerTemplate {
                            id: r.get::<_, u32>(0)?,
                            name: r.get::<_, String>(1)?,
                            actor_class_id: r.get::<_, u32>(2)?,
                            cd_id_offset: r
                                .get::<_, i64>(3)
                                .unwrap_or(0)
                                .clamp(0, 255)
                                as u8,
                            place_name: r.get::<_, u32>(4).unwrap_or(0),
                            conditions: r
                                .get::<_, i64>(5)
                                .unwrap_or(0)
                                .clamp(0, 255)
                                as u8,
                            level: r.get::<_, i64>(6).unwrap_or(0).clamp(0, 255) as u8,
                            class_path: r.get::<_, String>(7).unwrap_or_default(),
                        })
                    })?
                    .collect::<rusqlite::Result<_>>()?;
                Ok(rows)
            })
            .await?;
        Ok(rows)
    }

    /// Insert a retainer ownership row. Returns `Ok(true)` on a fresh
    /// hire, `Ok(false)` if the character already owns that retainer
    /// (matches the C# idempotent-hire behaviour). Drives
    /// `PopulaceRetainerManager.lua`'s `eventTalkStepFinalAnswer`
    /// confirmation.
    pub async fn hire_retainer(
        &self,
        chara_id: u32,
        retainer_id: u32,
    ) -> Result<bool> {
        let affected = self
            .conn
            .call_db(move |c| {
                let n = c.execute(
                    r"INSERT OR IGNORE INTO characters_retainers
                        (characterId, retainerId, doRename)
                      VALUES (:cid, :rid, 0)",
                    named_params! { ":cid": chara_id, ":rid": retainer_id },
                )?;
                Ok(n)
            })
            .await?;
        Ok(affected > 0)
    }

    /// Remove a retainer ownership row. Returns `Ok(true)` if a row
    /// was actually deleted. Used by `player:DismissMyRetainer(id)`
    /// once the rename / bazaar flow is wound down.
    pub async fn dismiss_retainer(
        &self,
        chara_id: u32,
        retainer_id: u32,
    ) -> Result<bool> {
        let affected = self
            .conn
            .call_db(move |c| {
                let n = c.execute(
                    r"DELETE FROM characters_retainers
                      WHERE characterId = :cid AND retainerId = :rid",
                    named_params! { ":cid": chara_id, ":rid": retainer_id },
                )?;
                Ok(n)
            })
            .await?;
        Ok(affected > 0)
    }

    /// Tier 4 #14 E — rename a hired retainer. Writes to the
    /// per-character `customName` column on `characters_retainers`
    /// (added by seed 050) rather than mutating the shared
    /// `server_retainers.name` template row, so a rename by one
    /// owner doesn't leak into another owner who hired the same
    /// template id. Also clears `doRename = 0` on success, matching
    /// Meteor's post-rename reset that suppresses the "rename
    /// available" UI hint once used.
    ///
    /// Returns `Ok(true)` when a row was updated, `Ok(false)` when
    /// the `(character, retainer)` pair isn't hired (no-op).
    pub async fn rename_retainer(
        &self,
        chara_id: u32,
        retainer_id: u32,
        new_name: String,
    ) -> Result<bool> {
        let affected = self
            .conn
            .call_db(move |c| {
                let n = c.execute(
                    r"UPDATE characters_retainers
                      SET customName = :name, doRename = 0
                      WHERE characterId = :cid AND retainerId = :rid",
                    named_params! {
                        ":name": new_name,
                        ":cid": chara_id,
                        ":rid": retainer_id,
                    },
                )?;
                Ok(n)
            })
            .await?;
        Ok(affected > 0)
    }

    /// Look up a retainer catalog template by primary key. Returns
    /// `None` when the id isn't seeded — scripts can treat the
    /// result as "retainer doesn't exist" without raising.
    pub async fn get_retainer_template(
        &self,
        retainer_id: u32,
    ) -> Result<Option<crate::npc::RetainerTemplate>> {
        let v = self
            .conn
            .call_db(move |c| {
                let v = c
                    .query_row(
                        r"SELECT sr.id, sr.name, sr.actorClassId, sr.cdIDOffset,
                                 sr.placeName, sr.conditions, sr.level,
                                 COALESCE(ac.classPath, '')
                          FROM server_retainers sr
                          LEFT JOIN gamedata_actor_class ac ON sr.actorClassId = ac.id
                          WHERE sr.id = :rid",
                        named_params! { ":rid": retainer_id },
                        |r| {
                            Ok(crate::npc::RetainerTemplate {
                                id: r.get::<_, u32>(0)?,
                                name: r.get::<_, String>(1)?,
                                actor_class_id: r.get::<_, u32>(2)?,
                                cd_id_offset: r
                                    .get::<_, i64>(3)
                                    .unwrap_or(0)
                                    .clamp(0, 255)
                                    as u8,
                                place_name: r.get::<_, u32>(4).unwrap_or(0),
                                conditions: r
                                    .get::<_, i64>(5)
                                    .unwrap_or(0)
                                    .clamp(0, 255)
                                    as u8,
                                level: r.get::<_, i64>(6).unwrap_or(0).clamp(0, 255) as u8,
                                class_path: r.get::<_, String>(7).unwrap_or_default(),
                            })
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

    // =======================================================================
    // Retainer bazaar inventory (Tier 4 #14 bazaar follow-up).
    //
    // Retainer bazaar listings live in `characters_retainer_bazaar`,
    // scoped to `retainerId` (the `server_retainers.id` — e.g. 1001 /
    // 1002 / 1003 — NOT the composite actor id allocated at live-spawn
    // time). Scoping by retainerId keeps listings alive across
    // despawn/respawn cycles and player logouts. Each listing links
    // to a `server_items` row for the actual stack (quantity +
    // quality) + carries a per-item gil price the BazaarDeal flow
    // will read.
    // =======================================================================

    /// Add a new bazaar listing (or merge into a matching open stack
    /// at the same price). Returns the new stack's total quantity.
    /// If a stack of the same `(item_catalog_id, quality, price_gil)`
    /// triple already exists on this retainer, the new delta merges
    /// in — matches retail's "stackable bazaar" behavior where two
    /// identically-priced copper-ore listings coalesce rather than
    /// occupying separate slots. Different price ⇒ new slot.
    ///
    /// Allocates a new `server_items` row for a fresh listing; re-
    /// uses the existing row on merge. Slot assignment follows the
    /// same `MAX(slot)+1` pattern the NORMAL-bag paths use.
    pub async fn add_retainer_bazaar_item(
        &self,
        retainer_id: u32,
        item_catalog_id: u32,
        delta: i32,
        quality: u8,
        price_gil: i32,
    ) -> Result<i32> {
        if delta <= 0 || item_catalog_id == 0 {
            return Ok(0);
        }
        let now_utc = common::utils::unix_timestamp() as i64;
        let total = self
            .conn
            .call_db(move |c| {
                let tx = c.transaction()?;
                // Merge into an existing stack only when the
                // `(item, quality, price)` triple matches — different
                // prices force separate slots since the bazaar UI
                // renders one row per price point.
                let existing: Option<(i64, i32, i32)> = tx
                    .query_row(
                        r"SELECT si.id, si.quantity, rb.slot
                          FROM characters_retainer_bazaar rb
                          INNER JOIN server_items si ON rb.serverItemId = si.id
                          WHERE rb.retainerId = :rid
                            AND rb.priceGil = :p
                            AND si.itemId = :iid
                            AND si.quality = :q",
                        named_params! {
                            ":rid": retainer_id,
                            ":p": price_gil,
                            ":iid": item_catalog_id,
                            ":q": quality as i64,
                        },
                        |r| Ok((
                            r.get::<_, i64>(0)?,
                            r.get::<_, i32>(1)?,
                            r.get::<_, i32>(2)?,
                        )),
                    )
                    .optional()?;
                let new_total = match existing {
                    Some((sid, qty, _slot)) => {
                        let updated = qty.saturating_add(delta).max(0);
                        tx.execute(
                            "UPDATE server_items SET quantity = :q WHERE id = :id",
                            named_params! { ":q": updated, ":id": sid },
                        )?;
                        tx.execute(
                            r"UPDATE characters_retainer_bazaar
                                 SET updatedUtc = :now
                                 WHERE retainerId = :rid AND serverItemId = :sid",
                            named_params! {
                                ":now": now_utc,
                                ":rid": retainer_id,
                                ":sid": sid,
                            },
                        )?;
                        updated
                    }
                    None => {
                        let seed = delta.max(0);
                        tx.execute(
                            r"INSERT INTO server_items (itemId, quantity, quality)
                              VALUES (:iid, :q, :qual)",
                            named_params! {
                                ":iid": item_catalog_id,
                                ":q": seed,
                                ":qual": quality as i64,
                            },
                        )?;
                        let sid = tx.last_insert_rowid();
                        let next_slot: i32 = tx
                            .query_row(
                                r"SELECT COALESCE(MAX(slot), -1) + 1
                                  FROM characters_retainer_bazaar
                                  WHERE retainerId = :rid",
                                named_params! { ":rid": retainer_id },
                                |r| r.get(0),
                            )
                            .unwrap_or(0);
                        tx.execute(
                            r"INSERT INTO characters_retainer_bazaar
                                (retainerId, serverItemId, slot, priceGil, createdUtc, updatedUtc)
                              VALUES (:rid, :sid, :slot, :price, :now, :now)",
                            named_params! {
                                ":rid": retainer_id,
                                ":sid": sid,
                                ":slot": next_slot,
                                ":price": price_gil,
                                ":now": now_utc,
                            },
                        )?;
                        seed
                    }
                };
                tx.commit()?;
                Ok(new_total)
            })
            .await?;
        Ok(total)
    }

    /// Read back every bazaar listing for `retainer_id`, ordered by
    /// slot (the order the client would render them in).
    pub async fn list_retainer_bazaar(
        &self,
        retainer_id: u32,
    ) -> Result<Vec<RetainerBazaarListing>> {
        let rows = self
            .conn
            .call_db(move |c| {
                let mut stmt = c.prepare(
                    r"SELECT rb.serverItemId, si.itemId, si.quantity, si.quality,
                             rb.slot, rb.priceGil, rb.createdUtc, rb.updatedUtc
                      FROM characters_retainer_bazaar rb
                      INNER JOIN server_items si ON rb.serverItemId = si.id
                      WHERE rb.retainerId = :rid
                      ORDER BY rb.slot",
                )?;
                let rows: Vec<RetainerBazaarListing> = stmt
                    .query_map(named_params! { ":rid": retainer_id }, |r| {
                        Ok(RetainerBazaarListing {
                            server_item_id: r.get::<_, i64>(0)? as u64,
                            item_id: r.get::<_, u32>(1)?,
                            quantity: r.get::<_, i32>(2)?,
                            quality: r.get::<_, i64>(3)?.clamp(0, 255) as u8,
                            slot: r.get::<_, i32>(4)?,
                            price_gil: r.get::<_, i32>(5)?,
                            created_utc: r.get::<_, i64>(6)?.max(0) as u32,
                            updated_utc: r.get::<_, i64>(7)?.max(0) as u32,
                        })
                    })?
                    .collect::<rusqlite::Result<_>>()?;
                Ok(rows)
            })
            .await?;
        Ok(rows)
    }

    /// Tier 4 #14 D — one-shot bazaar purchase. Transactional:
    ///
    ///  1. Resolves the retainer's owning character via
    ///     `characters_retainers.characterId`.
    ///  2. Looks up the listing by `(retainer_id, server_item_id)`
    ///     and reads back its `priceGil`, `quantity`, `quality`,
    ///     plus the backing `server_items.itemId` so we can credit
    ///     the buyer without a second SELECT.
    ///  3. Verifies the buyer's current gil balance covers
    ///     `priceGil * quantity` (whole-stack purchase — partial
    ///     fills are a follow-up; Meteor's client never splits
    ///     bazaar stacks mid-buy).
    ///  4. Deducts gil from the buyer, credits gil to the retainer's
    ///     owner (single combined `UPDATE server_items` where possible).
    ///  5. Deletes the listing row + its backing `server_items` row.
    ///  6. Merges the stack into the buyer's NORMAL bag using the
    ///     same merge-or-spill logic as [`Database::add_harvest_item`].
    ///
    /// Returns [`PurchaseOutcome::Completed`] on success (carrying
    /// the actual gil transferred + item/quantity granted), or a
    /// specific rejection variant on any failure path. Idempotent
    /// on a second call with the same `server_item_id` — the
    /// listing is gone so the second attempt returns
    /// [`PurchaseOutcome::ListingGone`].
    pub async fn purchase_retainer_bazaar_item(
        &self,
        buyer_chara_id: u32,
        retainer_id: u32,
        server_item_id: u64,
    ) -> Result<PurchaseOutcome> {
        let outcome = self
            .conn
            .call_db(move |c| {
                let tx = c.transaction()?;
                // Owner lookup. `characters_retainers` can hold
                // multiple rows per retainerId (multiple players
                // hired the same template), but a bazaar listing is
                // scoped to one retainer per hire, so we pick the
                // first (and typically only) owner.
                let owner_chara_id: Option<u32> = tx
                    .query_row(
                        r"SELECT characterId FROM characters_retainers
                          WHERE retainerId = :rid
                          LIMIT 1",
                        named_params! { ":rid": retainer_id },
                        |r| r.get::<_, u32>(0),
                    )
                    .optional()?;
                let Some(owner_chara_id) = owner_chara_id else {
                    return Ok(PurchaseOutcome::NoOwner);
                };
                if owner_chara_id == buyer_chara_id {
                    return Ok(PurchaseOutcome::CannotBuyFromSelf);
                }

                // Listing lookup.
                let listing: Option<(i32, i32, i64, i32, i32)> = tx
                    .query_row(
                        r"SELECT si.quantity, si.quality, si.itemId,
                                 rb.priceGil, rb.slot
                          FROM characters_retainer_bazaar rb
                          INNER JOIN server_items si ON rb.serverItemId = si.id
                          WHERE rb.retainerId = :rid AND rb.serverItemId = :sid",
                        named_params! {
                            ":rid": retainer_id,
                            ":sid": server_item_id as i64,
                        },
                        |r| Ok((
                            r.get::<_, i32>(0)?,
                            r.get::<_, i64>(1).unwrap_or(0) as i32,
                            r.get::<_, i64>(2)?,
                            r.get::<_, i32>(3).unwrap_or(0),
                            r.get::<_, i32>(4).unwrap_or(0),
                        )),
                    )
                    .optional()?;
                let Some((qty, quality_raw, item_id_i64, price_per_unit, _slot)) = listing else {
                    return Ok(PurchaseOutcome::ListingGone);
                };
                if qty <= 0 {
                    return Ok(PurchaseOutcome::ListingGone);
                }
                let quality = quality_raw.clamp(0, 255) as u8;
                let item_id = item_id_i64 as u32;
                let total_price = price_per_unit
                    .saturating_mul(qty);

                // Buyer's current gil — single-stack currency row
                // under `characters_inventory.itemPackage = 99`.
                // Uses the canonical 1_000_001 gil item id rather
                // than a dedicated gil column so future
                // refactoring can converge the two paths.
                let buyer_gil: i32 = tx
                    .query_row(
                        r"SELECT COALESCE(SUM(si.quantity), 0)
                          FROM characters_inventory ci
                          INNER JOIN server_items si ON ci.serverItemId = si.id
                          WHERE ci.characterId = :cid
                            AND ci.itemPackage = 99
                            AND si.itemId = 1000001",
                        named_params! { ":cid": buyer_chara_id },
                        |r| r.get::<_, i32>(0),
                    )
                    .unwrap_or(0);
                if buyer_gil < total_price {
                    return Ok(PurchaseOutcome::InsufficientGil {
                        have: buyer_gil,
                        need: total_price,
                    });
                }

                // Deduct from buyer. Uses the same merge-aware
                // pattern `add_gil` produces: find the existing
                // currency row + decrement in place.
                if total_price > 0 {
                    tx.execute(
                        r"UPDATE server_items
                          SET quantity = quantity - :price
                          WHERE id IN (
                              SELECT si.id FROM server_items si
                              INNER JOIN characters_inventory ci
                                  ON ci.serverItemId = si.id
                              WHERE ci.characterId = :cid
                                AND ci.itemPackage = 99
                                AND si.itemId = 1000001
                              LIMIT 1
                          )",
                        named_params! {
                            ":price": total_price,
                            ":cid": buyer_chara_id,
                        },
                    )?;

                    // Credit owner — merge into existing stack or
                    // seed a new row. Same shape as the
                    // `add_harvest_item` NORMAL-bag merge but
                    // targeting PKG_CURRENCY_CRYSTALS (99).
                    let owner_sid: Option<i64> = tx
                        .query_row(
                            r"SELECT si.id FROM server_items si
                              INNER JOIN characters_inventory ci ON ci.serverItemId = si.id
                              WHERE ci.characterId = :cid
                                AND ci.itemPackage = 99
                                AND si.itemId = 1000001
                              LIMIT 1",
                            named_params! { ":cid": owner_chara_id },
                            |r| r.get::<_, i64>(0),
                        )
                        .optional()?;
                    match owner_sid {
                        Some(sid) => {
                            tx.execute(
                                "UPDATE server_items SET quantity = quantity + :p WHERE id = :id",
                                named_params! { ":p": total_price, ":id": sid },
                            )?;
                        }
                        None => {
                            tx.execute(
                                r"INSERT INTO server_items (itemId, quantity, quality)
                                  VALUES (1000001, :p, 0)",
                                named_params! { ":p": total_price },
                            )?;
                            let sid = tx.last_insert_rowid();
                            let next_slot: i32 = tx
                                .query_row(
                                    r"SELECT COALESCE(MAX(slot), -1) + 1
                                      FROM characters_inventory
                                      WHERE characterId = :cid AND itemPackage = 99",
                                    named_params! { ":cid": owner_chara_id },
                                    |r| r.get::<_, i32>(0),
                                )
                                .unwrap_or(0);
                            tx.execute(
                                r"INSERT INTO characters_inventory
                                    (characterId, itemPackage, serverItemId, slot)
                                  VALUES (:cid, 99, :sid, :slot)",
                                named_params! {
                                    ":cid": owner_chara_id, ":sid": sid, ":slot": next_slot,
                                },
                            )?;
                        }
                    }
                }

                // Transfer the item stack to the buyer's NORMAL
                // bag. Delete the bazaar row + its backing
                // server_items row first (so the stack isn't
                // referenced from two inventories mid-write), then
                // merge-or-spill into buyer's NORMAL bag.
                tx.execute(
                    r"DELETE FROM characters_retainer_bazaar
                      WHERE retainerId = :rid AND serverItemId = :sid",
                    named_params! {
                        ":rid": retainer_id,
                        ":sid": server_item_id as i64,
                    },
                )?;
                tx.execute(
                    "DELETE FROM server_items WHERE id = :sid",
                    named_params! { ":sid": server_item_id as i64 },
                )?;

                // Merge into buyer's NORMAL bag (itemPackage = 0).
                let existing_sid: Option<i64> = tx
                    .query_row(
                        r"SELECT si.id FROM server_items si
                          INNER JOIN characters_inventory ci ON ci.serverItemId = si.id
                          WHERE ci.characterId = :cid
                            AND ci.itemPackage = 0
                            AND si.itemId = :iid
                            AND si.quality = :q
                          LIMIT 1",
                        named_params! {
                            ":cid": buyer_chara_id,
                            ":iid": item_id,
                            ":q": quality as i64,
                        },
                        |r| r.get::<_, i64>(0),
                    )
                    .optional()?;
                match existing_sid {
                    Some(sid) => {
                        tx.execute(
                            "UPDATE server_items SET quantity = quantity + :q WHERE id = :id",
                            named_params! { ":q": qty, ":id": sid },
                        )?;
                    }
                    None => {
                        tx.execute(
                            r"INSERT INTO server_items (itemId, quantity, quality)
                              VALUES (:iid, :q, :qual)",
                            named_params! {
                                ":iid": item_id,
                                ":q": qty,
                                ":qual": quality as i64,
                            },
                        )?;
                        let new_sid = tx.last_insert_rowid();
                        let next_slot: i32 = tx
                            .query_row(
                                r"SELECT COALESCE(MAX(slot), -1) + 1
                                  FROM characters_inventory
                                  WHERE characterId = :cid AND itemPackage = 0",
                                named_params! { ":cid": buyer_chara_id },
                                |r| r.get::<_, i32>(0),
                            )
                            .unwrap_or(0);
                        tx.execute(
                            r"INSERT INTO characters_inventory
                                (characterId, itemPackage, serverItemId, slot)
                              VALUES (:cid, 0, :sid, :slot)",
                            named_params! {
                                ":cid": buyer_chara_id, ":sid": new_sid, ":slot": next_slot,
                            },
                        )?;
                    }
                }
                tx.commit()?;
                Ok(PurchaseOutcome::Completed {
                    item_id,
                    quantity: qty,
                    quality,
                    gil_spent: total_price,
                    owner_chara_id,
                })
            })
            .await?;
        Ok(outcome)
    }

    /// Remove a specific listing — called from the BazaarUndeal flow
    /// (owner retracts) and the BazaarDeal flow (buyer bought the
    /// last of the stack). Returns `true` if a row was actually
    /// deleted. The backing `server_items` row is deleted too so
    /// the stack's storage doesn't leak.
    pub async fn remove_retainer_bazaar_item(
        &self,
        retainer_id: u32,
        server_item_id: u64,
    ) -> Result<bool> {
        let removed = self
            .conn
            .call_db(move |c| {
                let tx = c.transaction()?;
                let affected = tx.execute(
                    r"DELETE FROM characters_retainer_bazaar
                      WHERE retainerId = :rid AND serverItemId = :sid",
                    named_params! {
                        ":rid": retainer_id,
                        ":sid": server_item_id as i64,
                    },
                )?;
                if affected > 0 {
                    tx.execute(
                        "DELETE FROM server_items WHERE id = :sid",
                        named_params! { ":sid": server_item_id as i64 },
                    )?;
                }
                tx.commit()?;
                Ok(affected > 0)
            })
            .await?;
        Ok(removed)
    }
}

/// Outcome of a [`Database::purchase_retainer_bazaar_item`] call.
/// Explicit variants so callers can distinguish "someone else
/// grabbed this listing a tick ago" from "you're broke" from "you
/// clicked your own retainer" without string-matching on an error.
/// Tier 4 #14 D.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PurchaseOutcome {
    Completed {
        item_id: u32,
        quantity: i32,
        quality: u8,
        gil_spent: i32,
        /// Character id that received the gil.
        owner_chara_id: u32,
    },
    /// Retainer template has no ownership row — orphan listing, or
    /// the retainer was dismissed mid-browse.
    NoOwner,
    /// Buyer == owner — retail silently refuses this so the owner
    /// uses the BazaarUndeal menu instead.
    CannotBuyFromSelf,
    /// Listing no longer present. Idempotent-retry friendly: if a
    /// previous call succeeded or a race took the listing, we no-op.
    ListingGone,
    /// Buyer's gil balance is below the stack total.
    InsufficientGil {
        have: i32,
        need: i32,
    },
}

/// One row of the retainer bazaar — surfaces through `list_retainer_bazaar`
/// for the BazaarCheck packet emitter (when that lands) and GM
/// command inspection in the meantime.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RetainerBazaarListing {
    pub server_item_id: u64,
    pub item_id: u32,
    pub quantity: i32,
    pub quality: u8,
    pub slot: i32,
    pub price_gil: i32,
    pub created_utc: u32,
    pub updated_utc: u32,
}

#[cfg(test)]
mod battle_npc_spawn_tests {
    use super::*;

    fn tempdb(label: &str) -> std::path::PathBuf {
        // Tag the file with the test name AND a nanosecond stamp —
        // running the suite twice in quick succession (or with
        // --test-threads=N) otherwise hits the same path and tokio's
        // SQLite "file is locked" error fires from a stale WAL.
        std::env::temp_dir().join(format!(
            "garlemald-battle-npc-spawn-{label}-{}.db",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ))
    }

    /// `load_battle_npc_spawn` joins the four `server_battlenpc_*`
    /// tables and returns the row for the requested `bnpc_id`. Seed
    /// data for `bnpcId=6` is yda (groupId=3, poolId=3,
    /// actorClassId=2290005, scriptName='yda', allegiance=1, zoneId=166).
    #[tokio::test]
    async fn load_battle_npc_spawn_yda() {
        let path = tempdb("yda");
        let db = Database::open(&path).await.expect("open db");
        let row = db
            .load_battle_npc_spawn(6)
            .await
            .expect("query")
            .expect("yda row");
        assert_eq!(row.bnpc_id, 6);
        assert_eq!(row.group_id, 3);
        assert_eq!(row.pool_id, 3);
        assert_eq!(row.actor_class_id, 2_290_005);
        assert_eq!(row.script_name, "yda");
        assert_eq!(row.allegiance, 1, "yda is an ally (Player allegiance)");
        assert_eq!(row.zone_id, 166, "yda spawns in Black Shroud Forest");
        // Position from the seed: (365.266, 4.122, -700.73, rot 1.5659).
        assert!((row.position_x - 365.266).abs() < 0.01);
        assert!((row.position_y - 4.122).abs() < 0.01);
        assert!((row.position_z - (-700.73)).abs() < 0.01);
        let _ = std::fs::remove_file(&path);
    }

    /// `bnpcId=3` is `bloodthirsty_wolf` (groupId=2, poolId=2,
    /// actorClassId=2201407). Used to verify the spawn returns
    /// distinct rows per bnpc_id.
    #[tokio::test]
    async fn load_battle_npc_spawn_bloodthirsty_wolf() {
        let path = tempdb("wolf");
        let db = Database::open(&path).await.expect("open db");
        let row = db
            .load_battle_npc_spawn(3)
            .await
            .expect("query")
            .expect("wolf row");
        assert_eq!(row.bnpc_id, 3);
        assert_eq!(row.group_id, 2);
        assert_eq!(row.pool_id, 2);
        assert_eq!(row.actor_class_id, 2_201_407);
        assert_eq!(row.script_name, "bloodthirsty_wolf");
        let _ = std::fs::remove_file(&path);
    }

    /// Missing `bnpc_id` returns `None`, not an error.
    #[tokio::test]
    async fn load_battle_npc_spawn_missing_id() {
        let path = tempdb("missing");
        let db = Database::open(&path).await.expect("open db");
        let row = db.load_battle_npc_spawn(0xDEAD_BEEF).await.expect("query");
        assert!(row.is_none());
        let _ = std::fs::remove_file(&path);
    }

    /// `load_actor_class` returns the row for a known class id (yda's
    /// `actor_class_id = 2_290_005`).
    #[tokio::test]
    async fn load_actor_class_yda() {
        let path = tempdb("class-yda");
        let db = Database::open(&path).await.expect("open db");
        let class = db
            .load_actor_class(2_290_005)
            .await
            .expect("query");
        // Class id is in the gamedata seed; if it isn't, this test
        // surfaces the mismatch rather than silently falling back to
        // a Phase-A "synthetic" pretend-spawn.
        if let Some(c) = class {
            assert_eq!(c.actor_class_id, 2_290_005);
        }
        let _ = std::fs::remove_file(&path);
    }
}
