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

//! Damage formulas + hit resolution. Port of
//! `Actors/Chara/Ai/Utils/BattleUtils.cs`.
//!
//! This module is deliberately pure: it takes plain `CombatView` snapshots
//! (level, HP, modifier readings, boolean status flags) and mutates a
//! `CommandResult`. All side effects — DB writes, packet broadcasts, hate
//! updates — are emitted through the `BattleOutbox` + caller. That makes
//! the math trivially unit-testable.

#![allow(dead_code)]

use crate::actor::modifier::Modifier;
use crate::actor::modifier::ModifierMap;

use super::command::{BattleCommand, CommandResult, CommandResultContainer, CommandType};
use super::effects::{ActionProperty, ActionType, HitDirection, HitEffect, HitType};

// ---------------------------------------------------------------------------
// Text-id + HitEffect lookup tables (1:1 with the C# dictionaries).
// ---------------------------------------------------------------------------

pub fn physical_hit_text_id(ty: HitType) -> u16 {
    match ty {
        HitType::Miss => 30311,
        HitType::Evade => 30310,
        HitType::Parry => 30308,
        HitType::Block => 30306,
        HitType::Hit => 30301,
        HitType::Crit => 30302,
        _ => 0,
    }
}

pub fn magical_hit_text_id(ty: HitType) -> u16 {
    match ty {
        HitType::SingleResist => 30318,
        HitType::DoubleResist => 30317,
        HitType::TripleResist => 30316,
        HitType::FullResist => 30316,
        HitType::Hit => 30319,
        HitType::Crit => 30392,
        _ => 0,
    }
}

pub fn multi_hit_text_id(ty: HitType) -> u16 {
    match ty {
        HitType::Miss => 30449,
        HitType::Parry => 30448,
        HitType::Block => 30447,
        HitType::Hit => 30443,
        HitType::Crit => 30444,
        _ => 0,
    }
}

pub fn physical_hit_effect(ty: HitType) -> HitEffect {
    match ty {
        HitType::Miss => HitEffect::MISS,
        HitType::Evade => HitEffect::EVADE,
        HitType::Parry => HitEffect::PARRY,
        HitType::Block => HitEffect::BLOCK,
        HitType::Hit => HitEffect::HIT,
        HitType::Crit => HitEffect::CRIT | HitEffect::CRITICAL_HIT,
        _ => HitEffect::NONE,
    }
}

pub fn magical_hit_effect(ty: HitType) -> HitEffect {
    match ty {
        HitType::SingleResist => HitEffect::WEAK_RESIST,
        HitType::DoubleResist => HitEffect::WEAK_RESIST,
        HitType::TripleResist => HitEffect::WEAK_RESIST,
        HitType::FullResist => HitEffect::FULL_RESIST,
        HitType::Hit => HitEffect::NO_RESIST,
        HitType::Crit => HitEffect::CRIT,
        _ => HitEffect::NONE,
    }
}

// Level-based base EXP table (1..=50). Matches the C# `BASEEXP[]`.
pub const BASE_EXP: [u16; 50] = [
    150, 150, 150, 150, 150, 150, 150, 150, 150, 150, // level <= 10
    150, 150, 150, 150, 150, 150, 150, 150, 160, 170, // level <= 20
    180, 190, 190, 200, 210, 220, 230, 240, 250, 260, // level <= 30
    270, 280, 290, 300, 310, 320, 330, 340, 350, 360, // level <= 40
    370, 380, 380, 390, 400, 410, 420, 430, 430, 440, // level <= 50
];

// ---------------------------------------------------------------------------
// CombatView — lightweight snapshot of a Character used by pure formulas.
// ---------------------------------------------------------------------------

/// Just the subset of Character the math needs. Callers build this from
/// a live `Character` (Phase E) or construct one by hand in tests.
#[derive(Debug, Clone)]
pub struct CombatView<'a> {
    pub actor_id: u32,
    pub level: i16,
    pub max_hp: i16,
    pub mods: &'a ModifierMap,
    pub has_aegis_boon: bool,
    pub has_protect: bool,
    pub has_shell: bool,
    pub has_stoneskin: bool,
}

