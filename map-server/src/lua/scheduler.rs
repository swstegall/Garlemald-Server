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

//! Coroutine scheduler. Ported from the `mSleepingOnTime` / `mSleepingOnSignal`
//! / `mSleepingOnPlayerEvent` dictionaries in `LuaEngine.cs`.
//!
//! Scripts yield via `coroutine.yield("_WAIT_TIME", s)`,
//! `coroutine.yield("_WAIT_SIGNAL", name)`, or
//! `coroutine.yield("_WAIT_EVENT", player)`. The scheduler records the
//! pending thread and resumes it when the condition fires.
//!
//! Because `mlua::Thread` is tied to its source `Lua` runtime, each parked
//! coroutine stashes `(Arc<Lua>, Thread)`. This matches the C# shape of
//! holding a Coroutine reference alongside its script.

#![allow(dead_code)]

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use mlua::{Lua, Thread, Value};

use super::command::{CommandQueue, LuaCommandArg};

/// One parked coroutine, waiting for a condition.
///
/// The `queue` handle is the same `Arc<Mutex<CommandQueue>>` bound to
/// the script's userdata at spawn time — resumes may push commands
/// (e.g. `director:EndGuildleve(true)`), and the tick-driven resume
/// path drains those commands into the game loop's
/// `apply_runtime_lua_commands` pipeline.
pub struct ParkedCoroutine {
    pub lua: Arc<Lua>,
    pub thread: Thread,
    pub queue: Arc<Mutex<CommandQueue>>,
}

impl std::fmt::Debug for ParkedCoroutine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ParkedCoroutine").finish_non_exhaustive()
    }
}

#[derive(Debug, Default)]
pub struct CoroutineScheduler {
    /// Coroutines sleeping on a deadline (millis since UNIX epoch).
    sleeping_on_time: Vec<(u64, ParkedCoroutine)>,
    /// Coroutines sleeping on a named signal.
    sleeping_on_signal: HashMap<String, Vec<ParkedCoroutine>>,
    /// Coroutines sleeping on the next event update for a player.
    sleeping_on_player_event: HashMap<u32, ParkedCoroutine>,
}

impl CoroutineScheduler {
    pub fn shared() -> Arc<Mutex<Self>> {
        Arc::new(Mutex::new(Self::default()))
    }

    pub fn park_time(&mut self, seconds: f32, coroutine: ParkedCoroutine) {
        let now = common::utils::millis_unix_timestamp();
        let wake_at = now + (seconds.max(0.0) * 1000.0) as u64;
        self.sleeping_on_time.push((wake_at, coroutine));
    }

    pub fn park_signal(&mut self, signal: impl Into<String>, coroutine: ParkedCoroutine) {
        self.sleeping_on_signal
            .entry(signal.into())
            .or_default()
            .push(coroutine);
    }

    pub fn park_event(&mut self, player_id: u32, coroutine: ParkedCoroutine) {
        // If the player already has a parked coroutine, overwrite it; the
        // C# version explicitly dropped the old one on `_WAIT_EVENT`.
        self.sleeping_on_player_event.insert(player_id, coroutine);
    }

    /// Wake every time-parked coroutine whose deadline has passed. Returns
    /// them so the caller can `Thread::resume()` on each.
    pub fn drain_due_time(&mut self) -> Vec<ParkedCoroutine> {
        let now = common::utils::millis_unix_timestamp();
        let (due, pending): (Vec<_>, Vec<_>) = std::mem::take(&mut self.sleeping_on_time)
            .into_iter()
            .partition(|(t, _)| *t <= now);
        self.sleeping_on_time = pending;
        due.into_iter().map(|(_, c)| c).collect()
    }

    /// Wake every coroutine parked on `signal`.
    pub fn drain_signal(&mut self, signal: &str) -> Vec<ParkedCoroutine> {
        self.sleeping_on_signal.remove(signal).unwrap_or_default()
    }

    /// Pop the coroutine parked against a specific player's event channel.
    pub fn take_event(&mut self, player_id: u32) -> Option<ParkedCoroutine> {
        self.sleeping_on_player_event.remove(&player_id)
    }

    pub fn pending_time_count(&self) -> usize {
        self.sleeping_on_time.len()
    }

    pub fn pending_signal_count(&self) -> usize {
        self.sleeping_on_signal.values().map(|v| v.len()).sum()
    }

    pub fn pending_event_count(&self) -> usize {
        self.sleeping_on_player_event.len()
    }
}

