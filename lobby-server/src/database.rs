//! MySQL-backed lobby database queries. Ported from Lobby Server/Database.cs
//! using mysql_async for async, pooled access instead of the one-connection-
//! per-call pattern of the C# original.

use anyhow::{Context, Result};
use mysql_async::{Pool, Row, prelude::*};

use crate::character_creator::class_name_for_id;
use crate::data::{Appearance, CharaInfo, Character, Retainer, World};

pub struct Database {
    pool: Pool,
}

impl Database {
    pub fn new(url: &str) -> Result<Self> {
        let pool = Pool::from_url(url).context("parsing mysql url")?;
        Ok(Self { pool })
    }

    pub async fn ping(&self) -> Result<()> {
        let mut conn = self.pool.get_conn().await?;
        "SELECT 1".ignore(&mut conn).await?;
        Ok(())
    }

    pub async fn user_id_from_session(&self, session_id: &str) -> Result<u32> {
        let mut conn = self.pool.get_conn().await?;
        let row: Option<Row> = "SELECT userId FROM sessions WHERE id = :sid AND expiration > NOW()"
            .with(params! { "sid" => session_id })
            .first(&mut conn)
            .await?;
        Ok(row.and_then(|mut r| r.take::<u32, _>("userId")).unwrap_or(0))
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
        let mut conn = self.pool.get_conn().await?;

        let existing: Option<u32> = r"SELECT id FROM characters
            WHERE name = :name AND serverId = :sid AND state != 2 AND state != 1"
            .with(params! { "name" => name, "sid" => server_id })
            .first(&mut conn)
            .await?;

        if existing.is_some() {
            return Ok((true, 0, 0));
        }

        r"INSERT INTO characters(userId, slot, serverId, name, state)
          VALUES(:uid, :slot, :sid, :name, 0)"
            .with(params! {
                "uid" => user_id,
                "slot" => slot,
                "sid" => server_id,
                "name" => name,
            })
            .ignore(&mut conn)
            .await?;

        let cid = conn.last_insert_id().unwrap_or(0) as u32;
        Ok((false, 0xBABE, cid))
    }

    pub async fn make_character(&self, user_id: u32, cid: u32, info: &CharaInfo) -> Result<()> {
        let mut conn = self.pool.get_conn().await?;

        r"UPDATE characters SET
              state = 2,
              currentZoneId = :zoneId,
              positionX = :x, positionY = :y, positionZ = :z,
              rotation = :r,
              guardian = :guardian,
              birthDay = :birthDay, birthMonth = :birthMonth,
              initialTown = :initialTown,
              tribe = :tribe
          WHERE userId = :userId AND id = :cid"
            .with(params! {
                "zoneId" => info.zone_id,
                "x" => info.x, "y" => info.y, "z" => info.z,
                "r" => info.rot,
                "guardian" => info.guardian,
                "birthDay" => info.birth_day,
                "birthMonth" => info.birth_month,
                "initialTown" => info.initial_town,
                "tribe" => info.tribe,
                "userId" => user_id,
                "cid" => cid,
            })
            .ignore(&mut conn)
            .await?;

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
           :body, :hands, :legs, :feet, :waist)"
            .with(params! {
                "cid" => cid,
                "size" => info.appearance.size,
                "voice" => info.appearance.voice,
                "skinColor" => info.appearance.skin_color,
                "hairStyle" => info.appearance.hair_style,
                "hairColor" => info.appearance.hair_color,
                "hairHighlightColor" => info.appearance.hair_highlight_color,
                "hairVariation" => info.appearance.hair_variation,
                "eyeColor" => info.appearance.eye_color,
                "faceType" => info.appearance.face_type,
                "faceEyebrows" => info.appearance.face_eyebrows,
                "faceEyeShape" => info.appearance.face_eye_shape,
                "faceIrisSize" => info.appearance.face_iris_size,
                "faceNose" => info.appearance.face_nose,
                "faceMouth" => info.appearance.face_mouth,
                "faceFeatures" => info.appearance.face_features,
                "ears" => info.appearance.ears,
                "characteristics" => info.appearance.characteristics,
                "characteristicsColor" => info.appearance.characteristics_color,
                "mainhand" => info.weapon1,
                "offhand" => info.weapon2,
                "head" => info.head,
                "body" => info.body,
                "hands" => info.hands,
                "legs" => info.legs,
                "feet" => info.feet,
                "waist" => info.belt,
            })
            .ignore(&mut conn)
            .await?;

        let class_col = class_name_for_id(info.current_class as i16);
        let levels_sql = format!(
            "INSERT INTO characters_class_levels(characterId, {class_col}) VALUES(:cid, 1);
             INSERT INTO characters_class_exp(characterId) VALUES(:cid)"
        );
        conn.exec_drop(&levels_sql, params! { "cid" => cid })
            .await?;

