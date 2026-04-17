//! Stat-modifier enum, ported 1:1 from `Actors/Chara/Modifier.cs`.
//!
//! The C# server treats mods as a `Dictionary<uint, double>` keyed by this
//! enum; we do the same with `HashMap<Modifier, f64>`. Callers can also key
//! by raw u32 for the wire-format accessors (`GetMod(uint)`).

#![allow(dead_code)]

use std::collections::HashMap;

/// Every mod id the client / scripts can reference. Values match the u32
/// wire ids.
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Modifier {
    // Health
    Hp = 0,
    Mp = 1,
    Tp = 2,

    // Main stats
    Strength = 3,
    Vitality = 4,
    Dexterity = 5,
    Intelligence = 6,
    Mind = 7,
    Piety = 8,

    // Elemental resistances
    FireResistance = 9,
    IceResistance = 10,
    WindResistance = 11,
    EarthResistance = 12,
    LightningResistance = 13,
    WaterResistance = 14,

    // Physical secondary
    Accuracy = 15,
    Evasion = 16,
    Attack = 17,
    Defense = 18,

    // Physical crit
    CriticalHitRating = 19,
    CriticalHitEvasion = 20,
    CriticalHitAttackPower = 21,
    CriticalHitResilience = 22,

    // Magic secondary
    AttackMagicPotency = 23,
    HealingMagicPotency = 24,
    EnhancementMagicPotency = 25,
    EnfeeblingMagicPotency = 26,
    MagicAccuracy = 27,
    MagicEvasion = 28,

    // Crafting
    Craftsmanship = 29,
    MagicCraftsmanship = 30,
    Control = 31,
    Gathering = 32,
    Output = 33,
    Perception = 34,

    // Magic crit
    MagicCriticalHitRating = 35,
    MagicCriticalHitEvasion = 36,
    MagicCriticalHitPotency = 37,
    MagicCriticalHitResilience = 38,

    // Block / parry
    Parry = 39,
    BlockRate = 40,
    Block = 41,

    // Elemental potencies
    FireMagicPotency = 42,
    IceMagicPotency = 43,
    WindMagicPotency = 44,
    EarthMagicPotency = 45,
    LightningMagicPotency = 46,
    WaterMagicPotency = 47,

    // Misc
    Regen = 48,
    Refresh = 49,
    StoreTp = 50,
    Enmity = 51,
    Spikes = 52,
    Haste = 53,
    ReducedDurabilityLoss = 56,
    IncreasedSpiritbondGain = 57,
    Damage = 58,
    Delay = 59,
    Fastcast = 60,
    MovementSpeed = 61,
    Exp = 62,
    RestingHp = 63,
    RestingMp = 64,

    // Attack property resistances
    SlashingResistance = 65,
    PiercingResistance = 66,
    BluntResistance = 67,
    ProjectileResistance = 68,
    SonicResistance = 69,
    BreathResistance = 70,
    PhysicalResistance = 71,
    MagicResistance = 72,

    // Status resistances
    SlowResistance = 73,
    PetrificationResistance = 74,
    ParalysisResistance = 75,
    SilenceResistance = 76,
    BlindResistance = 77,
    PoisonResistance = 78,
    StunResistance = 79,
    SleepResistance = 80,
    BindResistance = 81,
    HeavyResistance = 82,
    DoomResistance = 83,

    // More misc
    ConserveMp = 101,
    SpellInterruptResistance = 102,
    DoubleDownOdds = 103,
    HqDiscoveryRate = 104,

    // Non-gear
    None = 105,
    NameplateShown = 106,
    Targetable = 107,
    NameplateShown2 = 108,

    HpPercent = 109,
    MpPercent = 110,
    TpPercent = 111,

    AttackRange = 112,

    Raise = 113,
    MinimumHpLock = 114,
    MinimumMpLock = 115,
    MinimumTpLock = 116,
    AttackType = 117,
    CanBlock = 118,
    HitCount = 119,

    RawEvadeRate = 120,
    RawParryRate = 121,
    RawBlockRate = 122,
    RawResistRate = 123,
    RawHitRate = 124,
    RawCritRate = 125,

    DamageTakenDown = 126,
    Regain = 127,
    RegenDown = 128,
    Stoneskin = 129,
    KnockbackImmune = 130,
    Stealth = 131,
}

