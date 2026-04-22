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

//! AI / input controllers. Ported from
//! `Actors/Chara/Ai/Controllers/Controller.cs` + `PlayerController.cs`
//! + `BattleNpcController.cs` + `PetController.cs` + `AllyController.cs`.
//!
//! The C# hierarchy has one abstract base and four concrete subclasses
//! that override `Update`, `Engage`, `Cast`, `Ability`, etc. In Rust we
//! fold that into a single `Controller` struct tagged by `ControllerKind`,
//! with the kind-specific AI state carried on `owner_state`.
//!
//! `tick` returns a `ControllerDecision` that the `AIContainer` acts on;
//! the controller itself is pure bookkeeping — it never mutates the owner
//! directly, never sends packets, never rolls random numbers.

#![allow(dead_code)]

use common::Vector3;

use super::target_find::{ActorArena, ActorView};

/// Who's driving this controller.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ControllerKind {
    #[default]
    Player,
    BattleNpc,
    Ally,
    Pet,
}

/// Aggro detection bitfield. Matches the C# `DetectionType`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct DetectionType(pub u32);

impl DetectionType {
    pub const NONE: Self = Self(0);
    pub const SIGHT: Self = Self(1 << 0);
    pub const SOUND: Self = Self(1 << 1);
    pub const MAGIC: Self = Self(1 << 2);
    pub const LOW_HP: Self = Self(1 << 3);
    pub const IGNORE_LEVEL_DIFFERENCE: Self = Self(1 << 4);

    pub const fn contains(self, other: Self) -> bool {
        (self.0 & other.0) == other.0
    }
}

impl std::ops::BitOr for DetectionType {
    type Output = Self;
    fn bitor(self, rhs: Self) -> Self {
        Self(self.0 | rhs.0)
    }
}

/// Shared fields carried by every controller kind.
#[derive(Debug, Clone)]
pub struct Controller {
    pub owner_actor_id: u32,
    pub kind: ControllerKind,
    pub last_update_ms: u64,
    pub last_combat_tick_ms: u64,
    pub can_update: bool,
    pub auto_attack_enabled: bool,
    pub casting_enabled: bool,
    pub weapon_skill_enabled: bool,

    /// BattleNpc-only fields (zero for players).
    pub battle: BattleNpcControllerState,

    /// Pet-only: master actor id. 0 = no master.
    pub pet_master_actor_id: u32,
}

/// State specific to BattleNpcController / AllyController.
#[derive(Debug, Clone, Default)]
pub struct BattleNpcControllerState {
    pub detection_type: DetectionType,
    pub sight_range: f32,
    pub sound_range: f32,
    pub level: i16,
    pub neutral: bool,
    pub is_moving_to_spawn: bool,
    pub spawn_position: Vector3,
    pub roam_delay_seconds: u32,
    pub last_action_time_ms: u64,
    pub last_spell_cast_time_ms: u64,
    pub last_skill_time_ms: u64,
    pub deaggro_time_ms: u64,
    pub neutral_time_ms: u64,
    pub wait_time_ms: u64,
    pub battle_start_time_ms: u64,
    pub first_spell: bool,
    pub last_roam_update_ms: u64,
    /// Content-group member ids, populated by AllyController before each
    /// `tick`. Empty for plain BattleNpcs.
    pub content_group_ids: Vec<u32>,
}

impl Controller {
    pub fn new(kind: ControllerKind, owner_actor_id: u32) -> Self {
        Self {
            owner_actor_id,
            kind,
            last_update_ms: 0,
            last_combat_tick_ms: 0,
            can_update: true,
            auto_attack_enabled: true,
            casting_enabled: true,
            weapon_skill_enabled: true,
            battle: BattleNpcControllerState {
                first_spell: true,
                sight_range: 20.0,
                sound_range: 10.0,
                roam_delay_seconds: 5,
                ..Default::default()
            },
            pet_master_actor_id: 0,
        }
    }

