//! Battle-state machine base. Port of the inheritance tree rooted at
//! `Actors/Chara/Ai/State/State.cs`.
//!
//! The C# has an abstract `State` class with virtual `Update/OnStart/
//! OnInterrupt/OnComplete/CanChangeState/TryInterrupt/Cleanup` hooks. Each
//! concrete state (AttackState, MagicState, …) overrides a subset.
//!
//! In Rust we model that without dynamic dispatch: `BattleState` is a
//! tagged union (one `BattleStateKind` per state type), and an `impl
//! BattleState` method dispatches on the kind. The actual lifecycle
//! (push/pop, interrupt checks) is managed by `AIContainer`.

#![allow(dead_code)]

use super::command::BattleCommand;

/// Stack depth cap — matches the C# `AIContainer.MAX_STATES = 10`.
pub const MAX_STATE_STACK: usize = 10;

/// What kind of action this state is tracking.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BattleStateKind {
    Attack,
    Magic,
    Ability,
    WeaponSkill,
    Item,
    Death,
    Despawn,
    Inactive,
}

/// Per-state body. Each variant carries just the fields that kind needs.
#[derive(Debug, Clone)]
pub enum BattleStateBody {
    Attack {
        /// ms timestamp of the next attack swing.
        next_swing_ms: u64,
    },
    Magic {
        command: BattleCommand,
        /// ms timestamp when the cast finishes.
        cast_finish_ms: u64,
        /// Have we transmitted the cast-start packet yet?
        cast_started: bool,
    },
    Ability {
        command: BattleCommand,
        cast_finish_ms: u64,
    },
    WeaponSkill {
        command: BattleCommand,
        cast_finish_ms: u64,
    },
    Item {
        item_id: u32,
        cast_finish_ms: u64,
    },
    /// Death fadeout timer. `despawn_at_ms` is when the corpse should
    /// despawn if un-raised.
    Death {
        despawn_at_ms: u64,
    },
    /// Despawn respawn timer — state exits once `respawn_at_ms` elapses.
    Despawn {
        respawn_at_ms: u64,
    },
    Inactive {
        /// ms timestamp when the lock ends. 0 means "indefinite".
        end_ms: u64,
    },
}

#[derive(Debug, Clone)]
pub struct BattleState {
    pub kind: BattleStateKind,
    pub body: BattleStateBody,
    pub owner_actor_id: u32,
    /// 0 = no target (self-cast / death / despawn).
    pub target_actor_id: u32,
    pub start_time_ms: u64,
    pub can_interrupt: bool,
    pub interrupt: bool,
    pub is_completed: bool,
    /// World-master text id for the "error" case, or 0 if none.
    pub error_text_id: u16,
}

/// Outcome of `update` — tells `AIContainer` what to do with the state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StateTickResult {
    /// State stays on the stack.
    Continue,
    /// State finished normally; pop and fire `on_complete`.
    Complete,
    /// State was interrupted; pop and fire `on_interrupt`.
    Interrupted,
}

impl BattleState {
    // ---- Constructors ----

    pub fn attack(owner: u32, target: u32, now_ms: u64, attack_delay_ms: u32) -> Self {
        Self {
            kind: BattleStateKind::Attack,
            body: BattleStateBody::Attack {
                next_swing_ms: now_ms + attack_delay_ms as u64,
            },
            owner_actor_id: owner,
            target_actor_id: target,
            start_time_ms: now_ms,
            can_interrupt: false,
            interrupt: false,
            is_completed: false,
            error_text_id: 0,
        }
    }

    pub fn magic(owner: u32, target: u32, cmd: BattleCommand, now_ms: u64) -> Self {
        let cast_finish_ms = now_ms + cmd.cast_time_ms as u64;
        Self {
            kind: BattleStateKind::Magic,
            body: BattleStateBody::Magic {
                command: cmd,
                cast_finish_ms,
                cast_started: false,
            },
            owner_actor_id: owner,
            target_actor_id: target,
            start_time_ms: now_ms,
            can_interrupt: true,
            interrupt: false,
            is_completed: false,
            error_text_id: 0,
        }
    }

