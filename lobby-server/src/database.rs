//! SQLite-backed lobby database queries. Ported from Lobby Server/Database.cs
//! via `tokio_rusqlite` for async, background-thread access against a single
//! on-disk SQLite file (see `common::db::open_or_create`).

use std::path::Path;

use anyhow::{Context, Result};
use rusqlite::{OptionalExtension, named_params};
use common::db::ConnCallExt;
use tokio_rusqlite::Connection;

use crate::character_creator::class_name_for_id;
use crate::data::{Appearance, CharaInfo, Character, Retainer, World};

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

    pub async fn user_id_from_session(&self, session_id: &str) -> Result<u32> {
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

    /// Attempts to reserve a character slot. Returns `(already_taken, pid, cid)`.
    /// `pid` is the magic `0xBABE` marker from the C# original, or 0 if the
    /// name is already taken.
    pub async fn reserve_character(
        &self,
        user_id: u32,
        slot: u32,
        server_id: u32,
        name: &str,
    ) -> Result<(bool, u32, u32)> {
        let name = name.to_owned();
        let out = self.conn
            .call_db(move |c| {
                let existing: Option<u32> = c
                    .query_row(
                        r"SELECT id FROM characters
                          WHERE name = :name AND serverId = :sid
                            AND state != 2 AND state != 1",
                        named_params! { ":name": name, ":sid": server_id },
                        |r| r.get(0),
                    )
                    .optional()?;
                if existing.is_some() {
                    return Ok((true, 0u32, 0u32));
                }
                c.execute(
                    r"INSERT INTO characters(userId, slot, serverId, name, state)
                      VALUES(:uid, :slot, :sid, :name, 0)",
                    named_params! {
                        ":uid": user_id,
                        ":slot": slot,
                        ":sid": server_id,
                        ":name": name,
                    },
                )?;
                let cid = c.last_insert_rowid() as u32;
                Ok((false, 0xBABEu32, cid))
            })
            .await?;
        Ok(out)
    }

    pub async fn make_character(&self, user_id: u32, cid: u32, info: &CharaInfo) -> Result<()> {
        let info = info.clone();
        let class_col = class_name_for_id(info.current_class as i16).to_string();
        self.conn
            .call_db(move |c| {
                c.execute(
                    r"UPDATE characters SET
                          state = 2,
                          currentZoneId = :zoneId,
                          positionX = :x, positionY = :y, positionZ = :z,
                          rotation = :r,
                          guardian = :guardian,
                          birthDay = :birthDay, birthMonth = :birthMonth,
                          initialTown = :initialTown,
                          tribe = :tribe
                      WHERE userId = :userId AND id = :cid",
                    named_params! {
                        ":zoneId": info.zone_id,
                        ":x": info.x, ":y": info.y, ":z": info.z,
                        ":r": info.rot,
                        ":guardian": info.guardian,
                        ":birthDay": info.birth_day,
                        ":birthMonth": info.birth_month,
                        ":initialTown": info.initial_town,
                        ":tribe": info.tribe,
                        ":userId": user_id,
                        ":cid": cid,
                    },
                )?;

                c.execute(
                    r"INSERT INTO characters_appearance
                      (characterId, baseId, size, voice, skinColor, hairStyle, hairColor,
                       hairHighlightColor, hairVariation, eyeColor, faceType, faceEyebrows,
                       faceEyeShape, faceIrisSize, faceNose, faceMouth, faceFeatures, ears,
                       characteristics, characteristicsColor, mainhand, offhand, head,
                       body, hands, legs, feet, waist)
                      VALUES
                      (:cid, 4294967295, :size, :voice, :skinColor, :hairStyle, :hairColor,
                       :hairHighlightColor, :hairVariation, :eyeColor, :faceType, :faceEyebrows,
                       :faceEyeShape, :faceIrisSize, :faceNose, :faceMouth, :faceFeatures, :ears,
                       :characteristics, :characteristicsColor, :mainhand, :offhand, :head,
                       :body, :hands, :legs, :feet, :waist)",
                    named_params! {
                        ":cid": cid,
                        ":size": info.appearance.size,
                        ":voice": info.appearance.voice,
                        ":skinColor": info.appearance.skin_color,
                        ":hairStyle": info.appearance.hair_style,
                        ":hairColor": info.appearance.hair_color,
                        ":hairHighlightColor": info.appearance.hair_highlight_color,
                        ":hairVariation": info.appearance.hair_variation,
                        ":eyeColor": info.appearance.eye_color,
                        ":faceType": info.appearance.face_type,
                        ":faceEyebrows": info.appearance.face_eyebrows,
                        ":faceEyeShape": info.appearance.face_eye_shape,
                        ":faceIrisSize": info.appearance.face_iris_size,
                        ":faceNose": info.appearance.face_nose,
                        ":faceMouth": info.appearance.face_mouth,
                        ":faceFeatures": info.appearance.face_features,
                        ":ears": info.appearance.ears,
                        ":characteristics": info.appearance.characteristics,
                        ":characteristicsColor": info.appearance.characteristics_color,
                        ":mainhand": info.weapon1,
                        ":offhand": info.weapon2,
                        ":head": info.head,
                        ":body": info.body,
                        ":hands": info.hands,
                        ":legs": info.legs,
                        ":feet": info.feet,
                        ":waist": info.belt,
                    },
                )?;

                // characters_class_levels has no auto-increment PK — one row
                // per character keyed on characterId, with 18 class columns
                // defaulting to 0. We insert a default row then bump only the
                // selected class, which keeps the dynamic column name out of
                // the INSERT (SQLite can't parameterise identifiers).
                c.execute(
                    "INSERT OR IGNORE INTO characters_class_levels(characterId) VALUES(:cid)",
                    named_params! { ":cid": cid },
                )?;
                let levels_sql =
                    format!("UPDATE characters_class_levels SET {class_col} = 1 WHERE characterId = :cid");
                c.execute(&levels_sql, named_params! { ":cid": cid })?;
                c.execute(
                    "INSERT OR IGNORE INTO characters_class_exp(characterId) VALUES(:cid)",
                    named_params! { ":cid": cid },
                )?;

                c.execute(
                    r"INSERT INTO characters_parametersave
                      (characterId, hp, hpMax, mp, mpMax, mainSkill, mainSkillLevel)
                      VALUES(:cid, 1900, 1000, 115, 115, :mainSkill, 1)",
                    named_params! { ":cid": cid, ":mainSkill": info.current_class },
                )?;

                let mut stmt = c.prepare(
                    r"SELECT id FROM server_battle_commands
                      WHERE classJob = :cls AND lvl = 1 ORDER BY id DESC",
                )?;
                let default_actions: Vec<u32> = stmt
                    .query_map(
                        named_params! { ":cls": info.current_class },
                        |r| r.get::<_, u32>(0),
                    )?
                    .collect::<rusqlite::Result<_>>()?;
                drop(stmt);

                for (slot, command_id) in default_actions.iter().enumerate() {
                    c.execute(
                        r"INSERT INTO characters_hotbar(characterId, classId, hotbarSlot, commandId, recastTime)
                          VALUES(:cid, :classId, :slot, :cmd, 0)",
                        named_params! {
                            ":cid": cid,
                            ":classId": info.current_class,
                            ":slot": slot as i64,
                            ":cmd": *command_id as i64,
                        },
                    )?;
                }

                Ok(())
            })
            .await?;
        Ok(())
    }

    pub async fn rename_character(
        &self,
        user_id: u32,
        character_id: u32,
        server_id: u32,
        new_name: &str,
    ) -> Result<bool> {
        let new_name = new_name.to_owned();
        let taken = self.conn
            .call_db(move |c| {
                let existing: Option<u32> = c
                    .query_row(
                        "SELECT id FROM characters WHERE name = :name AND serverId = :sid",
                        named_params! { ":name": new_name, ":sid": server_id },
                        |r| r.get(0),
                    )
                    .optional()?;
                if existing.is_some() {
                    return Ok(true);
                }
                c.execute(
                    r"UPDATE characters SET name = :name, doRename = 0
                      WHERE id = :cid AND userId = :uid",
                    named_params! {
                        ":name": new_name,
                        ":cid": character_id,
                        ":uid": user_id,
                    },
                )?;
                Ok(false)
            })
            .await?;
        Ok(taken)
    }

    pub async fn delete_character(&self, character_id: u32, name: &str) -> Result<()> {
        let name = name.to_owned();
        self.conn
            .call_db(move |c| {
                c.execute(
                    "UPDATE characters SET state = 1 WHERE id = :cid AND name = :name",
                    named_params! { ":cid": character_id, ":name": name },
                )?;
                Ok(())
            })
            .await?;
        Ok(())
    }

    pub async fn get_servers(&self) -> Result<Vec<World>> {
        let rows = self.conn
            .call_db(|c| {
                let mut stmt = c.prepare(
                    "SELECT id, address, port, listPosition, name, isActive
                     FROM servers WHERE isActive = 1",
                )?;
                let rows: Vec<World> = stmt
                    .query_map([], |r| {
                        Ok(World {
                            id: r.get::<_, u16>(0)?,
                            address: r.get::<_, String>(1)?,
                            port: r.get::<_, u16>(2)?,
                            list_position: r.get::<_, u16>(3)?,
                            population: 2,
                            name: r.get::<_, String>(4)?,
                            is_active: r.get::<_, i64>(5)? != 0,
                        })
                    })?
                    .collect::<rusqlite::Result<_>>()?;
                Ok(rows)
            })
            .await?;
        Ok(rows)
    }

    pub async fn get_server(&self, server_id: u32) -> Result<Option<World>> {
        let row = self.conn
            .call_db(move |c| {
                let v = c
                    .query_row(
                        "SELECT id, address, port, listPosition, name, isActive
                         FROM servers WHERE id = :sid",
                        named_params! { ":sid": server_id },
                        |r| {
                            Ok(World {
                                id: r.get::<_, u16>(0)?,
                                address: r.get::<_, String>(1)?,
                                port: r.get::<_, u16>(2)?,
                                list_position: r.get::<_, u16>(3)?,
                                population: 2,
                                name: r.get::<_, String>(4)?,
                                is_active: r.get::<_, i64>(5)? != 0,
                            })
                        },
                    )
                    .optional()?;
                Ok(v)
            })
            .await?;
        Ok(row)
    }

    pub async fn get_characters(&self, user_id: u32) -> Result<Vec<Character>> {
        let rows = self.conn
            .call_db(move |c| {
                let mut stmt = c.prepare(
                    r"SELECT c.id, c.slot, c.serverId, c.name, c.isLegacy, c.doRename,
                             c.currentZoneId, c.guardian, c.birthMonth, c.birthDay,
                             c.initialTown, c.tribe,
                             p.mainSkill, p.mainSkillLevel
                      FROM characters c
                      INNER JOIN characters_parametersave p ON c.id = p.characterId
                      WHERE c.userId = :uid AND c.state = 2
                      ORDER BY c.slot",
                )?;
                let rows: Vec<Character> = stmt
                    .query_map(named_params! { ":uid": user_id }, character_from_row)?
                    .collect::<rusqlite::Result<_>>()?;
                Ok(rows)
            })
            .await?;
        Ok(rows)
    }

    pub async fn get_character(&self, _user_id: u32, char_id: u32) -> Result<Option<Character>> {
        let row = self.conn
            .call_db(move |c| {
                let v = c
                    .query_row(
                        r"SELECT c.id, c.slot, c.serverId, c.name, c.isLegacy, c.doRename,
                                 c.currentZoneId, c.guardian, c.birthMonth, c.birthDay,
                                 c.initialTown, c.tribe,
                                 p.mainSkill, p.mainSkillLevel
                          FROM characters c
                          INNER JOIN characters_parametersave p ON c.id = p.characterId
                          WHERE c.id = :cid",
                        named_params! { ":cid": char_id },
                        character_from_row,
                    )
                    .optional()?;
                Ok(v)
            })
            .await?;
        Ok(row)
    }

    pub async fn get_appearance(&self, chara_id: u32) -> Result<Appearance> {
        let row = self.conn
            .call_db(move |c| {
                let v = c
                    .query_row(
                        r"SELECT baseId, size, voice, skinColor, hairStyle, hairColor,
                                 hairHighlightColor, hairVariation, eyeColor,
                                 characteristics, characteristicsColor, faceType, ears,
                                 faceMouth, faceFeatures, faceNose, faceEyeShape,
                                 faceIrisSize, faceEyebrows,
                                 mainHand, offHand, head, body, legs, hands, feet, waist,
                                 neck, leftIndex, rightIndex, leftFinger, rightFinger,
                                 leftEar, rightEar
                          FROM characters_appearance WHERE characterId = :cid",
                        named_params! { ":cid": chara_id },
                        appearance_from_row,
                    )
                    .optional()?;
                Ok(v)
            })
            .await?;
        Ok(row.unwrap_or_default())
    }

    pub async fn get_reserved_names(&self, user_id: u32) -> Result<Vec<String>> {
        let rows = self.conn
            .call_db(move |c| {
                let mut stmt = c.prepare("SELECT name FROM reserved_names WHERE userId = :uid")?;
                let rows: Vec<String> = stmt
                    .query_map(named_params! { ":uid": user_id }, |r| r.get::<_, String>(0))?
                    .collect::<rusqlite::Result<_>>()?;
                Ok(rows)
            })
            .await?;
        Ok(rows)
    }

    pub async fn get_retainers(&self, _user_id: u32) -> Result<Vec<Retainer>> {
        // The C# version returns an empty list here — schema isn't finalized.
        Ok(Vec::new())
    }
}

