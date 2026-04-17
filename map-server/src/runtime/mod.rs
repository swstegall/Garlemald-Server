//! Per-tick game-loop runtime.
//!
//! This module bridges the typed event outboxes (inventory / status / battle
//! / zone) and the real world: packet sends, database writes, Lua calls,
//! broadcast fan-out. The `GameTicker` owns the scheduler; the
//! `ActorRegistry` holds live `Character` state for every player/npc/mob;
//! the `dispatcher` submodule turns individual events into side effects.

#![allow(dead_code, unused_imports)]

pub mod actor_registry;
pub mod broadcast;
pub mod dispatcher;
pub mod ticker;

#[cfg(test)]
mod integration_tests;

pub use actor_registry::{ActorHandle, ActorKindTag, ActorRegistry};
pub use broadcast::broadcast_around_actor;
pub use dispatcher::{dispatch_area_event, dispatch_battle_event, dispatch_inventory_event, dispatch_status_event};
pub use ticker::{GameTicker, TickerConfig};