impl CombatView<'_> {
    pub fn get_mod(&self, m: Modifier) -> f64 {
        self.mods.get(m)
    }
}

/// Random source abstraction. The real game loop uses `rand::thread_rng`
/// wrapped; tests hand over a fixed-sequence implementation.
pub trait Rng {
    /// Returns a value in `[0.0, 1.0)`.
    fn next_f64(&mut self) -> f64;
}

/// Fixed-sequence RNG for deterministic testing.
pub struct FixedRng<'a> {
    values: &'a [f64],
    cursor: usize,
}

impl<'a> FixedRng<'a> {
    pub fn new(values: &'a [f64]) -> Self {
        Self { values, cursor: 0 }
    }
}

impl Rng for FixedRng<'_> {
    fn next_f64(&mut self) -> f64 {
        let v = self.values[self.cursor % self.values.len()];
        self.cursor += 1;
        v
    }
}

// ---------------------------------------------------------------------------
// Dlvl modifier.
// ---------------------------------------------------------------------------

/// Port of `CalculateDlvlModifier`. `dlvl = defender.level - attacker.level`.
pub fn calculate_dlvl_modifier(dlvl: i16) -> f64 {
    let raw = if dlvl >= 0 {
        (0.35 * dlvl as f64) + 0.225
    } else {
        (0.01 * dlvl as f64) + 0.25
    };
    raw.clamp(0.0, 1.0)
}

// ---------------------------------------------------------------------------
// Damage mutators — change `action.amount`/`amount_mitigated` in-place.
// ---------------------------------------------------------------------------

fn clamp_damage(v: f64) -> u16 {
    v.clamp(0.0, 9999.0) as u16
}

pub fn calculate_physical_damage_taken(
    attacker: &CombatView<'_>,
    defender: &CombatView<'_>,
    action: &mut CommandResult,
) {
    let dlvl = defender.level - attacker.level;
    let dmg_down = 1.0 - defender.get_mod(Modifier::DamageTakenDown) / 100.0;
    let mitigation = calculate_dlvl_modifier(dlvl) * defender.get_mod(Modifier::Defense);
    let pre = (action.amount as f64 - mitigation).max(0.0);
    action.amount = clamp_damage(pre);
    action.amount = clamp_damage(action.amount as f64 * dmg_down);
}

pub fn calculate_spell_damage_taken(
    attacker: &CombatView<'_>,
    defender: &CombatView<'_>,
    action: &mut CommandResult,
) {
    let dlvl = defender.level - attacker.level;
    let dmg_down = 1.0 - defender.get_mod(Modifier::DamageTakenDown) / 100.0;
    let mitigation = calculate_dlvl_modifier(dlvl)
        * (defender.get_mod(Modifier::Defense) + 0.67 * defender.get_mod(Modifier::Vitality));
    let pre = (action.amount as f64 - mitigation).max(0.0);
    action.amount = clamp_damage(pre);
    action.amount = clamp_damage(action.amount as f64 * dmg_down);
}

pub fn calculate_block_damage(defender: &CombatView<'_>, action: &mut CommandResult) {
    let percent_blocked = if defender.has_aegis_boon {
        1.0
    } else {
        defender.get_mod(Modifier::Block) * 0.002 + defender.get_mod(Modifier::Vitality) * 0.001
    };
    action.amount_mitigated = clamp_damage(action.amount as f64 * percent_blocked);
    action.amount = clamp_damage(action.amount as f64 * (1.0 - percent_blocked));
}

pub fn calculate_crit_damage(
    attacker: &CombatView<'_>,
    defender: &CombatView<'_>,
    action: &mut CommandResult,
) {
    let dlvl = (defender.level - attacker.level) as f64;
    let mut bonus = 0.04 * (dlvl * dlvl) - 2.0 * dlvl;
    bonus += 1.20;
    let bonus = bonus.clamp(1.15, 1.75);
    action.amount = clamp_damage(action.amount as f64 * bonus);
}

