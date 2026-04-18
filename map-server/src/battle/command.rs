//! `BattleCommand` + `CommandResult` + `CommandResultContainer`.
//! Ported from `Actors/Chara/Ai/BattleCommand.cs` and
//! `Packets/Send/Actor/Battle/CommandResult.cs`.
//!
//! A `BattleCommand` is metadata: what a skill does, who it can target,
//! its cost, and its animations. A `CommandResult` is a single resolved
//! hit against one target — damage, hit type, effects. A `CommandResult
//! Container` accumulates multiple rows (for AoE and multi-hit skills)
//! before the whole batch is broadcast via `BattleOutbox::DoBattleAction`.

#![allow(dead_code)]

use super::effects::{ActionProperty, ActionType, HitType};
use super::target_find::{TargetFindAOETarget, TargetFindAOEType, ValidTarget};

// ---------------------------------------------------------------------------
// Supporting enums (mostly 1:1 with the C# definitions).
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct BattleCommandRequirements(pub u16);

impl BattleCommandRequirements {
    pub const NONE: Self = Self(0);
    pub const DISCIPLE_OF_WAR: Self = Self(0x01);
    pub const DISCIPLE_OF_MAGIC: Self = Self(0x02);
    pub const HAND_TO_HAND: Self = Self(0x04);
    pub const SWORD: Self = Self(0x08);
    pub const SHIELD: Self = Self(0x10);
    pub const AXE: Self = Self(0x20);
    pub const ARCHERY: Self = Self(0x40);
    pub const POLEARM: Self = Self(0x80);
    pub const THAUMATURGY: Self = Self(0x100);
    pub const CONJURY: Self = Self(0x200);

    pub const fn bits(self) -> u16 {
        self.0
    }
    pub const fn contains(self, other: Self) -> bool {
        (self.0 & other.0) == other.0
    }
}

impl std::ops::BitOr for BattleCommandRequirements {
    type Output = Self;
    fn bitor(self, rhs: Self) -> Self {
        Self(self.0 | rhs.0)
    }
}

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BattleCommandPositionBonus {
    #[default]
    None = 0,
    Front = 0x01,
    Rear = 0x02,
    Flank = 0x04,
}

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BattleCommandProcRequirement {
    #[default]
    None = 0,
    Miss = 1,
    Evade = 2,
    Parry = 3,
    Block = 4,
}

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BattleCommandValidUser {
    #[default]
    All = 0,
    Player = 1,
    Monster = 2,
}

#[repr(u16)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BattleCommandCastType {
    #[default]
    None = 0,
    WeaponSkill = 1,
    WeaponSkill2 = 2,
    BlackMagic = 3,
    WhiteMagic = 4,
    SongMagic = 8,
}

/// Action kind — what category of command is being dispatched.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct CommandType(pub u16);

impl CommandType {
    pub const NONE: Self = Self(0);
    pub const AUTO_ATTACK: Self = Self(1);
    pub const WEAPON_SKILL: Self = Self(2);
    pub const ABILITY: Self = Self(3);
    pub const SPELL: Self = Self(4);

    pub const fn bits(self) -> u16 {
        self.0
    }
    pub const fn contains(self, other: Self) -> bool {
        (self.0 & other.0) == other.0
    }
}

impl std::ops::BitOr for CommandType {
    type Output = Self;
    fn bitor(self, rhs: Self) -> Self {
        Self(self.0 | rhs.0)
    }
}

#[repr(u16)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum KnockbackType {
    #[default]
    None = 0,
    Level1 = 1,
    Level2 = 2,
    Level3 = 3,
    Level4 = 4,
    Level5 = 5,
    Clockwise1 = 6,
    Clockwise2 = 7,
    CounterClockwise1 = 8,
    CounterClockwise2 = 9,
    DrawIn = 10,
}

