//! Hit-result enums. Ported from
//! `Packets/Send/Actor/Battle/CommandResult.cs`.

#![allow(dead_code)]

/// `HitEffect` bitfield. Values match the C# bit positions exactly so the
/// client sees identical packets. Comments preserve the retail notes on
/// what each bit triggers visually.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct HitEffect(pub u32);

impl HitEffect {
    pub const NONE: HitEffect = HitEffect(0);

    // Category bytes — pinned to bit 24 to pick one of ~8 effect categories.
    pub const HIT_EFFECT_TYPE: HitEffect = HitEffect(8 << 24);
    pub const ADDITIONAL_EFFECT_TYPE: HitEffect = HitEffect(24 << 24);
    pub const STATUS_EFFECT_TYPE: HitEffect = HitEffect(32 << 24);
    pub const STATUS_LOSS_TYPE: HitEffect = HitEffect(40 << 24);
    pub const MAGIC_EFFECT_TYPE: HitEffect = HitEffect(48 << 24);
    pub const SELF_HEAL_TYPE: HitEffect = HitEffect(72 << 24);
    pub const ANIMATION_EFFECT_TYPE: HitEffect = HitEffect(96 << 24);

    // HitEffectType recoil levels.
    pub const RECOIL_LV1: HitEffect = HitEffect(0);
    pub const RECOIL_LV2: HitEffect = HitEffect(1 << 0);
    pub const RECOIL_LV3: HitEffect = HitEffect(1 << 1);
    pub const CRITICAL_HIT: HitEffect = HitEffect((1 << 0) | (1 << 1));

    // Hit visuals — weapon/impact flavour.
    pub const HIT_VISUAL1: HitEffect = HitEffect(1 << 2);
    pub const HIT_VISUAL2: HitEffect = HitEffect(1 << 3);
    pub const HIT_VISUAL3: HitEffect = HitEffect(1 << 4);
    pub const HIT_VISUAL4: HitEffect = HitEffect(1 << 5);

    // Protect / Shell buff responses.
    pub const PROTECT: HitEffect = HitEffect(1 << 6);
    pub const SHELL: HitEffect = HitEffect(1 << 7);
    pub const PROTECT_SHELL_SPECIAL: HitEffect = HitEffect((1 << 6) | (1 << 7));

    // Hit / Evade / Miss / Parry / Block pop-up triggers.
    pub const HIT_EFFECT1: HitEffect = HitEffect(1 << 9);
    pub const HIT_EFFECT2: HitEffect = HitEffect(1 << 10);
    pub const HIT_EFFECT3: HitEffect = HitEffect(1 << 11);
    pub const HIT_EFFECT4: HitEffect = HitEffect(1 << 12);
    pub const HIT_EFFECT5: HitEffect = HitEffect(1 << 13);

    pub const MISS: HitEffect = HitEffect(0);
    pub const EVADE: HitEffect = HitEffect(1 << 9);
    pub const HIT: HitEffect = HitEffect((1 << 9) | (1 << 10));
    pub const CRIT: HitEffect = HitEffect(1 << 11);
    pub const PARRY: HitEffect = HitEffect((1 << 9) | (1 << 10) | (1 << 11));
    pub const BLOCK: HitEffect = HitEffect(1 << 12);

    // Knockbacks (stacked recoil + direction).
    pub const KNOCKBACK_LV1: HitEffect = HitEffect((1 << 12) | (1 << 10) | (1 << 9));
    pub const KNOCKBACK_LV2: HitEffect = HitEffect((1 << 12) | (1 << 11));
    pub const KNOCKBACK_LV3: HitEffect = HitEffect((1 << 12) | (1 << 11) | (1 << 9));
    pub const KNOCKBACK_LV4: HitEffect = HitEffect((1 << 12) | (1 << 11) | (1 << 10));
    pub const KNOCKBACK_LV5: HitEffect = HitEffect((1 << 12) | (1 << 11) | (1 << 10) | (1 << 9));

    pub const KNOCKBACK_CCW_LV1: HitEffect = HitEffect(1 << 13);
    pub const KNOCKBACK_CCW_LV2: HitEffect = HitEffect((1 << 13) | (1 << 9));
    pub const KNOCKBACK_CW_LV1: HitEffect = HitEffect((1 << 13) | (1 << 10));
    pub const KNOCKBACK_CW_LV2: HitEffect = HitEffect((1 << 13) | (1 << 10) | (1 << 9));

    pub const DRAW_IN: HitEffect = HitEffect((1 << 13) | (1 << 11));
    pub const UNKNOWN_SHIELD_EFFECT: HitEffect = HitEffect((1 << 13) | (1 << 12));
    pub const STONESKIN: HitEffect = HitEffect((1 << 13) | (1 << 12) | (1 << 9));

    // Skill combos.
    pub const SKILL_COMBO1: HitEffect = HitEffect(1 << 15);
    pub const SKILL_COMBO2: HitEffect = HitEffect(1 << 16);
    pub const SKILL_COMBO3: HitEffect = HitEffect((1 << 15) | (1 << 16));
    pub const SKILL_COMBO4: HitEffect = HitEffect(1 << 17);

    // MagicEffectType flags.
    pub const FULL_RESIST: HitEffect = HitEffect(0);
    pub const WEAK_RESIST: HitEffect = HitEffect(1 << 0);
    pub const NO_RESIST: HitEffect = HitEffect(1 << 1);