pub fn calculate_parry_damage(action: &mut CommandResult) {
    let percent_parry = 0.75;
    action.amount_mitigated = clamp_damage(action.amount as f64 * (1.0 - percent_parry));
    action.amount = clamp_damage(action.amount as f64 * percent_parry);
}

pub fn calculate_resist_damage(action: &mut CommandResult) {
    // Single=1, Double=2, Triple=3, Full=4 → 25% per tier.
    let tier = match action.hit_type {
        HitType::SingleResist => 1,
        HitType::DoubleResist => 2,
        HitType::TripleResist => 3,
        HitType::FullResist => 4,
        _ => 0,
    } as f64;
    let percent_resist = 0.25 * tier;
    action.amount_mitigated = clamp_damage(action.amount as f64 * (1.0 - percent_resist));
    action.amount = clamp_damage(action.amount as f64 * percent_resist);
}

/// Stoneskin absorbs damage from `action.amount` and subtracts the absorbed
/// amount from the Stoneskin modifier. Mirrors `HandleStoneskin`.
pub fn handle_stoneskin(defender_mods: &mut ModifierMap, action: &mut CommandResult) {
    let stoneskin = defender_mods.get(Modifier::Stoneskin);
    let mitigation = (action.amount as f64).min(stoneskin);
    action.amount = clamp_damage(action.amount as f64 - mitigation);
    defender_mods.subtract(Modifier::Stoneskin, mitigation);
}

// ---------------------------------------------------------------------------
// Rate functions — return 0..=100.
// ---------------------------------------------------------------------------

pub fn get_hit_rate(
    attacker: &CombatView<'_>,
    defender: &CombatView<'_>,
    skill: Option<&BattleCommand>,
) -> f64 {
    let mut hit_rate = 80.0;
    let hit_buff = attacker.get_mod(Modifier::RawHitRate);
    let evade_buff = defender.get_mod(Modifier::RawEvadeRate);
    let accuracy_modifier = skill.map(|s| s.accuracy_modifier).unwrap_or(0.0) as f64;
    hit_rate += (hit_buff + accuracy_modifier).clamp(0.0, 100.0);
    hit_rate -= evade_buff;
    hit_rate.clamp(0.0, 100.0)
}

pub fn get_parry_rate(defender: &CombatView<'_>, action: &CommandResult) -> f64 {
    // Shield prevents parry; can't parry from the rear.
    if defender.get_mod(Modifier::CanBlock) != 0.0 || action.param == HitDirection::REAR.bits() {
        return 0.0;
    }
    let mut parry = 10.0;
    parry += defender.get_mod(Modifier::Parry) * 0.1;
    parry + defender.get_mod(Modifier::RawParryRate)
}

pub fn get_crit_rate(
    attacker: &CombatView<'_>,
    action: &CommandResult,
    skill: Option<&BattleCommand>,
) -> f64 {
    if action.action_type == ActionType::Status {
        return 0.0;
    }
    let mut crit = 10.0;
    crit += 0.16 * skill.map(|s| s.bonus_crit_rate).unwrap_or(0.0) as f64;
    crit + attacker.get_mod(Modifier::RawCritRate)
}

pub fn get_resist_rate(defender: &CombatView<'_>, action: &CommandResult) -> f64 {
    let is_spell = action.command_type == CommandType::SPELL;
    let is_ele = matches!(
        action.action_property,
        ActionProperty::Fire
            | ActionProperty::Ice
            | ActionProperty::Wind
            | ActionProperty::Earth
            | ActionProperty::Lightning
            | ActionProperty::Water
            | ActionProperty::Astral
            | ActionProperty::Umbral
    );
    if !is_spell && !is_ele {
        return 0.0;
    }
    15.0 + defender.get_mod(Modifier::RawResistRate)
}

