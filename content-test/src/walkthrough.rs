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

//! The `walkthrough` bridge: a spec-side userdata that drives the real
//! map-server [`LuaEngine`] through a quest's hooks and lets a Lua spec
//! assert on the side effects.
//!
//! # How it works
//!
//! A quest script is a set of hooks (`onStart`, `onStateChange`, `onTalk`,
//! `onPush`, …) that, instead of touching the world directly, push typed
//! [`LuaCommand`]s onto an outbox the engine drains after each call. When a
//! hook calls `callClientFunction(...)` it *yields*, parking a coroutine,
//! and resumes only when the client's next `EventUpdate` arrives.
//!
//! This harness reproduces that loop headlessly. A [`Walkthrough`] holds:
//!   * a freshly-built [`LuaEngine`] (its own VM cache + coroutine scheduler,
//!     so specs can't contaminate each other),
//!   * the tracked quest state (`sequence`, `flags`, `counters`), which it
//!     threads into the [`PlayerSnapshot`] / [`LuaQuestHandle`] of every hook
//!     call. This is mandatory: the engine's `data:SetFlag(n)` binding only
//!     *enqueues* a `QuestSetFlag` command. It does **not** mutate the
//!     snapshot, so nothing but this harness advances the flags between
//!     steps.
//!
//! Drivers (`onStart`/`stateChange`/`talk`/`push`/`emote`) fire a hook and
//! reset the "current step" command buffer; resumes (`ack`/`answer`) feed a
//! simulated client `EventUpdate` into the parked coroutine and *extend* the
//! buffer. The `emote` driver additionally resumes through `wait(n)` time
//! parks (man0u1's `onEmote` opens with `wait(2.5)`) by fast-forwarding the
//! scheduler clock and ticking the engine — the headless mirror of the live
//! game-loop ticker.
//! Every quest-state command that comes back is applied to the tracked state.
//! Assertions (`expect*`) check the current step's commands (or the tracked
//! state) and raise a Lua error on mismatch, which the runner reports as a
//! failed test.

use std::path::Path;

use mlua::{AnyUserData, UserData, UserDataMethods};

use common::luaparam::LuaParam;
use map_server::lua::command::{CommandQueue, LuaCommand, LuaCommandArg};
use map_server::lua::userdata::{LuaQuestHandle, PlayerSnapshot};
use map_server::lua::{LuaEngine, LuaNpcSpec, QuestHookArg, QuestStateSnapshot};

/// Wrap a harness assertion/driver error (a `String`) as a Lua runtime error
/// so it surfaces as a failed spec instead of aborting the whole run.
fn lua_err(message: String) -> mlua::Error {
    mlua::Error::RuntimeError(message)
}

/// Synthetic player id used for every walkthrough. Each [`Walkthrough`] owns
/// its own engine + scheduler, so a fixed id can't collide across specs.
const PLAYER_ID: u32 = 0x0250_0001;

/// The spec-facing driver for one quest playthrough.
pub struct Walkthrough {
    engine: LuaEngine,
    script_path: std::path::PathBuf,
    quest_id: u32,
    zone_id: u32,

    // Tracked quest state, threaded into every hook call.
    sequence: u32,
    flags: u32,
    counters: [u16; 4],

    /// Commands drained by the most recent driver + its resumes. Reset by a
    /// driver, extended by `ack`/`answer`. Assertions read this.
    step: Vec<LuaCommand>,

    /// Quest ids handed to the player via `AddQuest` (e.g. the SEQ_010 → next
    /// quest handoff) and completed via `CompleteQuest`, for `expectHandoff`.
    added: Vec<u32>,
    completed: Vec<u32>,
}