    pub const MAGIC_SHELL: HitEffect = HitEffect(1 << 4);
    pub const MAGIC_SHIELD: HitEffect = HitEffect(1 << 5);

    pub const HEAL: HitEffect = HitEffect(1 << 8);
    pub const MP: HitEffect = HitEffect(1 << 9);
    pub const TP: HitEffect = HitEffect(1 << 10);

    // SelfHealType subflags.
    pub const SELF_HEAL_HP: HitEffect = HitEffect(0);
    pub const SELF_HEAL_MP: HitEffect = HitEffect(1 << 0);
    pub const SELF_HEAL_TP: HitEffect = HitEffect(1 << 1);
    pub const SELF_HEAL: HitEffect = HitEffect(1 << 10);

    // AdditionalEffectType — elemental flavours.
    pub const FIRE_EFFECT: HitEffect = HitEffect(1 << 10);
    pub const ICE_EFFECT: HitEffect = HitEffect(2 << 10);
    pub const WIND_EFFECT: HitEffect = HitEffect(3 << 10);
    pub const EARTH_EFFECT: HitEffect = HitEffect(4 << 10);
    pub const LIGHTNING_EFFECT: HitEffect = HitEffect(5 << 10);
    pub const WATER_EFFECT: HitEffect = HitEffect(6 << 10);
    pub const ASTRAL_EFFECT: HitEffect = HitEffect(7 << 10);
    pub const UMBRAL_EFFECT: HitEffect = HitEffect(8 << 10);

    pub const HP_ABSORB_EFFECT: HitEffect = HitEffect(14 << 10);
    pub const MP_ABSORB_EFFECT: HitEffect = HitEffect(15 << 10);
    pub const TP_ABSORB_EFFECT: HitEffect = HitEffect(16 << 10);
    pub const TRIPLE_ABSORB_EFFECT: HitEffect = HitEffect(17 << 10);
    pub const MOOGLE_EFFECT: HitEffect = HitEffect(18 << 10);

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

impl std::ops::BitOr for HitEffect {
    type Output = Self;
    fn bitor(self, rhs: Self) -> Self {
        Self(self.0 | rhs.0)
    }
}
impl std::ops::BitOrAssign for HitEffect {
    fn bitor_assign(&mut self, rhs: Self) {
        self.0 |= rhs.0;
    }
}
impl std::ops::BitAnd for HitEffect {
    type Output = Self;
    fn bitand(self, rhs: Self) -> Self {
        Self(self.0 & rhs.0)
    }
}

impl From<u32> for HitEffect {
    fn from(bits: u32) -> Self {
        Self(bits)
    }
}
impl From<HitEffect> for u32 {
    fn from(e: HitEffect) -> u32 {
        e.0
    }
}

/// Direction the attack hit from. Matches the C# `HitDirection` bitfield.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct HitDirection(pub u8);

impl HitDirection {
    pub const NONE: HitDirection = HitDirection(0);
    pub const FRONT: HitDirection = HitDirection(1 << 0);
    pub const RIGHT: HitDirection = HitDirection(1 << 1);
    pub const REAR: HitDirection = HitDirection(1 << 2);
    pub const LEFT: HitDirection = HitDirection(1 << 3);

    pub const fn bits(self) -> u8 {
        self.0
    }
    pub const fn contains(self, other: Self) -> bool {
        (self.0 & other.0) == other.0
    }
}

/// Hit resolution category — what the client displays.
#[repr(u16)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum HitType {
    Miss = 0,
    Evade = 1,
    Parry = 2,
    Block = 3,
    SingleResist = 4,
    DoubleResist = 5,
    TripleResist = 6,
    FullResist = 7,
    #[default]
    Hit = 8,
    Crit = 9,
}

impl HitType {
    /// True when the action connected (not missed/evaded/resisted).
    pub fn landed(self) -> bool {
        !matches!(
            self,
            Self::Miss | Self::Evade | Self::SingleResist | Self::DoubleResist | Self::FullResist
        )
    }

    pub fn as_u16(self) -> u16 {
        self as u16
    }
}

/// Action kind — physical / spell / heal / status.
#[repr(u16)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ActionType {
    #[default]
    None = 0,
    Physical = 1,
    Magic = 2,
    Heal = 3,
    Status = 4,
}

/// Action element/damage-property. Overlaps slightly with the element enum
/// in the C# source; the latter is commented-out so we just keep this one.
#[repr(u16)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ActionProperty {
    #[default]
    None = 0,
    Slashing = 1,
    Piercing = 2,
    Blunt = 3,
    Projectile = 4,

    Fire = 5,
    Ice = 6,
    Wind = 7,
    Earth = 8,
    Lightning = 9,
    Water = 10,

    Astral = 11,
    Umbral = 12,
    Heal = 13,
}

impl ActionType {
    pub fn from_u16(v: u16) -> Self {
        match v {
            1 => Self::Physical,
            2 => Self::Magic,
            3 => Self::Heal,
            4 => Self::Status,
            _ => Self::None,
        }
    }
}

impl ActionProperty {
    pub fn from_u16(v: u16) -> Self {
        match v {
            1 => Self::Slashing,
            2 => Self::Piercing,
            3 => Self::Blunt,
            4 => Self::Projectile,
            5 => Self::Fire,
            6 => Self::Ice,
            7 => Self::Wind,
            8 => Self::Earth,
            9 => Self::Lightning,
            10 => Self::Water,
            11 => Self::Astral,
            12 => Self::Umbral,
            13 => Self::Heal,
            _ => Self::None,
        }
    }
}