pub fn get_block_rate(
    attacker: &CombatView<'_>,
    defender: &CombatView<'_>,
    action: &CommandResult,
) -> f64 {
    // Need a shield, can't block from rear.
    if defender.get_mod(Modifier::CanBlock) == 0.0 || action.param == HitDirection::REAR.bits() {
        return 0.0;
    }
    let dlvl = (defender.level - attacker.level) as f64;
    let mut block = 2.5 * dlvl + 5.0;
    block += defender.get_mod(Modifier::Dexterity) * 0.1;
    block += defender.get_mod(Modifier::BlockRate) * 0.2;
    block.min(25.0) + defender.get_mod(Modifier::RawBlockRate)
}

// ---------------------------------------------------------------------------
// Try-* hit probes — roll, set hit type + damage, report whether it fired.
// ---------------------------------------------------------------------------

pub fn try_miss(
    attacker: &CombatView<'_>,
    defender: &CombatView<'_>,
    skill: Option<&BattleCommand>,
    action: &mut CommandResult,
    rng: &mut dyn Rng,
) -> bool {
    let roll = rng.next_f64() * 100.0;
    if roll >= get_hit_rate(attacker, defender, skill) {
        action.hit_type = HitType::Miss;
        action.amount_mitigated = action.amount;
        action.amount = 0;
        return true;
    }
    false
}

pub fn try_crit(
    attacker: &CombatView<'_>,
    defender: &CombatView<'_>,
    skill: Option<&BattleCommand>,
    action: &mut CommandResult,
    rng: &mut dyn Rng,
) -> bool {
    let roll = rng.next_f64() * 100.0;
    if roll <= action.crit_rate {
        action.hit_type = HitType::Crit;
        calculate_crit_damage(attacker, defender, action);
        return true;
    }
    // Keep the skill's rate fresh.
    let _ = get_crit_rate(attacker, action, skill);
    false
}

pub fn try_resist(action: &mut CommandResult, rng: &mut dyn Rng) -> bool {
    let mut rate = action.resist_rate;
    let mut i: i32 = -1;
    while rng.next_f64() * 100.0 <= rate && i < 4 {
        rate /= 2.0;
        i += 1;
    }
    if i != -1 {
        action.hit_type = match i {
            0 => HitType::SingleResist,
            1 => HitType::DoubleResist,
            2 => HitType::TripleResist,
            _ => HitType::FullResist,
        };
        calculate_resist_damage(action);
        return true;
    }
    false
}

pub fn try_block(defender: &CombatView<'_>, action: &mut CommandResult, rng: &mut dyn Rng) -> bool {
    if rng.next_f64() * 100.0 <= action.block_rate {
        action.hit_type = HitType::Block;
        calculate_block_damage(defender, action);
        return true;
    }
    false
}

pub fn try_parry(action: &mut CommandResult, rng: &mut dyn Rng) -> bool {
    if rng.next_f64() * 100.0 <= action.parry_rate {
        action.hit_type = HitType::Parry;
        calculate_parry_damage(action);
        return true;
    }
    false
}

// ---------------------------------------------------------------------------
// Rate calc — mirror of CommandResult.CalcRates.
// ---------------------------------------------------------------------------

pub fn calc_rates(
    attacker: &CombatView<'_>,
    defender: &CombatView<'_>,
    skill: Option<&BattleCommand>,
    action: &mut CommandResult,
) {
    action.hit_rate = get_hit_rate(attacker, defender, skill);
    action.crit_rate = get_crit_rate(attacker, action, skill);
    action.block_rate = get_block_rate(attacker, defender, action);
    action.parry_rate = get_parry_rate(defender, action);
    action.resist_rate = get_resist_rate(defender, action);
}

// ---------------------------------------------------------------------------
// Hit-effect painters — build the bitfield on `action.effect_id`.
// ---------------------------------------------------------------------------

