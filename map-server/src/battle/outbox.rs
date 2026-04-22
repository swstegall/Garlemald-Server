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

//! Events emitted by battle-runtime mutations. Same pattern as the
//! inventory/status outboxes: the AI container, states, controllers, and
//! BattleUtils push events here; the game loop drains them per tick and
//! turns them into packet sends, DB writes, and Lua dispatches.

#![allow(dead_code)]

use super::command::{BattleCommand, CommandResult};
use super::effects::HitEffect;
use super::target_find::ValidTarget;

#[derive(Debug, Clone)]
pub enum BattleEvent {
    // ---- State-machine lifecycle ----------------------------------------
    /// Pushed when AIContainer.Engage succeeds — owner's main state becomes
    /// ACTIVE and `owner.target` is set.
    Engage {
        owner_actor_id: u32,
        target_actor_id: u32,
    },
    Disengage {
        owner_actor_id: u32,
    },
    /// Target-change packet when the focus actor switches without leaving
    /// combat.
    TargetChange {
        owner_actor_id: u32,
        new_target_actor_id: Option<u32>,
    },

    // ---- Action resolution ----------------------------------------------
    /// `owner.DoBattleAction(skillHandler, battleAnimation, results)` —
    /// broadcasts the `CommandResult` rows to all players around `owner`.
    DoBattleAction {
        owner_actor_id: u32,
        skill_handler: u32,
        battle_animation: u32,
        results: Vec<CommandResult>,
    },
    /// `owner.PlayAnimation(animation)` — smaller pre-action animation, no
    /// CommandResults.
    PlayAnimation {
        owner_actor_id: u32,
        animation: u32,
    },
    /// Emitted by `AIContainer::update` when the owner's `Attack` state
    /// reports `is_attack_ready`. The dispatcher looks up both sides,
    /// snapshots them through `CombatView`, runs `utils::finish_action_physical`
    /// with `utils::attack_calculate_damage` as the base, applies the HP
    /// delta, and fans out `DoBattleAction`. The AIContainer re-arms the
    /// next swing via `schedule_next_swing` before emitting this.
    ResolveAutoAttack {
        attacker_actor_id: u32,
        defender_actor_id: u32,
    },
    /// Emitted by `AIContainer::update` when a Magic/Ability/WeaponSkill
    /// state returns `Complete` — i.e. the cast timer elapsed. Carries the
    /// full skill so the dispatcher can route to
    /// `finish_action_{physical,spell,heal,status}` based on
    /// `command.action_type`.
    ResolveAction {
        attacker_actor_id: u32,
        defender_actor_id: u32,
        command: BattleCommand,
    },
    /// Cast-bar notification.
    CastStart {
        owner_actor_id: u32,
        command_id: u16,
        cast_time_ms: u32,
    },
    CastComplete {
        owner_actor_id: u32,
        command_id: u16,
    },
    CastInterrupted {
        owner_actor_id: u32,
        command_id: u16,
    },

    // ---- Hate / enmity --------------------------------------------------
    HateAdd {
        owner_actor_id: u32,
        target_actor_id: u32,
        amount: i32,
    },
    HateClear {
        owner_actor_id: u32,
        target_actor_id: Option<u32>,
    },

    // ---- Target finding (for area queries) ------------------------------
    /// AoE query placeholder — the game loop populates a target list by
    /// running TargetFind against the zone.
    QueryTargets {
        owner_actor_id: u32,
        main_target_actor_id: u32,
        valid_target: ValidTarget,
        hit_effect: HitEffect,
    },

    // ---- Lifecycle ------------------------------------------------------
    Die {
        owner_actor_id: u32,
    },
    /// Bring a dead actor back to life. For Players this is the
    /// home-point revive button; for NPCs the spawner's respawn timer
    /// pushes this after a cooldown. The dispatcher resets HP/MP to max,
    /// flips `current_main_state` back to `MAIN_STATE_PASSIVE`, and
    /// broadcasts the state change around the actor.
    Revive {
        owner_actor_id: u32,
    },
    Despawn {
        owner_actor_id: u32,
    },
    Spawn {
        owner_actor_id: u32,
    },
    /// `Character.RecalculateStats` — stat-mod changes due to traits,
    /// equipment, or status effects.
    RecalcStats {
        owner_actor_id: u32,
    },

    // ---- Lua hooks ------------------------------------------------------
    /// `LuaEngine.CallLuaBattleCommandFunction(caster, command, fn, …)` or
    /// similar. The args payload is opaque; the game-loop dispatcher
    /// resolves names at call time.
    LuaCall {
        owner_actor_id: u32,
        function_name: &'static str,
        command_id: u16,
        target_actor_id: Option<u32>,
    },

    // ---- Debug / text ---------------------------------------------------
    WorldMasterText {
        owner_actor_id: u32,
        text_id: u16,
    },
}

#[derive(Debug, Default)]
pub struct BattleOutbox {
    pub events: Vec<BattleEvent>,
}

impl BattleOutbox {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push(&mut self, event: BattleEvent) {
        self.events.push(event);
    }

    pub fn drain(&mut self) -> Vec<BattleEvent> {
        std::mem::take(&mut self.events)
    }

    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }

    pub fn len(&self) -> usize {
        self.events.len()
    }
}