    pub fn is_auto_attack_enabled(&self) -> bool {
        self.auto_attack_enabled
    }
    pub fn set_auto_attack_enabled(&mut self, v: bool) {
        self.auto_attack_enabled = v;
    }
    pub fn is_casting_enabled(&self) -> bool {
        self.casting_enabled
    }
    pub fn set_casting_enabled(&mut self, v: bool) {
        self.casting_enabled = v;
    }
    pub fn is_weapon_skill_enabled(&self) -> bool {
        self.weapon_skill_enabled
    }
    pub fn set_weapon_skill_enabled(&mut self, v: bool) {
        self.weapon_skill_enabled = v;
    }
}

/// A view of the owner for controller queries. Tied to `ActorView` but
/// adds the engagement state.
#[derive(Debug, Clone, Copy)]
pub struct ControllerOwnerView {
    pub actor: ActorView,
    pub is_engaged: bool,
    pub is_spawned: bool,
    pub is_following_path: bool,
    pub at_path_end: bool,
    pub most_hated_actor_id: Option<u32>,
    pub current_target_actor_id: Option<u32>,
    pub has_prevent_movement: bool,
    pub max_hp: i16,
    pub current_hp: i16,
    /// Current target's HPP 0..100, or None if no target.
    pub target_hpp: Option<u8>,
    /// Does the target have a visible Stealth mod > 0.
    pub target_has_stealth: bool,
    /// Has this actor moved "close to spawn" recently.
    pub is_close_to_spawn: bool,
    /// Does the current target belong to a hate entry whose owner hates it back?
    pub target_is_locked: bool,
    /// Milliseconds between auto-attack swings for the owner. Read from
    /// `Character::get_attack_delay_ms`; defaults to 2500 ms when no weapon
    /// delay is set.
    pub attack_delay_ms: u32,
}

/// What the controller wants the AIContainer to do this tick. Emitted by
/// `tick()`.
#[derive(Debug, Clone, PartialEq)]
pub enum ControllerDecision {
    /// No action needed — idle tick.
    Idle,
    /// Start combat with the given target.
    Engage { target_actor_id: u32 },
    /// Drop aggro + path back to spawn.
    Disengage,
    /// Walk toward this position (used for both combat pursuit + roaming).
    MoveTo { position: Vector3 },
    /// Stop pathing and face the current target.
    FaceTarget,
    /// Start roaming — path to a random point near spawn.
    Roam,
    /// Change the locked target, no engage.
    ChangeTarget { target_actor_id: Option<u32> },
    /// Fire a Lua combat-tick hook. The container dispatches by name.
    LuaCombatTick { target_actor_id: Option<u32> },
}

// ---------------------------------------------------------------------------
// Main tick dispatch.
// ---------------------------------------------------------------------------

/// 3-second combat-tick cadence, matching the C# `DoCombatTick` guard.
pub const COMBAT_TICK_INTERVAL_MS: u64 = 3_000;
/// Default roaming delay when the mob mod isn't set.
pub const DEFAULT_ROAM_DELAY_MS: u64 = 5_000;
/// Aggro scan radius.
pub const AGGRO_SCAN_RADIUS: f32 = 50.0;
/// Max aggro level delta when `IgnoreLevelDifference` isn't set.
pub const AGGRO_LEVEL_CAP: i16 = 10;
/// Maximum detection distance regardless of detection type.
pub const MAX_DETECT_DISTANCE: f32 = 20.0;
/// Max vertical gap for detection.
pub const MAX_DETECT_Y_DIFF: f32 = 8.0;

impl Controller {
    /// Main update entry point. Returns a decision for the AIContainer to
    /// act on. The controller caches `last_update_ms` + `last_combat_tick_ms`
    /// so repeated same-ms calls don't re-fire Lua hooks.
    pub fn tick(
        &mut self,
        now_ms: u64,
        owner: ControllerOwnerView,
        arena: &dyn ActorArena,
    ) -> ControllerDecision {
        if !self.can_update {
            return ControllerDecision::Idle;
        }
        self.last_update_ms = now_ms;

        match self.kind {
            ControllerKind::Player => ControllerDecision::Idle,
            ControllerKind::Pet => ControllerDecision::Idle,
            ControllerKind::BattleNpc | ControllerKind::Ally => {
                self.tick_battle_npc(now_ms, owner, arena)
            }
        }
    }

