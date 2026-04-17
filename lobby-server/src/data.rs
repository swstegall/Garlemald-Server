//! Lobby data objects ported from project-meteor-mirror/Lobby Server/DataObjects.
//!
//! Several fields are read/written only by the DB layer or packet builders
//! (never both) — we `#[allow(dead_code)]` those rather than prune them, since
//! they are part of the wire schema the client expects.
#![allow(dead_code)]

#[derive(Debug, Clone, Default)]
pub struct Account {
    pub id: u32,
    pub name: String,
}

#[derive(Debug, Clone)]
pub struct World {
    pub id: u16,
    pub address: String,
    pub port: u16,
    pub list_position: u16,
    pub population: u16,
    pub name: String,
    pub is_active: bool,
}

#[derive(Debug, Clone)]
pub struct Retainer {
    pub id: u32,
    pub character_id: u32,
    pub name: String,
    pub do_rename: bool,
}

#[derive(Debug, Clone, Default)]
pub struct Character {
    pub id: u32,
    pub slot: u16,
    pub server_id: u16,
    pub name: String,
    pub state: u16,
    pub is_legacy: bool,
    pub do_rename: bool,
    pub current_zone_id: u32,
    pub guardian: u8,
    pub birth_month: u8,
    pub birth_day: u8,
    pub current_class: u32,
    pub current_job: u32,
    pub current_level: i32,
    pub initial_town: u8,
    pub tribe: u8,
}


#[derive(Debug, Clone, Default)]
pub struct Appearance {
    pub size: u8,
    pub voice: u8,
    pub skin_color: u16,

    pub hair_style: u16,
    pub hair_color: u16,
    pub hair_highlight_color: u16,
    pub hair_variation: u16,
    pub eye_color: u16,
    pub characteristics_color: u8,

    pub face_type: u8,
    pub face_eyebrows: u8,
    pub face_eye_shape: u8,
    pub face_iris_size: u8,
    pub face_nose: u8,
    pub face_mouth: u8,
    pub face_features: u8,
    pub characteristics: u8,
    pub ears: u8,

    pub main_hand: u32,
    pub off_hand: u32,

    pub head: u32,
    pub body: u32,
    pub legs: u32,
    pub hands: u32,
    pub feet: u32,
    pub waist: u32,
    pub neck: u32,
    pub right_ear: u32,
    pub left_ear: u32,
    pub right_index: u32,
    pub left_index: u32,
    pub right_finger: u32,
    pub left_finger: u32,
}

/// Bitfield layout used by `CharaInfo.BuildForCharaList` to pack face info
/// into a single u32. Total width is 32 bits.
#[derive(Debug, Clone, Copy, Default)]
pub struct FaceInfo {
    pub characteristics: u32,        // 5 bits
    pub characteristics_color: u32,  // 3 bits
    pub face_type: u32,              // 6 bits
    pub ears: u32,                   // 2 bits
    pub mouth: u32,                  // 2 bits
    pub features: u32,               // 2 bits
    pub nose: u32,                   // 3 bits
    pub eye_shape: u32,              // 3 bits
    pub iris_size: u32,              // 1 bit
    pub eyebrows: u32,               // 3 bits
    pub unknown: u32,                // 2 bits
}

impl FaceInfo {
    pub fn to_u32(self) -> u32 {
        common::bitfield::pack_u32(&[
            (self.characteristics, 5),
            (self.characteristics_color, 3),
            (self.face_type, 6),
            (self.ears, 2),
            (self.mouth, 2),
            (self.features, 2),
            (self.nose, 3),
            (self.eye_shape, 3),
            (self.iris_size, 1),
            (self.eyebrows, 3),
            (self.unknown, 2),
        ])
    }
}

#[derive(Debug, Clone, Default)]
pub struct CharaInfo {
    pub appearance: Appearance,
    pub guardian: u32,
    pub birth_month: u32,
    pub birth_day: u32,
    pub current_class: u32,
    pub current_job: u32,
    pub initial_town: u32,
    pub tribe: u32,

    pub zone_id: u16,
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub rot: f32,

    pub current_level: u32,

    pub weapon1: u32,
    pub weapon2: u32,
    pub head: u32,
    pub body: u32,
    pub hands: u32,
    pub legs: u32,
    pub feet: u32,
    pub belt: u32,
}

/// Map tribe id → base racial model id, matching `CharaInfo.GetTribeModel`.
pub fn get_tribe_model(tribe: u8) -> u32 {
    match tribe {
        2 => 2,            // Hyur Midlander Female
        4 | 6 => 3,        // Elezen Male
        5 | 7 => 4,        // Elezen Female
        8 | 10 => 5,       // Lalafell Male
        9 | 11 => 6,       // Lalafell Female
        12 | 13 => 8,      // Miqo'te Female
        14 | 15 => 7,      // Roegadyn Male
        3 => 9,            // Hyur Highlander Male
        _ => 1,            // Hyur Midlander Male (default)
    }
}

pub mod chara_info;
