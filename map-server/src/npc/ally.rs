//! `Ally` — a BattleNpc that fights alongside players. Port of
//! `Actors/Chara/Npc/Ally.cs`.
//!
//! The only real difference from `BattleNpc` is that the AI container
//! hosts an `AllyController` rather than a `BattleNpcController`. That
//! controller is set up by the spawner when it routes an Ally row; the
//! struct is otherwise a thin newtype so call sites can tell Allies
//! apart at a glance.

#![allow(dead_code)]

use super::actor_class::ActorClass;
use super::battle_npc::BattleNpc;

#[derive(Debug, Clone)]
pub struct Ally {
    pub battle_npc: BattleNpc,
}

impl Ally {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        actor_number: u32,
        actor_class: &ActorClass,
        unique_id: impl Into<String>,
        area_id: u32,
        x: f32,
        y: f32,
        z: f32,
        rotation: f32,
    ) -> Self {
        let mut battle_npc = BattleNpc::new(
            actor_number,
            actor_class,
            unique_id,
            area_id,
            x,
            y,
            z,
            rotation,
            0,
            0,
            None,
        );
        // Allies are "alive and moving" by default — the C# ctor
        // initialises `isAutoAttackEnabled = true` and skips the
        // isMovingToSpawn flag.
        battle_npc.npc.character.chara.is_auto_attack_enabled = true;
        battle_npc.npc.character.chara.is_moving_to_spawn = false;
        Self { battle_npc }
    }

    pub fn actor_id(&self) -> u32 {
        self.battle_npc.actor_id()
    }
}
