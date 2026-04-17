//! Event + quest runtime. Port of `Map Server/Actors/Quest/*` +
//! the event-dispatch half of `PacketProcessor.cs`.
//!
//! Layout:
//!
//! * `quest` — the per-player `Quest` runtime (phase, flag bits, JSON
//!   data blob, completion/abandon hooks).
//! * `outbox` — typed events emitted by event/quest mutations. Same
//!   pattern as inventory / status / battle / area: the game loop
//!   drains the outbox each tick and turns events into packet sends,
//!   DB writes, and Lua calls.
//! * `session` — per-player "currently running event" state
//!   (`current_event_owner`, `current_event_name`, `current_event_type`).
//! * `dispatcher` — packet + DB + Lua side-effect resolver.

#![allow(dead_code, unused_imports)]

pub mod dispatcher;
pub mod lua_bridge;
pub mod outbox;
pub mod quest;
pub mod session;

pub use dispatcher::dispatch_event_event;
pub use lua_bridge::translate_lua_commands_into_outbox;
pub use outbox::{EventEvent, EventOutbox};
pub use quest::{Quest, QuestFlags};
pub use session::EventSession;
