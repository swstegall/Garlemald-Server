//! `StatusEffectFlags` + `StatusEffectOverwrite`. Ported 1:1 from
//! `Actors/Chara/Ai/StatusEffect.cs` (lines 352–401).

#![allow(dead_code)]

/// Bitfield of behavioral flags on a status effect. Values match the C# bit
/// positions exactly so `gamedata_statuseffects.flags` rows stay compatible.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct StatusEffectFlags(pub u32);

impl StatusEffectFlags {
    pub const NONE: StatusEffectFlags = StatusEffectFlags(0);

    // Loss flags
    pub const LOSE_ON_DEATH: StatusEffectFlags = StatusEffectFlags(1 << 0);
    pub const LOSE_ON_ZONING: StatusEffectFlags = StatusEffectFlags(1 << 1);
    pub const LOSE_ON_ESUNA: StatusEffectFlags = StatusEffectFlags(1 << 2);
    pub const LOSE_ON_DISPEL: StatusEffectFlags = StatusEffectFlags(1 << 3);
    pub const LOSE_ON_LOGOUT: StatusEffectFlags = StatusEffectFlags(1 << 4);
    pub const LOSE_ON_ATTACKING: StatusEffectFlags = StatusEffectFlags(1 << 5);
    pub const LOSE_ON_CAST_START: StatusEffectFlags = StatusEffectFlags(1 << 6);
    pub const LOSE_ON_AGGRO: StatusEffectFlags = StatusEffectFlags(1 << 7);
    pub const LOSE_ON_CLASS_CHANGE: StatusEffectFlags = StatusEffectFlags(1 << 8);

    // Activate flags
    pub const ACTIVATE_ON_CAST_START: StatusEffectFlags = StatusEffectFlags(1 << 9);
    pub const ACTIVATE_ON_COMMAND_START: StatusEffectFlags = StatusEffectFlags(1 << 10);
    pub const ACTIVATE_ON_COMMAND_FINISH: StatusEffectFlags = StatusEffectFlags(1 << 11);
    pub const ACTIVATE_ON_PREACTION_TARGET: StatusEffectFlags = StatusEffectFlags(1 << 12);
    pub const ACTIVATE_ON_PREACTION_CASTER: StatusEffectFlags = StatusEffectFlags(1 << 13);
    pub const ACTIVATE_ON_DAMAGE_TAKEN: StatusEffectFlags = StatusEffectFlags(1 << 14);
    pub const ACTIVATE_ON_HEALED: StatusEffectFlags = StatusEffectFlags(1 << 15);
    pub const ACTIVATE_ON_MISS: StatusEffectFlags = StatusEffectFlags(1 << 16);
    pub const ACTIVATE_ON_EVADE: StatusEffectFlags = StatusEffectFlags(1 << 17);
    pub const ACTIVATE_ON_PARRY: StatusEffectFlags = StatusEffectFlags(1 << 18);
    pub const ACTIVATE_ON_BLOCK: StatusEffectFlags = StatusEffectFlags(1 << 19);
    pub const ACTIVATE_ON_HIT: StatusEffectFlags = StatusEffectFlags(1 << 20);
    pub const ACTIVATE_ON_CRIT: StatusEffectFlags = StatusEffectFlags(1 << 21);

    // Prevention flags
    pub const PREVENT_SPELL: StatusEffectFlags = StatusEffectFlags(1 << 22);
    pub const PREVENT_WEAPON_SKILL: StatusEffectFlags = StatusEffectFlags(1 << 23);
    pub const PREVENT_ABILITY: StatusEffectFlags = StatusEffectFlags(1 << 24);
    pub const PREVENT_ATTACK: StatusEffectFlags = StatusEffectFlags(1 << 25);
    pub const PREVENT_MOVEMENT: StatusEffectFlags = StatusEffectFlags(1 << 26);
    pub const PREVENT_TURN: StatusEffectFlags = StatusEffectFlags(1 << 27);
    pub const PREVENT_UNTARGET: StatusEffectFlags = StatusEffectFlags(1 << 28);

    /// Stances don't expire; their end-time is coerced to the max u32 so the
    /// client icon doesn't blink.
    pub const STANCE: StatusEffectFlags = StatusEffectFlags(1 << 29);

    pub const fn bits(self) -> u32 {
        self.0
    }

    pub const fn contains(self, other: Self) -> bool {
        (self.0 & other.0) == other.0
    }

    pub const fn intersects(self, other: Self) -> bool {
        (self.0 & other.0) != 0
    }
}

impl std::ops::BitOr for StatusEffectFlags {
    type Output = Self;
    fn bitor(self, rhs: Self) -> Self {
        Self(self.0 | rhs.0)
    }
}

impl std::ops::BitAnd for StatusEffectFlags {
    type Output = Self;
    fn bitand(self, rhs: Self) -> Self {
        Self(self.0 & rhs.0)
    }
}

impl std::ops::BitOrAssign for StatusEffectFlags {
    fn bitor_assign(&mut self, rhs: Self) {
        self.0 |= rhs.0;
    }
}

impl From<u32> for StatusEffectFlags {
    fn from(bits: u32) -> Self {
        Self(bits)
    }
}

impl From<StatusEffectFlags> for u32 {
    fn from(f: StatusEffectFlags) -> Self {
        f.0
    }
}

// ---------------------------------------------------------------------------

/// How an incoming effect with the same id should react when one is already
/// active. Matches `StatusEffectOverwrite` in C#.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum StatusEffectOverwrite {
    #[default]
    None = 0,
    Always = 1,
    GreaterOrEqualTo = 2,
    GreaterOnly = 3,
}

impl StatusEffectOverwrite {
    pub fn from_u8(b: u8) -> Self {
        match b {
            1 => Self::Always,
            2 => Self::GreaterOrEqualTo,
            3 => Self::GreaterOnly,
            _ => Self::None,
        }
    }

    pub fn as_u8(self) -> u8 {
        self as u8
    }
}
