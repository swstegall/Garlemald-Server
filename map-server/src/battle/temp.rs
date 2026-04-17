//! `BattleTemp` — transient per-character battle state. Ported from
//! `Actors/Chara/BattleTemp.cs`. These are the fields the client expects
//! to see in `charaWork` during combat but that we never persist.

#![allow(dead_code)]

// Same STAT_* indices as global.lua / BattleTemp.cs. Duplicated here for
// parity with the C# enum; real callers generally use the ones defined in
// `crate::actor::chara` (STAT_COUNT etc.).
pub const NAMEPLATE_SHOWN: u32 = 0;
pub const TARGETABLE: u32 = 1;
pub const NAMEPLATE_SHOWN2: u32 = 2;

pub const STAT_STRENGTH: u32 = 3;
pub const STAT_VITALITY: u32 = 4;
pub const STAT_DEXTERITY: u32 = 5;
pub const STAT_INTELLIGENCE: u32 = 6;
pub const STAT_MIND: u32 = 7;
pub const STAT_PIETY: u32 = 8;

pub const STAT_RESISTANCE_FIRE: u32 = 9;
pub const STAT_RESISTANCE_ICE: u32 = 10;
pub const STAT_RESISTANCE_WIND: u32 = 11;
pub const STAT_RESISTANCE_LIGHTNING: u32 = 12;
pub const STAT_RESISTANCE_EARTH: u32 = 13;
pub const STAT_RESISTANCE_WATER: u32 = 14;

pub const STAT_ACCURACY: u32 = 15;
pub const STAT_EVASION: u32 = 16;
pub const STAT_ATTACK: u32 = 17;
pub const STAT_NORMALDEFENSE: u32 = 18;

pub const STAT_ATTACK_MAGIC: u32 = 23;
pub const STAT_HEAL_MAGIC: u32 = 24;
pub const STAT_ENCHANCEMENT_MAGIC_POTENCY: u32 = 25;
pub const STAT_ENFEEBLING_MAGIC_POTENCY: u32 = 26;

pub const STAT_MAGIC_ACCURACY: u32 = 27;
pub const STAT_MAGIC_EVASION: u32 = 28;

pub const STAT_CRAFT_PROCESSING: u32 = 30;
pub const STAT_CRAFT_MAGIC_PROCESSING: u32 = 31;
pub const STAT_CRAFT_PROCESS_CONTROL: u32 = 32;

pub const STAT_HARVEST_POTENCY: u32 = 33;
pub const STAT_HARVEST_LIMIT: u32 = 34;
pub const STAT_HARVEST_RATE: u32 = 35;

#[derive(Debug, Clone)]
pub struct BattleTemp {
    /// Speed multipliers for the cast-gauge animation: index 0 = normal,
    /// index 1 = slow.
    pub cast_gauge_speed: [f32; 2],
    /// Per-timing-command flags (4 slots, matches the C# `bool[4]`).
    pub timing_command_flag: [bool; 4],
    /// `generalParameter[35]` — transient stat window the script engine
    /// reads/writes for effect tiers.
    pub general_parameter: [i16; 35],
}

impl Default for BattleTemp {
    fn default() -> Self {
        Self {
            cast_gauge_speed: [1.0, 0.25],
            timing_command_flag: [false; 4],
            general_parameter: [0; 35],
        }
    }
}