impl Walkthrough {
    /// Build a walkthrough for a quest by its class code (e.g. `"Man0l0"`).
    /// Resolves the on-disk script path and the quest id; errors if either is
    /// unknown so the spec fails loudly instead of silently no-opping.
    ///
    /// `start_sequence` seeds the tracked sequence so a spec can pick up a quest
    /// mid-flight: e.g. start at SEQ_010 to walk the post-combat town leg and
    /// the hand-off, without re-running the SEQ_000 tutorial (whose flags the
    /// content warp clears anyway).
    pub fn new(script_root: &Path, quest_code: &str, start_sequence: u32) -> Result<Self, String> {
        let lower = quest_code.to_lowercase();
        let prefix: String = lower.chars().take(3).collect();
        let script_path = script_root
            .join("quests")
            .join(&prefix)
            .join(format!("{lower}.lua"));
        if !script_path.exists() {
            return Err(format!(
                "quest script not found for '{quest_code}': {}",
                script_path.display()
            ));
        }
        let (quest_id, zone_id) = resolve_quest(quest_code).ok_or_else(|| {
            format!("unknown quest code '{quest_code}': add it to resolve_quest()")
        })?;
        Ok(Self {
            engine: LuaEngine::new(script_root.to_path_buf()),
            script_path,
            quest_id,
            zone_id,
            sequence: start_sequence,
            flags: 0,
            counters: [0; 4],
            step: Vec::new(),
            added: Vec::new(),
            completed: Vec::new(),
        })
    }

    fn snapshot(&self) -> PlayerSnapshot {
        PlayerSnapshot {
            actor_id: PLAYER_ID,
            zone_id: self.zone_id,
            active_quests: vec![self.quest_id],
            active_quest_states: vec![self.quest_state()],
            ..Default::default()
        }
    }

    fn quest_state(&self) -> QuestStateSnapshot {
        QuestStateSnapshot {
            quest_id: self.quest_id,
            sequence: self.sequence,
            flags: self.flags,
            counters: self.counters,
            npc_ls_from: 0,
            npc_ls_msg_step: 0,
        }
    }

    fn handle(&self) -> LuaQuestHandle {
        LuaQuestHandle {
            player_id: PLAYER_ID,
            quest_id: self.quest_id,
            has_quest: true,
            sequence: self.sequence,
            flags: self.flags,
            counters: self.counters,
            npc_ls_from: 0,
            npc_ls_msg_step: 0,
            // Replaced inside `call_quest_hook`; a placeholder is fine.
            queue: CommandQueue::new(),
        }
    }

    /// A throwaway NPC the quest's `onTalk`/`onPush` reads only via
    /// `npc:GetActorClassId()`, so only `actor_class_id` needs to be real.
    fn make_npc(&self, class_id: u32) -> QuestHookArg {
        QuestHookArg::Npc(LuaNpcSpec {
            actor_id: 0x4600_0000 | (class_id & 0x00FF_FFFF),
            name: String::new(),
            class_name: "PopulaceStandard".to_string(),
            class_path: "/Chara/Npc/Populace/PopulaceStandard".to_string(),
            unique_id: format!("npc_{class_id}"),
            zone_id: self.zone_id,
            zone_name: String::new(),
            state: 0,
            pos: (0.0, 0.0, 0.0),
            rotation: 0.0,
            actor_class_id: class_id,
            quest_graphic: 0,
        })
    }

    /// Fire a hook, replacing the current step's command buffer.
    fn fire(&mut self, hook: &str, extra: Vec<QuestHookArg>) -> Result<(), String> {
        let snapshot = self.snapshot();
        let handle = self.handle();
        let result = self
            .engine
            .call_quest_hook(&self.script_path, hook, snapshot, handle, extra);
        self.step = result.commands.clone();
        self.apply(&result.commands);
        if let Some(e) = result.error {
            return Err(format!("{hook} errored: {e}"));
        }
        Ok(())
    }