// ---------------------------------------------------------------------------
// BattleCommand — skill metadata.
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct BattleCommand {
    pub id: u16,
    pub name: String,
    pub job: u8,
    pub level: u8,
    pub requirements: BattleCommandRequirements,

    pub main_target: ValidTarget,
    pub valid_target: ValidTarget,
    pub aoe_type: TargetFindAOEType,
    pub aoe_target: TargetFindAOETarget,

    pub num_hits: u8,
    pub position_bonus: BattleCommandPositionBonus,
    pub proc_requirement: BattleCommandProcRequirement,

    pub range: f32,
    pub min_range: f32,
    pub aoe_range: f32,
    pub aoe_min_range: f32,
    pub aoe_cone_angle: f32,
    pub aoe_rotate_angle: f32,
    pub range_height: f32,
    pub range_width: f32,

    pub status_id: u32,
    pub status_duration: u32,
    pub status_chance: f32,
    pub status_tier: u8,
    pub status_magnitude: f64,

    pub cast_type: u8,
    pub cast_time_ms: u32,
    pub recast_time_ms: u32,
    pub max_recast_time_seconds: u32,
    pub mp_cost: i16,
    pub tp_cost: i16,

    pub animation_type: u8,
    pub effect_animation: u16,
    pub model_animation: u16,
    pub animation_duration_seconds: u16,
    pub battle_animation: u32,
    pub world_master_text_id: u16,

    pub combo_next_command_id: [i32; 2],
    pub combo_step: i16,
    pub is_combo: bool,
    pub combo_effect_added: bool,
    pub is_ranged: bool,
    pub action_crit: bool,

    pub command_type: CommandType,
    pub action_property: ActionProperty,
    pub action_type: ActionType,

    pub base_potency: u16,
    pub enmity_modifier: f32,
    pub accuracy_modifier: f32,
    pub bonus_crit_rate: f32,

    pub valid_user: BattleCommandValidUser,
}

impl BattleCommand {
    pub fn new(id: u16, name: impl Into<String>) -> Self {
        Self {
            id,
            name: name.into(),
            job: 0,
            level: 0,
            requirements: BattleCommandRequirements::NONE,
            main_target: ValidTarget::NONE,
            valid_target: ValidTarget::NONE,
            aoe_type: TargetFindAOEType::None,
            aoe_target: TargetFindAOETarget::Target,
            num_hits: 0,
            position_bonus: BattleCommandPositionBonus::None,
            proc_requirement: BattleCommandProcRequirement::None,
            range: 0.0,
            min_range: 0.0,
            aoe_range: 0.0,
            aoe_min_range: 0.0,
            aoe_cone_angle: 0.0,
            aoe_rotate_angle: 0.0,
            range_height: 0.0,
            range_width: 0.0,
            status_id: 0,
            status_duration: 0,
            status_chance: 50.0,
            status_tier: 1,
            status_magnitude: 0.0,
            cast_type: 0,
            cast_time_ms: 0,
            recast_time_ms: 0,
            max_recast_time_seconds: 0,
            mp_cost: 0,
            tp_cost: 0,
            animation_type: 0,
            effect_animation: 0,
            model_animation: 0,
            animation_duration_seconds: 0,
            battle_animation: 0,
            world_master_text_id: 0,
            combo_next_command_id: [0, 0],
            combo_step: 0,
            is_combo: false,
            combo_effect_added: false,
            is_ranged: false,
            action_crit: false,
            command_type: CommandType::NONE,
            action_property: ActionProperty::None,
            action_type: ActionType::None,
            base_potency: 0,
            enmity_modifier: 1.0,
            accuracy_modifier: 0.0,
            bonus_crit_rate: 0.0,
            valid_user: BattleCommandValidUser::All,
        }
    }

    /// `IsSpell()` — true if this command has an MP cost or cast time.
    pub fn is_spell(&self) -> bool {
        self.mp_cost != 0 || self.cast_time_ms != 0
    }

    /// `IsInstantCast()` — no cast bar.
    pub fn is_instant_cast(&self) -> bool {
        self.cast_time_ms == 0
    }

    /// MP-cost scaling by level, matching the piecewise curve in the C#.
    pub fn calculate_mp_cost(&self, user_level: i16, combo_bonus: f32) -> u16 {
        if self.mp_cost == 0 {
            return 0;
        }
        let level = user_level as i32;
        let base = if level <= 10 {
            100 + level * 10
        } else if level <= 20 {
            200 + (level - 10) * 20
        } else if level <= 30 {
            400 + (level - 20) * 40
        } else if level <= 40 {
            800 + (level - 30) * 70
        } else if level <= 50 {
            1500 + (level - 40) * 130
        } else if level <= 60 {
            2800 + (level - 50) * 200
        } else if level <= 70 {
            4800 + (level - 60) * 320
        } else {
            8000 + (level - 70) * 500
        };
        let scaled = (base as f64 * self.mp_cost as f64 * 0.001).ceil();
        let with_combo = (scaled * (1.0 - combo_bonus as f64)).ceil();
        with_combo.clamp(0.0, u16::MAX as f64) as u16
    }

    /// Direct TP scaling — just applies the combo bonus.
    pub fn calculate_tp_cost(&self, combo_bonus: f32) -> i16 {
        let cost = self.tp_cost as f32 * (1.0 - combo_bonus);
        cost.ceil().clamp(i16::MIN as f32, i16::MAX as f32) as i16
    }

