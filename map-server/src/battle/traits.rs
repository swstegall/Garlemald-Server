//! `BattleTrait` — passive bonus granted by class/level. Ported from
//! `Actors/Chara/Ai/BattleTrait.cs`.

#![allow(dead_code)]

#[derive(Debug, Clone)]
pub struct BattleTrait {
    pub id: u16,
    pub name: String,
    pub job: u8,
    pub level: u8,
    /// `Modifier` ordinal granted by this trait — matches the u32 key used
    /// inside `ModifierMap`.
    pub modifier: u32,
    pub bonus: i32,
}

impl BattleTrait {
    pub fn new(id: u16, name: impl Into<String>, job: u8, level: u8, modifier: u32, bonus: i32) -> Self {
        Self { id, name: name.into(), job, level, modifier, bonus }
    }
}