/// Inspect a `(status, value)` tuple returned by `coroutine.resume(thread)`
/// and decide how to re-park the thread. Returns the "what are you waiting
/// for" verdict, matching the C# `ResolveResume` helper.
#[derive(Debug)]
pub enum YieldDirective {
    /// Coroutine finished; don't re-park.
    Finished,
    /// Script returned `coroutine.yield("_WAIT_TIME", n)`.
    WaitTime(f32),
    /// Script returned `coroutine.yield("_WAIT_SIGNAL", name)`.
    WaitSignal(String),
    /// Script returned `coroutine.yield("_WAIT_EVENT", player)`.
    WaitEvent(u32),
    /// Neither — let caller handle it.
    Other,
}

pub fn classify_yield(value: &Value) -> YieldDirective {
    match value {
        Value::Nil => YieldDirective::Finished,
        Value::Table(tbl) => {
            let tag: Option<String> = tbl.get(1).ok();
            match tag.as_deref() {
                Some("_WAIT_TIME") => YieldDirective::WaitTime(tbl.get::<f32>(2).unwrap_or(0.0)),
                Some("_WAIT_SIGNAL") => {
                    YieldDirective::WaitSignal(tbl.get::<String>(2).unwrap_or_default())
                }
                Some("_WAIT_EVENT") => YieldDirective::WaitEvent(tbl.get::<u32>(2).unwrap_or(0)),
                _ => YieldDirective::Other,
            }
        }
        Value::String(s) if s.to_str().map(|c| c == "_WAIT_EVENT").unwrap_or(false) => {
            // The C# bare-string variant defers the player id to the
            // surrounding call context (see LuaEngine.ResolveResume).
            YieldDirective::WaitEvent(0)
        }
        _ => YieldDirective::Other,
    }
}

/// Adapter: turn a Lua value into the matching `LuaCommandArg` so scripts can
/// return structured values that the game loop consumes. UserData values
/// (LuaPlayer / LuaActor / LuaNpc / LuaDirectorHandle / LuaQuestHandle)
/// are coerced to `ActorId` so cutscene and event RPCs that pass `player`
/// or `quest` as Lua-param entries (e.g. `callClientFunction(player,
/// "delegateEvent", player, quest, "processTtrNomal001withHQ")`) end up
/// with type-byte 0x06 on the wire instead of being silently flattened
/// to `Nil`.
pub fn value_to_command_arg(value: &Value) -> LuaCommandArg {
    match value {
        Value::Nil => LuaCommandArg::Nil,
        Value::Boolean(b) => LuaCommandArg::Bool(*b),
        Value::Integer(i) => LuaCommandArg::Int(*i),
        Value::Number(n) => LuaCommandArg::Float(*n),
        Value::String(s) => {
            LuaCommandArg::String(s.to_str().map(|c| c.to_string()).unwrap_or_default())
        }
        Value::UserData(ud) => {
            use super::userdata::{
                LuaActor, LuaDirectorHandle, LuaNpc, LuaPlayer, LuaQuestHandle,
            };
            // Use `borrow_scoped` rather than `borrow`: the latter conflicts
            // with the mlua method binding's outer borrow when a script
            // passes `self` back into the call as a vararg
            // (`player:RunEventFunction("delegateEvent", player, …)`),
            // which silently dropped the player slot to Nil before this
            // change. `borrow_scoped` releases its handle as soon as the
            // closure returns, so it composes safely with the binding's
            // immutable borrow of `this`.
            if let Ok(id) = ud.borrow_scoped::<LuaPlayer, _>(|p| p.snapshot.actor_id) {
                LuaCommandArg::ActorId(id)
            } else if let Ok(id) = ud.borrow_scoped::<LuaActor, _>(|a| a.actor_id) {
                LuaCommandArg::ActorId(id)
            } else if let Ok(id) = ud.borrow_scoped::<LuaNpc, _>(|n| n.base.actor_id) {
                LuaCommandArg::ActorId(id)
            } else if let Ok(id) = ud.borrow_scoped::<LuaDirectorHandle, _>(|d| d.actor_id) {
                LuaCommandArg::ActorId(id)
            } else if let Ok(id) = ud.borrow_scoped::<LuaQuestHandle, _>(|q| 0xA0F0_0000 | q.quest_id) {
                // Meteor's CreateLuaParamList encodes a quest as
                // `0xA0F00000 | quest.GetQuestId()` (the same masking
                // StaticActors uses), then writes it as an Actor
                // LuaParam. Mirror that so the client recognises the
                // quest reference inside the cutscene RPC payload.
                LuaCommandArg::ActorId(id)
            } else {
                LuaCommandArg::Nil
            }
        }
        _ => LuaCommandArg::Nil,
    }
}