    /// Is `command_id` the next link in this command's combo chain?
    pub fn combo_matches(&self, command_id: i32) -> bool {
        self.combo_next_command_id[0] == command_id || self.combo_next_command_id[1] == command_id
    }

    pub fn get_command_type(&self) -> u16 {
        self.command_type.bits()
    }

    pub fn get_action_type(&self) -> u16 {
        self.action_type as u16
    }
}

// ---------------------------------------------------------------------------
// Errors returned by IsValidMainTarget — we use a typed enum instead of
// writing text ids into a CommandResult the way the C# does.
// ---------------------------------------------------------------------------

#[repr(u16)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TargetValidationError {
    TargetDoesNotExist = 32511,
    CannotTargetKoTarget = 32512,
    MustTargetKoTarget = 32513,
    CannotTargetSelf = 32514,
    MustTargetSelf = 32515,
    CannotTargetAlly = 32516,
    MustTargetAlly = 32517,
    CannotTargetEnemy = 32518,
    MustTargetEnemy = 32519,
    InvalidTarget = 32547,
    CannotTargetParty = 32548,
    OutOfZone = 32540,
}

// ---------------------------------------------------------------------------
// CommandResult — one resolved hit.
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default)]
pub struct CommandResult {
    pub target_actor_id: u32,
    pub amount: u16,
    pub amount_mitigated: u16,
    pub enmity: u16,
    pub world_master_text_id: u16,
    pub effect_id: u32,
    /// Which side the action is coming from (HitDirection bits).
    pub param: u8,
    pub hit_num: u8,

    pub animation: u32,
    pub command_type: CommandType,
    pub action_property: ActionProperty,
    pub action_type: ActionType,
    pub hit_type: HitType,

    pub parry_rate: f64,
    pub block_rate: f64,
    pub resist_rate: f64,
    pub hit_rate: f64,
    pub crit_rate: f64,
}

impl CommandResult {
    pub fn for_target(target_actor_id: u32, world_master_text_id: u16, effect_id: u32) -> Self {
        Self {
            target_actor_id,
            world_master_text_id,
            effect_id,
            hit_num: 1,
            hit_type: HitType::Hit,
            ..Default::default()
        }
    }

    /// Mirror of the C# `CommandResult(targetId, BattleCommand, …)` ctor.
    pub fn from_command(target_actor_id: u32, cmd: &BattleCommand, hit_num: u8) -> Self {
        Self {
            target_actor_id,
            world_master_text_id: cmd.world_master_text_id,
            command_type: cmd.command_type,
            action_property: cmd.action_property,
            action_type: cmd.action_type,
            hit_num,
            hit_type: HitType::Hit,
            ..Default::default()
        }
    }

    pub fn set_text_id(&mut self, id: u16) {
        self.world_master_text_id = id;
    }

    /// Whether the hit connected (non-miss/evade/resist).
    pub fn action_landed(&self) -> bool {
        self.hit_type.landed()
    }
}

// ---------------------------------------------------------------------------
// CommandResultContainer — accumulates per-target rows for one action.
// ---------------------------------------------------------------------------

#[derive(Debug, Default)]
pub struct CommandResultContainer {
    pub main_results: Vec<CommandResult>,
    pub additional_results: Vec<CommandResult>,
}

impl CommandResultContainer {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_action(&mut self, r: CommandResult) {
        self.main_results.push(r);
    }

    pub fn add_additional(&mut self, r: CommandResult) {
        self.additional_results.push(r);
    }

    /// Concatenate additional results onto main, matching the C# `CombineLists`.
    pub fn combine_lists(&mut self) -> Vec<CommandResult> {
        self.main_results.append(&mut self.additional_results);
        std::mem::take(&mut self.main_results)
    }

    pub fn is_empty(&self) -> bool {
        self.main_results.is_empty() && self.additional_results.is_empty()
    }
}

// ---------------------------------------------------------------------------
// Target validation — replaces IsValidMainTarget.
// ---------------------------------------------------------------------------

pub struct ActorSnapshot {
    pub actor_id: u32,
    pub is_alive: bool,
    pub is_static: bool,
    pub allegiance: u32,
    pub party_id: u64,
    pub zone_id: u32,
    pub is_updates_locked: bool,
}