impl Modifier {
    pub fn from_u32(value: u32) -> Option<Self> {
        Some(match value {
            0 => Self::Hp,
            1 => Self::Mp,
            2 => Self::Tp,
            3 => Self::Strength,
            4 => Self::Vitality,
            5 => Self::Dexterity,
            6 => Self::Intelligence,
            7 => Self::Mind,
            8 => Self::Piety,
            9 => Self::FireResistance,
            10 => Self::IceResistance,
            11 => Self::WindResistance,
            12 => Self::EarthResistance,
            13 => Self::LightningResistance,
            14 => Self::WaterResistance,
            15 => Self::Accuracy,
            16 => Self::Evasion,
            17 => Self::Attack,
            18 => Self::Defense,
            19 => Self::CriticalHitRating,
            20 => Self::CriticalHitEvasion,
            21 => Self::CriticalHitAttackPower,
            22 => Self::CriticalHitResilience,
            23 => Self::AttackMagicPotency,
            24 => Self::HealingMagicPotency,
            25 => Self::EnhancementMagicPotency,
            26 => Self::EnfeeblingMagicPotency,
            27 => Self::MagicAccuracy,
            28 => Self::MagicEvasion,
            29 => Self::Craftsmanship,
            30 => Self::MagicCraftsmanship,
            31 => Self::Control,
            32 => Self::Gathering,
            33 => Self::Output,
            34 => Self::Perception,
            35 => Self::MagicCriticalHitRating,
            36 => Self::MagicCriticalHitEvasion,
            37 => Self::MagicCriticalHitPotency,
            38 => Self::MagicCriticalHitResilience,
            39 => Self::Parry,
            40 => Self::BlockRate,
            41 => Self::Block,
            42 => Self::FireMagicPotency,
            43 => Self::IceMagicPotency,
            44 => Self::WindMagicPotency,
            45 => Self::EarthMagicPotency,
            46 => Self::LightningMagicPotency,
            47 => Self::WaterMagicPotency,
            48 => Self::Regen,
            49 => Self::Refresh,
            50 => Self::StoreTp,
            51 => Self::Enmity,
            52 => Self::Spikes,
            53 => Self::Haste,
            56 => Self::ReducedDurabilityLoss,
            57 => Self::IncreasedSpiritbondGain,
            58 => Self::Damage,
            59 => Self::Delay,
            60 => Self::Fastcast,
            61 => Self::MovementSpeed,
            62 => Self::Exp,
            63 => Self::RestingHp,
            64 => Self::RestingMp,
            65 => Self::SlashingResistance,
            66 => Self::PiercingResistance,
            67 => Self::BluntResistance,
            68 => Self::ProjectileResistance,
            69 => Self::SonicResistance,
            70 => Self::BreathResistance,
            71 => Self::PhysicalResistance,
            72 => Self::MagicResistance,
            73 => Self::SlowResistance,
            74 => Self::PetrificationResistance,
            75 => Self::ParalysisResistance,
            76 => Self::SilenceResistance,
            77 => Self::BlindResistance,
            78 => Self::PoisonResistance,
            79 => Self::StunResistance,
            80 => Self::SleepResistance,
            81 => Self::BindResistance,
            82 => Self::HeavyResistance,
            83 => Self::DoomResistance,
            101 => Self::ConserveMp,
            102 => Self::SpellInterruptResistance,
            103 => Self::DoubleDownOdds,
            104 => Self::HqDiscoveryRate,
            105 => Self::None,
            106 => Self::NameplateShown,
            107 => Self::Targetable,
            108 => Self::NameplateShown2,
            109 => Self::HpPercent,
            110 => Self::MpPercent,
            111 => Self::TpPercent,
            112 => Self::AttackRange,
            113 => Self::Raise,
            114 => Self::MinimumHpLock,
            115 => Self::MinimumMpLock,
            116 => Self::MinimumTpLock,
            117 => Self::AttackType,
            118 => Self::CanBlock,
            119 => Self::HitCount,
            120 => Self::RawEvadeRate,
            121 => Self::RawParryRate,
            122 => Self::RawBlockRate,
            123 => Self::RawResistRate,
            124 => Self::RawHitRate,
            125 => Self::RawCritRate,
            126 => Self::DamageTakenDown,
            127 => Self::Regain,
            128 => Self::RegenDown,
            129 => Self::Stoneskin,
            130 => Self::KnockbackImmune,
            131 => Self::Stealth,
            _ => return None,
        })
    }

    pub fn as_u32(self) -> u32 {
        self as u32
    }
}

/// `Dictionary<uint, double>` equivalent. Missing keys read as 0.0, matching
/// the C# `GetMod` behaviour.
#[derive(Debug, Clone, Default)]
pub struct ModifierMap {
    map: HashMap<u32, f64>,
}

impl ModifierMap {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn get(&self, modifier: Modifier) -> f64 {
        self.get_raw(modifier.as_u32())
    }

    pub fn get_raw(&self, key: u32) -> f64 {
        self.map.get(&key).copied().unwrap_or(0.0)
    }

    pub fn set(&mut self, modifier: Modifier, value: f64) {
        self.set_raw(modifier.as_u32(), value);
    }

    pub fn set_raw(&mut self, key: u32, value: f64) {
        self.map.insert(key, value);
    }

    pub fn add(&mut self, modifier: Modifier, delta: f64) {
        self.add_raw(modifier.as_u32(), delta);
    }

    pub fn add_raw(&mut self, key: u32, delta: f64) {
        let cur = self.get_raw(key);
        self.set_raw(key, cur + delta);
    }

    pub fn subtract(&mut self, modifier: Modifier, delta: f64) {
        self.add(modifier, -delta);
    }

    pub fn subtract_raw(&mut self, key: u32, delta: f64) {
        self.add_raw(key, -delta);
    }

    pub fn multiply(&mut self, modifier: Modifier, factor: f64) {
        let cur = self.get(modifier);
        self.set(modifier, cur * factor);
    }

    pub fn divide(&mut self, modifier: Modifier, divisor: f64) {
        if divisor != 0.0 {
            let cur = self.get(modifier);
            self.set(modifier, cur / divisor);
        }
    }

    pub fn clear(&mut self) {
        self.map.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_mod_reads_zero() {
        let m = ModifierMap::new();
        assert_eq!(m.get(Modifier::Strength), 0.0);
    }

    #[test]
    fn arithmetic_chains_through_map() {
        let mut m = ModifierMap::new();
        m.add(Modifier::Attack, 10.0);
        m.add(Modifier::Attack, 5.0);
        m.subtract(Modifier::Attack, 3.0);
        m.multiply(Modifier::Attack, 2.0);
        assert_eq!(m.get(Modifier::Attack), 24.0);
    }

    #[test]
    fn raw_key_matches_enum_value() {
        let mut m = ModifierMap::new();
        m.set_raw(Modifier::Haste.as_u32(), 10.0);
        assert_eq!(m.get(Modifier::Haste), 10.0);
    }
}