    pub fn ability(owner: u32, target: u32, cmd: BattleCommand, now_ms: u64) -> Self {
        let cast_finish_ms = now_ms + cmd.cast_time_ms as u64;
        Self {
            kind: BattleStateKind::Ability,
            body: BattleStateBody::Ability {
                command: cmd,
                cast_finish_ms,
            },
            owner_actor_id: owner,
            target_actor_id: target,
            start_time_ms: now_ms,
            can_interrupt: true,
            interrupt: false,
            is_completed: false,
            error_text_id: 0,
        }
    }

    pub fn weapon_skill(owner: u32, target: u32, cmd: BattleCommand, now_ms: u64) -> Self {
        let cast_finish_ms = now_ms + cmd.cast_time_ms as u64;
        Self {
            kind: BattleStateKind::WeaponSkill,
            body: BattleStateBody::WeaponSkill {
                command: cmd,
                cast_finish_ms,
            },
            owner_actor_id: owner,
            target_actor_id: target,
            start_time_ms: now_ms,
            can_interrupt: true,
            interrupt: false,
            is_completed: false,
            error_text_id: 0,
        }
    }

    pub fn item(owner: u32, target: u32, item_id: u32, cast_time_ms: u32, now_ms: u64) -> Self {
        Self {
            kind: BattleStateKind::Item,
            body: BattleStateBody::Item {
                item_id,
                cast_finish_ms: now_ms + cast_time_ms as u64,
            },
            owner_actor_id: owner,
            target_actor_id: target,
            start_time_ms: now_ms,
            can_interrupt: true,
            interrupt: false,
            is_completed: false,
            error_text_id: 0,
        }
    }

    pub fn death(owner: u32, now_ms: u64, fadeout_ms: u64) -> Self {
        Self {
            kind: BattleStateKind::Death,
            body: BattleStateBody::Death {
                despawn_at_ms: now_ms + fadeout_ms,
            },
            owner_actor_id: owner,
            target_actor_id: 0,
            start_time_ms: now_ms,
            can_interrupt: false,
            interrupt: false,
            is_completed: false,
            error_text_id: 0,
        }
    }

    pub fn despawn(owner: u32, now_ms: u64, respawn_ms: u64) -> Self {
        Self {
            kind: BattleStateKind::Despawn,
            body: BattleStateBody::Despawn {
                respawn_at_ms: now_ms + respawn_ms,
            },
            owner_actor_id: owner,
            target_actor_id: 0,
            start_time_ms: now_ms,
            can_interrupt: false,
            interrupt: false,
            is_completed: false,
            error_text_id: 0,
        }
    }

    pub fn inactive(owner: u32, now_ms: u64, duration_ms: u64) -> Self {
        Self {
            kind: BattleStateKind::Inactive,
            body: BattleStateBody::Inactive {
                end_ms: if duration_ms == 0 { 0 } else { now_ms + duration_ms },
            },
            owner_actor_id: owner,
            target_actor_id: 0,
            start_time_ms: now_ms,
            can_interrupt: false,
            interrupt: false,
            is_completed: false,
            error_text_id: 0,
        }
    }

    // ---- Lifecycle queries ----

    /// Whether another state can be pushed on top of this one. Port of
    /// `CanChangeState`. Most casting states return `false`; `AttackState`
    /// and the terminal states return `true`.
    pub fn can_change_state(&self) -> bool {
        matches!(
            self.kind,
            BattleStateKind::Attack | BattleStateKind::Death | BattleStateKind::Despawn
        )
    }

    pub fn set_interrupted(&mut self, flag: bool) {
        self.interrupt = flag;
    }

