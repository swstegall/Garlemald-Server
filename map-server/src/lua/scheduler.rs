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
    /// Actor id of the player this coroutine acts on behalf of (the
    /// `onEventStarted` trigger player for director/command scripts),
    /// or 0 when unknown (e.g. a director `main` with no player in
    /// scope). The ticker uses this to route the commands a resumed
    /// coroutine queues through the EVENT bridge for that player —
    /// without it, event-flavoured commands (`RunEventFunction` /
    /// `EndEvent` / `KickEvent`) drained from a timer resume would fall
    /// through the runtime drain's catch-all and never reach the wire
    /// (e.g. the SEQ_005 director's post-`wait(1)` `kickEventContinue`
    /// + `processTtrBtl002`). (Garlemald-Server #28.)
    pub owner_player_id: u32,
}

impl std::fmt::Debug for ParkedCoroutine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ParkedCoroutine").finish_non_exhaustive()
    }
}

/// "Owner unknown" stamp for an event park that landed outside an
/// [`EventOwnerContextScope`] (e.g. a pre-warp director park from the
/// `DoZoneChangeContent` drain). `handle_event_start`'s resume gate must
/// treat these as unattributable — neither resumable nor purgeable by
/// owner identity — and fall through to the legacy dispatch paths.
pub const EVENT_OWNER_UNKNOWN: u32 = 0;

/// A `_WAIT_EVENT` park plus the identity of the event that owns it.
///
/// pmeteor's `mSleepingOnPlayerEvent` carries no owner identity, so its
/// `LuaEngine.cs::EventStarted` resume arm resumes ANY parked coroutine
/// on ANY later EventStart — which is exactly the stale-coroutine hijack
/// garlemald hit live (a parked Charlys talk surviving a relog was
/// resumed by a Hobriaut talk, silently draining `QuestIncCounter` +
/// `AddGil(2000)` + `EndEvent`). Stamping the owning event's actor id at
/// park time lets the EventStart resume gate match pmeteor's
/// resume-or-fresh split WITHOUT the cross-owner hijack.
/// (Garlemald-Server #46.)
#[derive(Debug)]
struct ParkedEventCoroutine {
    /// Actor id of the event owner (NPC / director / command static
    /// actor) whose EventStart/EventUpdate dispatch parked this
    /// coroutine, or [`EVENT_OWNER_UNKNOWN`] when the park landed
    /// outside any owner-context scope.
    event_owner_actor_id: u32,
    coroutine: ParkedCoroutine,
}