    /// Resume through any `wait(n)` time parks the current step left behind:
    /// fast-forward every parked deadline, then tick the engine and fold the
    /// resumed batches into the step buffer. This is the headless mirror of
    /// the live game-loop ticker (`LuaEngine::tick` once per tick) and the
    /// testkit's `advance_one_time_park`, minus the wall-clock sleep. Bounded
    /// so a pathological wait-loop fails the spec instead of hanging the run.
    fn drive_time_parks(&mut self) -> Result<(), String> {
        const MAX_WAIT_HOPS: usize = 8;
        for _ in 0..MAX_WAIT_HOPS {
            let pending = self
                .engine
                .scheduler()
                .lock()
                .map(|s| s.pending_time_count())
                .unwrap_or(0);
            if pending == 0 {
                return Ok(());
            }
            if let Ok(mut scheduler) = self.engine.scheduler().lock() {
                scheduler.expire_time_parks();
            }
            for (_owner, commands) in self.engine.tick() {
                self.apply(&commands);
                self.step.extend(commands);
            }
        }
        Err(format!(
            "coroutines still parked on wait() after {MAX_WAIT_HOPS} scheduler hops"
        ))
    }

    /// Resume a parked coroutine with a simulated client `EventUpdate`,
    /// extending the current step's command buffer.
    fn resume(&mut self, params: &[LuaParam]) -> Result<(), String> {
        match self.engine.fire_player_event_and_drain(PLAYER_ID, params) {
            Some(commands) => {
                self.apply(&commands);
                self.step.extend(commands);
                Ok(())
            }
            None => Err(
                "no parked coroutine to resume: did the previous step delegate to the client \
                 (callClientFunction)? ack()/answer() only follow a step that yielded"
                    .to_string(),
            ),
        }
    }

    /// Fold the quest-state subset of a command batch into the tracked state.
    /// Non-quest commands (event RPCs, zone changes, director ops) are left
    /// for the spec to assert on but don't mutate state here.
    fn apply(&mut self, commands: &[LuaCommand]) {
        for c in commands {
            match c {
                LuaCommand::QuestStartSequence { sequence, .. } => self.sequence = *sequence,
                LuaCommand::QuestSetFlag { bit, .. } if *bit < 32 => self.flags |= 1u32 << *bit,
                LuaCommand::QuestClearFlag { bit, .. } if *bit < 32 => {
                    self.flags &= !(1u32 << *bit)
                }
                LuaCommand::QuestClearFlags { .. } => self.flags = 0,
                // `quest:GetData():ClearData()` wipes all per-quest data: the
                // openers call it in doContentArea/doExitTrigger right before
                // StartSequence(SEQ_005).
                LuaCommand::QuestClearData { .. } => {
                    self.flags = 0;
                    self.counters = [0; 4];
                }
                LuaCommand::QuestSetCounter { idx, value, .. } if (*idx as usize) < 4 => {
                    self.counters[*idx as usize] = *value;
                }
                LuaCommand::QuestIncCounter { idx, .. } if (*idx as usize) < 4 => {
                    let s = &mut self.counters[*idx as usize];
                    *s = s.saturating_add(1);
                }
                LuaCommand::QuestDecCounter { idx, .. } if (*idx as usize) < 4 => {
                    let s = &mut self.counters[*idx as usize];
                    *s = s.saturating_sub(1);
                }
                LuaCommand::AddQuest { quest_id, .. } => self.added.push(*quest_id),
                LuaCommand::CompleteQuest { quest_id, .. } => self.completed.push(*quest_id),
                _ => {}
            }
        }
    }

    // --- assertions ----------------------------------------------------

    fn expect_start_sequence(&self, seq: u32) -> Result<(), String> {
        if self.step.iter().any(
            |c| matches!(c, LuaCommand::QuestStartSequence { sequence, .. } if *sequence == seq),
        ) {
            Ok(())
        } else {
            Err(format!(
                "expected StartSequence({seq}), got [{}]",
                summarize(&self.step)
            ))
        }
    }

    fn expect_flag_set(&self, bit: u8) -> Result<(), String> {
        if self
            .step
            .iter()
            .any(|c| matches!(c, LuaCommand::QuestSetFlag { bit: b, .. } if *b == bit))
        {
            Ok(())
        } else {
            Err(format!(
                "expected SetFlag({bit}), got [{}]",
                summarize(&self.step)
            ))
        }
    }

