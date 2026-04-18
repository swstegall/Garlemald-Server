//! Character helper methods, ported from `Actors/Chara/Character.cs`.
//!
//! Only the pure / mostly-pure helpers are here — the side-effecting ones
//! (PlayAnimation, DoBattleAction, Cast, Engage, Spawn, Die) queue outbound
//! packets and will land alongside the game-loop integration.

#![allow(dead_code)]

use super::modifier::Modifier;
use super::{CharaState, Character};

/// Stat indices used by `GetStat` / `SetStat`. Matches the FFXIV 1.23b wire
/// ids that start at `STAT_STRENGTH = 3` in `scripts/global.lua`.
pub const STAT_COUNT: usize = 36;

/// Max stack size for TP.
pub const MAX_TP: u16 = 3000;

impl Character {
    // ----- HP / MP / TP ------------------------------------------------------

    pub fn is_dead(&self) -> bool {
        self.chara.is_dead()
    }
    pub fn is_alive(&self) -> bool {
        self.chara.is_alive()
    }

    pub fn get_hp(&self) -> i16 {
        self.chara.hp
    }
    pub fn get_max_hp(&self) -> i16 {
        self.chara.max_hp
    }
    pub fn get_mp(&self) -> i16 {
        self.chara.mp
    }
    pub fn get_max_mp(&self) -> i16 {
        self.chara.max_mp
    }
    pub fn get_tp(&self) -> u16 {
        self.chara.tp
    }
    pub fn get_hpp(&self) -> u8 {
        self.chara.hpp()
    }
    pub fn get_mpp(&self) -> u8 {
        self.chara.mpp()
    }
    pub fn get_tpp(&self) -> u8 {
        self.chara.tpp()
    }

    /// Set current HP, clamped to `[0, max_hp]`.
    pub fn set_hp(&mut self, hp: i32) {
        self.chara.hp = hp.clamp(0, self.chara.max_hp as i32) as i16;
    }

    /// Set maximum HP; clamps current HP down to the new max.
    pub fn set_max_hp(&mut self, max_hp: i32) {
        let m = max_hp.max(0) as i16;
        self.chara.max_hp = m;
        if self.chara.hp > m {
            self.chara.hp = m;
        }
    }

    pub fn set_mp(&mut self, mp: i32) {
        self.chara.mp = mp.clamp(0, self.chara.max_mp as i32) as i16;
    }

    pub fn set_max_mp(&mut self, max_mp: i32) {
        let m = max_mp.max(0) as i16;
        self.chara.max_mp = m;
        if self.chara.mp > m {
            self.chara.mp = m;
        }
    }

    pub fn add_hp(&mut self, delta: i32) {
        self.set_hp(self.chara.hp as i32 + delta);
    }

    pub fn add_mp(&mut self, delta: i32) {
        self.set_mp(self.chara.mp as i32 + delta);
    }

    pub fn add_tp(&mut self, delta: i32) {
        let new_tp = (self.chara.tp as i32 + delta).clamp(0, MAX_TP as i32);
        self.chara.tp = new_tp as u16;
    }

    pub fn del_hp(&mut self, amount: i32) {
        self.add_hp(-amount);
    }

    pub fn del_mp(&mut self, amount: i32) {
        self.add_mp(-amount);
    }

    pub fn del_tp(&mut self, amount: i32) {
        self.add_tp(-amount);
    }

    // ----- Stat recalculation ---------------------------------------------

    /// Port of `Character.CalculateBaseStats`. Reads `Modifier::Hp` /
    /// `HpPercent` / `Mp` / `MpPercent` off the modifier map and
    /// writes them to the max/current pools. HitCount is seeded to 1.
    /// Scripts drive the modifier map via `player:SetMod(id, val)` when
    /// gear changes, so running this after equip/unequip gives the
    /// client the correct totals.
    pub fn calculate_base_stats(&mut self) {
        let hp_mod = self.chara.mods.get(Modifier::Hp) as i32;
        if hp_mod > 0 {
            self.set_max_hp(hp_mod);
            let hpp = self.chara.mods.get(Modifier::HpPercent);
            let hp = if hpp > 0.0 {
                ((hpp / 100.0) * hp_mod as f64).ceil() as i32
            } else {
                hp_mod
            };
            self.set_hp(hp);
        }
        let mp_mod = self.chara.mods.get(Modifier::Mp) as i32;
        if mp_mod > 0 {
            self.set_max_mp(mp_mod);
            let mpp = self.chara.mods.get(Modifier::MpPercent);
            let mp = if mpp > 0.0 {
                ((mpp / 100.0) * mp_mod as f64).ceil() as i32
            } else {
                mp_mod
            };
            self.set_mp(mp);
        }
        // HitCount always starts at 1 — dual-wield etc. bumps it later.
        self.chara.mods.set(Modifier::HitCount, 1.0);
    }

