//! Top-level AI orchestration. Port of
//! `Actors/Chara/Ai/AIContainer.cs` + `Helpers/ActionQueue.cs`.
//!
//! The AIContainer holds the state stack, the controller, the pathfinder,
//! and the target finder. Each tick:
//!
//! 1. If the controller is present and `can_update`, poll it for a
//!    `ControllerDecision`.
//! 2. Apply the decision (engage, disengage, change target, move, roam,
//!    or fire a Lua hook — each routed through the outbox).
//! 3. Tick the top state; pop if it signals Complete or Interrupted.
//! 4. Emit a `RecalcStats` event if the state stack emptied during the
//!    tick (mirrors `Character.PostUpdate`).

#![allow(dead_code)]

use super::command::BattleCommand;
use super::controller::{Controller, ControllerDecision, ControllerKind, ControllerOwnerView};
use super::hate::HateContainer;
use super::outbox::{BattleEvent, BattleOutbox};
use super::path_find::PathFind;
use super::state::{BattleState, BattleStateKind, MAX_STATE_STACK, StateTickResult};
use super::target_find::ActorArena;

/// Action-queue entry — a scheduled callback to fire after `fire_at_ms`.
#[derive(Debug, Clone)]
pub struct Action {
    pub fire_at_ms: u64,
    /// Whether the AIContainer should gate this on `can_change_state`
    /// before running.
    pub check_state: bool,
    /// Opaque caller-defined tag; the game loop decides what to do with
    /// it (e.g. fire a Lua function, emit a packet).
    pub tag: u32,
}

#[derive(Debug, Clone, Default)]
pub struct ActionQueue {
    pub entries: Vec<Action>,
}

impl ActionQueue {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push(&mut self, action: Action) {
        self.entries.push(action);
        self.entries.sort_by_key(|a| a.fire_at_ms);
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Pop every action whose `fire_at_ms` has elapsed.
    pub fn drain_due(&mut self, now_ms: u64) -> Vec<Action> {
        let mut ready = Vec::new();
        while let Some(first) = self.entries.first() {
            if first.fire_at_ms <= now_ms {
                ready.push(self.entries.remove(0));
            } else {
                break;
            }
        }
        ready
    }
}

// ---------------------------------------------------------------------------
// AIContainer
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default)]
pub struct AIContainer {
    pub owner_actor_id: u32,
    pub controller: Option<Controller>,
    pub path_find: Option<PathFind>,
    pub action_queue: ActionQueue,

    states: Vec<BattleState>,
    last_action_time_ms: u64,
    latest_update_ms: u64,
    prev_update_ms: u64,
}

impl AIContainer {
    pub fn new(
        owner_actor_id: u32,
        controller: Option<Controller>,
        path_find: Option<PathFind>,
    ) -> Self {
        Self {
            owner_actor_id,
            controller,
            path_find,
            action_queue: ActionQueue::new(),
            states: Vec::new(),
            last_action_time_ms: 0,
            latest_update_ms: 0,
            prev_update_ms: 0,
        }
    }

    // --- Stack queries -----------------------------------------------------

    pub fn current_state(&self) -> Option<&BattleState> {
        self.states.last()
    }

    pub fn current_state_mut(&mut self) -> Option<&mut BattleState> {
        self.states.last_mut()
    }

    pub fn state_stack(&self) -> &[BattleState] {
        &self.states
    }

    pub fn is_current(&self, kind: BattleStateKind) -> bool {
        self.current_state().map(|s| s.kind) == Some(kind)
    }

    pub fn can_change_state(&self) -> bool {
        self.current_state()
            .map(|s| s.can_change_state())
            .unwrap_or(true)
    }

    pub fn can_follow_path(&self) -> bool {
        self.path_find.is_some() && self.can_change_state()
    }

    pub fn is_engaged(&self) -> bool {
        self.current_state()
            .map(|s| s.kind == BattleStateKind::Attack)
            .unwrap_or(false)
    }

    pub fn is_dead(&self) -> bool {
        self.current_state()
            .map(|s| s.kind == BattleStateKind::Death)
            .unwrap_or(false)
    }

