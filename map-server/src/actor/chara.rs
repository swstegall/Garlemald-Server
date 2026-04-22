// garlemald-server — Rust port of a FINAL FANTASY XIV v1.23b server emulator (lobby/world/map)
// Copyright (C) 2026  Samuel Stegall
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU Affero General Public License as published
// by the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU Affero General Public License for more details.
//
// You should have received a copy of the GNU Affero General Public License
// along with this program.  If not, see <https://www.gnu.org/licenses/>.
//
// SPDX-License-Identifier: AGPL-3.0-or-later

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

    /// Seed a Player's primary stats (STR/VIT/DEX/INT/MND/PIE) + Hp/Mp
    /// baselines from class + level. **Intentional placeholder** — the
    /// real FFXIV 1.x per-level growth curves, race base stats, and
    /// guardian-deity bonuses were never reversed (Meteor upstream
    /// `WorldManager.cs:522-524` only seeds BattleNpcs from
    /// `server_battlenpc_*` and leaves Players at zero).
    ///
    /// What this does give us: non-zero primaries so
    /// [`apply_player_stat_derivation`] produces non-zero secondaries,
    /// and combat formulas (which read `Attack` / `Accuracy` / `Defense`)
    /// stop returning floors. Values are deliberately small enough that
    /// a real reversed-from-client seeder can replace this without
    /// breaking any test that asserts on specific numbers — the tests
    /// assert shape (monotone in level, class-emphasis ordering) rather
    /// than exact values.
    ///
    /// Formula: `base + level * per_level`, with a per-class emphasis of
    /// +2 to the primaries that class cares about. A level-1 character
    /// ends up with ~10 in every primary and a bit more in class-relevant
    /// ones; a level-50 character ~108-112. Hp/Mp get separate base +
    /// per-level curves, MP-focused for casters.
    ///
    /// **Seed-if-zero semantics.** Every `set` this function does is
    /// gated on the current value being `<= 0.0`, so:
    ///   * A fresh `Character::new()` (all mods at zero) gets the full
    ///     class+level baseline on its first recalc.
    ///   * A character that already has non-zero primaries from any
    ///     prior source (unit test fixture, DB persisted values, future
    ///     gear-paramBonus sum, etc.) passes through untouched.
    ///   * Repeated calls within one recalc pass are a no-op for
    ///     primaries after the first (the emphasis `add` bump is also
    ///     gated so a re-run doesn't double it).
    ///
    /// That rule is what makes it safe to chain baseline → gear-sum →
    /// derivation: gear sum can add on top of the seeded primaries
    /// without the next recalc zeroing them back out.
    pub fn apply_player_stat_baseline(&mut self) {
        use Modifier::*;
        let level = self.chara.level.max(1) as f64;
        let class = self.chara.class;

        // Base values — deliberately modest. "8 + level * 2" gives 10
        // at L1, 108 at L50, which is inside the range Meteor's own
        // battle-command basePotency defaults assume (100-ish).
        let base = 8.0;
        let per_level = 2.0;
        let primary = base + level * per_level;

        // Same shape for Hp/Mp/Tp so combat pools exist from L1.
        let hp = 250.0 + level * 30.0;
        let tp = 1000.0;
        let is_caster = matches!(
            class,
            c if c == crate::gamedata::CLASSID_THM as i16
                || c == crate::gamedata::CLASSID_CNJ as i16
        );
        let mp = if is_caster {
            80.0 + level * 20.0
        } else {
            20.0 + level * 5.0
        };

        // Closure captures `self.chara.mods` via `&mut`, but borrowck
        // blocks holding the borrow across the match below — so use a
        // small helper fn working off the mods map directly.
        fn seed_if_zero(c: &mut Character, m: Modifier, v: f64) {
            if c.chara.mods.get(m) <= 0.0 {
                c.chara.mods.set(m, v);
            }
        }
        seed_if_zero(self, Hp, hp);
        seed_if_zero(self, Mp, mp);
        seed_if_zero(self, Tp, tp);

        // Track which primaries we actually seeded this call — the
        // emphasis `+2` bumps only apply to those, otherwise a test
        // that pre-seeds STR=90 (and wants Attack=60 from derivation)
        // would see its value drift by +2 every time baseline ran.
        let mut seeded = [false; 6];
        let primaries = [
            Strength, Vitality, Dexterity, Intelligence, Mind, Piety,
        ];
        for (i, m) in primaries.iter().enumerate() {
            if self.chara.mods.get(*m) <= 0.0 {
                self.chara.mods.set(*m, primary);
                seeded[i] = true;
            }
        }

        // Class emphasis — bumps the two primaries most relevant to the
        // class. Values small (+2) so the placeholder doesn't drift too
        // far from what a real seeder might choose; preserves a class
        // ordering (e.g. PUG STR > CNJ STR) that combat tests can rely
        // on without pinning exact numbers.
        let (emph1, emph2) = match class {
            // Physical DPS: STR-focused
            c if c == crate::gamedata::CLASSID_PUG as i16 => (Strength, Dexterity),
            c if c == crate::gamedata::CLASSID_LNC as i16 => (Strength, Vitality),
            c if c == crate::gamedata::CLASSID_ARC as i16 => (Dexterity, Strength),
            // Tanks: VIT-focused
            c if c == crate::gamedata::CLASSID_GLA as i16 => (Vitality, Strength),
            c if c == crate::gamedata::CLASSID_MRD as i16 => (Strength, Vitality),
            // Casters
            c if c == crate::gamedata::CLASSID_THM as i16 => (Intelligence, Mind),
            c if c == crate::gamedata::CLASSID_CNJ as i16 => (Mind, Piety),
            // Disciples of Hand — DEX-focused (precision work)
            c if (crate::gamedata::CLASSID_CRP as i16..=crate::gamedata::CLASSID_CUL as i16)
                .contains(&c) =>
            {
                (Dexterity, Intelligence)
            }
            // Disciples of Land — DEX + MND (perception/gathering)
            c if (crate::gamedata::CLASSID_MIN as i16..=crate::gamedata::CLASSID_FSH as i16)
                .contains(&c) =>
            {
                (Dexterity, Mind)
            }
            // Unknown / unset class — no emphasis.
            _ => return,
        };
        let emph_idx = |m: Modifier| -> Option<usize> {
            primaries.iter().position(|p| *p == m)
        };
        if let Some(i) = emph_idx(emph1)
            && seeded[i]
        {
            self.chara.mods.add(emph1, 2.0);
        }
        if let Some(i) = emph_idx(emph2)
            && seeded[i]
        {
            self.chara.mods.add(emph2, 2.0);
        }
    }

    /// Port of the Player-specific tail of `Player.CalculateBaseStats`:
    /// derive physical/magic secondaries from the primary ability scores.
    /// Meteor uses `AddMod` (additive), so repeated calls stack — match
    /// that behavior exactly; callers fire this only in response to an
    /// event that changed the primaries (equip / trait toggle / level-up).
    ///
    /// Ratios from `Map Server/Actors/Chara/Player/Player.cs:2765-2779`:
    ///   STR → Attack (×0.667),  DEX → Accuracy,  VIT → Defense
    ///   INT → AttackMagicPotency (×0.25)
    ///   MND → MagicAccuracy + HealingMagicPotency
    ///   Piety → MagicEvasion + EnfeeblingMagicPotency
    ///
    /// We skip Meteor's `AddMod(Modifier.Hp, (float)Modifier.Vitality)` —
    /// that line casts the enum's integer value rather than `GetMod(...)`,
    /// so it adds a constant 4 to Hp regardless of VIT. Treated as a
    /// known-bad Meteor line rather than copied verbatim.
    ///
    /// **Call ordering with [`apply_player_stat_baseline`]:** baseline
    /// runs first (seeds primaries with `set`), derivation second
    /// (reads primaries and adds secondaries). Between those two steps
    /// is where a future gear-paramBonus summer will slot in —
    /// `gamedata_items_equipment.paramBonusType*` ids `15001..=15100`
    /// map 1:1 to [`Modifier`] ids via `paramBonusType - 15001`, so the
    /// summer's job is to walk equipped slots, resolve each to an
    /// [`ItemData`](crate::data::ItemData) with pre-parsed bonuses, and
    /// `add_mod` them before derivation reads primaries.
    pub fn apply_player_stat_derivation(&mut self) {
        use Modifier::*;
        let str_v = self.chara.mods.get(Strength);
        let dex_v = self.chara.mods.get(Dexterity);
        let vit_v = self.chara.mods.get(Vitality);
        let int_v = self.chara.mods.get(Intelligence);
        let mnd_v = self.chara.mods.get(Mind);
        let pie_v = self.chara.mods.get(Piety);

        self.chara.mods.add(Attack, (str_v * 0.667).floor());
        self.chara.mods.add(Accuracy, (dex_v * 0.667).floor());
        self.chara.mods.add(Defense, (vit_v * 0.667).floor());

        self.chara.mods.add(AttackMagicPotency, (int_v * 0.25).floor());
        self.chara.mods.add(MagicAccuracy, (mnd_v * 0.25).floor());
        self.chara.mods.add(HealingMagicPotency, (mnd_v * 0.25).floor());
        self.chara.mods.add(MagicEvasion, (pie_v * 0.25).floor());
        self.chara
            .mods
            .add(EnfeeblingMagicPotency, (pie_v * 0.25).floor());
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

    // -----------------------------------------------------------------
    // Tier 1 #3 follow-up — class+level baseline seeder tests.
    // -----------------------------------------------------------------

    #[test]
    fn baseline_seeds_nonzero_primaries_for_any_class() {
        // Smoke test: the pre-seeder state was "every Player has STR=0,
        // so derivation produces Attack=0". After the baseline runs
        // every primary must be strictly positive, regardless of class
        // (including the "unknown class, no emphasis" path).
        for class_id in [
            crate::gamedata::CLASSID_GLA as i16,
            crate::gamedata::CLASSID_CNJ as i16,
            crate::gamedata::CLASSID_CRP as i16,
            crate::gamedata::CLASSID_MIN as i16,
            42, // unmapped class
        ] {
            let mut c = Character::new(1);
            c.chara.class = class_id;
            c.chara.level = 1;
            c.apply_player_stat_baseline();
            for stat in [
                Modifier::Strength,
                Modifier::Vitality,
                Modifier::Dexterity,
                Modifier::Intelligence,
                Modifier::Mind,
                Modifier::Piety,
            ] {
                assert!(
                    c.chara.mods.get(stat) > 0.0,
                    "class {class_id} primary {stat:?} should be > 0, got {}",
                    c.chara.mods.get(stat)
                );
            }
            assert!(
                c.chara.mods.get(Modifier::Hp) > 0.0,
                "class {class_id} Hp should be > 0"
            );
            assert!(
                c.chara.mods.get(Modifier::Mp) > 0.0,
                "class {class_id} Mp should be > 0"
            );
        }
    }

    #[test]
    fn baseline_primaries_grow_monotonically_with_level() {
        let mut low = Character::new(1);
        low.chara.class = crate::gamedata::CLASSID_GLA as i16;
        low.chara.level = 1;
        low.apply_player_stat_baseline();

        let mut high = Character::new(2);
        high.chara.class = crate::gamedata::CLASSID_GLA as i16;
        high.chara.level = 50;
        high.apply_player_stat_baseline();

        assert!(high.chara.mods.get(Modifier::Strength) > low.chara.mods.get(Modifier::Strength));
        assert!(high.chara.mods.get(Modifier::Hp) > low.chara.mods.get(Modifier::Hp));
    }

    #[test]
    fn baseline_caster_gets_more_mp_than_physical_at_same_level() {
        // Caster MP pool should outpace a melee class's at the same
        // level. Nothing here pins exact numbers — only that the relative
        // ordering the `is_caster` branch enforces holds.
        let mut thm = Character::new(1);
        thm.chara.class = crate::gamedata::CLASSID_THM as i16;
        thm.chara.level = 20;
        thm.apply_player_stat_baseline();

        let mut gla = Character::new(2);
        gla.chara.class = crate::gamedata::CLASSID_GLA as i16;
        gla.chara.level = 20;
        gla.apply_player_stat_baseline();

        assert!(
            thm.chara.mods.get(Modifier::Mp) > gla.chara.mods.get(Modifier::Mp),
            "THM Mp {} should exceed GLA Mp {}",
            thm.chara.mods.get(Modifier::Mp),
            gla.chara.mods.get(Modifier::Mp),
        );
    }

    #[test]
    fn baseline_class_emphasis_biases_the_right_primary() {
        // Assert shape, not numbers — a tank's VIT should outrank its
        // INT (emphasis); a caster's INT outranks its STR.
        let mut gla = Character::new(1);
        gla.chara.class = crate::gamedata::CLASSID_GLA as i16;
        gla.chara.level = 10;
        gla.apply_player_stat_baseline();
        assert!(
            gla.chara.mods.get(Modifier::Vitality) > gla.chara.mods.get(Modifier::Intelligence),
            "GLA VIT should exceed INT after emphasis"
        );

        let mut thm = Character::new(2);
        thm.chara.class = crate::gamedata::CLASSID_THM as i16;
        thm.chara.level = 10;
        thm.apply_player_stat_baseline();
        assert!(
            thm.chara.mods.get(Modifier::Intelligence) > thm.chara.mods.get(Modifier::Strength),
            "THM INT should exceed STR after emphasis"
        );
    }

    #[test]
    fn baseline_then_derivation_produces_nonzero_secondaries() {
        // The whole point of the baseline seeder: running
        // `apply_player_stat_derivation` without any prior manual STR
        // seeding should now produce non-zero secondaries — that's the
        // regression guard for "derivation ran on zeros" (the Tier 1 #3
        // gap the roadmap calls out).
        let mut c = Character::new(1);
        c.chara.class = crate::gamedata::CLASSID_PUG as i16;
        c.chara.level = 10;
        c.apply_player_stat_baseline();
        c.apply_player_stat_derivation();
        assert!(c.chara.mods.get(Modifier::Attack) > 0.0);
        assert!(c.chara.mods.get(Modifier::Accuracy) > 0.0);
        assert!(c.chara.mods.get(Modifier::Defense) > 0.0);
    }

    #[test]
    fn baseline_is_idempotent_for_primaries() {
        // Two back-to-back calls must leave primaries at the same
        // value (the function uses `set` for primaries and `add` only
        // for the +2 emphasis bump — which re-applies on repeat).
        // Emphasis re-apply drift is *intentionally* out of scope: the
        // dispatcher calls baseline once per recalc pass.
        let mut c = Character::new(1);
        c.chara.class = crate::gamedata::CLASSID_PUG as i16;
        c.chara.level = 5;
        c.apply_player_stat_baseline();
        let after_first = c.chara.mods.get(Modifier::Vitality);
        c.apply_player_stat_baseline();
        let after_second = c.chara.mods.get(Modifier::Vitality);
        assert_eq!(
            after_first, after_second,
            "non-emphasis primary should be idempotent across repeated baseline calls"
        );
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
