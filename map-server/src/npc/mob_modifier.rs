//! `MobModifier` — per-BattleNpc behaviour tuning. Port of
//! `Actors/Chara/Npc/MobModifier.cs`. Values live inside
//! `MobModifierMap`, keyed by variant — mirrors the C# `Dict<MobMod, i64>`.

#![allow(dead_code)]

use std::collections::HashMap;

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum MobModifier {
    #[default]
    None = 0,
    /// How far can I move before I deaggro target.
    SpawnLeash = 1,
    /// XZ distance at which the mob can spot a target by sight.
    SightRange = 2,
    /// XZ distance at which the mob can spot a target by sound.
    SoundRange = 3,
    BuffChance = 4,
    HealChance = 5,
    SkillUseChance = 6,
    LinkRadius = 7,
    MagicDelay = 8,
    SpecialDelay = 9,
    ExpBonus = 10,
    /// Pursue target forever, ignoring spawn leash.
    IgnoreSpawnLeash = 11,
    /// Mob has a draw-in effect against the player.
    DrawIn = 12,
    HpScale = 13,
    /// Call for help if engaged.
    Assist = 14,
    /// Mob is immobile.
    NoMove = 15,
    /// Use this actor's id as target id for share-target mechanics.
    ShareTarget = 16,
    /// Fire Lua `onAttack` each swing.
    AttackScript = 17,
    /// Fire Lua `onDamageTaken`.
    DefendScript = 18,
    /// Fire Lua `onSpellCast` after a cast finishes.
    SpellScript = 19,
    /// Fire Lua `onWeaponSkill` after a weaponskill finishes.
    WeaponSkillScript = 20,
    /// Fire Lua `onAbility` after an ability finishes.
    AbilityScript = 21,
    /// Allow out-of-party actors to attack if the mob calls for help.
    CallForHelp = 22,
    /// Any actor can engage this mob (quest/event flag).
    FreeForAll = 23,
    /// Mob walks around while unengaged.
    Roams = 24,
    /// Delay (in seconds) between roam ticks.
    RoamDelay = 25,
    /// Was this mob aggroed through a link chain?
    Linked = 26,
    /// How many BattleNpcs got linked into the fight with me.
    LinkCount = 27,
}

impl MobModifier {
    /// u8 that matches the C# ordinal — used as the hash key and the DB id.
    pub fn as_u8(self) -> u8 {
        self as u8
    }

    pub fn from_u8(v: u8) -> Self {
        match v {
            1 => Self::SpawnLeash,
            2 => Self::SightRange,
            3 => Self::SoundRange,
            4 => Self::BuffChance,
            5 => Self::HealChance,
            6 => Self::SkillUseChance,
            7 => Self::LinkRadius,
            8 => Self::MagicDelay,
            9 => Self::SpecialDelay,
            10 => Self::ExpBonus,
            11 => Self::IgnoreSpawnLeash,
            12 => Self::DrawIn,
            13 => Self::HpScale,
            14 => Self::Assist,
            15 => Self::NoMove,
            16 => Self::ShareTarget,
            17 => Self::AttackScript,
            18 => Self::DefendScript,
            19 => Self::SpellScript,
            20 => Self::WeaponSkillScript,
            21 => Self::AbilityScript,
            22 => Self::CallForHelp,
            23 => Self::FreeForAll,
            24 => Self::Roams,
            25 => Self::RoamDelay,
            26 => Self::Linked,
            27 => Self::LinkCount,
            _ => Self::None,
        }
    }
}

/// Container — matches the C# `Dict<MobModifier, long>`. Missing keys
/// default to `0` on read.
#[derive(Debug, Clone, Default)]
pub struct MobModifierMap {
    entries: HashMap<MobModifier, i64>,
}

impl MobModifierMap {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn get(&self, m: MobModifier) -> i64 {
        self.entries.get(&m).copied().unwrap_or(0)
    }

    pub fn set(&mut self, m: MobModifier, v: i64) {
        self.entries.insert(m, v);
    }

    pub fn add(&mut self, m: MobModifier, delta: i64) {
        let cur = self.get(m);
        self.entries.insert(m, cur + delta);
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Merge another map over this one, with `other` taking precedence.
    /// Used to stack pool → genus → spawn modifier layers.
    pub fn overlay(&mut self, other: &MobModifierMap) {
        for (k, v) in &other.entries {
            self.entries.insert(*k, *v);
        }
    }

    pub fn iter(&self) -> impl Iterator<Item = (&MobModifier, &i64)> {
        self.entries.iter()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mob_modifier_roundtrip() {
        for i in 0u8..28 {
            let m = MobModifier::from_u8(i);
            assert_eq!(m.as_u8(), i);
        }
    }

    #[test]
    fn mob_modifier_map_defaults_to_zero() {
        let map = MobModifierMap::new();
        assert_eq!(map.get(MobModifier::SightRange), 0);
    }

    #[test]
    fn mob_modifier_map_overlay() {
        let mut a = MobModifierMap::new();
        a.set(MobModifier::SightRange, 20);
        a.set(MobModifier::RoamDelay, 5);
        let mut b = MobModifierMap::new();
        b.set(MobModifier::SightRange, 35);
        a.overlay(&b);
        assert_eq!(a.get(MobModifier::SightRange), 35);
        assert_eq!(a.get(MobModifier::RoamDelay), 5);
    }
}
