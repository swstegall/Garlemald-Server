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

//! Gamedata DTOs referenced by the Database port. These mirror the subset of
//! C# types that cross the DB boundary (BattleCommand / BattleTrait /
//! StatusEffect / …). Rich in-memory behavior (AI state, cooldown timers,
//! combat math) lives in future modules; these are plain value types.

#![allow(dead_code)]

use crate::data::InventoryItem;

// ---------------------------------------------------------------------------
// Class identifiers. Values match the C# `Player.CLASSID_*` constants and
// the column names on the characters_class_levels / _exp tables.
// ---------------------------------------------------------------------------

pub const CLASSID_PUG: u8 = 2;
pub const CLASSID_GLA: u8 = 3;
pub const CLASSID_MRD: u8 = 4;
pub const CLASSID_ARC: u8 = 7;
pub const CLASSID_LNC: u8 = 8;
pub const CLASSID_THM: u8 = 22;
pub const CLASSID_CNJ: u8 = 23;
pub const CLASSID_CRP: u8 = 29;
pub const CLASSID_BSM: u8 = 30;
pub const CLASSID_ARM: u8 = 31;
pub const CLASSID_GSM: u8 = 32;
pub const CLASSID_LTW: u8 = 33;
pub const CLASSID_WVR: u8 = 34;
pub const CLASSID_ALC: u8 = 35;
pub const CLASSID_CUL: u8 = 36;
pub const CLASSID_MIN: u8 = 39;
pub const CLASSID_BTN: u8 = 40;
pub const CLASSID_FSH: u8 = 41;

/// All 18 class column names in the `characters_class_levels` / `_exp` tables,
/// in the same order the C# Player.LOAD_PLAYER code reads them.
pub const CLASS_COLUMNS: &[&str] = &[
    "pug", "gla", "mrd", "arc", "lnc", "thm", "cnj", "crp", "bsm", "arm", "gsm", "ltw", "wvr",
    "alc", "cul", "min", "btn", "fsh",
];

/// Map a class id to the DB column name, matching `CharacterUtils.GetClassNameForId`.
pub fn class_column(class_id: u8) -> Option<&'static str> {
    Some(match class_id {
        CLASSID_PUG => "pug",
        CLASSID_GLA => "gla",
        CLASSID_MRD => "mrd",
        CLASSID_ARC => "arc",
        CLASSID_LNC => "lnc",
        CLASSID_THM => "thm",
        CLASSID_CNJ => "cnj",
        CLASSID_CRP => "crp",
        CLASSID_BSM => "bsm",
        CLASSID_ARM => "arm",
        CLASSID_GSM => "gsm",
        CLASSID_LTW => "ltw",
        CLASSID_WVR => "wvr",
        CLASSID_ALC => "alc",
        CLASSID_CUL => "cul",
        CLASSID_MIN => "min",
        CLASSID_BTN => "btn",
        CLASSID_FSH => "fsh",
        _ => return None,
    })
}

pub const TIMER_COLUMNS: &[&str] = &[
    "thousandmaws",
    "dzemaeldarkhold",
    "bowlofembers_hard",
    "bowlofembers",
    "thornmarch",
    "aurumvale",
    "cutterscry",
    "battle_aleport",
    "battle_hyrstmill",
    "battle_goldenbazaar",
    "howlingeye_hard",
    "howlingeye",
    "castrumnovum",
    "bowlofembers_extreme",
    "rivenroad",
    "rivenroad_hard",
    "behests",
    "companybehests",
    "returntimer",
    "skirmish",
];

// ---------------------------------------------------------------------------
// Mirror of the C# `Player` sub-structs that the DB load populates.
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct CharaBattleSave {
    /// Indexed by class id − 1. One entry per class column above.
    pub skill_level: [i16; 42],
    pub skill_point: [i32; 42],
}

// Can't `#[derive(Default)]`: arrays longer than 32 elements aren't covered
// by std's auto-derived Default impl.
#[allow(clippy::derivable_impls)]
impl Default for CharaBattleSave {
    fn default() -> Self {
        Self {
            skill_level: [0; 42],
            skill_point: [0; 42],
        }
    }
}

