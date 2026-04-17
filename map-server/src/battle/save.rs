//! `BattleSave` — per-character persistent battle state. Ported from
//! `Actors/Chara/BattleSave.cs`. These survive logout/login and get
//! flushed back to the DB via the character-save hooks.

#![allow(dead_code)]

/// Number of class/skill slots — mirrors `short[52]` in the C#.
pub const NUM_SKILLS: usize = 52;

#[derive(Debug, Clone)]
pub struct BattleSave {
    pub potencial: f32,
    pub skill_level: [i16; NUM_SKILLS],
    pub skill_level_cap: [i16; NUM_SKILLS],
    pub skill_point: [i32; NUM_SKILLS],

    pub physical_level: i16,
    pub physical_exp: i32,

    pub negotiation_flag: [bool; 2],
}

impl Default for BattleSave {
    fn default() -> Self {
        Self {
            potencial: 6.6,
            skill_level: [0; NUM_SKILLS],
            skill_level_cap: [0; NUM_SKILLS],
            skill_point: [0; NUM_SKILLS],
            physical_level: 0,
            physical_exp: 0,
            negotiation_flag: [false; 2],
        }
    }
}