pub fn set_hit_effect_physical(
    defender: &CombatView<'_>,
    skill: Option<&mut BattleCommand>,
    action: &mut CommandResult,
) {
    let mut effect = HitEffect::HIT_EFFECT_TYPE;

    if action.hit_type == HitType::Crit {
        effect |= HitEffect::CRITICAL_HIT;
    } else {
        let percent_dealt = 100.0 * action.amount as f64 / defender.max_hp.max(1) as f64;
        if percent_dealt > 5.0 {
            effect |= HitEffect::RECOIL_LV2;
        } else if percent_dealt > 10.0 {
            effect |= HitEffect::RECOIL_LV3;
        }
    }

    effect |= physical_hit_effect(action.hit_type);

    if let Some(skill) = skill
        && skill.is_combo
        && action.action_landed()
        && !skill.combo_effect_added
    {
        // combo_step << 15 lines up with SKILL_COMBO{1,2,3,4}.
        let combo_bit = (skill.combo_step.max(0) as u32) << 15;
        effect |= HitEffect::from(combo_bit);
        skill.combo_effect_added = true;
    }

    if action.hit_type as u16 >= HitType::Parry as u16 {
        if defender.has_protect {
            effect |= HitEffect::PROTECT;
        }
        if defender.has_stoneskin {
            effect |= HitEffect::STONESKIN;
        }
    }

    action.effect_id = effect.bits();
}

pub fn set_hit_effect_spell(
    defender: &CombatView<'_>,
    skill: Option<&mut BattleCommand>,
    action: &mut CommandResult,
) {
    let mut effect = HitEffect::MAGIC_EFFECT_TYPE;
    effect |= magical_hit_effect(action.hit_type);

    if let Some(skill) = skill
        && skill.is_combo
        && !skill.combo_effect_added
    {
        let combo_bit = (skill.combo_step.max(0) as u32) << 15;
        effect |= HitEffect::from(combo_bit);
        skill.combo_effect_added = true;
    }

    if action.action_landed() && defender.has_shell {
        effect |= HitEffect::MAGIC_SHELL;
    }
    action.effect_id = effect.bits();
}

pub fn set_hit_effect_heal(action: &mut CommandResult) {
    let mut effect = HitEffect::MAGIC_EFFECT_TYPE | HitEffect::HEAL;
    effect |= HitEffect::RECOIL_LV3;
    action.effect_id = effect.bits();
}

pub fn set_hit_effect_status(skill: &BattleCommand, action: &mut CommandResult) {
    let effect = HitEffect::STATUS_EFFECT_TYPE.bits() | skill.status_id;
    action.effect_id = effect;
    action.hit_type = HitType::Hit;
}

// ---------------------------------------------------------------------------
// Full hit chains — mirror FinishAction* without doing DB/packet work.
// The returned `DamageRequest` tells the caller (game loop) what HP delta
// to apply and whether to trigger hate.
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy)]
pub struct DamageRequest {
    pub target_actor_id: u32,
    pub amount: u16,
    pub enmity: u16,
}

#[derive(Debug, Clone, Copy)]
pub struct HealRequest {
    pub target_actor_id: u32,
    pub amount: u16,
}