#[derive(Debug, Default)]
pub struct CoroutineScheduler {
    /// Coroutines sleeping on a deadline (millis since UNIX epoch).
    sleeping_on_time: Vec<(u64, ParkedCoroutine)>,
    /// Coroutines sleeping on a named signal.
    sleeping_on_signal: HashMap<String, Vec<ParkedCoroutine>>,
    /// Coroutines sleeping on the next event update for a player,
    /// stamped with the actor id of the event that owns them.
    sleeping_on_player_event: HashMap<u32, ParkedEventCoroutine>,
    /// Owner actor id to stamp onto any `_WAIT_EVENT` park that lands
    /// for a player while an [`EventOwnerContextScope`] is alive. Keyed
    /// by player actor id; set/cleared by the scope guard around the
    /// EventStart/EventUpdate dispatch sections (the park itself happens
    /// deep inside a `spawn_blocking` script drain that has no owner in
    /// scope — see `LuaEngine::repark`).
    event_owner_context: HashMap<u32, u32>,
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
        //
        // DIAGNOSTIC (Garlemald-Server #46 — man0l1 SEQ_007 IncCounter drop):
        // a re-park while one is ALREADY pending DROPS the displaced
        // coroutine's not-yet-run continuation — including any `data:`/`quest:`
        // mutation it would queue on resume (e.g. Isandorel's
        // `data:IncCounter(CNTR_SEQ7_MSK)` queued AFTER the processEvent035
        // park). If the MSK counter never advancing is caused by such a
        // double-park, this warning fires during the Isandorel talk; if it
        // never fires, the counter is lost elsewhere. Temporary triage.
        if self.sleeping_on_player_event.contains_key(&player_id) {
            tracing::warn!(
                player = player_id,
                "park_event DISPLACING an already-parked coroutine — its pending \
                 continuation/mutations are dropped (Garlemald-Server #46 diagnostic)",
            );
        }
        // Stamp the owning event's identity from the surrounding
        // owner-context scope. The park key can be the historical
        // player_id=0 fallback (mlua drops yield args past the first for
        // some capture shapes), so prefer the coroutine's own
        // `owner_player_id` when looking the context up.
        let event_owner_actor_id = [coroutine.owner_player_id, player_id]
            .into_iter()
            .filter(|id| *id != 0)
            .find_map(|id| self.event_owner_context.get(&id).copied())
            .unwrap_or(EVENT_OWNER_UNKNOWN);
        self.sleeping_on_player_event.insert(
            player_id,
            ParkedEventCoroutine {
                event_owner_actor_id,
                coroutine,
            },
        );
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
        self.sleeping_on_player_event
            .remove(&player_id)
            .map(|p| p.coroutine)
    }

    /// The stamped event-owner actor id of the player's parked
    /// `_WAIT_EVENT` coroutine, or `None` when nothing is parked. Uses
    /// the same player-id → 0 fallback lookup
    /// `LuaEngine::fire_player_event_and_drain` resumes through, so the
    /// gate that reads this and the resume that follows always see the
    /// same coroutine. [`EVENT_OWNER_UNKNOWN`] means "parked but
    /// unattributable" (landed outside an owner-context scope).
    pub fn parked_event_owner(&self, player_id: u32) -> Option<u32> {
        self.sleeping_on_player_event
            .get(&player_id)
            .or_else(|| {
                if player_id != 0 {
                    self.sleeping_on_player_event.get(&0)
                } else {
                    None
                }
            })
            .map(|p| p.event_owner_actor_id)
    }

    /// Drop the player's parked `_WAIT_EVENT` coroutine (same
    /// player-id → 0 fallback as [`Self::parked_event_owner`]) without
    /// resuming it. The EventStart gate calls this when a NEW event
    /// arrives while a coroutine stamped with a DIFFERENT owner is still
    /// parked — the stale continuation must not survive to hijack a
    /// later resume. Returns `true` if something was dropped.
    pub fn discard_parked_event(&mut self, player_id: u32) -> bool {
        if self.sleeping_on_player_event.remove(&player_id).is_some() {
            return true;
        }
        player_id != 0 && self.sleeping_on_player_event.remove(&0).is_some()
    }

    /// Record that any `_WAIT_EVENT` park landing for `player_id` (until
    /// [`Self::clear_event_owner_context`]) belongs to the event owned by
    /// `owner_actor_id`. Prefer the RAII [`EventOwnerContextScope`] over
    /// calling this directly.
    pub fn set_event_owner_context(&mut self, player_id: u32, owner_actor_id: u32) {
        self.event_owner_context.insert(player_id, owner_actor_id);
    }

    pub fn clear_event_owner_context(&mut self, player_id: u32) {
        self.event_owner_context.remove(&player_id);
    }

    /// RAII wrapper around `set_event_owner_context` /
    /// `clear_event_owner_context` — hold the returned scope across a
    /// dispatch section (which may `await` into `spawn_blocking` script
    /// drains) so every `_WAIT_EVENT` park the section produces is
    /// stamped with the owning event's actor id, and the stamp context
    /// can't leak past an early return. Last-writer-wins per player:
    /// the packet pipeline is per-session sequential, so nested scopes
    /// for one player don't occur in practice.
    pub fn event_owner_scope(
        scheduler: &Arc<Mutex<Self>>,
        player_id: u32,
        owner_actor_id: u32,
    ) -> EventOwnerContextScope {
        if let Ok(mut s) = scheduler.lock() {
            s.set_event_owner_context(player_id, owner_actor_id);
        }
        EventOwnerContextScope {
            scheduler: Arc::clone(scheduler),
            player_id,
        }
    }

    /// Drop every parked coroutine owned by `player_id`, across all
    /// three park kinds. The `ContentFinished` teardown calls this so
    /// a stale park (e.g. a director `_WAIT_EVENT` left behind by an
    /// abandoned tutorial) can never resume into a torn-down instance.
    /// Owner-0 (ownerless) parks are kept — they're not attributable
    /// to the leaving player. Returns the number of coroutines dropped.
    /// (#28 S1.3.)
    pub fn purge_owner(&mut self, player_id: u32) -> usize {
        if player_id == 0 {
            return 0;
        }
        let before =
            self.pending_time_count() + self.pending_signal_count() + self.pending_event_count();
        self.sleeping_on_time
            .retain(|(_, c)| c.owner_player_id != player_id);
        for parked in self.sleeping_on_signal.values_mut() {
            parked.retain(|c| c.owner_player_id != player_id);
        }
        self.sleeping_on_signal.retain(|_, v| !v.is_empty());
        // Event parks key by the yield's player id, which can differ
        // from the owning player (the historical player_id=0 fallback)
        // — purge by either identity.
        self.sleeping_on_player_event
            .retain(|k, v| *k != player_id && v.coroutine.owner_player_id != player_id);
        // Drop any pending owner-context stamp too — the player's
        // dispatch section can't outlive the purge (session teardown /
        // ContentFinished), and a leaked stamp would mis-attribute the
        // next unrelated park.
        self.event_owner_context.remove(&player_id);
        before
            - (self.pending_time_count() + self.pending_signal_count() + self.pending_event_count())
    }

    pub fn pending_time_count(&self) -> usize {
        self.sleeping_on_time.len()
    }

    /// The earliest pending time-park deadline (millis since UNIX epoch,
    /// the same clock `drain_due_time` compares against), or `None` when
    /// nothing is parked on the clock. Used by the headless test harness
    /// (`crate::testkit`) to sleep just past the next `wait(n)` instead of
    /// guessing a fixed duration.
    #[cfg(feature = "testkit")]
    pub(crate) fn next_time_deadline_ms(&self) -> Option<u64> {
        self.sleeping_on_time.iter().map(|(t, _)| *t).min()
    }

    /// Testkit-only: fast-forward every time-parked deadline to *now* so a
    /// following [`super::LuaEngine::tick`] resumes straight through script
    /// `wait(n)` calls without wall-clock sleeping. The content-test
    /// walkthrough bridge drives quest hooks that park on time (e.g.
    /// man0u1's `onEmote` → `wait(2.5)`) through this — the same
    /// expire-then-tick shape as `testkit::advance_one_time_park`, minus
    /// the real sleep.
    #[cfg(feature = "testkit")]
    pub fn expire_time_parks(&mut self) {
        let now = common::utils::millis_unix_timestamp();
        for (deadline, _) in self.sleeping_on_time.iter_mut() {
            *deadline = now;
        }
    }

    pub fn pending_signal_count(&self) -> usize {
        self.sleeping_on_signal.values().map(|v| v.len()).sum()
    }

    pub fn pending_event_count(&self) -> usize {
        self.sleeping_on_player_event.len()
    }
}