#[derive(Debug, Clone)]
pub struct CharaParameterSave {
    pub hp: [i16; 4],
    pub hp_max: [i16; 4],
    pub mp: i16,
    pub mp_max: i16,
    pub state_main_skill: [u8; 1],
    pub state_main_skill_level: i16,
    pub command_slot_recast_time: [u32; 32],
}

// Same reason as CharaBattleSave above.
#[allow(clippy::derivable_impls)]
impl Default for CharaParameterSave {
    fn default() -> Self {
        Self {
            hp: [0; 4],
            hp_max: [0; 4],
            mp: 0,
            mp_max: 0,
            state_main_skill: [0; 1],
            state_main_skill_level: 0,
            command_slot_recast_time: [0; 32],
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct PlayerWorkSave {
    pub guardian: u8,
    pub birthday_day: u8,
    pub birthday_month: u8,
    pub initial_town: u8,
    pub tribe: u8,
    pub rest_bonus_exp_rate: i32,
    pub quest_scenario: [u32; 16],
    pub quest_guildleve: [u32; 8],
    pub npc_linkshell_chat_calling: Vec<bool>,
    pub npc_linkshell_chat_extra: Vec<bool>,
}

#[derive(Debug, Clone, Default)]
pub struct AppearanceFull {
    pub base_id: u32,
    pub size: u8,
    pub voice: u8,
    pub skin_color: u16,
    pub hair_style: u16,
    pub hair_color: u16,
    pub hair_highlight_color: u16,
    pub hair_variation: u16,
    pub eye_color: u16,
    pub characteristics: u8,
    pub characteristics_color: u8,
    pub face_type: u8,
    pub ears: u8,
    pub face_mouth: u8,
    pub face_features: u8,
    pub face_nose: u8,
    pub face_eye_shape: u8,
    pub face_iris_size: u8,
    pub face_eyebrows: u8,
    pub main_hand: u32,
    pub off_hand: u32,
    pub head: u32,
    pub body: u32,
    pub legs: u32,
    pub hands: u32,
    pub feet: u32,
    pub waist: u32,
    pub neck: u32,
    pub left_finger: u32,
    pub right_finger: u32,
    pub left_ear: u32,
    pub right_ear: u32,
}

impl AppearanceFull {
    /// Pack the raw DB fields into the 28-slot table the client expects in
    /// `SetActorAppearancePacket` (opcode 0x00D6). Layout mirrors the C#
    /// `Database.LoadCharacter` → `player.appearanceIds[...]` assignments:
    /// slot 0 is SIZE; slot 1 packs skin/hair/eye color; slot 2 packs face
    /// features (characteristics, type, ears, mouth, features, nose,
    /// eye-shape, iris-size, eyebrows, each one byte wide in appearance
    /// order); slot 3 packs hair highlight/variation/style; slot 4 is VOICE;
    /// slots 5+ are weapon/gear slots in the enum order defined on the
    /// C# packet class.
    pub fn to_slot_ids(&self) -> [u32; 28] {
        let mut ids = [0u32; 28];
        ids[0] = self.size as u32; // SIZE
        ids[1] = (self.skin_color as u32)
            | ((self.hair_color as u32) << 10)
            | ((self.eye_color as u32) << 20); // COLORINFO
        ids[2] = pack_face_info(
            self.characteristics,
            self.characteristics_color,
            self.face_type,
            self.ears,
            self.face_mouth,
            self.face_features,
            self.face_nose,
            self.face_eye_shape,
            self.face_iris_size,
            self.face_eyebrows,
        ); // FACEINFO
        ids[3] = (self.hair_highlight_color as u32)
            | ((self.hair_variation as u32) << 5)
            | ((self.hair_style as u32) << 10); // HIGHLIGHT_HAIR
        ids[4] = self.voice as u32; // VOICE
        ids[5] = self.main_hand; // MAINHAND
        ids[6] = self.off_hand; // OFFHAND
        // 7..11 unused in the load path (SPMAINHAND..POUCH)
        ids[12] = self.head; // HEADGEAR
        ids[13] = self.body; // BODYGEAR
        ids[14] = self.legs; // LEGSGEAR
        ids[15] = self.hands; // HANDSGEAR
        ids[16] = self.feet; // FEETGEAR
        ids[17] = self.waist; // WAISTGEAR
        ids[18] = self.neck; // NECKGEAR
        ids[19] = self.left_ear; // L_EAR
        ids[20] = self.right_ear; // R_EAR
        ids[23] = self.right_finger; // R_RINGFINGER
        ids[24] = self.left_finger; // L_RINGFINGER
        ids
    }

    /// Resolve the player's model id. C# falls back to `GetTribeModel(tribe)`
    /// when `baseId == 0xFFFFFFFF` — the sentinel used by the lobby server
    /// to mean "use tribe-default model". For now we return a fixed
    /// Hyur-Midlander-male model when the sentinel is set; a later pass
    /// can table-drive the full tribe mapping.
    pub fn resolve_model_id(&self, tribe: u8) -> u32 {
        if self.base_id == 0xFFFFFFFF {
            tribe_default_model(tribe)
        } else {
            self.base_id
        }
    }
}

/// Pack the 10 face-feature bytes into a single u32 bitfield. Mirrors the
/// C# `CharacterUtils.FaceInfo` struct packed by `PrimitiveConversion.
/// ToUInt32` (Common Class Lib/Bitfield.cs). Bit layout (LSB-first):
///   characteristics       5 bits (0..5)
///   characteristicsColor  3 bits (5..8)
///   type                  6 bits (8..14)
///   ears                  2 bits (14..16)
///   mouth                 2 bits (16..18)
///   features              2 bits (18..20)
///   nose                  3 bits (20..23)
///   eyeShape              3 bits (23..26)
///   irisSize              1 bit  (26..27)
///   eyebrows              3 bits (27..30)
///   unknown               2 bits (30..32)  — left zero
/// Previous port byte-packed just the first four fields, which sent the
/// client a face_type of 0 in almost every slot and left mouth/nose/
/// eye-shape/eyebrows zero — the avatar renderer read those as "no
/// face" and drew a mannequin head (no eyes, no mouth).
pub(crate) fn pack_face_info(
    characteristics: u8,
    characteristics_color: u8,
    face_type: u8,
    ears: u8,
    face_mouth: u8,
    face_features: u8,
    face_nose: u8,
    face_eye_shape: u8,
    face_iris_size: u8,
    face_eyebrows: u8,
) -> u32 {
    (characteristics as u32 & 0x1F)
        | ((characteristics_color as u32 & 0x7) << 5)
        | ((face_type as u32 & 0x3F) << 8)
        | ((ears as u32 & 0x3) << 14)
        | ((face_mouth as u32 & 0x3) << 16)
        | ((face_features as u32 & 0x3) << 18)
        | ((face_nose as u32 & 0x7) << 20)
        | ((face_eye_shape as u32 & 0x7) << 23)
        | ((face_iris_size as u32 & 0x1) << 26)
        | ((face_eyebrows as u32 & 0x7) << 27)
}

/// Fallback model id when the DB stores the `baseId = 0xFFFFFFFF` sentinel.
/// Full port of C# `CharacterUtils.GetTribeModel` — model ids are tiny
/// integers (1..9) that index into the client's player-race model table.
/// The earlier stub returned 0x10000001 for every tribe, which is a
/// non-existent model id; the client couldn't resolve it so no player
/// avatars rendered, including the player's own character.
fn tribe_default_model(tribe: u8) -> u32 {
    match tribe {
        // Hyur Midlander Male
        1 => 1,
        // Hyur Midlander Female
        2 => 2,
        // Hyur Highlander Male
        3 => 9,
        // Elezen Male (Wildwood, Duskwight)
        4 | 6 => 3,
        // Elezen Female
        5 | 7 => 4,
        // Lalafell Male (Plainsfolk, Dunesfolk)
        8 | 10 => 5,
        // Lalafell Female
        9 | 11 => 6,
        // Miqo'te Female (Seeker, Keeper)
        12 | 13 => 8,
        // Roegadyn Male (Sea Wolves, Hellsguard)
        14 | 15 => 7,
        // Unknown tribe — fall back to Hyur Midlander Male so we still
        // send a renderable avatar rather than a client-nil lookup.
        _ => 1,
    }
}

#[derive(Debug, Clone, Default)]
pub struct StatusEffectEntry {
    pub status_id: u32,
    pub duration: u32,
    pub magnitude: u64,
    pub tick: u32,
    pub tier: u8,
    pub extra: u64,
}

#[derive(Debug, Clone, Default)]
pub struct ChocoboData {
    pub has_chocobo: bool,
    pub has_goobbue: bool,
    pub chocobo_appearance: u8,
    pub chocobo_name: String,
}

/// One row of `gamedata_quests` — the metadata catalog ported from
/// Meteor's `origin/ioncannon/quest_system` seed. Keyed by quest id
/// (110_001..=112_048 in 1.23b). `class_name` is the Pascal-case code
/// used in the Lua path (e.g. `"Man0l0"` → `scripts/lua/quests/man/man0l0.lua`).
#[derive(Debug, Clone, Default)]
pub struct QuestMeta {
    pub id: u32,
    pub quest_name: String,
    pub class_name: String,
    pub prerequisite: u32,
    pub min_level: u16,
}

/// One row hydrated from `characters_quest_scenario` post-redesign.
/// The free-form `questData` JSON blob is gone; runtime state lives in
/// `flags` + three 16-bit counters and the phase counter has been
/// renamed to `sequence`.
#[derive(Debug, Clone, Default)]
pub struct QuestScenarioEntry {
    pub slot: u16,
    pub quest_id: u32,
    pub sequence: u32,
    pub flags: u32,
    pub counter1: u16,
    pub counter2: u16,
    pub counter3: u16,
    /// Per-quest NpcLs scratchpad (migration 050). 0 = no chain
    /// active. See `actor::quest::QuestData::npc_ls_from`.
    pub npc_ls_from: u32,
    pub npc_ls_msg_step: u8,
}

#[derive(Debug, Clone, Default)]
pub struct GuildleveLocalEntry {
    pub slot: u16,
    pub quest_id: u32,
    pub abandoned: bool,
    pub completed: bool,
}

#[derive(Debug, Clone, Default)]
pub struct GuildleveRegionalEntry {
    pub slot: u16,
    pub guildleve_id: u16,
    pub abandoned: bool,
    pub completed: bool,
}

#[derive(Debug, Clone, Default)]
pub struct NpcLinkshellEntry {
    pub npc_ls_id: u16,
    pub is_calling: bool,
    pub is_extra: bool,
}

#[derive(Debug, Clone, Default)]
pub struct HotbarEntry {
    pub hotbar_slot: u16,
    pub command_id: u32,
    pub recast_time: u32,
}

#[derive(Debug, Clone, Default)]
pub struct EquipmentSlot {
    pub equip_slot: u16,
    pub item_id: u64,
}

/// Aggregate returned by `Database::load_player_character`. Rolls up every
/// query that LoadPlayerCharacter runs in C# into a single DTO.
#[derive(Debug, Clone, Default)]
pub struct LoadedPlayer {
    pub name: String,
    pub position_x: f32,
    pub position_y: f32,
    pub position_z: f32,
    pub rotation: f32,
    pub actor_state: u16,
    pub current_zone_id: u32,
    pub gc_current: u8,
    pub gc_limsa_rank: u8,
    pub gc_gridania_rank: u8,
    pub gc_uldah_rank: u8,
    pub current_title: u32,
    pub guardian: u8,
    pub birth_day: u8,
    pub birth_month: u8,
    pub initial_town: u8,
    pub tribe: u8,
    pub rest_bonus_exp_rate: i32,
    pub achievement_points: u32,
    pub play_time: u32,
    pub destination_zone_id: u32,
    pub destination_spawn_type: u8,
    pub current_private_area: String,
    pub current_private_area_type: u32,
    pub homepoint: u32,
    pub homepoint_inn: u8,

    /// Migration-051 SNpc / Path Companion scratchpad. See the
    /// per-field doc on `actor::CharaState::snpc_*`.
    pub snpc_nickname: String,
    pub snpc_skin: u8,
    pub snpc_personality: u8,
    pub snpc_coordinate: i16,

    pub class_levels: CharaBattleSave,
    pub parameter_save: CharaParameterSave,
    pub appearance: AppearanceFull,
    pub status_effects: Vec<StatusEffectEntry>,
    pub chocobo: ChocoboData,
    pub timers: [u32; 20],
    pub hotbar: Vec<HotbarEntry>,
    pub quest_scenario: Vec<QuestScenarioEntry>,
    pub guildleves_local: Vec<GuildleveLocalEntry>,
    pub guildleves_regional: Vec<GuildleveRegionalEntry>,
    pub npc_linkshells: Vec<NpcLinkshellEntry>,

    pub inventory_normal: Vec<InventoryItem>,
    pub inventory_key_items: Vec<InventoryItem>,
    pub inventory_currency: Vec<InventoryItem>,
    pub inventory_bazaar: Vec<InventoryItem>,
    pub inventory_meldrequest: Vec<InventoryItem>,
    pub inventory_loot: Vec<InventoryItem>,

    pub equipment: Vec<EquipmentSlot>,
}

// ---------------------------------------------------------------------------
// Gamedata shells. These were loaded into large dictionaries at server
// startup in the C# original. The fields capture every column touched by
// the C# row reader so the subsequent battle/status/item code has full
// access to the values.
// ---------------------------------------------------------------------------

pub const ITEM_PACKAGE_NORMAL: u32 = 0;
pub const ITEM_PACKAGE_KEY_ITEMS: u32 = 1;
pub const ITEM_PACKAGE_CURRENCY_CRYSTALS: u32 = 2;
pub const ITEM_PACKAGE_BAZAAR: u32 = 3;
pub const ITEM_PACKAGE_MELDREQUEST: u32 = 4;
pub const ITEM_PACKAGE_LOOT: u32 = 5;

#[derive(Debug, Clone, Default)]
pub struct BattleCommand {
    pub id: u16,
    pub name: String,
    pub job: u8,
    pub level: u8,
    pub requirements: u16,
    pub main_target: u16,
    pub valid_target: u16,
    pub aoe_type: u8,
    pub base_potency: u16,
    pub num_hits: u8,
    pub position_bonus: u8,
    pub proc_requirement: u8,
    pub range: f32,
    pub min_range: f32,
    pub range_height: i32,
    pub range_width: i32,
    pub status_id: u32,
    pub status_duration: u32,
    pub status_chance: f32,
    pub cast_type: u8,
    pub cast_time_ms: u32,
    pub max_recast_time_seconds: u32,
    pub recast_time_ms: u32,
    pub mp_cost: i16,
    pub tp_cost: i16,
    pub animation_type: u8,
    pub effect_animation: u16,
    pub model_animation: u16,
    pub animation_duration_seconds: u16,
    pub aoe_range: f32,
    pub aoe_min_range: f32,
    pub aoe_cone_angle: f32,
    pub aoe_rotate_angle: f32,
    pub aoe_target: u8,
    pub battle_animation: u32,
    pub valid_user: u8,
    pub combo_next_command_id: [i32; 2],
    pub combo_step: i16,
    pub command_type: i16,
    pub action_property: i16,
    pub action_type: i16,
    pub accuracy_modifier: f32,
    pub world_master_text_id: u16,
}

#[derive(Debug, Clone, Default)]
pub struct BattleTrait {
    pub id: u16,
    pub name: String,
    pub job: u8,
    pub level: u8,
    pub modifier: u32,
    pub bonus: i32,
}

#[derive(Debug, Clone, Default)]
pub struct StatusEffectDef {
    pub id: u32,
    pub name: String,
    pub flags: u32,
    pub overwrite: u8,
    pub tick_ms: u32,
    pub hidden: bool,
    pub silent_on_gain: bool,
    pub silent_on_loss: bool,
    pub status_gain_text_id: u16,
    pub status_loss_text_id: u16,
}

#[derive(Debug, Clone, Default)]
pub struct GuildleveGamedata {
    pub id: u32,
    pub zone_id: u32,
    pub name: String,
    pub difficulty: u8,
    pub leve_type: u8,
    pub reward_exp: u32,
    pub reward_gil: u32,
}

// Gamedata row types (ItemData, InventoryItem) live in `crate::data`; import
// from there at call sites.

/// Wire-shape modifier block persisted alongside inventory rows.
#[derive(Debug, Clone, Default)]
pub struct ItemModifiers {
    pub durability: u32,
    pub main_quality: u8,
    pub sub_quality: [u8; 3],
    pub param: [u32; 3],
    pub spiritbind: u16,
    pub materia: [u32; 5],
}

/// Bazaar / mail-attachment dealing info (server_items_dealing table).
#[derive(Debug, Clone, Default)]
pub struct ItemDealingInfo {
    pub dealing_value: u32,
    pub dealing_mode: u8,
    pub dealing_attached: [u64; 3],
    pub dealing_tag: u64,
    pub bazaar_mode: u8,
}