// ---------------------------------------------------------------------------
// Row → DTO helpers. Ordered by the SELECT lists above so column indexes are
// stable; rusqlite's `get` takes either an index or a name but positional is
// cheaper and the queries are local.
// ---------------------------------------------------------------------------

fn character_from_row(r: &rusqlite::Row<'_>) -> rusqlite::Result<Character> {
    Ok(Character {
        id: r.get::<_, u32>(0)?,
        slot: r.get::<_, u16>(1)?,
        server_id: r.get::<_, u16>(2)?,
        name: r.get::<_, String>(3)?,
        state: 2,
        is_legacy: r.get::<_, i64>(4).unwrap_or_default() != 0,
        do_rename: r.get::<_, i64>(5).unwrap_or_default() != 0,
        current_zone_id: r.get::<_, u32>(6).unwrap_or_default(),
        guardian: r.get::<_, u8>(7).unwrap_or_default(),
        birth_month: r.get::<_, u8>(8).unwrap_or_default(),
        birth_day: r.get::<_, u8>(9).unwrap_or_default(),
        initial_town: r.get::<_, u8>(10).unwrap_or_default(),
        tribe: r.get::<_, u8>(11).unwrap_or_default(),
        current_class: r.get::<_, u8>(12).unwrap_or_default() as u32,
        current_job: 0,
        current_level: r.get::<_, i16>(13).unwrap_or_default() as i32,
    })
}

fn appearance_from_row(r: &rusqlite::Row<'_>) -> rusqlite::Result<Appearance> {
    Ok(Appearance {
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
        left_index: r.get::<_, u32>(28).unwrap_or_default(),
        right_index: r.get::<_, u32>(29).unwrap_or_default(),
        left_finger: r.get::<_, u32>(30).unwrap_or_default(),
        right_finger: r.get::<_, u32>(31).unwrap_or_default(),
        left_ear: r.get::<_, u32>(32).unwrap_or_default(),
        right_ear: r.get::<_, u32>(33).unwrap_or_default(),
    })
}