/// RAII guard for the scheduler's per-player event-owner context — see
/// [`CoroutineScheduler::event_owner_scope`]. Clearing on `Drop` keeps
/// early-returning dispatch paths from leaking a stamp onto the next
/// unrelated park.
pub struct EventOwnerContextScope {
    scheduler: Arc<Mutex<CoroutineScheduler>>,
    player_id: u32,
}

impl Drop for EventOwnerContextScope {
    fn drop(&mut self) {
        if let Ok(mut s) = self.scheduler.lock() {
            s.clear_event_owner_context(self.player_id);
        }
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

/// Multi-value-aware variant of [`classify_yield`]. `global.lua`'s wait
/// helpers yield BARE value pairs — `coroutine.yield("_WAIT_SIGNAL",
/// signal)` / `("_WAIT_TIME", seconds)` / `("_WAIT_EVENT", player)` —
/// so a resume captured as a single `Value` keeps only the tag string
/// and DISCARDS the argument. `classify_yield` then has no bare-string
/// arm for `_WAIT_SIGNAL`/`_WAIT_TIME` and returns `Other`, which the
/// repark path treats as "drop the coroutine": the SEQ_005 director
/// resumed off the cinematic's EventUpdate yielded
/// `("_WAIT_SIGNAL", "playerActive")`, got classified `Other`, and was
/// silently dropped — so the F press's `sendSignal("playerActive")`
/// found nothing parked and the tutorial soft-locked. Every resume site
/// must capture `mlua::MultiValue` and classify through this function.
/// (Garlemald-Server #28.)
pub fn classify_yield_mv(values: &mlua::MultiValue) -> YieldDirective {
    let mut it = values.iter();
    let Some(first) = it.next() else {
        return YieldDirective::Finished;
    };
    let second = it.next();
    match first {
        Value::String(s) => {
            let tag = s.to_str().map(|c| c.to_string()).unwrap_or_default();
            match tag.as_str() {
                "_WAIT_TIME" => YieldDirective::WaitTime(match second {
                    Some(Value::Number(n)) => *n as f32,
                    Some(Value::Integer(i)) => *i as f32,
                    _ => 0.0,
                }),
                "_WAIT_SIGNAL" => YieldDirective::WaitSignal(match second {
                    Some(Value::String(name)) => {
                        name.to_str().map(|c| c.to_string()).unwrap_or_default()
                    }
                    _ => String::new(),
                }),
                "_WAIT_EVENT" => YieldDirective::WaitEvent(match second {
                    // `waitForEvent(player)` passes the LuaPlayer
                    // userdata; coerce it to the actor id so the park
                    // lands under the real player (with the historical
                    // player_id=0 fallback still honoured by
                    // `fire_player_event_and_drain`).
                    Some(v) => match value_to_command_arg(v) {
                        LuaCommandArg::ActorId(id) => id,
                        LuaCommandArg::Int(i) => i as u32,
                        LuaCommandArg::UInt(u) => u as u32,
                        _ => 0,
                    },
                    None => 0,
                }),
                _ => YieldDirective::Other,
            }
        }
        // Table-shaped yields (and Nil = finished) keep the original
        // single-value semantics.
        other => classify_yield(other),
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
                LuaActor, LuaDirectorHandle, LuaNpc, LuaPlayer, LuaQuestHandle, LuaStaticActor,
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
            } else if let Ok(id) = ud.borrow_scoped::<LuaStaticActor, _>(|s| s.actor_id) {
                // GetStaticActor(name) reference (e.g. DftSea) — must ride
                // the wire as ActorId 0x06; an Integer 0x00 here crashes
                // the client's delegateEvent (2026-07-03 Baderon capture).
                LuaCommandArg::ActorId(id)
            } else if let Ok(id) =
                ud.borrow_scoped::<LuaQuestHandle, _>(|q| 0xA0F0_0000 | q.quest_id)
            {
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

#[cfg(test)]
mod event_owner_stamp_tests {
    use super::*;

    fn parked(lua: &Arc<Lua>, owner_player_id: u32) -> ParkedCoroutine {
        let f = lua.create_function(|_, ()| Ok(())).unwrap();
        ParkedCoroutine {
            lua: Arc::clone(lua),
            thread: lua.create_thread(f).unwrap(),
            queue: CommandQueue::new(),
            owner_player_id,
        }
    }

    /// A park landing inside an owner-context scope must carry the
    /// event owner's actor id; one landing outside must carry
    /// `EVENT_OWNER_UNKNOWN` (the EventStart gate falls through on
    /// those instead of purging a legit pre-warp director park).
    #[test]
    fn park_event_stamps_owner_from_context() {
        let lua = Arc::new(Lua::new());
        let mut sched = CoroutineScheduler::default();

        sched.set_event_owner_context(42, 0x46D0_0123);
        sched.park_event(42, parked(&lua, 42));
        assert_eq!(sched.parked_event_owner(42), Some(0x46D0_0123));

        sched.clear_event_owner_context(42);
        assert!(sched.take_event(42).is_some());
        sched.park_event(42, parked(&lua, 42));
        assert_eq!(sched.parked_event_owner(42), Some(EVENT_OWNER_UNKNOWN));
    }

    /// The historical player_id=0 park-key fallback: the context is
    /// keyed by the coroutine's real `owner_player_id`, and lookups by
    /// the real player id fall back to the 0-keyed entry — matching
    /// `fire_player_event_and_drain`'s take path.
    #[test]
    fn zero_key_fallback_still_resolves_owner_stamp() {
        let lua = Arc::new(Lua::new());
        let mut sched = CoroutineScheduler::default();

        sched.set_event_owner_context(42, 0x46D0_0456);
        // Yield lost the player arg → parked under key 0, but the
        // coroutine still knows its owning player.
        sched.park_event(0, parked(&lua, 42));
        assert_eq!(sched.parked_event_owner(42), Some(0x46D0_0456));

        assert!(sched.discard_parked_event(42));
        assert_eq!(sched.parked_event_owner(42), None);
        assert!(!sched.discard_parked_event(42));
    }

    /// `purge_owner` (session teardown / ContentFinished) must drop the
    /// event park AND the pending owner-context stamp so a relog can
    /// never resume a stale continuation (the Charlys→Hobriaut 2,000
    /// gil hijack). (Garlemald-Server #46.)
    #[test]
    fn purge_owner_drops_event_park_and_context() {
        let lua = Arc::new(Lua::new());
        let mut sched = CoroutineScheduler::default();

        sched.set_event_owner_context(42, 0x46D0_0789);
        sched.park_event(42, parked(&lua, 42));
        assert_eq!(sched.purge_owner(42), 1);
        assert_eq!(sched.parked_event_owner(42), None);

        // The stamp is gone too: a fresh park is unattributable.
        sched.park_event(42, parked(&lua, 42));
        assert_eq!(sched.parked_event_owner(42), Some(EVENT_OWNER_UNKNOWN));
    }

    /// The RAII scope stamps while alive and stops stamping after drop
    /// (early-return safety in `handle_event_start`).
    #[test]
    fn event_owner_scope_clears_on_drop() {
        let lua = Arc::new(Lua::new());
        let shared = CoroutineScheduler::shared();

        {
            let _scope = CoroutineScheduler::event_owner_scope(&shared, 42, 0x46D0_0AAA);
            let mut s = shared.lock().unwrap();
            s.park_event(42, parked(&lua, 42));
            assert_eq!(s.parked_event_owner(42), Some(0x46D0_0AAA));
            assert!(s.take_event(42).is_some());
        }

        let mut s = shared.lock().unwrap();
        s.park_event(42, parked(&lua, 42));
        assert_eq!(s.parked_event_owner(42), Some(EVENT_OWNER_UNKNOWN));
    }
}

#[cfg(test)]
mod classify_yield_tests {
    use super::*;
    use mlua::{Lua, MultiValue};

    fn mv(lua: &Lua, vals: Vec<Value>) -> MultiValue {
        let mut m = MultiValue::new();
        for v in vals {
            m.push_back(v);
        }
        let _ = lua; // values already constructed against this Lua
        m
    }

    /// `global.lua`'s wait helpers yield BARE value pairs — the resume
    /// capture must not lose the second value. A single-`Value` capture
    /// classified `("_WAIT_SIGNAL", name)` as `Other` and dropped the
    /// SEQ_005 director at the `waitForSignal("playerActive")` re-park
    /// (the F-press softlock). (Garlemald-Server #28.)
    #[test]
    fn bare_pair_yields_classify_correctly() {
        let lua = Lua::new();

        let sig = mv(
            &lua,
            vec![
                Value::String(lua.create_string("_WAIT_SIGNAL").unwrap()),
                Value::String(lua.create_string("playerActive").unwrap()),
            ],
        );
        match classify_yield_mv(&sig) {
            YieldDirective::WaitSignal(name) => assert_eq!(name, "playerActive"),
            other => panic!("expected WaitSignal, got {other:?}"),
        }

        let time = mv(
            &lua,
            vec![
                Value::String(lua.create_string("_WAIT_TIME").unwrap()),
                Value::Integer(3),
            ],
        );
        match classify_yield_mv(&time) {
            YieldDirective::WaitTime(s) => assert_eq!(s, 3.0),
            other => panic!("expected WaitTime, got {other:?}"),
        }

        let event = mv(
            &lua,
            vec![
                Value::String(lua.create_string("_WAIT_EVENT").unwrap()),
                Value::Integer(42),
            ],
        );
        match classify_yield_mv(&event) {
            YieldDirective::WaitEvent(id) => assert_eq!(id, 42),
            other => panic!("expected WaitEvent, got {other:?}"),
        }
    }

    #[test]
    fn table_and_terminal_forms_still_classify() {
        let lua = Lua::new();

        let tbl = lua.create_table().unwrap();
        tbl.set(1, "_WAIT_SIGNAL").unwrap();
        tbl.set(2, "battleComplete").unwrap();
        let table_form = mv(&lua, vec![Value::Table(tbl)]);
        match classify_yield_mv(&table_form) {
            YieldDirective::WaitSignal(name) => assert_eq!(name, "battleComplete"),
            other => panic!("expected WaitSignal, got {other:?}"),
        }

        // Coroutine returned nothing → finished.
        match classify_yield_mv(&MultiValue::new()) {
            YieldDirective::Finished => {}
            other => panic!("expected Finished, got {other:?}"),
        }

        // Unknown tag → Other (caller drops).
        let unk = mv(
            &lua,
            vec![Value::String(lua.create_string("_SOMETHING").unwrap())],
        );
        assert!(matches!(classify_yield_mv(&unk), YieldDirective::Other));
    }
}