    fn expect_enpc(&self, class: u32, flag: u8) -> Result<(), String> {
        match self.step.iter().find_map(|c| match c {
            LuaCommand::QuestSetEnpc {
                actor_class_id,
                quest_flag_type,
                ..
            } if *actor_class_id == class => Some(*quest_flag_type),
            _ => None,
        }) {
            Some(f) if f == flag => Ok(()),
            Some(f) => Err(format!("ENPC {class}: expected flag {flag}, got {f}")),
            None => Err(format!(
                "expected SetEnpc for class {class}, got [{}]",
                summarize(&self.step)
            )),
        }
    }

    /// Negative mirror of [`expect_enpc`]: the step must contain NO
    /// `QuestSetEnpc` for the class at all — the scoping assertion for
    /// split-class casts (e.g. man0u1's stage vs gate F'lhaminn, where
    /// arming one class must not touch the other's spawn).
    fn expect_no_enpc(&self, class: u32) -> Result<(), String> {
        match self.step.iter().find_map(|c| match c {
            LuaCommand::QuestSetEnpc {
                actor_class_id,
                quest_flag_type,
                ..
            } if *actor_class_id == class => Some(*quest_flag_type),
            _ => None,
        }) {
            Some(f) => Err(format!(
                "expected NO SetEnpc for class {class}, but found one with flag {f}: [{}]",
                summarize(&self.step)
            )),
            None => Ok(()),
        }
    }

    /// `quest:NewNpcLsMsg(from)` surfaces as a `QuestSetNpcLsFrom` command
    /// (the linkpearl-glow trigger); the companion `PlayerSetNpcLs` pair is
    /// implied by the same binding, so asserting the `from` write is enough.
    fn expect_npc_ls_msg(&self, from: u32) -> Result<(), String> {
        if self
            .step
            .iter()
            .any(|c| matches!(c, LuaCommand::QuestSetNpcLsFrom { from: f, .. } if *f == from))
        {
            Ok(())
        } else {
            Err(format!(
                "expected NewNpcLsMsg({from}) (QuestSetNpcLsFrom), got [{}]",
                summarize(&self.step)
            ))
        }
    }

    fn expect_delegate(&self, name: &str) -> Result<(), String> {
        let found = self.step.iter().any(|c| {
            matches!(c, LuaCommand::RunEventFunction { args, .. }
                if args.iter().any(|a| matches!(a, LuaCommandArg::String(s) if s == name)))
        });
        if found {
            Ok(())
        } else {
            Err(format!(
                "expected client delegate '{name}', got [{}]",
                summarize(&self.step)
            ))
        }
    }

    fn expect_create_content_area(&self) -> Result<(), String> {
        if self
            .step
            .iter()
            .any(|c| matches!(c, LuaCommand::CreateContentArea { .. }))
        {
            Ok(())
        } else {
            Err(format!(
                "expected CreateContentArea, got [{}]",
                summarize(&self.step)
            ))
        }
    }

    fn expect_zone_change_content(&self) -> Result<(), String> {
        if self
            .step
            .iter()
            .any(|c| matches!(c, LuaCommand::DoZoneChangeContent { .. }))
        {
            Ok(())
        } else {
            Err(format!(
                "expected DoZoneChangeContent, got [{}]",
                summarize(&self.step)
            ))
        }
    }

    fn expect_handoff(&self, new_quest_id: u32) -> Result<(), String> {
        if self.added.contains(&new_quest_id) {
            Ok(())
        } else {
            Err(format!(
                "expected handoff to quest {new_quest_id} (AddQuest); added={:?} completed={:?}",
                self.added, self.completed
            ))
        }
    }

    fn expect_sequence(&self, seq: u32) -> Result<(), String> {
        if self.sequence == seq {
            Ok(())
        } else {
            Err(format!(
                "expected tracked sequence {seq}, but it is {}",
                self.sequence
            ))
        }
    }