    fn tick_battle_npc(
        &mut self,
        now_ms: u64,
        owner: ControllerOwnerView,
        arena: &dyn ActorArena,
    ) -> ControllerDecision {
        if !owner.actor.is_alive {
            return ControllerDecision::Idle;
        }

        // Engaged → combat tick. Not engaged → try aggro, else roam.
        if owner.is_engaged {
            self.do_combat_tick(now_ms, owner)
        } else if let Some(target_id) = self.try_aggro(now_ms, owner, arena) {
            ControllerDecision::Engage {
                target_actor_id: target_id,
            }
        } else if self.battle.is_moving_to_spawn || self.battle.roam_delay_seconds > 0 {
            self.do_roam_tick(now_ms, owner)
        } else {
            ControllerDecision::Idle
        }
    }

    /// Port of `TryAggro` + `CanAggroTarget` + `CanDetectTarget`.
    fn try_aggro(
        &mut self,
        now_ms: u64,
        owner: ControllerOwnerView,
        arena: &dyn ActorArena,
    ) -> Option<u32> {
        if now_ms < self.battle.neutral_time_ms || self.battle.is_moving_to_spawn {
            return None;
        }
        if self.battle.neutral {
            return None;
        }
        // For AllyController: restrict to content group members.
        let is_ally = self.kind == ControllerKind::Ally;
        for candidate in arena.actors_around(owner.actor.actor_id, AGGRO_SCAN_RADIUS) {
            if candidate.actor_id == owner.actor.actor_id {
                continue;
            }
            if candidate.allegiance == owner.actor.allegiance {
                continue;
            }
            if !candidate.is_alive {
                continue;
            }
            if is_ally && !self.battle.content_group_ids.contains(&candidate.actor_id) {
                // Allies only aggro into content group fights.
                continue;
            }
            if !self.can_aggro(owner, candidate) {
                continue;
            }
            if !self.can_detect(owner, candidate) {
                continue;
            }
            return Some(candidate.actor_id);
        }
        None
    }

    fn can_aggro(&self, owner: ControllerOwnerView, target: ActorView) -> bool {
        if self.battle.neutral || self.battle.detection_type == DetectionType::NONE {
            return false;
        }
        if !target.is_alive {
            return false;
        }
        // Level diff check unless IgnoreLevelDifference is set.
        if !self
            .battle
            .detection_type
            .contains(DetectionType::IGNORE_LEVEL_DIFFERENCE)
        {
            let lvl_diff = (self.battle.level - owner_target_level(target)).abs();
            if lvl_diff > AGGRO_LEVEL_CAP {
                return false;
            }
        }
        // Only aggro when spawn state is "active".
        owner.is_spawned && !owner.is_engaged
    }

    fn can_detect(&self, owner: ControllerOwnerView, target: ActorView) -> bool {
        let dx = target.position.x - owner.actor.position.x;
        let dz = target.position.z - owner.actor.position.z;
        let dist = (dx * dx + dz * dz).sqrt();
        let y_diff = (target.position.y - owner.actor.position.y).abs();
        if y_diff > MAX_DETECT_Y_DIFF {
            return false;
        }
        if dist > MAX_DETECT_DISTANCE {
            return false;
        }

        let detect_type = self.battle.detection_type;
        let has_stealth = self.target_view_has_stealth(target);
        let is_facing =
            Self::is_facing(owner.actor.position, owner.actor.rotation, target.position);

        if detect_type.contains(DetectionType::SIGHT)
            && !has_stealth
            && is_facing
            && dist <= self.battle.sight_range
        {
            return true;
        }
        if detect_type.contains(DetectionType::SOUND)
            && !has_stealth
            && dist <= self.battle.sound_range
        {
            return true;
        }
        if detect_type.contains(DetectionType::LOW_HP) {
            // We don't know target HPP from ActorView, so reject; callers
            // that want LowHP aggro pre-filter the candidate list.
            return false;
        }
        false
    }

    fn target_view_has_stealth(&self, _target: ActorView) -> bool {
        // The underlying Character carries the Stealth mod, but ActorView
        // is intentionally narrow. Callers that need Stealth-aware
        // detection extend ActorView with a custom arena adapter.
        false
    }

