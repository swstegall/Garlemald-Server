//! `Pet` — a BattleNpc tied to a specific master actor. Port of
//! `Actors/Chara/Npc/Pet.cs`. The AI container hosts a `PetController`;
//! the struct carries `master_actor_id` so the controller can keep the
//! pet glued to its owner.

#![allow(dead_code)]

use super::actor_class::ActorClass;
use super::battle_npc::BattleNpc;

#[derive(Debug, Clone)]
pub struct Pet {
    pub battle_npc: BattleNpc,
    pub master_actor_id: u32,
}

impl Pet {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        actor_number: u32,
        actor_class: &ActorClass,
        unique_id: impl Into<String>,
        area_id: u32,
        master_actor_id: u32,
        x: f32,
        y: f32,
        z: f32,
    ) -> Self {
        let battle_npc = BattleNpc::new(
            actor_number,
            actor_class,
            unique_id,
            area_id,
            x,
            y,
            z,
            0.0,
            0,
            0,
            None,
        );
        Self {
            battle_npc,
            master_actor_id,
        }
    }

    pub fn actor_id(&self) -> u32 {
        self.battle_npc.actor_id()
    }
}
