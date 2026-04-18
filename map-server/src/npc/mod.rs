//! NPC actor types + spawning pipeline. Port of `Map Server/Actors/Chara/Npc/*`.
//!
//! Layout:
//!
//! * `actor_class` — `ActorClass` metadata + push-command hints.
//! * `mob_modifier` — `MobModifier` enum + per-NPC tuning values.
//! * `npc_work` — transient NPC state (push command + hate type).
//! * `npc` — the `Npc` struct (wraps `Character` + class metadata).
//! * `battle_npc` — `BattleNpc` (wraps `Npc` + modifier layers + respawn).
//! * `ally`, `pet`, `retainer` — thin specialisations.
//! * `spawner` — the boot-time spawn pipeline that turns
//!   `SpawnLocation` seeds into live actors in the registry.

#![allow(dead_code, unused_imports, clippy::module_inception)]

pub mod actor_class;
pub mod ally;
pub mod battle_npc;
pub mod mob_modifier;
pub mod npc;
pub mod npc_work;
pub mod pet;
pub mod retainer;
pub mod spawner;

pub use actor_class::ActorClass;
pub use ally::Ally;
pub use battle_npc::{BattleNpc, DetectionType, KindredType, ModifierLayer};
pub use mob_modifier::{MobModifier, MobModifierMap};
pub use npc::{EventConditionMap, Npc};
pub use npc_work::{HATE_TYPE_ENGAGED, HATE_TYPE_ENGAGED_PARTY, HATE_TYPE_NONE, NpcWork};
pub use pet::Pet;
pub use retainer::Retainer;
pub use spawner::{SpawnContext, spawn_all_actors, spawn_from_location};