pub fn finish_action_physical(
    attacker: &CombatView<'_>,
    defender: &CombatView<'_>,
    defender_mods: &mut ModifierMap,
    mut skill: Option<&mut BattleCommand>,
    action: &mut CommandResult,
    results: &mut CommandResultContainer,
    rng: &mut dyn Rng,
) -> Option<DamageRequest> {
    if !try_miss(attacker, defender, skill.as_deref(), action, rng) {
        handle_stoneskin(defender_mods, action);

        if !try_crit(attacker, defender, skill.as_deref(), action, rng)
            && !try_block(defender, action, rng)
            && !try_parry(action, rng)
        {
            action.hit_type = HitType::Hit;
        }
    }

    let multi_hit = matches!(skill.as_deref(), Some(s) if s.num_hits > 1);
    if multi_hit && action.hit_num == 1 {
        // Add the "You use [command] on [target]" preamble.
        results.add_action(CommandResult {
            target_actor_id: attacker.actor_id,
            world_master_text_id: 30441,
            effect_id: 0,
            hit_num: 1,
            ..Default::default()
        });
    }

    action.world_master_text_id = if multi_hit {
        multi_hit_text_id(action.hit_type)
    } else {
        physical_hit_text_id(action.hit_type)
    };

    set_hit_effect_physical(defender, skill.as_deref_mut(), action);
    calculate_physical_damage_taken(attacker, defender, action);

    let enmity_mod = skill.as_deref().map(|s| s.enmity_modifier).unwrap_or(1.0) as f64;
    action.enmity = ((action.enmity as f64) * enmity_mod).min(u16::MAX as f64) as u16;

    let request = if defender.actor_id == 0 {
        None
    } else {
        Some(DamageRequest {
            target_actor_id: defender.actor_id,
            amount: action.amount,
            enmity: action.enmity,
        })
    };

    results.add_action(action.clone());
    request
}

pub fn finish_action_spell(
    attacker: &CombatView<'_>,
    defender: &CombatView<'_>,
    defender_mods: &mut ModifierMap,
    skill: Option<&mut BattleCommand>,
    action: &mut CommandResult,
    results: &mut CommandResultContainer,
    rng: &mut dyn Rng,
) -> Option<DamageRequest> {
    // C# calls HandleStoneskin twice — once pre-resist, once post. We
    // preserve the double-apply because the second hit will be a no-op
    // if the first consumed the pool.
    handle_stoneskin(defender_mods, action);

    if !try_resist(action, rng) && !try_crit(attacker, defender, skill.as_deref(), action, rng) {
        action.hit_type = HitType::Hit;
    }

    action.world_master_text_id = magical_hit_text_id(action.hit_type);
    set_hit_effect_spell(defender, skill, action);

    handle_stoneskin(defender_mods, action);
    calculate_spell_damage_taken(attacker, defender, action);

    results.add_action(action.clone());
    if defender.actor_id == 0 {
        None
    } else {
        Some(DamageRequest {
            target_actor_id: defender.actor_id,
            amount: action.amount,
            enmity: action.enmity,
        })
    }
}

pub fn finish_action_heal(
    _caster: &CombatView<'_>,
    target: &CombatView<'_>,
    action: &mut CommandResult,
    results: &mut CommandResultContainer,
) -> HealRequest {
    set_hit_effect_heal(action);
    results.add_action(action.clone());
    HealRequest {
        target_actor_id: target.actor_id,
        amount: action.amount,
    }
}

/// Try to apply a status as a side effect of a command. Returns `true` if
/// the status should be added by the caller — we just report the roll.
pub fn try_status(action: &CommandResult, skill: &BattleCommand, rng: &mut dyn Rng) -> bool {
    if skill.status_id == 0 {
        return false;
    }
    if !action.action_landed() {
        return false;
    }
    // C# compares NextDouble() (0..1) to statusChance directly — values above
    // 1.0 are effectively "always lands". Match that.
    rng.next_f64() < skill.status_chance as f64
}

pub fn finish_action_status(
    skill: &BattleCommand,
    action: &mut CommandResult,
    results: &mut CommandResultContainer,
) {
    set_hit_effect_status(skill, action);
    results.add_action(action.clone());
}

// ---------------------------------------------------------------------------
// Position helpers.
// ---------------------------------------------------------------------------

pub fn hit_dir_to_position(dir: HitDirection) -> super::command::BattleCommandPositionBonus {
    use super::command::BattleCommandPositionBonus as P;
    if dir.contains(HitDirection::FRONT) {
        P::Front
    } else if dir.contains(HitDirection::RIGHT) || dir.contains(HitDirection::LEFT) {
        P::Flank
    } else if dir.contains(HitDirection::REAR) {
        P::Rear
    } else {
        P::None
    }
}