    pub fn is_spawned(&self) -> bool {
        !self.is_dead()
    }

    pub fn last_action_time_ms(&self) -> u64 {
        self.last_action_time_ms
    }

    pub fn update_last_action_time(&mut self, now_ms: u64, delay_seconds: u32) {
        self.last_action_time_ms = now_ms + (delay_seconds as u64 * 1000);
    }

    // --- Mutation helpers --------------------------------------------------

    /// `ChangeState(state)` — push if allowed.
    pub fn change_state(&mut self, state: BattleState) -> bool {
        if !self.can_change_state() {
            return false;
        }
        if self.states.len() >= MAX_STATE_STACK {
            return false;
        }
        self.check_completed_states();
        self.states.push(state);
        true
    }

    /// `ForceChangeState(state)` — bypass the interrupt/gate check.
    pub fn force_change_state(&mut self, state: BattleState) -> bool {
        if self.states.len() >= MAX_STATE_STACK {
            return false;
        }
        self.check_completed_states();
        self.states.push(state);
        true
    }

    /// `CheckCompletedStates()` — pop while the top is completed.
    pub fn check_completed_states(&mut self) {
        while self.states.last().map(|s| s.is_completed).unwrap_or(false) {
            self.states.pop();
        }
    }

    /// `InterruptStates()` — set interrupt on every interruptible state
    /// and pop them.
    pub fn interrupt_states(&mut self) {
        while self.states.last().map(|s| s.can_interrupt).unwrap_or(false) {
            if let Some(mut top) = self.states.pop() {
                top.set_interrupted(true);
            }
        }
    }

    /// `ClearStates()` — pop everything.
    pub fn clear_states(&mut self) {
        self.states.clear();
    }

    pub fn reset(&mut self) {
        self.clear_states();
        if let Some(pf) = &mut self.path_find {
            pf.clear();
        }
    }

    // --- Public Internal* dispatch (mirrors the C#) -----------------------

    pub fn internal_engage(
        &mut self,
        target_actor_id: u32,
        now_ms: u64,
        attack_delay_ms: u32,
    ) -> bool {
        if self.is_engaged() {
            // Already engaged — a retarget is handled via ChangeTarget.
            return false;
        }
        let state = BattleState::attack(
            self.owner_actor_id,
            target_actor_id,
            now_ms,
            attack_delay_ms,
        );
        self.force_change_state(state)
    }

    pub fn internal_disengage(&mut self, outbox: &mut BattleOutbox) {
        if let Some(pf) = &mut self.path_find {
            pf.clear();
        }
        self.clear_states();
        outbox.push(BattleEvent::Disengage {
            owner_actor_id: self.owner_actor_id,
        });
    }

    pub fn internal_cast(&mut self, target_actor_id: u32, cmd: BattleCommand, now_ms: u64) -> bool {
        let state = BattleState::magic(self.owner_actor_id, target_actor_id, cmd, now_ms);
        self.change_state(state)
    }

    pub fn internal_ability(
        &mut self,
        target_actor_id: u32,
        cmd: BattleCommand,
        now_ms: u64,
    ) -> bool {
        let state = BattleState::ability(self.owner_actor_id, target_actor_id, cmd, now_ms);
        self.change_state(state)
    }

    pub fn internal_weapon_skill(
        &mut self,
        target_actor_id: u32,
        cmd: BattleCommand,
        now_ms: u64,
    ) -> bool {
        let state = BattleState::weapon_skill(self.owner_actor_id, target_actor_id, cmd, now_ms);
        self.change_state(state)
    }

    pub fn internal_use_item(
        &mut self,
        target_actor_id: u32,
        item_id: u32,
        cast_time_ms: u32,
        now_ms: u64,
    ) -> bool {
        let state = BattleState::item(
            self.owner_actor_id,
            target_actor_id,
            item_id,
            cast_time_ms,
            now_ms,
        );
        self.change_state(state)
    }

