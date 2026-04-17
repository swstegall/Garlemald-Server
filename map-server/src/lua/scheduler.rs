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

use super::command::LuaCommandArg;

/// One parked coroutine, waiting for a condition.
pub struct ParkedCoroutine {
    pub lua: Arc<Lua>,
    pub thread: Thread,
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
        self.sleeping_on_signal.entry(signal.into()).or_default().push(coroutine);
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
        let (due, pending): (Vec<_>, Vec<_>) =
            std::mem::take(&mut self.sleeping_on_time).into_iter().partition(|(t, _)| *t <= now);
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
                Some("_WAIT_SIGNAL") => YieldDirective::WaitSignal(tbl.get::<String>(2).unwrap_or_default()),
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
/// return structured values that the game loop consumes.
pub fn value_to_command_arg(value: &Value) -> LuaCommandArg {
    match value {
        Value::Nil => LuaCommandArg::Nil,
        Value::Boolean(b) => LuaCommandArg::Bool(*b),
        Value::Integer(i) => LuaCommandArg::Int(*i),
        Value::Number(n) => LuaCommandArg::Float(*n),
        Value::String(s) => LuaCommandArg::String(s.to_str().map(|c| c.to_string()).unwrap_or_default()),
        _ => LuaCommandArg::Nil,
    }
}