        r"INSERT INTO characters_parametersave
          (characterId, hp, hpMax, mp, mpMax, mainSkill, mainSkillLevel)
          VALUES(:cid, 1900, 1000, 115, 115, :mainSkill, 1)"
            .with(params! { "cid" => cid, "mainSkill" => info.current_class })
            .ignore(&mut conn)
            .await?;

        let default_actions: Vec<u32> =
            r"SELECT id FROM server_battle_commands
              WHERE classJob = :cls AND lvl = 1 ORDER BY id DESC"
                .with(params! { "cls" => info.current_class })
                .map(&mut conn, |id: u32| id)
                .await?;

        for (slot, command_id) in default_actions.iter().enumerate() {
            r"INSERT INTO characters_hotbar(characterId, classId, hotbarSlot, commandId, recastTime)
              VALUES(:cid, :classId, :slot, :cmd, 0)"
                .with(params! {
                    "cid" => cid,
                    "classId" => info.current_class,
                    "slot" => slot as i16,
                    "cmd" => *command_id as i16,
                })
                .ignore(&mut conn)
                .await?;
        }

        Ok(())
    }

    pub async fn rename_character(
        &self,
        user_id: u32,
        character_id: u32,
        server_id: u32,
        new_name: &str,
    ) -> Result<bool> {
        let mut conn = self.pool.get_conn().await?;

        let existing: Option<u32> = r"SELECT id FROM characters
            WHERE name = :name AND serverId = :sid"
            .with(params! { "name" => new_name, "sid" => server_id })
            .first(&mut conn)
            .await?;

        if existing.is_some() {
            return Ok(true);
        }

        r"UPDATE characters SET name = :name, DoRename = 0
            WHERE id = :cid AND userId = :uid"
            .with(params! {
                "name" => new_name,
                "cid" => character_id,
                "uid" => user_id,
            })
            .ignore(&mut conn)
            .await?;

        Ok(false)
    }

    pub async fn delete_character(&self, character_id: u32, name: &str) -> Result<()> {
        let mut conn = self.pool.get_conn().await?;
        r"UPDATE characters SET state = 1 WHERE id = :cid AND name = :name"
            .with(params! { "cid" => character_id, "name" => name })
            .ignore(&mut conn)
            .await?;
        Ok(())
    }

    pub async fn get_servers(&self) -> Result<Vec<World>> {
        let mut conn = self.pool.get_conn().await?;
        let rows: Vec<(u16, String, u16, u16, String, bool)> =
            "SELECT id, address, port, listPosition, name, isActive FROM servers WHERE isActive = true"
                .with(())
                .map(
                    &mut conn,
                    |(id, address, port, list_position, name, is_active): (
                        u16,
                        String,
                        u16,
                        u16,
                        String,
                        bool,
                    )| (id, address, port, list_position, name, is_active),
                )
                .await?;

        Ok(rows
            .into_iter()
            .map(|(id, address, port, list_position, name, is_active)| World {
                id,
                address,
                port,
                list_position,
                population: 2,
                name,
                is_active,
            })
            .collect())
    }

    pub async fn get_server(&self, server_id: u32) -> Result<Option<World>> {
        let mut conn = self.pool.get_conn().await?;
        let row: Option<(u16, String, u16, u16, String, bool)> =
            "SELECT id, address, port, listPosition, name, isActive FROM servers WHERE id = :sid"
                .with(params! { "sid" => server_id })
                .first(&mut conn)
                .await?;
        Ok(row.map(
            |(id, address, port, list_position, name, is_active)| World {
                id,
                address,
                port,
                list_position,
                population: 2,
                name,
                is_active,
            },
        ))
    }

    pub async fn get_characters(&self, user_id: u32) -> Result<Vec<Character>> {
        let mut conn = self.pool.get_conn().await?;
        let rows: Vec<Row> = r"
            SELECT id, slot, serverId, name, isLegacy, doRename, currentZoneId,
                   guardian, birthMonth, birthDay, initialTown, tribe,
                   mainSkill, mainSkillLevel
            FROM characters
            INNER JOIN characters_parametersave ON id = characters_parametersave.characterId
            WHERE userId = :uid AND state = 2
            ORDER BY slot"
            .with(params! { "uid" => user_id })
            .fetch(&mut conn)
            .await?;

        Ok(rows.into_iter().map(character_from_row).collect())
    }

    pub async fn get_character(&self, _user_id: u32, char_id: u32) -> Result<Option<Character>> {
        let mut conn = self.pool.get_conn().await?;
        let row: Option<Row> = r"
            SELECT id, slot, serverId, name, isLegacy, doRename, currentZoneId,
                   guardian, birthMonth, birthDay, initialTown, tribe,
                   mainSkill, mainSkillLevel
            FROM characters
            INNER JOIN characters_parametersave ON id = characters_parametersave.characterId
            WHERE id = :cid"
            .with(params! { "cid" => char_id })
            .first(&mut conn)
            .await?;
        Ok(row.map(character_from_row))
    }

    pub async fn get_appearance(&self, chara_id: u32) -> Result<Appearance> {
        let mut conn = self.pool.get_conn().await?;
        let row: Option<Row> = r"
            SELECT baseId, size, voice, skinColor, hairStyle, hairColor,
                   hairHighlightColor, hairVariation, eyeColor,
                   characteristics, characteristicsColor, faceType, ears,
                   faceMouth, faceFeatures, faceNose, faceEyeShape,
                   faceIrisSize, faceEyebrows,
                   mainHand, offHand, head, body, legs, hands, feet, waist,
                   neck, leftIndex, rightIndex, leftFinger, rightFinger,
                   leftEar, rightEar
            FROM characters_appearance WHERE characterId = :cid"
            .with(params! { "cid" => chara_id })
            .first(&mut conn)
            .await?;
        Ok(row.map(appearance_from_row).unwrap_or_default())
    }

    pub async fn get_reserved_names(&self, user_id: u32) -> Result<Vec<String>> {
        let mut conn = self.pool.get_conn().await?;
        let rows: Vec<String> = "SELECT name FROM reserved_names WHERE userId = :uid"
            .with(params! { "uid" => user_id })
            .map(&mut conn, |name: String| name)
            .await?;
        Ok(rows)
    }

    pub async fn get_retainers(&self, _user_id: u32) -> Result<Vec<Retainer>> {
        // The C# version returns an empty list here — schema isn't finalized.
        Ok(Vec::new())
    }
}