impl BattleCommand {
    /// Port of `IsValidMainTarget(user, target)`. Returns Ok or a specific
    /// worldmaster text id that the caller can turn into a CommandResult.
    pub fn validate_main_target(
        &self,
        user: &ActorSnapshot,
        target: Option<&ActorSnapshot>,
    ) -> Result<(), TargetValidationError> {
        use TargetValidationError::*;
        let Some(target) = target else {
            return Err(TargetDoesNotExist);
        };

        if !self.main_target.intersects(ValidTarget::CORPSE) && !target.is_alive {
            return Err(CannotTargetKoTarget);
        }
        if self.main_target.intersects(ValidTarget::CORPSE_ONLY) && target.is_alive {
            return Err(MustTargetKoTarget);
        }

        let is_self = user.actor_id == target.actor_id;
        if !self.main_target.intersects(ValidTarget::SELF) && is_self {
            return Err(CannotTargetSelf);
        }
        if self.main_target.intersects(ValidTarget::SELF_ONLY) && !is_self {
            return Err(MustTargetSelf);
        }

        let is_ally = target.allegiance == user.allegiance;
        if !self.main_target.intersects(ValidTarget::ALLY) && is_ally && !is_self {
            return Err(CannotTargetAlly);
        }
        if self.main_target.intersects(ValidTarget::ALLY_ONLY) && !is_ally {
            return Err(MustTargetAlly);
        }
        if !self.main_target.intersects(ValidTarget::ENEMY) && !is_ally {
            return Err(CannotTargetEnemy);
        }
        if self.main_target.intersects(ValidTarget::ENEMY_ONLY) && is_ally {
            return Err(MustTargetEnemy);
        }

        let in_same_party = user.party_id != 0 && target.party_id == user.party_id;
        if !self.main_target.intersects(ValidTarget::PARTY) && in_same_party {
            return Err(CannotTargetParty);
        }
        if self.main_target.intersects(ValidTarget::PARTY_ONLY) && !in_same_party {
            return Err(InvalidTarget);
        }

        if !self.main_target.intersects(ValidTarget::NPC) && target.is_static {
            return Err(InvalidTarget);
        }
        if self.main_target.intersects(ValidTarget::NPC_ONLY) && !target.is_static {
            return Err(InvalidTarget);
        }

        if target.is_updates_locked {
            return Err(InvalidTarget);
        }
        if target.zone_id != user.zone_id {
            return Err(OutOfZone);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stub_command(id: u16) -> BattleCommand {
        let mut c = BattleCommand::new(id, "stub");
        c.main_target = ValidTarget::ENEMY;
        c.valid_target = ValidTarget::ENEMY;
        c
    }

    fn user_snap() -> ActorSnapshot {
        ActorSnapshot {
            actor_id: 1,
            is_alive: true,
            is_static: false,
            allegiance: 1,
            party_id: 0,
            zone_id: 100,
            is_updates_locked: false,
        }
    }

    fn target_snap() -> ActorSnapshot {
        ActorSnapshot {
            actor_id: 2,
            is_alive: true,
            is_static: false,
            allegiance: 2,
            party_id: 0,
            zone_id: 100,
            is_updates_locked: false,
        }
    }

    #[test]
    fn mp_cost_scales_with_level() {
        let mut c = BattleCommand::new(1, "thunder");
        c.mp_cost = 100; // base fraction
        // Level 10 → base 200; 200 * 100 * 0.001 = 20
        assert_eq!(c.calculate_mp_cost(10, 0.0), 20);
        // Level 50 → base 2800; 2800 * 100 * 0.001 = 280
        assert_eq!(c.calculate_mp_cost(50, 0.0), 280);
    }

    #[test]
    fn mp_cost_combo_reduces() {
        let mut c = BattleCommand::new(1, "thunder");
        c.mp_cost = 100;
        let no_combo = c.calculate_mp_cost(50, 0.0);
        let with_combo = c.calculate_mp_cost(50, 0.5);
        assert!(with_combo < no_combo);
    }

    #[test]
    fn validate_enemy_target_on_enemy_ok() {
        let c = stub_command(1);
        assert!(
            c.validate_main_target(&user_snap(), Some(&target_snap()))
                .is_ok()
        );
    }

    #[test]
    fn validate_rejects_dead_target() {
        let c = stub_command(1);
        let mut t = target_snap();
        t.is_alive = false;
        assert_eq!(
            c.validate_main_target(&user_snap(), Some(&t)),
            Err(TargetValidationError::CannotTargetKoTarget)
        );
    }

    #[test]
    fn validate_rejects_missing_target() {
        let c = stub_command(1);
        assert_eq!(
            c.validate_main_target(&user_snap(), None),
            Err(TargetValidationError::TargetDoesNotExist)
        );
    }

    #[test]
    fn validate_rejects_zoned_target() {
        let c = stub_command(1);
        let mut t = target_snap();
        t.zone_id = 999;
        assert_eq!(
            c.validate_main_target(&user_snap(), Some(&t)),
            Err(TargetValidationError::OutOfZone)
        );
    }
}