    /// Run the quest's `getJournalMapMarkerList` getter against the tracked
    /// state (same direct-call path the live journal pane uses) and compare
    /// the returned marker ids, in order, against the spec's expectation.
    fn expect_markers(&self, expected: &[i64]) -> Result<(), String> {
        let params = self
            .engine
            .call_quest_getter(
                &self.script_path,
                "getJournalMapMarkerList",
                self.snapshot(),
                self.handle(),
            )
            .map_err(|e| format!("getJournalMapMarkerList errored: {e}"))?;
        let mut got: Vec<i64> = Vec::with_capacity(params.len());
        for p in &params {
            match p {
                LuaParam::Int32(i) => got.push(i64::from(*i)),
                other => {
                    return Err(format!(
                        "getJournalMapMarkerList returned a non-integer param {other:?} \
                         at sequence {} — marker lists must be integer-only",
                        self.sequence
                    ));
                }
            }
        }
        if got == expected {
            Ok(())
        } else {
            Err(format!(
                "expected journal markers {expected:?} at sequence {}, got {got:?}",
                self.sequence
            ))
        }
    }

    /// Assert the current step granted exactly this much gil (`AddGil`).
    fn expect_gil(&self, amount: i32) -> Result<(), String> {
        let found = self
            .step
            .iter()
            .any(|c| matches!(c, LuaCommand::AddGil { amount: a, .. } if *a == amount));
        if found {
            Ok(())
        } else {
            Err(format!(
                "expected AddGil({amount}), got [{}]",
                summarize(&self.step)
            ))
        }
    }

    /// Assert the current step granted exactly this much exp (`AddExp`).
    fn expect_exp(&self, exp: i32) -> Result<(), String> {
        let found = self
            .step
            .iter()
            .any(|c| matches!(c, LuaCommand::AddExp { exp: e, .. } if *e == exp));
        if found {
            Ok(())
        } else {
            Err(format!(
                "expected AddExp({exp}), got [{}]",
                summarize(&self.step)
            ))
        }
    }

    /// Assert the current step armed Baderon's linkpearl:
    /// `quest:NewNpcLsMsg(from)` enqueues a `QuestSetNpcLsFrom` alongside
    /// the ALERT icon flip — a dropped arm soft-locks the wait sequence in
    /// live play while the hook walk otherwise stays green.
    fn expect_npc_ls_alert(&self, from: u32) -> Result<(), String> {
        let found = self
            .step
            .iter()
            .any(|c| matches!(c, LuaCommand::QuestSetNpcLsFrom { from: f, .. } if *f == from));
        if found {
            Ok(())
        } else {
            Err(format!(
                "expected QuestSetNpcLsFrom({from}) (quest:NewNpcLsMsg), got [{}]",
                summarize(&self.step)
            ))
        }
    }

    fn expect_completed(&self, quest_id: u32) -> Result<(), String> {
        if self.completed.contains(&quest_id) {
            Ok(())
        } else {
            Err(format!(
                "expected CompleteQuest({quest_id}); completed={:?} added={:?}",
                self.completed, self.added
            ))
        }
    }
}

/// Resolve a quest class code to `(quest_id, default_zone_id)`. Only the three
/// starting-city openers and their immediate follow-ups are wired today; the
/// zone is informational (quest hooks don't gate on it).
fn resolve_quest(code: &str) -> Option<(u32, u32)> {
    Some(match code {
        "Man0l0" => (110_001, 193),
        "Man0l1" => (110_002, 193),
        "Man1l0" => (110_003, 230),
        "Man0g0" => (110_005, 0),
        "Man0g1" => (110_006, 0),
        "Man0u0" => (110_009, 0),
        "Man0u1" => (110_010, 0),
        _ => return None,
    })
}

/// Compact one-line rendering of a command batch for assertion failures.
fn summarize(commands: &[LuaCommand]) -> String {
    commands
        .iter()
        .map(cmd_token)
        .collect::<Vec<_>>()
        .join(", ")
}