    fn is_facing(origin: Vector3, rotation: f32, target: Vector3) -> bool {
        let dx = target.x - origin.x;
        let dz = target.z - origin.z;
        let angle_to = dx.atan2(dz);
        let diff = (angle_to - rotation).rem_euclid(std::f32::consts::TAU);
        let gap = diff.min(std::f32::consts::TAU - diff);
        gap <= (std::f32::consts::PI / 2.0)
    }

    fn do_combat_tick(&mut self, now_ms: u64, owner: ControllerOwnerView) -> ControllerDecision {
        // Deaggro when the target is gone or we've roamed too far from spawn.
        if self.should_deaggro(owner) {
            return ControllerDecision::Disengage;
        }

        // Drive hate → retarget focus if needed.
        if owner.most_hated_actor_id != owner.current_target_actor_id {
            return ControllerDecision::ChangeTarget {
                target_actor_id: owner.most_hated_actor_id,
            };
        }

        // Move toward target every combat tick until we're in attack range.
        // The AIContainer turns `MoveTo` into pathfinding.
        if let Some(target_id) = owner.current_target_actor_id
            && !owner.has_prevent_movement
            && now_ms.saturating_sub(self.last_combat_tick_ms) >= COMBAT_TICK_INTERVAL_MS
        {
            self.last_combat_tick_ms = now_ms;
            return ControllerDecision::LuaCombatTick {
                target_actor_id: Some(target_id),
            };
        }
        ControllerDecision::Idle
    }

    fn should_deaggro(&self, owner: ControllerOwnerView) -> bool {
        if owner.most_hated_actor_id.is_none() {
            return true;
        }
        if !owner.is_close_to_spawn {
            return true;
        }
        false
    }

    fn do_roam_tick(&mut self, now_ms: u64, _owner: ControllerOwnerView) -> ControllerDecision {
        if now_ms < self.battle.wait_time_ms {
            return ControllerDecision::Idle;
        }
        self.battle.neutral_time_ms = now_ms + 5_000;
        self.battle.wait_time_ms =
            now_ms + (self.battle.roam_delay_seconds as u64 * 1000).max(DEFAULT_ROAM_DELAY_MS);
        ControllerDecision::Roam
    }
}

fn owner_target_level(target: ActorView) -> i16 {
    // ActorView doesn't carry level directly; the BattleNpc arena adapter
    // is expected to synthesize one onto `allegiance` bits if needed. The
    // neutral default is 1, which means level_diff is tolerant when the
    // real level is unknown.
    let _ = target;
    1
}

// ---------------------------------------------------------------------------
// Legacy zero-sized wrappers — retained for Phase D naming parity. The
// real struct is `Controller` above; these just tag intent.
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default)]
pub struct PlayerController;
#[derive(Debug, Clone, Default)]
pub struct BattleNpcController;
#[derive(Debug, Clone, Default)]
pub struct AllyController;
#[derive(Debug, Clone, Default)]
pub struct PetController;

