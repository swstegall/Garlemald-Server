//! Map-server DB layer. Full port of `Map Server/Database.cs` (2782 lines,
//! 52 public static methods) against mysql_async with a shared pool.
//!
//! Every method maps 1:1 to a C# counterpart; names are snake_cased. Where
//! the C# signature took a Player and mutated it, the Rust equivalent takes
//! the `chara_id` and returns a DTO so call sites can do the mutation.
#![allow(dead_code)]

use std::collections::HashMap;

use anyhow::{Context, Result};
use mysql_async::{Pool, Row, prelude::*};

use crate::data::{InventoryItem, ItemData, ItemTag};
use crate::gamedata::{
    AppearanceFull, BattleCommand, BattleTrait, CharaBattleSave, CharaParameterSave, ChocoboData,
    EquipmentSlot, GuildleveGamedata, GuildleveLocalEntry, GuildleveRegionalEntry, HotbarEntry,
    ItemDealingInfo, ItemModifiers, LoadedPlayer, NpcLinkshellEntry, QuestScenarioEntry,
    StatusEffectDef, StatusEffectEntry, TIMER_COLUMNS, class_column,
};

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

    // =======================================================================
    // Session
    // =======================================================================

    /// Ported from `GetUserIdFromSession`.
    pub async fn user_id_from_session(&self, session_id: &str) -> Result<u32> {
        let mut conn = self.pool.get_conn().await?;
        let row: Option<u32> =
            "SELECT userId FROM sessions WHERE id = :sid AND expiration > NOW()"
                .with(params! { "sid" => session_id })
                .first(&mut conn)
                .await?;
        Ok(row.unwrap_or(0))
    }

    // =======================================================================
    // Gamedata loaders (called once at startup)
    // =======================================================================

    /// Ported from `GetItemGamedata` (big LEFT JOIN across 6 tables).
    pub async fn get_item_gamedata(&self) -> Result<HashMap<u32, ItemData>> {
        let mut conn = self.pool.get_conn().await?;
        let rows: Vec<Row> = r"SELECT *
            FROM gamedata_items
            LEFT JOIN gamedata_items_equipment        ON gamedata_items.catalogID = gamedata_items_equipment.catalogID
            LEFT JOIN gamedata_items_accessory        ON gamedata_items.catalogID = gamedata_items_accessory.catalogID
            LEFT JOIN gamedata_items_armor            ON gamedata_items.catalogID = gamedata_items_armor.catalogID
            LEFT JOIN gamedata_items_weapon           ON gamedata_items.catalogID = gamedata_items_weapon.catalogID
            LEFT JOIN gamedata_items_graphics         ON gamedata_items.catalogID = gamedata_items_graphics.catalogID
            LEFT JOIN gamedata_items_graphics_extra   ON gamedata_items.catalogID = gamedata_items_graphics_extra.catalogID"
            .with(())
            .fetch(&mut conn)
            .await?;

        let mut out = HashMap::with_capacity(rows.len());
        for mut row in rows {
            let id: u32 = row.take("catalogID").unwrap_or_default();
            let item = ItemData {
                id,
                name: row.take("name").unwrap_or_default(),
                singular: row.take("singular").unwrap_or_default(),
                plural: row.take("plural").unwrap_or_default(),
                icon: row.take("icon").unwrap_or_default(),
                rarity: row.take("rarity").unwrap_or_default(),
                item_ui_category: row.take("itemUICategory").unwrap_or_default(),
                stack_size: row.take("stackSize").unwrap_or_default(),
                item_level: row.take("itemLevel").unwrap_or_default(),
                equip_level: row.take("equipLevel").unwrap_or_default(),
                price: row.take("price").unwrap_or_default(),
                buy_price: row.take("buyPrice").unwrap_or_default(),
                sell_price: row.take("sellPrice").unwrap_or_default(),
                ..Default::default()
            };
            out.insert(id, item);
        }
        Ok(out)
    }

    /// Ported from `GetGuildleveGamedata`.
    pub async fn get_guildleve_gamedata(&self) -> Result<HashMap<u32, GuildleveGamedata>> {
        let mut conn = self.pool.get_conn().await?;
        let rows: Vec<Row> = "SELECT * FROM gamedata_guildleves"
            .with(())
            .fetch(&mut conn)
            .await?;

        let mut out = HashMap::with_capacity(rows.len());
        for mut row in rows {
            let id: u32 = row.take("id").unwrap_or_default();
            let g = GuildleveGamedata {
                id,
                zone_id: row.take("zoneId").unwrap_or_default(),
                name: row.take("name").unwrap_or_default(),
                difficulty: row.take("difficulty").unwrap_or_default(),
                leve_type: row.take("leveType").unwrap_or_default(),
                reward_exp: row.take("rewardExp").unwrap_or_default(),
                reward_gil: row.take("rewardGil").unwrap_or_default(),
            };
            out.insert(id, g);
        }
        Ok(out)
    }

    /// Ported from `LoadGlobalStatusEffectList`.
    pub async fn load_global_status_effect_list(&self) -> Result<HashMap<u32, StatusEffectDef>> {
        let mut conn = self.pool.get_conn().await?;
        let rows: Vec<Row> = r"SELECT id, name, flags, overwrite, tickMs, hidden, silentOnGain,
                                       silentOnLoss, statusGainTextId, statusLossTextId
                                FROM server_statuseffects"
            .with(())
            .fetch(&mut conn)
            .await?;

        let mut out = HashMap::with_capacity(rows.len());
        for mut row in rows {
            let id: u32 = row.take("id").unwrap_or_default();
            let effect = StatusEffectDef {
                id,
                name: row.take("name").unwrap_or_default(),
                flags: row.take("flags").unwrap_or_default(),
                overwrite: row.take("overwrite").unwrap_or_default(),
                tick_ms: row.take("tickMs").unwrap_or_default(),
                hidden: row.take::<i64, _>("hidden").unwrap_or(0) != 0,
                silent_on_gain: row.take::<i64, _>("silentOnGain").unwrap_or(0) != 0,
                silent_on_loss: row.take::<i64, _>("silentOnLoss").unwrap_or(0) != 0,
                status_gain_text_id: row.take("statusGainTextId").unwrap_or_default(),
                status_loss_text_id: row.take("statusLossTextId").unwrap_or_default(),
            };
            out.insert(id, effect);
        }
        Ok(out)
    }

    /// Ported from `LoadGlobalBattleCommandList`. Returns the loaded map plus
    /// a `(job, level) → [commandId]` index mirroring the C# out-param.
    pub async fn load_global_battle_command_list(
        &self,
    ) -> Result<(
        HashMap<u16, BattleCommand>,
        HashMap<(u8, i16), Vec<u16>>,
    )> {
        let mut conn = self.pool.get_conn().await?;
        let rows: Vec<Row> = r"SELECT `id`, name, classJob, lvl, requirements, mainTarget,
                                   validTarget, aoeType, aoeRange, aoeMinRange, aoeConeAngle,
                                   aoeRotateAngle, aoeTarget, basePotency, numHits,
                                   positionBonus, procRequirement, `range`, minRange,
                                   rangeHeight, rangeWidth, statusId, statusDuration,
                                   statusChance, castType, castTime, recastTime, mpCost,
                                   tpCost, animationType, effectAnimation, modelAnimation,
                                   animationDuration, battleAnimation, validUser, comboId1,
                                   comboId2, comboStep, accuracyMod, worldMasterTextId,
                                   commandType, actionType, actionProperty
                               FROM server_battle_commands"
            .with(())
            .fetch(&mut conn)
            .await?;

        let mut dict = HashMap::with_capacity(rows.len());
        let mut by_level: HashMap<(u8, i16), Vec<u16>> = HashMap::new();

        for mut row in rows {
            let id: u16 = row.take("id").unwrap_or_default();
            let bc = BattleCommand {
                id,
                name: row.take("name").unwrap_or_default(),
                job: row.take("classJob").unwrap_or_default(),
                level: row.take("lvl").unwrap_or_default(),
                requirements: row.take("requirements").unwrap_or_default(),
                main_target: row.take("mainTarget").unwrap_or_default(),
                valid_target: row.take("validTarget").unwrap_or_default(),
                aoe_type: row.take("aoeType").unwrap_or_default(),
                base_potency: row.take("basePotency").unwrap_or_default(),
                num_hits: row.take("numHits").unwrap_or_default(),
                position_bonus: row.take("positionBonus").unwrap_or_default(),
                proc_requirement: row.take("procRequirement").unwrap_or_default(),
                range: row.take("range").unwrap_or_default(),
                min_range: row.take("minRange").unwrap_or_default(),
                range_height: row.take("rangeHeight").unwrap_or_default(),
                range_width: row.take("rangeWidth").unwrap_or_default(),
                status_id: row.take("statusId").unwrap_or_default(),
                status_duration: row.take("statusDuration").unwrap_or_default(),
                status_chance: row.take("statusChance").unwrap_or_default(),
                cast_type: row.take("castType").unwrap_or_default(),
                cast_time_ms: row.take("castTime").unwrap_or_default(),
                max_recast_time_seconds: row.take("recastTime").unwrap_or_default(),
                recast_time_ms: row.take::<u32, _>("recastTime").unwrap_or_default() * 1000,
                mp_cost: row.take("mpCost").unwrap_or_default(),
                tp_cost: row.take("tpCost").unwrap_or_default(),
                animation_type: row.take("animationType").unwrap_or_default(),
                effect_animation: row.take("effectAnimation").unwrap_or_default(),
                model_animation: row.take("modelAnimation").unwrap_or_default(),
                animation_duration_seconds: row.take("animationDuration").unwrap_or_default(),
                aoe_range: row.take("aoeRange").unwrap_or_default(),
                aoe_min_range: row.take("aoeMinRange").unwrap_or_default(),
                aoe_cone_angle: row.take("aoeConeAngle").unwrap_or_default(),
                aoe_rotate_angle: row.take("aoeRotateAngle").unwrap_or_default(),
                aoe_target: row.take("aoeTarget").unwrap_or_default(),
                battle_animation: row.take("battleAnimation").unwrap_or_default(),
                valid_user: row.take("validUser").unwrap_or_default(),
                combo_next_command_id: [
                    row.take("comboId1").unwrap_or_default(),
                    row.take("comboId2").unwrap_or_default(),
                ],
                combo_step: row.take("comboStep").unwrap_or_default(),
                command_type: row.take("commandType").unwrap_or_default(),
                action_property: row.take("actionProperty").unwrap_or_default(),
                action_type: row.take("actionType").unwrap_or_default(),
                accuracy_modifier: row.take("accuracyMod").unwrap_or_default(),
                world_master_text_id: row.take("worldMasterTextId").unwrap_or_default(),
            };
            by_level.entry((bc.job, bc.level as i16)).or_default().push(id);
            dict.insert(id, bc);
        }
        Ok((dict, by_level))
    }

    /// Ported from `LoadGlobalBattleTraitList`.
    pub async fn load_global_battle_trait_list(
        &self,
    ) -> Result<(HashMap<u16, BattleTrait>, HashMap<u8, Vec<u16>>)> {
        let mut conn = self.pool.get_conn().await?;
        let rows: Vec<Row> = "SELECT `id`, name, classJob, lvl, modifier, bonus FROM server_battle_traits"
            .with(())
            .fetch(&mut conn)
            .await?;

        let mut dict = HashMap::with_capacity(rows.len());
        let mut by_job: HashMap<u8, Vec<u16>> = HashMap::new();
        for mut row in rows {
            let id: u16 = row.take("id").unwrap_or_default();
            let trait_ = BattleTrait {
                id,
                name: row.take("name").unwrap_or_default(),
                job: row.take("classJob").unwrap_or_default(),
                level: row.take("lvl").unwrap_or_default(),
                modifier: row.take("modifier").unwrap_or_default(),
                bonus: row.take("bonus").unwrap_or_default(),
            };
            by_job.entry(trait_.job).or_default().push(id);
            dict.insert(id, trait_);
        }
        Ok((dict, by_job))
    }

    // =======================================================================
    // LoadPlayerCharacter — the big one. Issues 10+ independent SELECTs
    // against the characters_* tables and aggregates into a LoadedPlayer.
    // =======================================================================

    pub async fn load_player_character(&self, chara_id: u32) -> Result<Option<LoadedPlayer>> {
        let mut conn = self.pool.get_conn().await?;

        let basic: Option<Row> = r"SELECT name, positionX, positionY, positionZ, rotation,
                                           actorState, currentZoneId, gcCurrent, gcLimsaRank,
                                           gcGridaniaRank, gcUldahRank, currentTitle, guardian,
                                           birthDay, birthMonth, initialTown, tribe, restBonus,
                                           achievementPoints, playTime, destinationZoneId,
                                           destinationSpawnType, currentPrivateArea,
                                           currentPrivateAreaType, homepoint, homepointInn
                                    FROM characters WHERE id = :cid"
            .with(params! { "cid" => chara_id })
            .first(&mut conn)
            .await?;

        let Some(mut row) = basic else {
            return Ok(None);
        };
        let mut player = LoadedPlayer {
            name: row.take("name").unwrap_or_default(),
            position_x: row.take("positionX").unwrap_or_default(),
            position_y: row.take("positionY").unwrap_or_default(),
            position_z: row.take("positionZ").unwrap_or_default(),
            rotation: row.take("rotation").unwrap_or_default(),
            actor_state: row.take("actorState").unwrap_or_default(),
            current_zone_id: row.take("currentZoneId").unwrap_or_default(),
            gc_current: row.take("gcCurrent").unwrap_or_default(),
            gc_limsa_rank: row.take("gcLimsaRank").unwrap_or_default(),
            gc_gridania_rank: row.take("gcGridaniaRank").unwrap_or_default(),
            gc_uldah_rank: row.take("gcUldahRank").unwrap_or_default(),
            current_title: row.take("currentTitle").unwrap_or_default(),
            guardian: row.take("guardian").unwrap_or_default(),
            birth_day: row.take("birthDay").unwrap_or_default(),
            birth_month: row.take("birthMonth").unwrap_or_default(),
            initial_town: row.take("initialTown").unwrap_or_default(),
            tribe: row.take("tribe").unwrap_or_default(),
            rest_bonus_exp_rate: row.take("restBonus").unwrap_or_default(),
            achievement_points: row.take("achievementPoints").unwrap_or_default(),
            play_time: row.take("playTime").unwrap_or_default(),
            destination_zone_id: row.take("destinationZoneId").unwrap_or_default(),
            destination_spawn_type: row.take("destinationSpawnType").unwrap_or_default(),
            current_private_area: row.take("currentPrivateArea").unwrap_or_default(),
            current_private_area_type: row.take("currentPrivateAreaType").unwrap_or_default(),
            homepoint: row.take("homepoint").unwrap_or_default(),
            homepoint_inn: row.take("homepointInn").unwrap_or_default(),
            ..Default::default()
        };
        if player.destination_zone_id != 0 {
            player.current_zone_id = player.destination_zone_id;
        }

        player.class_levels = self.load_class_levels_and_exp(chara_id).await.unwrap_or_default();
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
        player.guildleves_local = self.load_guildleves_local(chara_id).await.unwrap_or_default();
        player.guildleves_regional = self
            .load_guildleves_regional(chara_id)
            .await
            .unwrap_or_default();
        player.npc_linkshells = self
            .load_npc_linkshells(chara_id)
            .await
            .unwrap_or_default();

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
        let mut conn = self.pool.get_conn().await?;

        let columns: [u8; 18] = [
            2, 3, 4, 7, 8, 22, 23, 29, 30, 31, 32, 33, 34, 35, 36, 39, 40, 41,
        ];

        let mut save = CharaBattleSave::default();

        let row: Option<Row> = r"SELECT pug, gla, mrd, arc, lnc, thm, cnj, crp, bsm, arm, gsm, ltw,
                                         wvr, alc, cul, min, btn, fsh
                                  FROM characters_class_levels WHERE characterId = :cid"
            .with(params! { "cid" => chara_id })
            .first(&mut conn)
            .await?;
        if let Some(mut row) = row {
            for cid in columns {
                let col = class_column(cid).expect("class id in table");
                save.skill_level[(cid - 1) as usize] = row.take(col).unwrap_or_default();
            }
        }

        let row: Option<Row> = r"SELECT pug, gla, mrd, arc, lnc, thm, cnj, crp, bsm, arm, gsm, ltw,
                                         wvr, alc, cul, min, btn, fsh
                                  FROM characters_class_exp WHERE characterId = :cid"
            .with(params! { "cid" => chara_id })
            .first(&mut conn)
            .await?;
        if let Some(mut row) = row {
            for cid in columns {
                let col = class_column(cid).expect("class id in table");
                save.skill_point[(cid - 1) as usize] = row.take(col).unwrap_or_default();
            }
        }

        Ok(save)
    }

    async fn load_parameter_save(&self, chara_id: u32) -> Result<CharaParameterSave> {
        let mut conn = self.pool.get_conn().await?;
        let row: Option<Row> = r"SELECT hp, hpMax, mp, mpMax, mainSkill
                                  FROM characters_parametersave WHERE characterId = :cid"
            .with(params! { "cid" => chara_id })
            .first(&mut conn)
            .await?;
        let mut save = CharaParameterSave::default();
        if let Some(mut row) = row {
            save.hp[0] = row.take("hp").unwrap_or_default();
            save.hp_max[0] = row.take("hpMax").unwrap_or_default();
            save.mp = row.take("mp").unwrap_or_default();
            save.mp_max = row.take("mpMax").unwrap_or_default();
            save.state_main_skill[0] = row.take("mainSkill").unwrap_or_default();
        }
        Ok(save)
    }

    async fn load_appearance_full(&self, chara_id: u32) -> Result<AppearanceFull> {
        let mut conn = self.pool.get_conn().await?;
        let row: Option<Row> = r"SELECT baseId, size, voice, skinColor, hairStyle, hairColor,
                                       hairHighlightColor, hairVariation, eyeColor, characteristics,
                                       characteristicsColor, faceType, ears, faceMouth,
                                       faceFeatures, faceNose, faceEyeShape, faceIrisSize,
                                       faceEyebrows, mainHand, offHand, head, body, legs, hands,
                                       feet, waist, neck, leftFinger, rightFinger, leftEar, rightEar
                                  FROM characters_appearance WHERE characterId = :cid"
            .with(params! { "cid" => chara_id })
            .first(&mut conn)
            .await?;
        let mut a = AppearanceFull::default();
        if let Some(mut row) = row {
            a.base_id = row.take("baseId").unwrap_or(0xFFFFFFFF);
            a.size = row.take("size").unwrap_or_default();
            a.voice = row.take("voice").unwrap_or_default();
            a.skin_color = row.take("skinColor").unwrap_or_default();
            a.hair_style = row.take("hairStyle").unwrap_or_default();
            a.hair_color = row.take("hairColor").unwrap_or_default();
            a.hair_highlight_color = row.take("hairHighlightColor").unwrap_or_default();
            a.hair_variation = row.take("hairVariation").unwrap_or_default();
            a.eye_color = row.take("eyeColor").unwrap_or_default();
            a.characteristics = row.take("characteristics").unwrap_or_default();
            a.characteristics_color = row.take("characteristicsColor").unwrap_or_default();
            a.face_type = row.take("faceType").unwrap_or_default();
            a.ears = row.take("ears").unwrap_or_default();
            a.face_mouth = row.take("faceMouth").unwrap_or_default();
            a.face_features = row.take("faceFeatures").unwrap_or_default();
            a.face_nose = row.take("faceNose").unwrap_or_default();
            a.face_eye_shape = row.take("faceEyeShape").unwrap_or_default();
            a.face_iris_size = row.take("faceIrisSize").unwrap_or_default();
            a.face_eyebrows = row.take("faceEyebrows").unwrap_or_default();
            a.main_hand = row.take("mainHand").unwrap_or_default();
            a.off_hand = row.take("offHand").unwrap_or_default();
            a.head = row.take("head").unwrap_or_default();
            a.body = row.take("body").unwrap_or_default();
            a.legs = row.take("legs").unwrap_or_default();
            a.hands = row.take("hands").unwrap_or_default();
            a.feet = row.take("feet").unwrap_or_default();
            a.waist = row.take("waist").unwrap_or_default();
            a.neck = row.take("neck").unwrap_or_default();
            a.left_finger = row.take("leftFinger").unwrap_or_default();
            a.right_finger = row.take("rightFinger").unwrap_or_default();
            a.left_ear = row.take("leftEar").unwrap_or_default();
            a.right_ear = row.take("rightEar").unwrap_or_default();
        }
        Ok(a)
    }

    async fn load_character_status_effects(
        &self,
        chara_id: u32,
    ) -> Result<Vec<StatusEffectEntry>> {
        let mut conn = self.pool.get_conn().await?;
        let rows: Vec<Row> = r"SELECT statusId, duration, magnitude, tick, tier, extra
                                FROM characters_statuseffect WHERE characterId = :cid"
            .with(params! { "cid" => chara_id })
            .fetch(&mut conn)
            .await?;
        Ok(rows
            .into_iter()
            .map(|mut row| StatusEffectEntry {
                status_id: row.take("statusId").unwrap_or_default(),
                duration: row.take("duration").unwrap_or_default(),
                magnitude: row.take("magnitude").unwrap_or_default(),
                tick: row.take("tick").unwrap_or_default(),
                tier: row.take("tier").unwrap_or_default(),
                extra: row.take("extra").unwrap_or_default(),
            })
            .collect())
    }

    async fn load_chocobo(&self, chara_id: u32) -> Result<ChocoboData> {
        let mut conn = self.pool.get_conn().await?;
        let row: Option<Row> = r"SELECT hasChocobo, hasGoobbue, chocoboAppearance, chocoboName
                                  FROM characters_chocobo WHERE characterId = :cid"
            .with(params! { "cid" => chara_id })
            .first(&mut conn)
            .await?;
        let mut c = ChocoboData::default();
        if let Some(mut row) = row {
            c.has_chocobo = row.take::<i64, _>("hasChocobo").unwrap_or(0) != 0;
            c.has_goobbue = row.take::<i64, _>("hasGoobbue").unwrap_or(0) != 0;
            c.chocobo_appearance = row.take("chocoboAppearance").unwrap_or_default();
            c.chocobo_name = row.take("chocoboName").unwrap_or_default();
        }
        Ok(c)
    }

    async fn load_timers(&self, chara_id: u32) -> Result<[u32; 20]> {
        let mut conn = self.pool.get_conn().await?;
        let cols = TIMER_COLUMNS.join(", ");
        let sql = format!("SELECT {cols} FROM characters_timers WHERE characterId = :cid");
        let row: Option<Row> = sql
            .with(params! { "cid" => chara_id })
            .first(&mut conn)
            .await?;
        let mut out = [0u32; 20];
        if let Some(mut row) = row {
            for (i, col) in TIMER_COLUMNS.iter().enumerate() {
                out[i] = row.take::<u32, _>(*col).unwrap_or_default();
            }
        }
        Ok(out)
    }

    async fn load_quest_scenario(&self, chara_id: u32) -> Result<Vec<QuestScenarioEntry>> {
        let mut conn = self.pool.get_conn().await?;
        let rows: Vec<Row> = r"SELECT slot, questId, questData, questFlags, currentPhase
                                FROM characters_quest_scenario WHERE characterId = :cid"
            .with(params! { "cid" => chara_id })
            .fetch(&mut conn)
            .await?;
        Ok(rows
            .into_iter()
            .map(|mut row| QuestScenarioEntry {
                slot: row.take("slot").unwrap_or_default(),
                quest_id: row.take("questId").unwrap_or_default(),
                quest_data: row.take("questData").unwrap_or_else(|| "{}".to_string()),
                quest_flags: row.take("questFlags").unwrap_or_default(),
                current_phase: row.take("currentPhase").unwrap_or_default(),
            })
            .collect())
    }

    async fn load_guildleves_local(&self, chara_id: u32) -> Result<Vec<GuildleveLocalEntry>> {
        let mut conn = self.pool.get_conn().await?;
        let rows: Vec<Row> = r"SELECT slot, questId, abandoned, completed
                                FROM characters_quest_guildleve_local WHERE characterId = :cid"
            .with(params! { "cid" => chara_id })
            .fetch(&mut conn)
            .await?;
        Ok(rows
            .into_iter()
            .map(|mut row| GuildleveLocalEntry {
                slot: row.take("slot").unwrap_or_default(),
                quest_id: row.take("questId").unwrap_or_default(),
                abandoned: row.take::<i64, _>("abandoned").unwrap_or(0) != 0,
                completed: row.take::<i64, _>("completed").unwrap_or(0) != 0,
            })
            .collect())
    }

    async fn load_guildleves_regional(&self, chara_id: u32) -> Result<Vec<GuildleveRegionalEntry>> {
        let mut conn = self.pool.get_conn().await?;
        let rows: Vec<Row> = r"SELECT slot, guildleveId, abandoned, completed
                                FROM characters_quest_guildleve_regional WHERE characterId = :cid"
            .with(params! { "cid" => chara_id })
            .fetch(&mut conn)
            .await?;
        Ok(rows
            .into_iter()
            .map(|mut row| GuildleveRegionalEntry {
                slot: row.take("slot").unwrap_or_default(),
                guildleve_id: row.take("guildleveId").unwrap_or_default(),
                abandoned: row.take::<i64, _>("abandoned").unwrap_or(0) != 0,
                completed: row.take::<i64, _>("completed").unwrap_or(0) != 0,
            })
            .collect())
    }

    async fn load_npc_linkshells(&self, chara_id: u32) -> Result<Vec<NpcLinkshellEntry>> {
        let mut conn = self.pool.get_conn().await?;
        let rows: Vec<Row> = r"SELECT npcLinkshellId, isCalling, isExtra
                                FROM characters_npclinkshell WHERE characterId = :cid"
            .with(params! { "cid" => chara_id })
            .fetch(&mut conn)
            .await?;
        Ok(rows
            .into_iter()
            .map(|mut row| NpcLinkshellEntry {
                npc_ls_id: row.take("npcLinkshellId").unwrap_or_default(),
                is_calling: row.take::<i64, _>("isCalling").unwrap_or(0) != 0,
                is_extra: row.take::<i64, _>("isExtra").unwrap_or(0) != 0,
            })
            .collect())
    }

    // =======================================================================
    // Character saves (flat SQL updates)
    // =======================================================================

    /// Ported from `SavePlayerAppearance`. Expects a 28-element `ids` array
    /// indexed by the `SetActorAppearancePacket.*` column constants.
    pub async fn save_player_appearance(
        &self,
        chara_id: u32,
        ids: &[u32; 28],
    ) -> Result<()> {
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

        let mut conn = self.pool.get_conn().await?;
        r"UPDATE characters_appearance SET
              mainHand = :mh, offHand = :oh, head = :head, body = :body,
              legs = :legs, hands = :hands, feet = :feet, waist = :waist,
              neck = :neck, leftFinger = :lf, rightFinger = :rf,
              leftEar = :le, rightEar = :re
           WHERE characterId = :cid"
            .with(params! {
                "mh" => ids[MAINHAND],
                "oh" => ids[OFFHAND],
                "head" => ids[HEADGEAR],
                "body" => ids[BODYGEAR],
                "legs" => ids[LEGSGEAR],
                "hands" => ids[HANDSGEAR],
                "feet" => ids[FEETGEAR],
                "waist" => ids[WAISTGEAR],
                "neck" => ids[NECKGEAR],
                "lf" => ids[L_RINGFINGER],
                "rf" => ids[R_RINGFINGER],
                "le" => ids[L_EAR],
                "re" => ids[R_EAR],
                "cid" => chara_id,
            })
            .ignore(&mut conn)
            .await?;
        Ok(())
    }

    /// Ported from `SavePlayerCurrentClass`.
    pub async fn save_player_current_class(
        &self,
        chara_id: u32,
        class_id: u8,
        class_level: i16,
    ) -> Result<()> {
        let mut conn = self.pool.get_conn().await?;
        r"UPDATE characters_parametersave SET mainSkill = :cid, mainSkillLevel = :lvl
          WHERE characterId = :charaId"
            .with(params! { "cid" => class_id, "lvl" => class_level, "charaId" => chara_id })
            .ignore(&mut conn)
            .await?;
        Ok(())
    }

    /// Ported from `SavePlayerPosition`. Takes a packed param bundle so the
    /// signature doesn't balloon past clippy's `too_many_arguments` limit.
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
        let mut conn = self.pool.get_conn().await?;
        r"UPDATE characters SET
              positionX = :x, positionY = :y, positionZ = :z, rotation = :rot,
              destinationZoneId = :dz, destinationSpawnType = :ds,
              currentZoneId = :zid, currentPrivateArea = :pa, currentPrivateAreaType = :pat
           WHERE id = :cid"
            .with(params! {
                "x" => x, "y" => y, "z" => z, "rot" => rotation,
                "dz" => dest_zone, "ds" => dest_spawn,
                "zid" => zone_id, "pa" => private_area, "pat" => private_area_type,
                "cid" => chara_id,
            })
            .ignore(&mut conn)
            .await?;
        Ok(())
    }

    /// Ported from `SavePlayerPlayTime`.
    pub async fn save_player_play_time(&self, chara_id: u32, play_time: u32) -> Result<()> {
        let mut conn = self.pool.get_conn().await?;
        "UPDATE characters SET playTime = :pt WHERE id = :cid"
            .with(params! { "pt" => play_time, "cid" => chara_id })
            .ignore(&mut conn)
            .await?;
        Ok(())
    }

    /// Ported from `SavePlayerHomePoints`.
    pub async fn save_player_home_points(
        &self,
        chara_id: u32,
        homepoint: u32,
        homepoint_inn: u8,
    ) -> Result<()> {
        let mut conn = self.pool.get_conn().await?;
        "UPDATE characters SET homepoint = :hp, homepointInn = :hpi WHERE id = :cid"
            .with(params! { "hp" => homepoint, "hpi" => homepoint_inn, "cid" => chara_id })
            .ignore(&mut conn)
            .await?;
        Ok(())
    }

    // =======================================================================
    // Quests / guildleves
    // =======================================================================

    /// Ported from `SaveQuest(Player, Quest, int slot)`.
    pub async fn save_quest(
        &self,
        chara_id: u32,
        slot: i32,
        quest_actor_id: u32,
        phase: u32,
        quest_data: &str,
        quest_flags: u32,
    ) -> Result<()> {
        let mut conn = self.pool.get_conn().await?;
        r"INSERT INTO characters_quest_scenario
            (characterId, slot, questId, currentPhase, questData, questFlags)
          VALUES (:cid, :slot, :qid, :phase, :data, :flags)
          ON DUPLICATE KEY UPDATE
            questId = :qid, currentPhase = :phase, questData = :data, questFlags = :flags"
            .with(params! {
                "cid" => chara_id,
                "slot" => slot,
                "qid" => 0xF_FFFF & quest_actor_id,
                "phase" => phase,
                "data" => quest_data,
                "flags" => quest_flags,
            })
            .ignore(&mut conn)
            .await?;
        Ok(())
    }

    /// Ported from `MarkGuildleve`.
    pub async fn mark_guildleve(
        &self,
        chara_id: u32,
        gl_id: u32,
        is_abandoned: bool,
        is_completed: bool,
    ) -> Result<()> {
        let mut conn = self.pool.get_conn().await?;
        r"UPDATE characters_quest_guildleve_regional
          SET abandoned = :ab, completed = :cmp
          WHERE characterId = :cid AND guildleveId = :gid"
            .with(params! {
                "ab" => is_abandoned,
                "cmp" => is_completed,
                "cid" => chara_id,
                "gid" => gl_id,
            })
            .ignore(&mut conn)
            .await?;
        Ok(())
    }

    /// Ported from `SaveGuildleve`.
    pub async fn save_guildleve(&self, chara_id: u32, gl_id: u32, slot: i32) -> Result<()> {
        let mut conn = self.pool.get_conn().await?;
        r"INSERT INTO characters_quest_guildleve_regional
            (characterId, slot, guildleveId, abandoned, completed)
          VALUES (:cid, :slot, :gid, 0, 0)
          ON DUPLICATE KEY UPDATE guildleveId = :gid, abandoned = 0, completed = 0"
            .with(params! { "cid" => chara_id, "slot" => slot, "gid" => gl_id })
            .ignore(&mut conn)
            .await?;
        Ok(())
    }

    /// Ported from `RemoveGuildleve`.
    pub async fn remove_guildleve(&self, chara_id: u32, gl_id: u32) -> Result<()> {
        let mut conn = self.pool.get_conn().await?;
        "DELETE FROM characters_quest_guildleve_regional WHERE characterId = :cid AND guildleveId = :gid"
            .with(params! { "cid" => chara_id, "gid" => gl_id })
            .ignore(&mut conn)
            .await?;
        Ok(())
    }

    /// Ported from `RemoveQuest`.
    pub async fn remove_quest(&self, chara_id: u32, quest_id: u32) -> Result<()> {
        let mut conn = self.pool.get_conn().await?;
        "DELETE FROM characters_quest_scenario WHERE characterId = :cid AND questId = :qid"
            .with(params! { "cid" => chara_id, "qid" => 0xF_FFFF & quest_id })
            .ignore(&mut conn)
            .await?;
        Ok(())
    }

    /// Ported from `CompleteQuest`.
    pub async fn complete_quest(&self, chara_id: u32, quest_id: u32) -> Result<()> {
        let mut conn = self.pool.get_conn().await?;
        r"INSERT INTO characters_quest_completed (characterId, questId)
          VALUES (:cid, :qid)
          ON DUPLICATE KEY UPDATE characterId = characterId"
            .with(params! { "cid" => chara_id, "qid" => 0xF_FFFF & quest_id })
            .ignore(&mut conn)
            .await?;
        Ok(())
    }

    /// Ported from `IsQuestCompleted`.
    pub async fn is_quest_completed(&self, chara_id: u32, quest_id: u32) -> Result<bool> {
        let mut conn = self.pool.get_conn().await?;
        let row: Option<u32> = "SELECT questId FROM characters_quest_completed
                                WHERE characterId = :cid AND questId = :qid"
            .with(params! { "cid" => chara_id, "qid" => quest_id })
            .first(&mut conn)
            .await?;
        Ok(row.is_some())
    }

    // =======================================================================
    // Equipment / hotbar
    // =======================================================================

    /// Ported from `GetEquipment(Player, ushort classId)`.
    pub async fn get_equipment(&self, chara_id: u32, class_id: u16) -> Result<Vec<EquipmentSlot>> {
        let mut conn = self.pool.get_conn().await?;
        let rows: Vec<Row> = r"SELECT equipSlot, itemId FROM characters_inventory_equipment
            WHERE characterId = :cid AND (classId = :class OR classId = 0)
            ORDER BY equipSlot"
            .with(params! { "cid" => chara_id, "class" => class_id })
            .fetch(&mut conn)
            .await?;
        Ok(rows
            .into_iter()
            .map(|mut row| EquipmentSlot {
                equip_slot: row.take("equipSlot").unwrap_or_default(),
                item_id: row.take("itemId").unwrap_or_default(),
            })
            .collect())
    }

    /// Ported from `EquipItem`.
    pub async fn equip_item(
        &self,
        chara_id: u32,
        class_id: u8,
        equip_slot: u16,
        unique_item_id: u64,
        is_undergarment: bool,
    ) -> Result<()> {
        let mut conn = self.pool.get_conn().await?;
        let effective_class: u8 = if is_undergarment { 0 } else { class_id };
        r"INSERT INTO characters_inventory_equipment (characterId, classId, equipSlot, itemId)
          VALUES (:cid, :class, :slot, :iid)
          ON DUPLICATE KEY UPDATE itemId = :iid"
            .with(params! {
                "cid" => chara_id,
                "class" => effective_class,
                "slot" => equip_slot,
                "iid" => unique_item_id,
            })
            .ignore(&mut conn)
            .await?;
        Ok(())
    }

    /// Ported from `UnequipItem`.
    pub async fn unequip_item(
        &self,
        chara_id: u32,
        class_id: u8,
        equip_slot: u16,
    ) -> Result<()> {
        let mut conn = self.pool.get_conn().await?;
        r"DELETE FROM characters_inventory_equipment
          WHERE characterId = :cid AND classId = :class AND equipSlot = :slot"
            .with(params! {
                "cid" => chara_id,
                "class" => class_id,
                "slot" => equip_slot,
            })
            .ignore(&mut conn)
            .await?;
        Ok(())
    }

    /// Ported from `EquipAbility` (non-zero commandId branch — callers should
    /// call `unequip_ability` for zeros, matching the C# fallthrough).
    pub async fn equip_ability(
        &self,
        chara_id: u32,
        class_id: u8,
        hotbar_slot: u16,
        command_id: u32,
        recast_time: u32,
    ) -> Result<()> {
        let command_id = command_id & 0xFFFF;
        let mut conn = self.pool.get_conn().await?;
        r"INSERT INTO characters_hotbar (characterId, classId, hotbarSlot, commandId, recastTime)
          VALUES (:cid, :class, :slot, :cmd, :rcst)
          ON DUPLICATE KEY UPDATE commandId = :cmd, recastTime = :rcst"
            .with(params! {
                "cid" => chara_id, "class" => class_id, "slot" => hotbar_slot,
                "cmd" => command_id, "rcst" => recast_time,
            })
            .ignore(&mut conn)
            .await?;
        Ok(())
    }

    /// Ported from `UnequipAbility`.
    pub async fn unequip_ability(
        &self,
        chara_id: u32,
        class_id: u8,
        hotbar_slot: u16,
    ) -> Result<()> {
        let mut conn = self.pool.get_conn().await?;
        r"DELETE FROM characters_hotbar
          WHERE characterId = :cid AND classId = :class AND hotbarSlot = :slot"
            .with(params! { "cid" => chara_id, "class" => class_id, "slot" => hotbar_slot })
            .ignore(&mut conn)
            .await?;
        Ok(())
    }

    /// Ported from `LoadHotbar`.
    pub async fn load_hotbar(&self, chara_id: u32, class_id: u8) -> Result<Vec<HotbarEntry>> {
        let mut conn = self.pool.get_conn().await?;
        let rows: Vec<Row> = r"SELECT hotbarSlot, commandId, recastTime
                                FROM characters_hotbar
                                WHERE characterId = :cid AND classId = :class
                                ORDER BY hotbarSlot"
            .with(params! { "cid" => chara_id, "class" => class_id })
            .fetch(&mut conn)
            .await?;
        Ok(rows
            .into_iter()
            .map(|mut row| HotbarEntry {
                hotbar_slot: row.take("hotbarSlot").unwrap_or_default(),
                command_id: row.take("commandId").unwrap_or_default(),
                recast_time: row.take("recastTime").unwrap_or_default(),
            })
            .collect())
    }

    /// Ported from `FindFirstCommandSlot`.
    pub async fn find_first_command_slot(&self, chara_id: u32, class_id: u8) -> Result<u16> {
        let mut conn = self.pool.get_conn().await?;
        let rows: Vec<u16> = r"SELECT hotbarSlot FROM characters_hotbar
                                WHERE characterId = :cid AND classId = :class
                                ORDER BY hotbarSlot"
            .with(params! { "cid" => chara_id, "class" => class_id })
            .map(&mut conn, |slot: u16| slot)
            .await?;
        let mut expected: u16 = 0;
        for slot in rows {
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

    /// Ported from `GetItemPackage`.
    pub async fn get_item_package(
        &self,
        owner_id: u32,
        item_package: u32,
    ) -> Result<Vec<InventoryItem>> {
        let mut conn = self.pool.get_conn().await?;
        let rows: Vec<Row> = r"SELECT
                serverItemId, itemId, server_items_modifiers.id AS modifierId, quantity, quality,
                dealingValue, dealingMode, dealingAttached1, dealingAttached2, dealingAttached3,
                dealingTag, bazaarMode,
                durability, mainQuality, subQuality1, subQuality2, subQuality3,
                param1, param2, param3, spiritbind,
                materia1, materia2, materia3, materia4, materia5
            FROM characters_inventory
            INNER JOIN server_items ON serverItemId = server_items.id
            LEFT JOIN server_items_modifiers ON server_items.id = server_items_modifiers.id
            LEFT JOIN server_items_dealing ON server_items.id = server_items_dealing.id
            WHERE characterId = :cid AND itemPackage = :pkg
            ORDER BY slot ASC"
            .with(params! { "cid" => owner_id, "pkg" => item_package })
            .fetch(&mut conn)
            .await?;
        Ok(rows.into_iter().map(inventory_item_from_row).collect())
    }

    /// Ported from `CreateItem(uint, int, byte, modifiers)`.
    pub async fn create_item(
        &self,
        item_id: u32,
        quantity: i32,
        quality: u8,
        modifiers: Option<&ItemModifiers>,
    ) -> Result<InventoryItem> {
        let mut conn = self.pool.get_conn().await?;
        r"INSERT INTO server_items (itemId, quantity, quality)
          VALUES (:iid, :qty, :qual)"
            .with(params! { "iid" => item_id, "qty" => quantity, "qual" => quality })
            .ignore(&mut conn)
            .await?;
        let unique_id = conn.last_insert_id().unwrap_or(0);

        let mut item = InventoryItem {
            unique_id,
            item_id,
            quantity,
            quality,
            ..Default::default()
        };

        if let Some(m) = modifiers {
            r"INSERT INTO server_items_modifiers (id, durability) VALUES (:id, :d)"
                .with(params! { "id" => unique_id, "d" => m.durability })
                .ignore(&mut conn)
                .await?;
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
    }

    /// Ported from `AddItem`.
    pub async fn add_item(
        &self,
        owner_id: u32,
        server_item_id: u64,
        item_package: u16,
        slot: u16,
    ) -> Result<()> {
        let mut conn = self.pool.get_conn().await?;
        r"INSERT INTO characters_inventory (characterId, itemPackage, serverItemId, slot)
          VALUES (:cid, :pkg, :iid, :slot)"
            .with(params! {
                "cid" => owner_id, "pkg" => item_package,
                "iid" => server_item_id, "slot" => slot,
            })
            .ignore(&mut conn)
            .await?;
        Ok(())
    }

    /// Ported from `RemoveItem`.
    pub async fn remove_item(&self, owner_id: u32, server_item_id: u64) -> Result<()> {
        let mut conn = self.pool.get_conn().await?;
        "DELETE FROM characters_inventory WHERE characterId = :cid AND serverItemId = :iid"
            .with(params! { "cid" => owner_id, "iid" => server_item_id })
            .ignore(&mut conn)
            .await?;
        Ok(())
    }

    /// Ported from `UpdateItemPositions`.
    pub async fn update_item_positions(&self, updates: &[InventoryItem]) -> Result<()> {
        let mut conn = self.pool.get_conn().await?;
        let mut txn = conn.start_transaction(Default::default()).await?;
        for item in updates {
            r"UPDATE characters_inventory SET slot = :slot WHERE serverItemId = :iid"
                .with(params! { "slot" => item.slot, "iid" => item.unique_id })
                .ignore(&mut txn)
                .await?;
        }
        txn.commit().await?;
        Ok(())
    }

    /// Ported from `SetQuantity`.
    pub async fn set_quantity(&self, server_item_id: u64, quantity: i32) -> Result<()> {
        let mut conn = self.pool.get_conn().await?;
        "UPDATE server_items SET quantity = :q WHERE id = :iid"
            .with(params! { "q" => quantity, "iid" => server_item_id })
            .ignore(&mut conn)
            .await?;
        Ok(())
    }

    /// Ported from `SetDealingInfo`.
    pub async fn set_dealing_info(
        &self,
        server_item_id: u64,
        info: &ItemDealingInfo,
    ) -> Result<()> {
        let mut conn = self.pool.get_conn().await?;
        r"REPLACE INTO server_items_dealing
            (id, dealingValue, dealingMode, dealingAttached1, dealingAttached2,
             dealingAttached3, dealingTag, bazaarMode)
          VALUES
            (:iid, :dv, :dm, :da1, :da2, :da3, :dt, :bm)"
            .with(params! {
                "iid" => server_item_id,
                "dv" => info.dealing_value,
                "dm" => info.dealing_mode,
                "da1" => info.dealing_attached[0],
                "da2" => info.dealing_attached[1],
                "da3" => info.dealing_attached[2],
                "dt" => info.dealing_tag,
                "bm" => info.bazaar_mode,
            })
            .ignore(&mut conn)
            .await?;
        Ok(())
    }

    /// Ported from `ClearDealingInfo`.
    pub async fn clear_dealing_info(&self, server_item_id: u64) -> Result<()> {
        let mut conn = self.pool.get_conn().await?;
        "DELETE FROM server_items_dealing WHERE id = :iid"
            .with(params! { "iid" => server_item_id })
            .ignore(&mut conn)
            .await?;
        Ok(())
    }

    // =======================================================================
    // Achievements
    // =======================================================================

    /// Ported from `GetLatestAchievements`. The C# returned a SubPacket; here
    /// we return the raw ids so packet construction stays in the `packets`
    /// module.
    pub async fn get_latest_achievements(&self, chara_id: u32) -> Result<[u32; 5]> {
        let mut conn = self.pool.get_conn().await?;
        let rows: Vec<u32> = r"SELECT characters_achievements.achievementId
                                FROM characters_achievements
                                INNER JOIN gamedata_achievements
                                    ON characters_achievements.achievementId = gamedata_achievements.achievementId
                                WHERE characterId = :cid AND rewardPoints <> 0
                                      AND timeDone IS NOT NULL
                                ORDER BY timeDone LIMIT 5"
            .with(params! { "cid" => chara_id })
            .map(&mut conn, |id: u32| id)
            .await?;
        let mut out = [0u32; 5];
        for (i, v) in rows.into_iter().take(5).enumerate() {
            out[i] = v;
        }
        Ok(out)
    }

    /// Ported from `GetAchievementsPacket`. Returns a bitset of completed
    /// offsets; caller turns it into a SubPacket.
    pub async fn get_achievements(&self, chara_id: u32) -> Result<Vec<u32>> {
        let mut conn = self.pool.get_conn().await?;
        let rows: Vec<u32> = r"SELECT packetOffsetId
                                FROM characters_achievements
                                INNER JOIN gamedata_achievements
                                    ON characters_achievements.achievementId = gamedata_achievements.achievementId
                                WHERE characterId = :cid AND timeDone IS NOT NULL"
            .with(params! { "cid" => chara_id })
            .map(&mut conn, |o: u32| o)
            .await?;
        Ok(rows)
    }

    /// Ported from `GetAchievementProgress`. Returns `(progress, progressFlags)`.
    pub async fn get_achievement_progress(
        &self,
        chara_id: u32,
        achievement_id: u32,
    ) -> Result<(u32, u32)> {
        let mut conn = self.pool.get_conn().await?;
        let row: Option<(u32, u32)> =
            "SELECT progress, progressFlags FROM characters_achievements
             WHERE characterId = :cid AND achievementId = :aid"
                .with(params! { "cid" => chara_id, "aid" => achievement_id })
                .first(&mut conn)
                .await?;
        Ok(row.unwrap_or((0, 0)))
    }

    // =======================================================================
    // Linkshells, support tickets, FAQ, chocobo, status save
    // =======================================================================

    /// Ported from `CreateLinkshell`.
    pub async fn create_linkshell(
        &self,
        chara_id: u32,
        ls_name: &str,
        ls_crest: u16,
    ) -> Result<bool> {
        let mut conn = self.pool.get_conn().await?;
        let ok = r"INSERT INTO server_linkshells (name, master, crest)
          VALUES (:name, :master, :crest)"
            .with(params! { "name" => ls_name, "master" => chara_id, "crest" => ls_crest })
            .ignore(&mut conn)
            .await;
        Ok(ok.is_ok())
    }

    /// Ported from `SaveNpcLS`.
    pub async fn save_npc_ls(
        &self,
        chara_id: u32,
        npc_ls_id: u32,
        is_calling: bool,
        is_extra: bool,
    ) -> Result<()> {
        let mut conn = self.pool.get_conn().await?;
        r"INSERT INTO characters_npclinkshell (characterId, npcLinkshellId, isCalling, isExtra)
          VALUES (:cid, :lsid, :c, :e)
          ON DUPLICATE KEY UPDATE isCalling = :c, isExtra = :e"
            .with(params! {
                "cid" => chara_id, "lsid" => npc_ls_id,
                "c" => is_calling, "e" => is_extra,
            })
            .ignore(&mut conn)
            .await?;
        Ok(())
    }

    /// Ported from `SaveSupportTicket`. Returns `true` on error (matches C#).
    pub async fn save_support_ticket(
        &self,
        player_name: &str,
        title: &str,
        body: &str,
        lang_code: u32,
    ) -> Result<bool> {
        let mut conn = self.pool.get_conn().await?;
        let err = r"INSERT INTO supportdesk_tickets (name, title, body, langCode)
          VALUES (:name, :title, :body, :lang)"
            .with(params! { "name" => player_name, "title" => title, "body" => body, "lang" => lang_code })
            .ignore(&mut conn)
            .await
            .is_err();
        Ok(err)
    }

    /// Ported from `isTicketOpen`.
    pub async fn is_ticket_open(&self, player_name: &str) -> Result<bool> {
        let mut conn = self.pool.get_conn().await?;
        let row: Option<i64> = "SELECT isOpen FROM supportdesk_tickets WHERE name = :n"
            .with(params! { "n" => player_name })
            .first(&mut conn)
            .await?;
        Ok(row.unwrap_or(0) != 0)
    }

    /// Ported from `closeTicket`.
    pub async fn close_ticket(&self, player_name: &str) -> Result<()> {
        let mut conn = self.pool.get_conn().await?;
        "UPDATE supportdesk_tickets SET isOpen = 0 WHERE name = :n"
            .with(params! { "n" => player_name })
            .ignore(&mut conn)
            .await?;
        Ok(())
    }

    /// Ported from `getFAQNames`.
    pub async fn get_faq_names(&self, lang_code: u32) -> Result<Vec<String>> {
        let mut conn = self.pool.get_conn().await?;
        Ok("SELECT title FROM supportdesk_faqs WHERE languageCode = :l ORDER BY slot"
            .with(params! { "l" => lang_code })
            .map(&mut conn, |t: String| t)
            .await?)
    }

    /// Ported from `getFAQBody`.
    pub async fn get_faq_body(&self, slot: u32, lang_code: u32) -> Result<String> {
        let mut conn = self.pool.get_conn().await?;
        let row: Option<String> =
            "SELECT body FROM supportdesk_faqs WHERE slot = :s AND languageCode = :l"
                .with(params! { "s" => slot, "l" => lang_code })
                .first(&mut conn)
                .await?;
        Ok(row.unwrap_or_default())
    }

    /// Ported from `getIssues`.
    pub async fn get_issues(&self, _lang_code: u32) -> Result<Vec<String>> {
        // The C# original ignored the lang code on this one.
        let mut conn = self.pool.get_conn().await?;
        Ok("SELECT title FROM supportdesk_issues ORDER BY slot"
            .with(())
            .map(&mut conn, |t: String| t)
            .await?)
    }

    /// Ported from `IssuePlayerChocobo`.
    pub async fn issue_player_chocobo(
        &self,
        chara_id: u32,
        appearance_id: u8,
        name: &str,
    ) -> Result<()> {
        let mut conn = self.pool.get_conn().await?;
        r"INSERT INTO characters_chocobo (characterId, hasChocobo, chocoboAppearance, chocoboName)
          VALUES (:cid, 1, :app, :name)
          ON DUPLICATE KEY UPDATE hasChocobo = 1, chocoboAppearance = :app, chocoboName = :name"
            .with(params! { "cid" => chara_id, "app" => appearance_id, "name" => name })
            .ignore(&mut conn)
            .await?;
        Ok(())
    }

    /// Ported from `ChangePlayerChocoboAppearance`.
    pub async fn change_player_chocobo_appearance(
        &self,
        chara_id: u32,
        appearance_id: u8,
    ) -> Result<()> {
        let mut conn = self.pool.get_conn().await?;
        "UPDATE characters_chocobo SET chocoboAppearance = :app WHERE characterId = :cid"
            .with(params! { "app" => appearance_id, "cid" => chara_id })
            .ignore(&mut conn)
            .await?;
        Ok(())
    }

    /// Ported from `SavePlayerStatusEffects`.
    pub async fn save_player_status_effects(
        &self,
        chara_id: u32,
        effects: &[StatusEffectEntry],
    ) -> Result<()> {
        let mut conn = self.pool.get_conn().await?;
        let mut txn = conn.start_transaction(Default::default()).await?;
        for effect in effects {
            r"REPLACE INTO characters_statuseffect
                (characterId, statusId, magnitude, duration, tick, tier, extra)
              VALUES (:cid, :sid, :mag, :dur, :tick, :tier, :extra)"
                .with(params! {
                    "cid" => chara_id,
                    "sid" => effect.status_id,
                    "mag" => effect.magnitude,
                    "dur" => effect.duration,
                    "tick" => effect.tick,
                    "tier" => effect.tier,
                    "extra" => effect.extra,
                })
                .ignore(&mut txn)
                .await?;
        }
        txn.commit().await?;
        Ok(())
    }

    // =======================================================================
    // XP / level / retainer
    // =======================================================================

    /// Ported from `SetExp`.
    pub async fn set_exp(&self, chara_id: u32, class_id: u8, exp: i32) -> Result<()> {
        let Some(col) = class_column(class_id) else {
            return Ok(());
        };
        let mut conn = self.pool.get_conn().await?;
        let sql = format!("UPDATE characters_class_exp SET {col} = :exp WHERE characterId = :cid");
        sql.with(params! { "exp" => exp, "cid" => chara_id })
            .ignore(&mut conn)
            .await?;
        Ok(())
    }

    /// Ported from `SetLevel`.
    pub async fn set_level(&self, chara_id: u32, class_id: u8, level: i16) -> Result<()> {
        let Some(col) = class_column(class_id) else {
            return Ok(());
        };
        let mut conn = self.pool.get_conn().await?;
        let sql = format!("UPDATE characters_class_levels SET {col} = :lvl WHERE characterId = :cid");
        sql.with(params! { "lvl" => level, "cid" => chara_id })
            .ignore(&mut conn)
            .await?;
        Ok(())
    }

    /// Ported from `LoadRetainer`. Returns `(retainerId, name, actorClassId)`
    /// — the C# built a Retainer actor, we defer actor construction to the
    /// caller.
    pub async fn load_retainer(
        &self,
        chara_id: u32,
        retainer_index: i32,
    ) -> Result<Option<(u32, String, u32)>> {
        let mut conn = self.pool.get_conn().await?;
        let offset = (retainer_index - 1).max(0);
        let row: Option<(u32, String, u32)> = r"SELECT server_retainers.id AS retainerId,
                                                      server_retainers.name AS name,
                                                      actorClassId
                                               FROM characters_retainers
                                               INNER JOIN server_retainers
                                                   ON characters_retainers.retainerId = server_retainers.id
                                               WHERE characterId = :cid
                                               ORDER BY id
                                               LIMIT 1 OFFSET :off"
            .with(params! { "cid" => chara_id, "off" => offset })
            .first(&mut conn)
            .await?;
        Ok(row)
    }

    /// Ported from `PlayerCharacterUpdateClassLevel` — alias for `set_level`
    /// since the C# version simply dispatched the same query.
    pub async fn player_character_update_class_level(
        &self,
        chara_id: u32,
        class_id: u8,
        level: i16,
    ) -> Result<()> {
        self.set_level(chara_id, class_id, level).await
    }
}

// ---------------------------------------------------------------------------
// Row → DTO helpers.
// ---------------------------------------------------------------------------

fn inventory_item_from_row(mut row: Row) -> InventoryItem {
    let mut item = InventoryItem {
        unique_id: row.take("serverItemId").unwrap_or_default(),
        item_id: row.take("itemId").unwrap_or_default(),
        quantity: row.take("quantity").unwrap_or(1),
        quality: row.take("quality").unwrap_or(1),
        ..Default::default()
    };
    item.tag = ItemTag {
        durability: row.take("durability").unwrap_or_default(),
        main_quality: row.take("mainQuality").unwrap_or_default(),
        param1: row.take("param1").unwrap_or_default(),
        param2: row.take("param2").unwrap_or_default(),
        param3: row.take("param3").unwrap_or_default(),
        spiritbind: row.take("spiritbind").unwrap_or_default(),
        materia_id: row.take("materia1").unwrap_or_default(),
        ..Default::default()
    };
    item
}