    /// Dispatch one tick. See `StateTickResult` for outcomes.
    pub fn update(&mut self, now_ms: u64) -> StateTickResult {
        if self.interrupt && self.can_interrupt {
            return StateTickResult::Interrupted;
        }

        match &mut self.body {
            BattleStateBody::Attack { next_swing_ms } => {
                // Attacks never self-complete; AIContainer drives DoBattleCommand
                // on each ready tick via `is_attack_ready()`.
                if now_ms >= *next_swing_ms {
                    // Stay on the stack; the container consumes the tick and
                    // resets `next_swing_ms` via `schedule_next_swing`.
                    StateTickResult::Continue
                } else {
                    StateTickResult::Continue
                }
            }
            BattleStateBody::Magic { cast_finish_ms, .. }
            | BattleStateBody::Ability { cast_finish_ms, .. }
            | BattleStateBody::WeaponSkill { cast_finish_ms, .. }
            | BattleStateBody::Item { cast_finish_ms, .. } => {
                if now_ms >= *cast_finish_ms {
                    self.is_completed = true;
                    StateTickResult::Complete
                } else {
                    StateTickResult::Continue
                }
            }
            BattleStateBody::Death { despawn_at_ms } => {
                if now_ms >= *despawn_at_ms {
                    self.is_completed = true;
                    StateTickResult::Complete
                } else {
                    StateTickResult::Continue
                }
            }
            BattleStateBody::Despawn { respawn_at_ms } => {
                if now_ms >= *respawn_at_ms {
                    self.is_completed = true;
                    StateTickResult::Complete
                } else {
                    StateTickResult::Continue
                }
            }
            BattleStateBody::Inactive { end_ms } => {
                if *end_ms != 0 && now_ms >= *end_ms {
                    self.is_completed = true;
                    StateTickResult::Complete
                } else {
                    StateTickResult::Continue
                }
            }
        }
    }

    /// `AttackState::IsAttackReady` — true when the next-swing clock has
    /// elapsed. Only meaningful for `Attack`.
    pub fn is_attack_ready(&self, now_ms: u64) -> bool {
        matches!(&self.body, BattleStateBody::Attack { next_swing_ms } if now_ms >= *next_swing_ms)
    }

    /// Arm the next swing after a successful attack.
    pub fn schedule_next_swing(&mut self, now_ms: u64, delay_ms: u32) {
        if let BattleStateBody::Attack { next_swing_ms } = &mut self.body {
            *next_swing_ms = now_ms + delay_ms as u64;
        }
    }

    /// Command payload for states that carry one.
    pub fn command(&self) -> Option<&BattleCommand> {
        match &self.body {
            BattleStateBody::Magic { command, .. }
            | BattleStateBody::Ability { command, .. }
            | BattleStateBody::WeaponSkill { command, .. } => Some(command),
            _ => None,
        }
    }

    pub fn command_mut(&mut self) -> Option<&mut BattleCommand> {
        match &mut self.body {
            BattleStateBody::Magic { command, .. }
            | BattleStateBody::Ability { command, .. }
            | BattleStateBody::WeaponSkill { command, .. } => Some(command),
            _ => None,
        }
    }

    pub fn is_casting(&self) -> bool {
        matches!(
            self.kind,
            BattleStateKind::Magic | BattleStateKind::Ability | BattleStateKind::WeaponSkill
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn attack_state_update_is_noop() {
        let mut s = BattleState::attack(1, 2, 0, 2000);
        assert_eq!(s.update(1500), StateTickResult::Continue);
        assert!(!s.is_attack_ready(1500));
        assert!(s.is_attack_ready(2500));
    }

    #[test]
    fn magic_state_completes_when_cast_time_elapses() {
        let mut cmd = BattleCommand::new(100, "stone");
        cmd.cast_time_ms = 3000;
        let mut s = BattleState::magic(1, 2, cmd, 0);
        assert_eq!(s.update(1000), StateTickResult::Continue);
        assert_eq!(s.update(3500), StateTickResult::Complete);
        assert!(s.is_completed);
    }

    #[test]
    fn interrupt_flag_short_circuits() {
        let mut cmd = BattleCommand::new(100, "stone");
        cmd.cast_time_ms = 3000;
        let mut s = BattleState::magic(1, 2, cmd, 0);
        s.set_interrupted(true);
        assert_eq!(s.update(100), StateTickResult::Interrupted);
    }

    #[test]
    fn death_state_fades_out() {
        let mut s = BattleState::death(1, 0, 5000);
        assert_eq!(s.update(1000), StateTickResult::Continue);
        assert_eq!(s.update(5000), StateTickResult::Complete);
    }

    #[test]
    fn inactive_zero_duration_never_ends() {
        let mut s = BattleState::inactive(1, 0, 0);
        assert_eq!(s.update(1_000_000), StateTickResult::Continue);
    }

    #[test]
    fn attack_state_allows_other_states_to_push() {
        let s = BattleState::attack(1, 2, 0, 2000);
        assert!(s.can_change_state());
        let mut cmd = BattleCommand::new(100, "stone");
        cmd.cast_time_ms = 3000;
        let casting = BattleState::magic(1, 2, cmd, 0);
        assert!(!casting.can_change_state());
    }
}