impl PlayerController {
    pub fn new_for(owner_actor_id: u32) -> Controller {
        Controller::new(ControllerKind::Player, owner_actor_id)
    }
}
impl BattleNpcController {
    pub fn new_for(owner_actor_id: u32) -> Controller {
        Controller::new(ControllerKind::BattleNpc, owner_actor_id)
    }
}
impl AllyController {
    pub fn new_for(owner_actor_id: u32) -> Controller {
        Controller::new(ControllerKind::Ally, owner_actor_id)
    }
}
impl PetController {
    pub fn new_for(owner_actor_id: u32) -> Controller {
        Controller::new(ControllerKind::Pet, owner_actor_id)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn view(id: u32, x: f32, z: f32, allegiance: u32, alive: bool) -> ActorView {
        ActorView {
            actor_id: id,
            position: Vector3::new(x, 0.0, z),
            rotation: 0.0,
            is_alive: alive,
            is_static: false,
            allegiance,
            party_id: 0,
            zone_id: 1,
            is_updates_locked: false,
            is_player: allegiance == 1,
            is_battle_npc: allegiance != 1,
        }
    }

    fn owner_view(
        alive: bool,
        engaged: bool,
        most_hated: Option<u32>,
        current: Option<u32>,
    ) -> ControllerOwnerView {
        ControllerOwnerView {
            actor: view(1, 0.0, 0.0, 2, alive),
            is_engaged: engaged,
            is_spawned: true,
            is_following_path: false,
            at_path_end: true,
            most_hated_actor_id: most_hated,
            current_target_actor_id: current,
            has_prevent_movement: false,
            max_hp: 1000,
            current_hp: 1000,
            target_hpp: Some(100),
            target_has_stealth: false,
            is_close_to_spawn: true,
            target_is_locked: false,
            attack_delay_ms: 2500,
        }
    }

    fn arena_with(actors: &[ActorView]) -> HashMap<u32, ActorView> {
        actors.iter().map(|a| (a.actor_id, *a)).collect()
    }

    #[test]
    fn player_controller_is_idle_each_tick() {
        let mut c = PlayerController::new_for(1);
        let arena = arena_with(&[view(1, 0.0, 0.0, 1, true)]);
        let out = c.tick(1_000, owner_view(true, false, None, None), &arena);
        assert_eq!(out, ControllerDecision::Idle);
    }

    #[test]
    fn battle_npc_engages_most_hated_target() {
        let mut c = BattleNpcController::new_for(1);
        c.battle.detection_type = DetectionType::SIGHT;
        c.battle.sight_range = 50.0;
        let player = view(10, 3.0, 0.0, 1, true);
        let owner_actor = view(1, 0.0, 0.0, 2, true);
        let arena = arena_with(&[owner_actor, player]);
        let out = c.tick(1_000, owner_view(true, false, None, None), &arena);
        match out {
            ControllerDecision::Engage { target_actor_id } => assert_eq!(target_actor_id, 10),
            other => panic!("unexpected: {:?}", other),
        }
    }

    #[test]
    fn battle_npc_retargets_on_hate_change() {
        let mut c = BattleNpcController::new_for(1);
        let arena = arena_with(&[view(1, 0.0, 0.0, 2, true)]);
        let out = c.tick(
            1_000,
            owner_view(true, /* engaged */ true, Some(20), Some(10)),
            &arena,
        );
        assert_eq!(
            out,
            ControllerDecision::ChangeTarget {
                target_actor_id: Some(20)
            }
        );
    }

    #[test]
    fn battle_npc_disengages_on_no_hate() {
        let mut c = BattleNpcController::new_for(1);
        let arena = arena_with(&[view(1, 0.0, 0.0, 2, true)]);
        let out = c.tick(1_000, owner_view(true, true, None, Some(10)), &arena);
        assert_eq!(out, ControllerDecision::Disengage);
    }

    #[test]
    fn battle_npc_disengages_if_roamed_too_far() {
        let mut c = BattleNpcController::new_for(1);
        let arena = arena_with(&[view(1, 0.0, 0.0, 2, true)]);
        let mut v = owner_view(true, true, Some(20), Some(20));
        v.is_close_to_spawn = false;
        let out = c.tick(1_000, v, &arena);
        assert_eq!(out, ControllerDecision::Disengage);
    }

    #[test]
    fn pet_controller_is_idle() {
        let mut c = PetController::new_for(1);
        let arena = arena_with(&[view(1, 0.0, 0.0, 2, true)]);
        let out = c.tick(1_000, owner_view(true, false, None, None), &arena);
        assert_eq!(out, ControllerDecision::Idle);
    }

    #[test]
    fn combat_tick_fires_lua_hook_every_3s() {
        let mut c = BattleNpcController::new_for(1);
        let arena = arena_with(&[view(1, 0.0, 0.0, 2, true)]);
        let v = owner_view(true, true, Some(10), Some(10));
        // First tick at t=3000 — exactly past the cadence window.
        let t1 = c.tick(3_000, v, &arena);
        assert!(matches!(t1, ControllerDecision::LuaCombatTick { .. }));
        // A second tick within the 3s window should be idle.
        let t2 = c.tick(3_500, v, &arena);
        assert_eq!(t2, ControllerDecision::Idle);
        // After the window elapses the hook fires again.
        let t3 = c.tick(6_500, v, &arena);
        assert!(matches!(t3, ControllerDecision::LuaCombatTick { .. }));
    }
}
