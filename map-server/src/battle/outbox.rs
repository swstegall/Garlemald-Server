//! Events emitted by battle-runtime mutations. Same pattern as the
//! inventory/status outboxes: the AI container, states, controllers, and
//! BattleUtils push events here; the game loop drains them per tick and
//! turns them into packet sends, DB writes, and Lua dispatches.

#![allow(dead_code)]

use super::command::CommandResult;
use super::effects::HitEffect;
use super::target_find::ValidTarget;

#[derive(Debug, Clone)]
pub enum BattleEvent {
    // ---- State-machine lifecycle ----------------------------------------
    /// Pushed when AIContainer.Engage succeeds — owner's main state becomes
    /// ACTIVE and `owner.target` is set.
    Engage { owner_actor_id: u32, target_actor_id: u32 },
    Disengage { owner_actor_id: u32 },
    /// Target-change packet when the focus actor switches without leaving
    /// combat.
    TargetChange { owner_actor_id: u32, new_target_actor_id: Option<u32> },

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
    PlayAnimation { owner_actor_id: u32, animation: u32 },
    /// Cast-bar notification.
    CastStart { owner_actor_id: u32, command_id: u16, cast_time_ms: u32 },
    CastComplete { owner_actor_id: u32, command_id: u16 },
    CastInterrupted { owner_actor_id: u32, command_id: u16 },

    // ---- Hate / enmity --------------------------------------------------
    HateAdd { owner_actor_id: u32, target_actor_id: u32, amount: i32 },
    HateClear { owner_actor_id: u32, target_actor_id: Option<u32> },

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
    Die { owner_actor_id: u32 },
    Despawn { owner_actor_id: u32 },
    Spawn { owner_actor_id: u32 },
    /// `Character.RecalculateStats` — stat-mod changes due to traits,
    /// equipment, or status effects.
    RecalcStats { owner_actor_id: u32 },

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
    WorldMasterText { owner_actor_id: u32, text_id: u16 },
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