// ---------------------------------------------------------------------------
// AttackUtils — auto-attack damage placeholder. The C# version returns a
// pseudorandom 0..90 value; we mirror it while exposing the rng.
// ---------------------------------------------------------------------------

pub fn attack_calculate_base_damage(rng: &mut dyn Rng) -> i32 {
    (rng.next_f64() * 10.0) as i32 * 10
}

pub fn attack_calculate_damage(
    _attacker: &CombatView<'_>,
    _defender: &CombatView<'_>,
    rng: &mut dyn Rng,
) -> i32 {
    attack_calculate_base_damage(rng)
}

// ---------------------------------------------------------------------------
// EXP helpers.
// ---------------------------------------------------------------------------

/// Port of `GetBaseEXP`. `party_member_count` is 1 when solo and the group
/// size otherwise — the C# party_modifier is 0.667 for 2+ members.
pub fn get_base_exp(player_level: i16, mob_level: i16, party_member_count: usize) -> u16 {
    let dlvl = mob_level as i32 - player_level as i32;
    if dlvl <= -20 {
        return 0;
    }
    let base_level = player_level
        .min(mob_level)
        .max(1)
        .min(BASE_EXP.len() as i16) as usize;
    let base_exp = BASE_EXP[base_level - 1] as f64;

    let mut dlvl_modifier = 1.0;
    if dlvl >= 0 {
        dlvl_modifier += 0.2 * dlvl as f64;
    } else {
        dlvl_modifier += 0.1 * dlvl as f64 + 0.0025 * (dlvl * dlvl) as f64;
    }

    let party_modifier = if party_member_count <= 1 { 1.0 } else { 0.667 };
    (base_exp * dlvl_modifier * party_modifier).clamp(0.0, u16::MAX as f64) as u16
}

pub fn get_link_bonus(link_count: u16) -> u8 {
    match link_count {
        0 => 0,
        1 => 25,
        2 => 50,
        3 => 75,
        _ => 100,
    }
}

pub fn get_chain_bonus(tier: u16) -> u8 {
    match tier {
        0 => 0,
        1 => 20,
        2 => 25,
        3 => 30,
        4 => 40,
        _ => 50,
    }
}