    /// Port of `Character.RecalculateStats`. The C# original is a
    /// thin wrapper that used to call `CalculateBaseStats`; our port
    /// does the same. Triggered on equip, unequip, level-up, trait
    /// change.
    pub fn recalculate_stats(&mut self) {
        self.calculate_base_stats();
    }

    // ----- Class / level ----------------------------------------------------

    pub fn get_class(&self) -> i16 {
        self.chara.class
    }

    pub fn get_level(&self) -> i16 {
        self.chara.level
    }

    // ----- Modifiers --------------------------------------------------------

    pub fn get_mod(&self, modifier: Modifier) -> f64 {
        self.chara.mods.get(modifier)
    }

    pub fn get_mod_raw(&self, key: u32) -> f64 {
        self.chara.mods.get_raw(key)
    }

    pub fn set_mod(&mut self, modifier: Modifier, value: f64) {
        self.chara.mods.set(modifier, value);
    }

    pub fn set_mod_raw(&mut self, key: u32, value: f64) {
        self.chara.mods.set_raw(key, value);
    }

    pub fn add_mod(&mut self, modifier: Modifier, delta: f64) {
        self.chara.mods.add(modifier, delta);
    }

    pub fn add_mod_raw(&mut self, key: u32, delta: f64) {
        self.chara.mods.add_raw(key, delta);
    }

    pub fn subtract_mod(&mut self, modifier: Modifier, delta: f64) {
        self.chara.mods.subtract(modifier, delta);
    }

    pub fn subtract_mod_raw(&mut self, key: u32, delta: f64) {
        self.chara.mods.subtract_raw(key, delta);
    }

    pub fn multiply_mod(&mut self, modifier: Modifier, factor: f64) {
        self.chara.mods.multiply(modifier, factor);
    }

    pub fn divide_mod(&mut self, modifier: Modifier, divisor: f64) {
        self.chara.mods.divide(modifier, divisor);
    }

    // ----- Stats -----------------------------------------------------------

    pub fn get_stat(&self, stat_id: u32) -> i16 {
        self.chara.stats.get(stat_id as usize).copied().unwrap_or(0)
    }

    pub fn set_stat(&mut self, stat_id: u32, value: i32) {
        if let Some(slot) = self.chara.stats.get_mut(stat_id as usize) {
            *slot = value.clamp(i16::MIN as i32, i16::MAX as i32) as i16;
        }
    }

    // ----- Combat predicates (pure) -----------------------------------------

    /// True when this actor can enter combat. Matches the C#
    /// `CanAttack` — returns false if dead, stunned (status 1), or stealthed.
    pub fn can_attack(&self) -> bool {
        self.is_alive()
            && self.chara.mods.get(Modifier::Stealth) <= 0.0
            && self.chara.mods.get(Modifier::CanBlock) >= 0.0
    }

    /// Attack range in yalms. The C# default is 3.0 for h2h; non-default is
    /// stored under the AttackRange modifier.
    pub fn get_attack_range(&self) -> f32 {
        let raw = self.chara.mods.get(Modifier::AttackRange);
        if raw > 0.0 { raw as f32 } else { 3.0 }
    }

    /// Attack delay in milliseconds, derived from the `Delay` modifier.
    /// Matches the C# clamp: a `Delay` of 0 means "use the weapon default".
    pub fn get_attack_delay_ms(&self) -> u32 {
        let delay = self.chara.mods.get(Modifier::Delay);
        if delay > 0.0 {
            (delay * 1000.0) as u32
        } else {
            2500
        }
    }

    /// Movement speed in units/sec. The C# stores it as a raw float; we
    /// mirror that.
    pub fn get_speed(&self) -> f32 {
        let ms = self.chara.mods.get(Modifier::MovementSpeed);
        if ms > 0.0 { ms as f32 } else { 5.0 }
    }

