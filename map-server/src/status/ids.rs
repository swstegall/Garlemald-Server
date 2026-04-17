//! Canonical status-effect id constants. Subset of the ~500-entry
//! `StatusEffectId` enum in `Actors/Chara/Ai/StatusEffect.cs` — the entries
//! scripts actually reach for. Add more as scripts land.
//!
//! `status_id_of(effect_id)` gives the 16-bit short-id the client uses in
//! the 20-slot `charaWork.status` array (effect id minus 0x30D40 / 200000).

#![allow(dead_code)]

pub const STATUS_RAGE_OF_HALONE: u32 = 221021;

// Crowd control
pub const STATUS_QUICK: u32 = 223001;
pub const STATUS_HASTE: u32 = 223002;
pub const STATUS_SLOW: u32 = 223003;
pub const STATUS_PETRIFICATION: u32 = 223004;
pub const STATUS_PARALYSIS: u32 = 223005;
pub const STATUS_SILENCE: u32 = 223006;
pub const STATUS_BLIND: u32 = 223007;
pub const STATUS_MUTE: u32 = 223008;
pub const STATUS_SLOWCAST: u32 = 223009;
pub const STATUS_GLARE: u32 = 223010;
pub const STATUS_POISON: u32 = 223011;
pub const STATUS_TRANSFIXION: u32 = 223012;
pub const STATUS_PACIFICATION: u32 = 223013;
pub const STATUS_AMNESIA: u32 = 223014;
pub const STATUS_STUN: u32 = 223015;
pub const STATUS_DAZE: u32 = 223016;

// Exposed N/E/S/W
pub const STATUS_EXPOSED_FRONT: u32 = 223017;
pub const STATUS_EXPOSED_RIGHT: u32 = 223018;
pub const STATUS_EXPOSED_REAR: u32 = 223019;
pub const STATUS_EXPOSED_LEFT: u32 = 223020;

// Stat +/-
pub const STATUS_HP_BOOST: u32 = 223029;
pub const STATUS_HP_PENALTY: u32 = 223030;
pub const STATUS_MP_BOOST: u32 = 223031;
pub const STATUS_MP_PENALTY: u32 = 223032;
pub const STATUS_ATTACK_UP: u32 = 223033;
pub const STATUS_ATTACK_DOWN: u32 = 223034;
pub const STATUS_ACCURACY_UP: u32 = 223035;
pub const STATUS_ACCURACY_DOWN: u32 = 223036;
pub const STATUS_DEFENSE_UP: u32 = 223037;
pub const STATUS_DEFENSE_DOWN: u32 = 223038;
pub const STATUS_EVASION_UP: u32 = 223039;
pub const STATUS_EVASION_DOWN: u32 = 223040;
pub const STATUS_MAGIC_POTENCY_UP: u32 = 223041;
pub const STATUS_MAGIC_POTENCY_DOWN: u32 = 223042;
pub const STATUS_MAGIC_ACCURACY_UP: u32 = 223043;
pub const STATUS_MAGIC_ACCURACY_DOWN: u32 = 223044;
pub const STATUS_MAGIC_DEFENSE_UP: u32 = 223045;
pub const STATUS_MAGIC_DEFENSE_DOWN: u32 = 223046;
pub const STATUS_MAGIC_RESISTANCE_UP: u32 = 223047;
pub const STATUS_MAGIC_RESISTANCE_DOWN: u32 = 223048;

// Higher-order combat buffs/debuffs
pub const STATUS_COMBAT_FINESSE: u32 = 223049;
pub const STATUS_COMBAT_HINDRANCE: u32 = 223050;
pub const STATUS_MAGIC_FINESSE: u32 = 223051;
pub const STATUS_MAGIC_HINDRANCE: u32 = 223052;
pub const STATUS_COMBAT_RESILIENCE: u32 = 223053;
pub const STATUS_COMBAT_VULNERABILITY: u32 = 223054;
pub const STATUS_MAGIC_VULNERABILITY: u32 = 223055;
pub const STATUS_MAGIC_RESILIENCE: u32 = 223056;

// Job-specific stances
pub const STATUS_AEGIS_BOON: u32 = 223058;
pub const STATUS_DEFLECTION: u32 = 223059;
pub const STATUS_OUTMANEUVER: u32 = 223060;
pub const STATUS_PROVOKED: u32 = 223061;
pub const STATUS_SENTINEL: u32 = 223062;
pub const STATUS_COVER: u32 = 223063;
pub const STATUS_RAMPART: u32 = 223064;

// Movement/CC
pub const STATUS_SLEEP: u32 = 228001;
pub const STATUS_BIND: u32 = 228011;
pub const STATUS_FIXATION: u32 = 228012;
pub const STATUS_BIND2: u32 = 228013;
pub const STATUS_HEAVY: u32 = 228021;
pub const STATUS_CHARM: u32 = 228031;
pub const STATUS_FLEE: u32 = 228041;
pub const STATUS_DOOM: u32 = 228051;

// Synthesis / gathering support
pub const STATUS_SYNTHESIS_SUPPORT: u32 = 230001;
pub const STATUS_GEAR_CHANGE: u32 = 230013;
pub const STATUS_GEAR_DAMAGE: u32 = 230014;
pub const STATUS_HEAVY_GEAR_DAMAGE: u32 = 230015;

// Custom server-side procs (not in the retail client)
pub const STATUS_EVADE_PROC: u32 = 300000;
pub const STATUS_BLOCK_PROC: u32 = 300001;
pub const STATUS_PARRY_PROC: u32 = 300002;
pub const STATUS_MISS_PROC: u32 = 300003;
pub const STATUS_EXP_CHAIN: u32 = 300004;

/// The wire `statusId` the client expects in each of the 20 slots is the
/// effect id minus 200000 (0x30D40). Used in SetStatusPacket.
pub fn status_id_of(effect_id: u32) -> u16 {
    effect_id.saturating_sub(200_000) as u16
}