fn cmd_token(c: &LuaCommand) -> String {
    match c {
        LuaCommand::QuestStartSequence { sequence, .. } => format!("StartSequence({sequence})"),
        LuaCommand::QuestSetFlag { bit, .. } => format!("SetFlag({bit})"),
        LuaCommand::QuestClearFlag { bit, .. } => format!("ClearFlag({bit})"),
        LuaCommand::QuestSetEnpc {
            actor_class_id,
            quest_flag_type,
            ..
        } => format!("SetEnpc({actor_class_id},{quest_flag_type})"),
        LuaCommand::QuestUpdateEnpcs { .. } => "UpdateEnpcs".to_string(),
        LuaCommand::RunEventFunction { args, .. } => {
            let target = args
                .iter()
                .find_map(|a| match a {
                    LuaCommandArg::String(s) => Some(s.clone()),
                    _ => None,
                })
                .unwrap_or_default();
            format!("Delegate({target})")
        }
        LuaCommand::KickEvent { trigger, .. } => format!("KickEvent({trigger})"),
        LuaCommand::EndEvent { .. } => "EndEvent".to_string(),
        LuaCommand::AddQuest { quest_id, .. } => format!("AddQuest({quest_id})"),
        LuaCommand::CompleteQuest { quest_id, .. } => format!("CompleteQuest({quest_id})"),
        LuaCommand::CreateContentArea { .. } => "CreateContentArea".to_string(),
        LuaCommand::DoZoneChangeContent { .. } => "DoZoneChangeContent".to_string(),
        LuaCommand::StartDirectorMain { .. } => "StartDirectorMain".to_string(),
        LuaCommand::SetLoginDirector { .. } => "SetLoginDirector".to_string(),
        _ => "<other>".to_string(),
    }
}