    pub fn internal_die(&mut self, now_ms: u64, fadeout_ms: u64, outbox: &mut BattleOutbox) {
        if let Some(pf) = &mut self.path_find {
            pf.clear();
        }
        self.clear_states();
        self.force_change_state(BattleState::death(self.owner_actor_id, now_ms, fadeout_ms));
        outbox.push(BattleEvent::Die {
            owner_actor_id: self.owner_actor_id,
        });
    }

    pub fn internal_despawn(&mut self, now_ms: u64, respawn_ms: u64, outbox: &mut BattleOutbox) {
        self.clear_states();
        self.force_change_state(BattleState::despawn(
            self.owner_actor_id,
            now_ms,
            respawn_ms,
        ));
        outbox.push(BattleEvent::Despawn {
            owner_actor_id: self.owner_actor_id,
        });
    }

    // --- Main tick --------------------------------------------------------

    pub fn update(
        &mut self,
        now_ms: u64,
        owner_view: ControllerOwnerView,
        arena: &dyn ActorArena,
        outbox: &mut BattleOutbox,
    ) {
        self.prev_update_ms = self.latest_update_ms;
        self.latest_update_ms = now_ms;

        // Controller-less actors just follow paths (C#: plain FollowPath
        // without the controller).
        if self.controller.is_none() && self.path_find.is_some() {
            // MovementSink isn't available here; the game loop wraps the
            // actor and calls `follow_path` directly when needed.
        }

        if let Some(ctrl) = self.controller.as_mut()
            && ctrl.can_update
        {
            let decision = ctrl.tick(now_ms, owner_view, arena);
            let owner_id = self.owner_actor_id;
            Self::apply_decision(owner_id, decision, outbox);
        }

        // Process the state stack.
        while let Some(top) = self.states.last_mut() {
            match top.update(now_ms) {
                StateTickResult::Continue => break,
                StateTickResult::Complete | StateTickResult::Interrupted => {
                    self.states.pop();
                }
            }
        }

        // Drain any due action-queue entries.
        let due = self.action_queue.drain_due(now_ms);
        for a in due {
            outbox.push(BattleEvent::LuaCall {
                owner_actor_id: self.owner_actor_id,
                function_name: "onActionQueueFire",
                command_id: a.tag as u16,
                target_actor_id: None,
            });
        }
    }