// ---------------------------------------------------------------------------
// Manual row extractors. mysql_async's auto `FromRow` only derives for tuples
// up to 12 columns, but we hit 14 and 34 column selects here. So unpack by
// column name, which is also more robust against `SELECT` reorderings.
// ---------------------------------------------------------------------------

fn character_from_row(mut row: Row) -> Character {
    Character {
        id: row.take("id").unwrap_or_default(),
        slot: row.take("slot").unwrap_or_default(),
        server_id: row.take("serverId").unwrap_or_default(),
        name: row.take("name").unwrap_or_default(),
        state: 2,
        is_legacy: row.take("isLegacy").unwrap_or_default(),
        do_rename: row.take("doRename").unwrap_or_default(),
        current_zone_id: row.take("currentZoneId").unwrap_or_default(),
        guardian: row.take("guardian").unwrap_or_default(),
        birth_month: row.take("birthMonth").unwrap_or_default(),
        birth_day: row.take("birthDay").unwrap_or_default(),
        current_class: row.take::<u8, _>("mainSkill").unwrap_or_default() as u32,
        current_job: 0,
        current_level: row.take::<i16, _>("mainSkillLevel").unwrap_or_default() as i32,
        initial_town: row.take("initialTown").unwrap_or_default(),
        tribe: row.take("tribe").unwrap_or_default(),
    }
}

fn appearance_from_row(mut row: Row) -> Appearance {
    Appearance {
        size: row.take("size").unwrap_or_default(),
        voice: row.take("voice").unwrap_or_default(),
        skin_color: row.take("skinColor").unwrap_or_default(),
        hair_style: row.take("hairStyle").unwrap_or_default(),
        hair_color: row.take("hairColor").unwrap_or_default(),
        hair_highlight_color: row.take("hairHighlightColor").unwrap_or_default(),
        hair_variation: row.take("hairVariation").unwrap_or_default(),
        eye_color: row.take("eyeColor").unwrap_or_default(),
        characteristics: row.take("characteristics").unwrap_or_default(),
        characteristics_color: row.take("characteristicsColor").unwrap_or_default(),
        face_type: row.take("faceType").unwrap_or_default(),
        ears: row.take("ears").unwrap_or_default(),
        face_mouth: row.take("faceMouth").unwrap_or_default(),
        face_features: row.take("faceFeatures").unwrap_or_default(),
        face_nose: row.take("faceNose").unwrap_or_default(),
        face_eye_shape: row.take("faceEyeShape").unwrap_or_default(),
        face_iris_size: row.take("faceIrisSize").unwrap_or_default(),
        face_eyebrows: row.take("faceEyebrows").unwrap_or_default(),
        main_hand: row.take("mainHand").unwrap_or_default(),
        off_hand: row.take("offHand").unwrap_or_default(),
        head: row.take("head").unwrap_or_default(),
        body: row.take("body").unwrap_or_default(),
        legs: row.take("legs").unwrap_or_default(),
        hands: row.take("hands").unwrap_or_default(),
        feet: row.take("feet").unwrap_or_default(),
        waist: row.take("waist").unwrap_or_default(),
        neck: row.take("neck").unwrap_or_default(),
        left_index: row.take("leftIndex").unwrap_or_default(),
        right_index: row.take("rightIndex").unwrap_or_default(),
        left_finger: row.take("leftFinger").unwrap_or_default(),
        right_finger: row.take("rightFinger").unwrap_or_default(),
        left_ear: row.take("leftEar").unwrap_or_default(),
        right_ear: row.take("rightEar").unwrap_or_default(),
    }
}