impl UserData for Walkthrough {
    fn add_methods<M: UserDataMethods<Self>>(methods: &mut M) {
        // --- drivers (reset the step buffer) ---------------------------
        methods.add_function("onStart", |_, this: AnyUserData| {
            this.borrow_mut::<Walkthrough>()?
                .fire("onStart", Vec::new())
                .map_err(lua_err)?;
            Ok(this)
        });
        methods.add_function("stateChange", |_, (this, seq): (AnyUserData, u32)| {
            this.borrow_mut::<Walkthrough>()?
                .fire("onStateChange", vec![QuestHookArg::Int(seq as i64)])
                .map_err(lua_err)?;
            Ok(this)
        });
        methods.add_function("talk", |_, (this, class_id): (AnyUserData, u32)| {
            {
                let mut w = this.borrow_mut::<Walkthrough>()?;
                let npc = w.make_npc(class_id);
                w.fire("onTalk", vec![npc]).map_err(lua_err)?;
            }
            Ok(this)
        });
        methods.add_function("push", |_, (this, class_id): (AnyUserData, u32)| {
            {
                let mut w = this.borrow_mut::<Walkthrough>()?;
                let npc = w.make_npc(class_id);
                w.fire("onPush", vec![npc]).map_err(lua_err)?;
            }
            Ok(this)
        });
        // `emote(classId, "emoteDefault1")` fires the quest's `onEmote` hook
        // with the Meteor calling convention `(player, quest, npc, eventName)`,
        // then resumes through any `wait(n)` park the hook opened with (the
        // DoEmote animation delay) so the spec observes the post-wait commands
        // in the same step.
        methods.add_function(
            "emote",
            |_, (this, class_id, event_name): (AnyUserData, u32, String)| {
                {
                    let mut w = this.borrow_mut::<Walkthrough>()?;
                    let npc = w.make_npc(class_id);
                    w.fire("onEmote", vec![npc, QuestHookArg::Str(event_name)])
                        .map_err(lua_err)?;
                    w.drive_time_parks().map_err(lua_err)?;
                }
                Ok(this)
            },
        );

        // --- resumes (extend the step buffer) --------------------------
        methods.add_function("ack", |_, this: AnyUserData| {
            this.borrow_mut::<Walkthrough>()?
                .resume(&[])
                .map_err(lua_err)?;
            Ok(this)
        });
        methods.add_function("answer", |_, (this, choice): (AnyUserData, i64)| {
            this.borrow_mut::<Walkthrough>()?
                .resume(&[LuaParam::Int32(choice as i32)])
                .map_err(lua_err)?;
            Ok(this)
        });

        // --- assertions (return self for chaining) ---------------------
        methods.add_function(
            "expectStartSequence",
            |_, (this, seq): (AnyUserData, u32)| {
                this.borrow::<Walkthrough>()?
                    .expect_start_sequence(seq)
                    .map_err(lua_err)?;
                Ok(this)
            },
        );
        methods.add_function(
            "expectMarkers",
            |_, (this, expected): (AnyUserData, mlua::Variadic<i64>)| {
                this.borrow::<Walkthrough>()?
                    .expect_markers(&expected)
                    .map_err(lua_err)?;
                Ok(this)
            },
        );
        methods.add_function("expectGil", |_, (this, amount): (AnyUserData, i32)| {
            this.borrow::<Walkthrough>()?
                .expect_gil(amount)
                .map_err(lua_err)?;
            Ok(this)
        });
        methods.add_function("expectExp", |_, (this, exp): (AnyUserData, i32)| {
            this.borrow::<Walkthrough>()?
                .expect_exp(exp)
                .map_err(lua_err)?;
            Ok(this)
        });
        methods.add_function("expectNpcLsAlert", |_, (this, from): (AnyUserData, u32)| {
            this.borrow::<Walkthrough>()?
                .expect_npc_ls_alert(from)
                .map_err(lua_err)?;
            Ok(this)
        });
        methods.add_function("expectFlagSet", |_, (this, bit): (AnyUserData, u8)| {
            this.borrow::<Walkthrough>()?
                .expect_flag_set(bit)
                .map_err(lua_err)?;
            Ok(this)
        });
        methods.add_function(
            "expectEnpc",
            |_, (this, class, flag): (AnyUserData, u32, u8)| {
                this.borrow::<Walkthrough>()?
                    .expect_enpc(class, flag)
                    .map_err(lua_err)?;
                Ok(this)
            },
        );
        methods.add_function("expectNoEnpc", |_, (this, class): (AnyUserData, u32)| {
            this.borrow::<Walkthrough>()?
                .expect_no_enpc(class)
                .map_err(lua_err)?;
            Ok(this)
        });
        methods.add_function("expectGil", |_, (this, amount): (AnyUserData, i32)| {
            this.borrow::<Walkthrough>()?
                .expect_gil(amount)
                .map_err(lua_err)?;
            Ok(this)
        });
        methods.add_function("expectNpcLsMsg", |_, (this, from): (AnyUserData, u32)| {
            this.borrow::<Walkthrough>()?
                .expect_npc_ls_msg(from)
                .map_err(lua_err)?;
            Ok(this)
        });
        methods.add_function(
            "expectDelegate",
            |_, (this, name): (AnyUserData, String)| {
                this.borrow::<Walkthrough>()?
                    .expect_delegate(&name)
                    .map_err(lua_err)?;
                Ok(this)
            },
        );
        methods.add_function("expectCreateContentArea", |_, this: AnyUserData| {
            this.borrow::<Walkthrough>()?
                .expect_create_content_area()
                .map_err(lua_err)?;
            Ok(this)
        });
        methods.add_function("expectZoneChangeContent", |_, this: AnyUserData| {
            this.borrow::<Walkthrough>()?
                .expect_zone_change_content()
                .map_err(lua_err)?;
            Ok(this)
        });
        methods.add_function(
            "expectHandoff",
            |_, (this, quest_id): (AnyUserData, u32)| {
                this.borrow::<Walkthrough>()?
                    .expect_handoff(quest_id)
                    .map_err(lua_err)?;
                Ok(this)
            },
        );
        methods.add_function("expectSequence", |_, (this, seq): (AnyUserData, u32)| {
            this.borrow::<Walkthrough>()?
                .expect_sequence(seq)
                .map_err(lua_err)?;
            Ok(this)
        });
        methods.add_function(
            "expectCompleted",
            |_, (this, quest_id): (AnyUserData, u32)| {
                this.borrow::<Walkthrough>()?
                    .expect_completed(quest_id)
                    .map_err(lua_err)?;
                Ok(this)
            },
        );
    }
}