    // ----- Discipline-of predicates (for class ranges) ----------------------

    pub fn is_disciple_of_war(&self) -> bool {
        matches!(self.chara.class as u8, 2..=8)
    }
    pub fn is_disciple_of_magic(&self) -> bool {
        matches!(self.chara.class as u8, 22..=23)
    }
    pub fn is_disciple_of_hand(&self) -> bool {
        matches!(self.chara.class as u8, 29..=36)
    }
    pub fn is_disciple_of_land(&self) -> bool {
        matches!(self.chara.class as u8, 39..=41)
    }

    /// True if this actor is still engaged in combat (main-state bit set).
    pub fn is_engaged(&self) -> bool {
        self.chara.current_target != crate::actor::INVALID_ACTORID
    }

    /// Generic is-valid-target helper mirroring the ValidTarget bitmask in
    /// `BattleCommand.cs`.
    pub fn is_valid_target(&self, target: &Character, valid_mask: u32) -> bool {
        // Port of the coarse checks from the C# shared helper. Specific
        // variants (healing vs offensive, pet/ally handling) come with
        // BattleCommand; this covers the "not dead, not self, targetable"
        // base case the shared method handles.
        const FLAG_SELF: u32 = 0x01;
        const FLAG_NPC: u32 = 0x02;
        const FLAG_PARTY: u32 = 0x04;
        const FLAG_ALLIANCE: u32 = 0x08;
        const FLAG_ENEMY: u32 = 0x10;
        const FLAG_CORPSE: u32 = 0x20;

        if !target.is_alive() && (valid_mask & FLAG_CORPSE) == 0 {
            return false;
        }
        if self.base.actor_id == target.base.actor_id && (valid_mask & FLAG_SELF) == 0 {
            return false;
        }
        if target.chara.mods.get(Modifier::Targetable) < 0.0 {
            return false;
        }
        let _ = (FLAG_NPC, FLAG_PARTY, FLAG_ALLIANCE, FLAG_ENEMY);
        true
    }
}

/// Default-init helpers on the inner state struct.
impl CharaState {
    pub fn with_stats(mut self, stats: [i16; STAT_COUNT]) -> Self {
        self.stats = stats;
        self
    }

    /// Convenience: fully reset mods and stats.
    pub fn clear_derived(&mut self) {
        self.mods.clear();
        self.stats = [0; STAT_COUNT];
    }
}

#[cfg(test)]
mod recalc_tests {
    use super::*;
    use crate::actor::Character;

    #[test]
    fn recalculate_stats_applies_hp_and_mp_mods() {
        let mut c = Character::new(1);
        c.chara.mods.set(Modifier::Hp, 800.0);
        c.chara.mods.set(Modifier::Mp, 300.0);
        c.recalculate_stats();
        assert_eq!(c.chara.max_hp, 800);
        assert_eq!(c.chara.hp, 800);
        assert_eq!(c.chara.max_mp, 300);
        assert_eq!(c.chara.mp, 300);
        assert_eq!(c.chara.mods.get(Modifier::HitCount), 1.0);
    }

    #[test]
    fn hp_percent_scales_current_from_max() {
        let mut c = Character::new(1);
        c.chara.mods.set(Modifier::Hp, 1000.0);
        c.chara.mods.set(Modifier::HpPercent, 75.0);
        c.recalculate_stats();
        assert_eq!(c.chara.max_hp, 1000);
        assert_eq!(c.chara.hp, 750);
    }

    #[test]
    fn zero_hp_mod_leaves_pool_unchanged() {
        let mut c = Character::new(1);
        c.chara.max_hp = 1234;
        c.chara.hp = 1000;
        c.recalculate_stats();
        assert_eq!(c.chara.max_hp, 1234);
        assert_eq!(c.chara.hp, 1000);
    }
}

/// Trait reference used by Player::has_trait — imported here because the
/// Character-side reference check only needs class/level comparison.
#[derive(Debug, Clone, Copy)]
pub struct TraitRef {
    pub id: u16,
    pub job: u8,
    pub level: u8,
}

impl Character {
    /// Does the character meet `trait_ref`'s class/level requirement?
    pub fn meets_trait(&self, trait_ref: TraitRef) -> bool {
        self.chara.class as u8 == trait_ref.job && trait_ref.level as i16 <= self.chara.level
    }
}
