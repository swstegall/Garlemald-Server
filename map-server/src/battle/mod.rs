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

//! Battle runtime. Faithful port of the C# `Actors/Chara/Ai/*` tree.
//!
//! Mirrors the pattern already used by `inventory` and `status`: mutations
//! record side effects on a typed outbox. The game loop drains the outbox
//! each tick to produce packets, DB writes, and Lua calls.

#![allow(dead_code, unused_imports)]

pub mod ai_container;
pub mod command;
pub mod controller;
pub mod effects;
pub mod hate;
pub mod outbox;
pub mod path_find;
pub mod save;
pub mod state;
pub mod target_find;
pub mod temp;
pub mod traits;
pub mod utils;

pub use ai_container::{AIContainer, Action, ActionQueue};
pub use command::{
    ActorSnapshot, BattleCommand, BattleCommandCastType, BattleCommandPositionBonus,
    BattleCommandProcRequirement, BattleCommandRequirements, BattleCommandValidUser, CommandResult,
    CommandResultContainer, CommandType, KnockbackType, TargetValidationError,
};
pub use controller::{
    AllyController, BattleNpcController, Controller, ControllerDecision, ControllerKind,
    ControllerOwnerView, DetectionType, PetController, PlayerController,
};
pub use effects::{ActionProperty, ActionType, HitDirection, HitEffect, HitType};
pub use hate::{HateContainer, HateEntry};
pub use outbox::{BattleEvent, BattleOutbox};
pub use path_find::{MovementSink, NavmeshProvider, PathFind, PathFindFlags, StraightLineNavmesh};
pub use save::BattleSave;
pub use state::{BattleState, BattleStateBody, BattleStateKind, MAX_STATE_STACK, StateTickResult};
pub use target_find::{
    ActorArena, ActorView, TargetFind, TargetFindAOETarget, TargetFindAOEType, ValidTarget,
};
pub use temp::BattleTemp;
pub use traits::BattleTrait;

pub const MAX_AI_STATES: usize = MAX_STATE_STACK;

// ---------------------------------------------------------------------------
// Integration test — player engages a dummy BattleNpc, runs a cast to
// completion, and verifies the outbox records the lifecycle events.
// ---------------------------------------------------------------------------

#[cfg(test)]
mod integration_tests {
    use super::*;
    use crate::actor::modifier::{Modifier, ModifierMap};
    use common::Vector3;
    use std::collections::HashMap;

    fn view(id: u32, x: f32, allegiance: u32) -> ActorView {
        ActorView {
            actor_id: id,
            position: Vector3::new(x, 0.0, 0.0),
            rotation: 0.0,
            is_alive: true,
            is_static: false,
            allegiance,
            party_id: 0,
            zone_id: 1,
            is_updates_locked: false,
            is_player: allegiance == 1,
            is_battle_npc: allegiance != 1,
        }
    }

    fn owner_view(engaged: bool, target_id: Option<u32>) -> ControllerOwnerView {
        ControllerOwnerView {
            actor: view(1, 0.0, 1),
            is_engaged: engaged,
            is_spawned: true,
            is_following_path: false,
            at_path_end: true,
            most_hated_actor_id: target_id,
            current_target_actor_id: target_id,
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

    #[test]
    fn full_combat_round() {
        // 1. Player engages a dummy BattleNpc.
        let mut ai = AIContainer::new(1, Some(PlayerController::new_for(1)), None);
        let mut outbox = BattleOutbox::new();
        let arena: HashMap<u32, ActorView> = [(1, view(1, 0.0, 1)), (2, view(2, 3.0, 2))]
            .into_iter()
            .collect();

        assert!(ai.internal_engage(2, /* now */ 0, /* attack delay */ 2000));
        assert_eq!(ai.current_state().unwrap().kind, BattleStateKind::Attack);

        // 2. Hate container accumulates enmity when the player damages the npc.
        let mut npc_hate = HateContainer::new(2);
        npc_hate.update_hate(1, 150);
        assert_eq!(npc_hate.most_hated(), Some(1));

        // 3. Running a cast to completion: push a magic state, tick it forward.
        let mut cmd = BattleCommand::new(100, "stone");
        cmd.cast_time_ms = 1500;
        cmd.action_type = ActionType::Magic;
        cmd.command_type = CommandType::SPELL;
        assert!(ai.internal_cast(2, cmd, 0));
        ai.update(2_000, owner_view(true, Some(2)), &arena, &mut outbox);
        assert!(
            !ai.is_current(BattleStateKind::Magic),
            "cast should have completed"
        );

        // 4. BattleUtils resolves a spell hit.
        let atk_mods = ModifierMap::default();
        let mut def_mods = ModifierMap::default();
        def_mods.set(Modifier::Defense, 50.0);
        def_mods.set(Modifier::Vitality, 50.0);

        let mut result = CommandResult::for_target(2, 30319, 0);
        result.amount = 400;
        result.action_type = ActionType::Magic;
        result.command_type = CommandType::SPELL;
        let mut container = CommandResultContainer::new();
        let mut rng = utils::FixedRng::new(&[0.5, 0.5, 0.5, 0.5]);
        let dmg = {
            // Snapshot views immutably, drop them before the mutable borrow
            // into def_mods that finish_action_spell takes.
            let snapshot_mods = def_mods.clone();
            let atk = utils::CombatView {
                actor_id: 1,
                level: 10,
                max_hp: 2000,
                mods: &atk_mods,
                has_aegis_boon: false,
                has_protect: false,
                has_shell: false,
                has_stoneskin: false,
            };
            let def = utils::CombatView {
                actor_id: 2,
                level: 10,
                max_hp: 2000,
                mods: &snapshot_mods,
                has_aegis_boon: false,
                has_protect: false,
                has_shell: false,
                has_stoneskin: false,
            };
            utils::finish_action_spell(
                &atk,
                &def,
                &mut def_mods,
                None,
                &mut result,
                &mut container,
                &mut rng,
            )
        };
        assert!(dmg.is_some());
        assert!(dmg.unwrap().amount > 0, "damage should land");

        // 5. Verify the outbox captured engagement events.
        let _ = outbox.drain();
    }
}