pub fn get_chain_time_limit(tier: u16) -> u8 {
    match tier {
        0 => 100,
        1 => 80,
        2 => 60,
        3 => 20,
        _ => 10,
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn zero_mods() -> ModifierMap {
        ModifierMap::default()
    }

    fn stub_view<'a>(
        level: i16,
        max_hp: i16,
        mods: &'a ModifierMap,
        actor_id: u32,
    ) -> CombatView<'a> {
        CombatView {
            actor_id,
            level,
            max_hp,
            mods,
            has_aegis_boon: false,
            has_protect: false,
            has_shell: false,
            has_stoneskin: false,
        }
    }

    #[test]
    fn dlvl_modifier_clamps() {
        assert!((calculate_dlvl_modifier(0) - 0.225).abs() < 1e-6);
        assert!((calculate_dlvl_modifier(2) - 0.925).abs() < 1e-6);
        // +3 gives 0.35*3 + 0.225 = 1.275 -> clamped to 1.0.
        assert!((calculate_dlvl_modifier(3) - 1.0).abs() < 1e-6);
        assert!((calculate_dlvl_modifier(-50) - 0.0).abs() < 1e-6);
    }

    #[test]
    fn stoneskin_absorbs_damage() {
        let mut mods = ModifierMap::default();
        mods.set(Modifier::Stoneskin, 30.0);
        let mut action = CommandResult::for_target(1, 0, 0);
        action.amount = 100;
        handle_stoneskin(&mut mods, &mut action);
        assert_eq!(action.amount, 70);
        assert_eq!(mods.get(Modifier::Stoneskin), 0.0);
    }

    #[test]
    fn stoneskin_caps_at_pool() {
        let mut mods = ModifierMap::default();
        mods.set(Modifier::Stoneskin, 200.0);
        let mut action = CommandResult::for_target(1, 0, 0);
        action.amount = 50;
        handle_stoneskin(&mut mods, &mut action);
        assert_eq!(action.amount, 0);
        assert_eq!(mods.get(Modifier::Stoneskin), 150.0);
    }

    #[test]
    fn physical_damage_scales_with_defense() {
        let mut atk_mods = zero_mods();
        let mut def_mods = zero_mods();
        def_mods.set(Modifier::Defense, 100.0);
        let atk = stub_view(10, 2000, &atk_mods, 1);
        let def = stub_view(10, 2000, &def_mods, 2);
        let mut action = CommandResult::for_target(2, 0, 0);
        action.amount = 500;
        calculate_physical_damage_taken(&atk, &def, &mut action);
        // Dlvl=0 -> modifier 0.225; 500 - 0.225*100 = 477.5 floor 477.
        assert_eq!(action.amount, 477);
        // Silence unused warning.
        let _ = &mut atk_mods;
    }

    #[test]
    fn try_miss_uses_rng() {
        let mods = zero_mods();
        let atk = stub_view(10, 2000, &mods, 1);
        let def = stub_view(10, 2000, &mods, 2);
        let mut action = CommandResult::for_target(2, 0, 0);
        action.amount = 100;

        let mut rng = FixedRng::new(&[0.99]); // guaranteed miss (99 >= 80)
        let missed = try_miss(&atk, &def, None, &mut action, &mut rng);
        assert!(missed);
        assert_eq!(action.amount, 0);
        assert_eq!(action.amount_mitigated, 100);

        action.amount = 100;
        action.amount_mitigated = 0;
        let mut rng = FixedRng::new(&[0.0]); // guaranteed hit (0 < 80)
        let missed = try_miss(&atk, &def, None, &mut action, &mut rng);
        assert!(!missed);
        assert_eq!(action.amount, 100);
    }

    #[test]
    fn resist_tier_math() {
        let mut action = CommandResult::for_target(1, 0, 0);
        action.amount = 1000;
        action.hit_type = HitType::DoubleResist;
        calculate_resist_damage(&mut action);
        // 50% resist -> 500 mitigated, 500 damage.
        assert_eq!(action.amount, 500);
        assert_eq!(action.amount_mitigated, 500);
    }

    #[test]
    fn block_rate_zero_without_shield() {
        let atk_mods = zero_mods();
        let def_mods = zero_mods(); // no CanBlock
        let atk = stub_view(10, 2000, &atk_mods, 1);
        let def = stub_view(10, 2000, &def_mods, 2);
        let action = CommandResult::for_target(2, 0, 0);
        assert_eq!(get_block_rate(&atk, &def, &action), 0.0);
    }

    #[test]
    fn exp_curve() {
        // Level 50 vs level 45 enemy, solo party: 400 * (1 + 0.1*-5 + 0.0025*25) * 1.0
        // = 400 * (1 - 0.5 + 0.0625) = 400 * 0.5625 = 225
        assert_eq!(get_base_exp(50, 45, 1), 225);
        // 2-member party: * 0.667 = 150.07 → 150
        assert_eq!(get_base_exp(50, 45, 2), 150);
        // Below -19 dlvl yields 0.
        assert_eq!(get_base_exp(50, 30, 1), 0);
    }

    #[test]
    fn link_and_chain_bonus_tables() {
        assert_eq!(get_link_bonus(0), 0);
        assert_eq!(get_link_bonus(2), 50);
        assert_eq!(get_link_bonus(99), 100);

        assert_eq!(get_chain_bonus(0), 0);
        assert_eq!(get_chain_bonus(2), 25);
        assert_eq!(get_chain_bonus(99), 50);

        assert_eq!(get_chain_time_limit(0), 100);
        assert_eq!(get_chain_time_limit(2), 60);
        assert_eq!(get_chain_time_limit(99), 10);
    }
}