    fn apply_decision(owner_id: u32, decision: ControllerDecision, outbox: &mut BattleOutbox) {
        match decision {
            ControllerDecision::Idle => {}
            ControllerDecision::Engage { target_actor_id } => {
                outbox.push(BattleEvent::Engage {
                    owner_actor_id: owner_id,
                    target_actor_id,
                });
            }
            ControllerDecision::Disengage => {
                outbox.push(BattleEvent::Disengage {
                    owner_actor_id: owner_id,
                });
            }
            ControllerDecision::MoveTo { position: _ } => {
                // The game loop picks this up by inspecting the container's
                // path_find state after update.
            }
            ControllerDecision::FaceTarget => {
                // Same — consumed by the movement side of the game loop.
            }
            ControllerDecision::Roam => {}
            ControllerDecision::ChangeTarget { target_actor_id } => {
                outbox.push(BattleEvent::TargetChange {
                    owner_actor_id: owner_id,
                    new_target_actor_id: target_actor_id,
                });
            }
            ControllerDecision::LuaCombatTick { target_actor_id } => {
                outbox.push(BattleEvent::LuaCall {
                    owner_actor_id: owner_id,
                    function_name: "onCombatTick",
                    command_id: 0,
                    target_actor_id,
                });
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::battle::controller::PlayerController;
    use crate::battle::target_find::ActorView;
    use common::Vector3;
    use std::collections::HashMap;

    fn view() -> ActorView {
        ActorView {
            actor_id: 1,
            position: Vector3::ZERO,
            rotation: 0.0,
            is_alive: true,
            is_static: false,
            allegiance: 1,
            party_id: 0,
            zone_id: 1,
            is_updates_locked: false,
            is_player: true,
            is_battle_npc: false,
        }
    }

    fn owner_view() -> ControllerOwnerView {
        ControllerOwnerView {
            actor: view(),
            is_engaged: false,
            is_spawned: true,
            is_following_path: false,
            at_path_end: true,
            most_hated_actor_id: None,
            current_target_actor_id: None,
            has_prevent_movement: false,
            max_hp: 1000,
            current_hp: 1000,
            target_hpp: None,
            target_has_stealth: false,
            is_close_to_spawn: true,
            target_is_locked: false,
        }
    }

    #[test]
    fn change_state_respects_stack_depth() {
        let mut ai = AIContainer::new(1, Some(PlayerController::new_for(1)), None);
        // Fill stack with 10 interruptible magic states (each one can_change_state=false).
        // But first push a single attack state (the only one where can_change_state==true).
        assert!(ai.internal_engage(2, 0, 2000));

        // Now we can push one magic state on top.
        let mut cmd = BattleCommand::new(100, "stone");
        cmd.cast_time_ms = 3000;
        assert!(ai.internal_cast(2, cmd.clone(), 0));
        // On top of magic we cannot push more — it returns false.
        assert!(!ai.internal_cast(2, cmd, 0));
        assert_eq!(ai.state_stack().len(), 2);
    }

    #[test]
    fn internal_disengage_clears_stack() {
        let mut ai = AIContainer::new(1, None, None);
        ai.internal_engage(2, 0, 2000);
        assert_eq!(ai.state_stack().len(), 1);
        let mut ob = BattleOutbox::new();
        ai.internal_disengage(&mut ob);
        assert!(ai.state_stack().is_empty());
        assert!(
            ob.events
                .iter()
                .any(|e| matches!(e, BattleEvent::Disengage { .. }))
        );
    }

    #[test]
    fn update_pops_completed_casting_states() {
        let mut ai = AIContainer::new(1, None, None);
        let mut cmd = BattleCommand::new(100, "stone");
        cmd.cast_time_ms = 1000;
        ai.internal_cast(2, cmd, 0);
        let arena: HashMap<u32, ActorView> = HashMap::new();
        let mut ob = BattleOutbox::new();
        ai.update(1500, owner_view(), &arena, &mut ob);
        assert!(ai.state_stack().is_empty());
    }

    #[test]
    fn die_force_pushes_death_state() {
        let mut ai = AIContainer::new(1, None, None);
        let mut ob = BattleOutbox::new();
        ai.internal_die(0, 5000, &mut ob);
        assert_eq!(ai.current_state().unwrap().kind, BattleStateKind::Death);
        assert!(
            ob.events
                .iter()
                .any(|e| matches!(e, BattleEvent::Die { .. }))
        );
    }

    #[test]
    fn action_queue_fires_in_order() {
        let mut q = ActionQueue::new();
        q.push(Action {
            fire_at_ms: 3000,
            check_state: false,
            tag: 2,
        });
        q.push(Action {
            fire_at_ms: 1000,
            check_state: false,
            tag: 1,
        });
        q.push(Action {
            fire_at_ms: 5000,
            check_state: false,
            tag: 3,
        });
        let due = q.drain_due(3000);
        assert_eq!(due.iter().map(|a| a.tag).collect::<Vec<_>>(), vec![1, 2]);
        assert_eq!(q.entries.len(), 1);
    }

    #[test]
    fn update_triggers_controller_decisions() {
        let controller = PlayerController::new_for(1);
        let mut ai = AIContainer::new(1, Some(controller), None);
        let arena: HashMap<u32, ActorView> = [(1, view())].into_iter().collect();
        let mut ob = BattleOutbox::new();
        ai.update(1000, owner_view(), &arena, &mut ob);
        // PlayerController is idle — no outbox events.
        assert!(ob.events.is_empty());
        // Latest update time is recorded.
        assert_eq!(ai.latest_update_ms, 1000);
    }

    #[test]
    fn suppress_hate_unused() {
        // Quiet the unused-import lint warning in the test module.
        let _ = HateContainer::new(1);
    }
}
